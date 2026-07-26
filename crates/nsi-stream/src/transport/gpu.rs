//! GPU-resident transport on Vulkan (`ash`).
//!
//! This is the R2 path: publication images and the publication timeline live
//! in device memory, and they cross the client/renderer boundary **only as
//! exportable OS handles** -- `VK_KHR_external_memory_fd` for the images,
//! `VK_KHR_external_semaphore_fd` for the timeline semaphore. No raw pointer
//! to device memory is ever put on the wire; that is reserved for the
//! in-process [callback transport](super::callback).
//!
//! # Exported Handle Semantics (frozen at `stream.version` 1)
//!
//! - Handle type is `OPAQUE_FD` on Linux. Windows (`OPAQUE_WIN32`) is the
//!   next platform; macOS is deferred (`plan.md`, target platforms).
//! - Every exported descriptor is a **new reference** to the underlying
//!   payload: the driver keeps its own `vk::DeviceMemory` alive, and the
//!   client owns and closes the descriptor it received.
//! - Exported descriptors are sent over the [rendezvous
//!   channel](crate::channel) with `SCM_RIGHTS`, in the `Open`/`Resize`
//!   messages, in ring-slot order.
//! - The importing client must recreate the image with the identical
//!   `vk::ImageCreateInfo` -- format, extent, tiling, usage -- which the
//!   `Open` message carries. There is no negotiation and no fallback: a
//!   mismatch is a client bug and is not repaired silently.
//! - The timeline semaphore is exported once per stream; each publication's
//!   [`timeline_value`](crate::ring::Publication::timeline_value) is the
//!   value to wait on (R4).
//!
//! # Availability
//!
//! Every entry point that touches the loader returns
//! [`Error::TransportUnavailable`] when no Vulkan loader or ICD is present,
//! so a build with this feature runs -- and its tests skip -- on machines
//! with no GPU. Nothing in this module panics on a missing driver.

use crate::{
    Error, Result,
    layer::{Extent, Layer, LayerFormat},
};
use ash::{Device, Entry, Instance, khr, vk};
use std::os::fd::{FromRawFd, OwnedFd};

/// The external handle type this transport exports.
const HANDLE_TYPE: vk::ExternalMemoryHandleTypeFlags =
    vk::ExternalMemoryHandleTypeFlags::OPAQUE_FD;

/// The external handle type the timeline semaphore is exported with.
const SEMAPHORE_HANDLE_TYPE: vk::ExternalSemaphoreHandleTypeFlags =
    vk::ExternalSemaphoreHandleTypeFlags::OPAQUE_FD;

fn unavailable(reason: impl core::fmt::Display) -> Error {
    Error::unavailable("gpu", reason.to_string())
}

/// The ɴsɪ pixel format as a Vulkan format.
#[inline]
pub const fn vulkan_format(format: LayerFormat) -> vk::Format {
    match format {
        LayerFormat::RgbaF16 => vk::Format::R16G16B16A16_SFLOAT,
        LayerFormat::RgbaF32 => vk::Format::R32G32B32A32_SFLOAT,
    }
}

/// Render a Vulkan `deviceUUID` the way `stream.device.uuid` spells it.
pub fn format_uuid(uuid: &[u8; vk::UUID_SIZE]) -> String {
    let hex = uuid
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    [
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32],
    ]
    .join("-")
}

/// Compare two UUID spellings, ignoring case and dashes.
fn uuid_matches(left: &str, right: &str) -> bool {
    let normalize = |value: &str| {
        value
            .chars()
            .filter(char::is_ascii_hexdigit)
            .map(|character| character.to_ascii_lowercase())
            .collect::<String>()
    };

    normalize(left) == normalize(right)
}

/// Whether a Vulkan loader with at least one physical device exists.
///
/// This is what [`StaticProbe::for_this_build`](super::StaticProbe::for_this_build)
/// asks before declaring the GPU transport viable.
///
/// # Errors
///
/// [`Error::TransportUnavailable`] when the loader, an instance, or a
/// physical device is missing.
pub fn probe() -> Result<()> {
    let context = InstanceContext::load()?;

    // SAFETY: `context.instance` is a live instance created just above.
    let devices = unsafe { context.instance.enumerate_physical_devices() }
        .map_err(|error| unavailable(format!("no physical device: {error}")))?;

    if devices.is_empty() {
        Err(unavailable("the Vulkan loader reports no physical device"))?;
    }

    Ok(())
}

// ─── Instance ───────────────────────────────────────────────────────────────

/// A loaded entry point plus an instance. Destroys the instance on drop.
struct InstanceContext {
    /// Kept alive: the instance's function pointers borrow from the loader.
    _entry: Entry,
    instance: Instance,
}

impl core::fmt::Debug for InstanceContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("InstanceContext")
    }
}

impl Drop for InstanceContext {
    fn drop(&mut self) {
        // SAFETY: the instance is live, was created by this type, and no
        // child object outlives it -- `VulkanContext` destroys its device
        // first.
        unsafe { self.instance.destroy_instance(None) };
    }
}

impl InstanceContext {
    fn load() -> Result<Self> {
        // SAFETY: `Entry::load` dynamically loads the system Vulkan loader.
        // It is `unsafe` because the loader runs arbitrary initialization
        // code; a missing loader is reported as an error, not a panic.
        let entry = unsafe { Entry::load() }.map_err(|error| {
            unavailable(format!("no Vulkan loader: {error}"))
        })?;

        let application = vk::ApplicationInfo::default()
            .application_name(c"nsi-stream")
            .api_version(vk::API_VERSION_1_2);
        let create_info =
            vk::InstanceCreateInfo::default().application_info(&application);

        // SAFETY: `create_info` and the application info it borrows are
        // alive for the duration of the call.
        let instance = unsafe { entry.create_instance(&create_info, None) }
            .map_err(|error| {
                unavailable(format!("instance creation failed: {error}"))
            })?;

        Ok(Self {
            _entry: entry,
            instance,
        })
    }
}

// ─── Device ─────────────────────────────────────────────────────────────────

/// A Vulkan device selected for streaming, with the external-handle
/// extensions enabled.
pub struct VulkanContext {
    instance: InstanceContext,
    physical_device: vk::PhysicalDevice,
    device: Device,
    queue_family: u32,
    device_uuid: String,
}

impl core::fmt::Debug for VulkanContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VulkanContext")
            .field("device_uuid", &self.device_uuid)
            .field("queue_family", &self.queue_family)
            .finish()
    }
}

impl Drop for VulkanContext {
    fn drop(&mut self) {
        // SAFETY: every object allocated from this device (`RingImages`,
        // `VulkanTimeline`) borrows the context and is therefore already
        // destroyed. Waiting for idle first is required before destroying a
        // device with in-flight work.
        unsafe {
            let _ = self.device.device_wait_idle();
            self.device.destroy_device(None);
        }
    }
}

impl VulkanContext {
    /// Open the device the client asked for.
    ///
    /// `requested_uuid` is `stream.device.uuid`. When it is `Some`, only a
    /// physical device with that `deviceUUID` is acceptable: a mismatch is
    /// [`Error::DeviceMismatch`], never a silent substitution
    /// (constitution principle V). When it is `None`, the first device that
    /// supports the external-handle extensions wins.
    ///
    /// # Errors
    ///
    /// - [`Error::TransportUnavailable`] -- no loader, no instance, no
    ///   suitable device, or device creation failed.
    /// - [`Error::DeviceMismatch`] -- `stream.device.uuid` names an adapter
    ///   that is not present.
    pub fn open(requested_uuid: Option<&str>) -> Result<Self> {
        let instance = InstanceContext::load()?;

        // SAFETY: the instance is live for the whole function.
        let physical_devices =
            unsafe { instance.instance.enumerate_physical_devices() }.map_err(
                |error| unavailable(format!("no physical device: {error}")),
            )?;

        let candidates = physical_devices
            .into_iter()
            .map(|physical_device| {
                let uuid =
                    physical_device_uuid(&instance.instance, physical_device);

                (physical_device, uuid)
            })
            .collect::<Vec<_>>();

        let (physical_device, device_uuid) = match requested_uuid {
            Some(requested) => candidates
                .iter()
                .find(|(_, uuid)| uuid_matches(uuid, requested))
                .cloned()
                .ok_or_else(|| Error::DeviceMismatch {
                    requested: requested.to_string(),
                    actual: candidates
                        .iter()
                        .map(|(_, uuid)| uuid.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                })?,
            None => candidates
                .first()
                .cloned()
                .ok_or_else(|| unavailable("no physical device"))?,
        };

        // SAFETY: `physical_device` came from this instance.
        let families = unsafe {
            instance
                .instance
                .get_physical_device_queue_family_properties(physical_device)
        };

        let queue_family = families
            .iter()
            .position(|family| {
                family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    || family.queue_flags.contains(vk::QueueFlags::COMPUTE)
            })
            .ok_or_else(|| unavailable("no graphics or compute queue family"))?
            as u32;

        let priorities = [1.0f32];
        let queue_infos = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(queue_family)
            .queue_priorities(&priorities)];

        let extensions = [
            khr::external_memory_fd::NAME.as_ptr(),
            khr::external_semaphore_fd::NAME.as_ptr(),
        ];

        let mut timeline_features =
            vk::PhysicalDeviceTimelineSemaphoreFeatures::default()
                .timeline_semaphore(true);

        let device_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_infos)
            .enabled_extension_names(&extensions)
            .push_next(&mut timeline_features);

        // SAFETY: all borrowed create-info structures outlive the call.
        let device = unsafe {
            instance
                .instance
                .create_device(physical_device, &device_info, None)
        }
        .map_err(|error| {
            unavailable(format!("device creation failed: {error}"))
        })?;

        Ok(Self {
            instance,
            physical_device,
            device,
            queue_family,
            device_uuid,
        })
    }

    /// The `deviceUUID` of the selected adapter, spelled the way
    /// `stream.device.uuid` spells it.
    #[inline]
    pub fn device_uuid(&self) -> &str {
        &self.device_uuid
    }

    /// The queue family the driver submits on.
    #[inline]
    pub const fn queue_family(&self) -> u32 {
        self.queue_family
    }

    /// The opened logical device.
    #[inline]
    pub const fn device(&self) -> &Device {
        &self.device
    }

    fn memory_type(
        &self,
        requirements: vk::MemoryRequirements,
        properties: vk::MemoryPropertyFlags,
    ) -> Result<u32> {
        // SAFETY: `physical_device` belongs to the live instance.
        let memory = unsafe {
            self.instance
                .instance
                .get_physical_device_memory_properties(self.physical_device)
        };

        (0..memory.memory_type_count)
            .find(|index| {
                let supported =
                    requirements.memory_type_bits & (1 << index) != 0;

                supported
                    && memory.memory_types[*index as usize]
                        .property_flags
                        .contains(properties)
            })
            .ok_or_else(|| unavailable("no suitable memory type"))
    }
}

/// Read a physical device's `deviceUUID`.
fn physical_device_uuid(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> String {
    let mut id = vk::PhysicalDeviceIDProperties::default();
    let mut properties =
        vk::PhysicalDeviceProperties2::default().push_next(&mut id);

    // SAFETY: `physical_device` belongs to `instance`, and `properties`
    // (with the chained `id`) is alive across the call.
    unsafe {
        instance
            .get_physical_device_properties2(physical_device, &mut properties)
    };

    format_uuid(&id.device_uuid)
}

// ─── Ring Images ────────────────────────────────────────────────────────────

/// One ring slot's device-side images -- one per layer.
#[derive(Debug)]
pub struct SlotImages {
    images: Vec<vk::Image>,
    memories: Vec<vk::DeviceMemory>,
}

/// The device-side mirror of the publication ring.
///
/// Layout matches [`PublicationRing`](crate::ring::PublicationRing) exactly:
/// slot `i`, layer `j` here is slot `i`, plane `j` there.
#[derive(Debug)]
pub struct RingImages<'a> {
    context: &'a VulkanContext,
    slots: Vec<SlotImages>,
    extent: Extent,
}

impl Drop for RingImages<'_> {
    fn drop(&mut self) {
        self.slots.iter().for_each(|slot| {
            // SAFETY: every handle was allocated from this device and is
            // destroyed exactly once. The device is still alive because
            // `RingImages` borrows the context.
            unsafe {
                slot.images.iter().for_each(|image| {
                    self.context.device.destroy_image(*image, None)
                });
                slot.memories.iter().for_each(|memory| {
                    self.context.device.free_memory(*memory, None)
                });
            }
        });
    }
}

impl<'a> RingImages<'a> {
    /// Allocate `ring` slots of exportable images, one image per layer.
    ///
    /// # Errors
    ///
    /// [`Error::TransportUnavailable`] when image creation, memory
    /// allocation or binding fails.
    pub fn allocate(
        context: &'a VulkanContext,
        layers: &[Layer],
        extent: Extent,
        ring: usize,
    ) -> Result<Self> {
        let mut allocated = Self {
            context,
            slots: Vec::with_capacity(ring),
            extent,
        };

        (0..ring).try_for_each(|_| {
            allocated
                .allocate_slot(layers, extent)
                .map(|slot| allocated.slots.push(slot))
        })?;

        Ok(allocated)
    }

    /// The extent every image was created at.
    #[inline]
    pub const fn extent(&self) -> Extent {
        self.extent
    }

    /// Number of slots.
    #[inline]
    pub fn ring_size(&self) -> usize {
        self.slots.len()
    }

    /// The image backing slot `slot`, layer `layer`.
    pub fn image(&self, slot: usize, layer: usize) -> Option<vk::Image> {
        self.slots
            .get(slot)
            .and_then(|slot| slot.images.get(layer))
            .copied()
    }

    /// Export slot `slot`, layer `layer` as an OS handle for the client.
    ///
    /// The returned descriptor is a new reference to the same device
    /// memory; the driver keeps its own. Send it over the rendezvous
    /// channel, never an address.
    ///
    /// # Errors
    ///
    /// [`Error::TransportUnavailable`] when the export fails or the indices
    /// are out of range.
    pub fn export_memory_fd(
        &self,
        slot: usize,
        layer: usize,
    ) -> Result<OwnedFd> {
        let memory = self
            .slots
            .get(slot)
            .and_then(|slot| slot.memories.get(layer))
            .copied()
            .ok_or_else(|| {
                unavailable(format!("no image at slot {slot}, layer {layer}"))
            })?;

        let loader = khr::external_memory_fd::Device::new(
            &self.context.instance.instance,
            &self.context.device,
        );
        let info = vk::MemoryGetFdInfoKHR::default()
            .memory(memory)
            .handle_type(HANDLE_TYPE);

        // SAFETY: `memory` was allocated from this device with
        // `ExportMemoryAllocateInfo` naming the same handle type.
        let raw = unsafe { loader.get_memory_fd(&info) }.map_err(|error| {
            unavailable(format!("memory export failed: {error}"))
        })?;

        // SAFETY: `get_memory_fd` returns a fresh, owned descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(raw) })
    }

    fn allocate_slot(
        &self,
        layers: &[Layer],
        extent: Extent,
    ) -> Result<SlotImages> {
        let device = &self.context.device;
        let mut slot = SlotImages {
            images: Vec::with_capacity(layers.len()),
            memories: Vec::with_capacity(layers.len()),
        };

        layers.iter().try_for_each(|layer| {
            let mut external = vk::ExternalMemoryImageCreateInfo::default()
                .handle_types(HANDLE_TYPE);
            let image_info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vulkan_format(layer.format))
                .extent(vk::Extent3D {
                    width: extent.width,
                    height: extent.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(
                    vk::ImageUsageFlags::TRANSFER_DST
                        | vk::ImageUsageFlags::SAMPLED,
                )
                .sharing_mode(vk::SharingMode::EXCLUSIVE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .push_next(&mut external);

            // SAFETY: the create info and its chained struct are alive
            // across the call.
            let image = unsafe { device.create_image(&image_info, None) }
                .map_err(|error| {
                    unavailable(format!("image creation failed: {error}"))
                })?;

            // SAFETY: `image` was just created from this device.
            let requirements =
                unsafe { device.get_image_memory_requirements(image) };

            let memory_type = self.context.memory_type(
                requirements,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;

            let mut export = vk::ExportMemoryAllocateInfo::default()
                .handle_types(HANDLE_TYPE);
            let allocate_info = vk::MemoryAllocateInfo::default()
                .allocation_size(requirements.size)
                .memory_type_index(memory_type)
                .push_next(&mut export);

            // SAFETY: the allocate info and its chained struct are alive
            // across the call.
            let memory =
                unsafe { device.allocate_memory(&allocate_info, None) }
                    .map_err(|error| {
                        unavailable(format!(
                            "memory allocation failed: {error}"
                        ))
                    })?;

            // SAFETY: `image` and `memory` come from this device and neither
            // is bound yet.
            unsafe { device.bind_image_memory(image, memory, 0) }.map_err(
                |error| unavailable(format!("memory binding failed: {error}")),
            )?;

            slot.images.push(image);
            slot.memories.push(memory);

            Ok(())
        })?;

        Ok(slot)
    }
}

// ─── Timeline ───────────────────────────────────────────────────────────────

/// The publication timeline semaphore.
///
/// Same shape as [`CpuTimeline`](crate::timeline::CpuTimeline): monotonic
/// [`signal`](VulkanTimeline::signal), blocking
/// [`wait`](VulkanTimeline::wait) with a typed timeout. One per stream (R4).
#[derive(Debug)]
pub struct VulkanTimeline<'a> {
    context: &'a VulkanContext,
    semaphore: vk::Semaphore,
}

impl Drop for VulkanTimeline<'_> {
    fn drop(&mut self) {
        // SAFETY: the semaphore came from this device and is destroyed
        // exactly once; the device outlives it through the borrow.
        unsafe { self.context.device.destroy_semaphore(self.semaphore, None) };
    }
}

impl<'a> VulkanTimeline<'a> {
    /// Create an exportable timeline semaphore starting at 0.
    ///
    /// # Errors
    ///
    /// [`Error::TransportUnavailable`] when creation fails.
    pub fn new(context: &'a VulkanContext) -> Result<Self> {
        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let mut export = vk::ExportSemaphoreCreateInfo::default()
            .handle_types(SEMAPHORE_HANDLE_TYPE);
        let create_info = vk::SemaphoreCreateInfo::default()
            .push_next(&mut type_info)
            .push_next(&mut export);

        // SAFETY: the create info and both chained structs are alive across
        // the call.
        let semaphore =
            unsafe { context.device.create_semaphore(&create_info, None) }
                .map_err(|error| {
                    unavailable(format!(
                        "timeline semaphore creation failed: {error}"
                    ))
                })?;

        Ok(Self { context, semaphore })
    }

    /// The semaphore handle, for submission.
    #[inline]
    pub const fn semaphore(&self) -> vk::Semaphore {
        self.semaphore
    }

    /// The current counter value.
    ///
    /// # Errors
    ///
    /// [`Error::TransportUnavailable`] when the query fails.
    pub fn value(&self) -> Result<u64> {
        // SAFETY: the semaphore belongs to this device.
        unsafe {
            self.context
                .device
                .get_semaphore_counter_value(self.semaphore)
        }
        .map_err(|error| unavailable(format!("counter query failed: {error}")))
    }

    /// Signal the timeline from the host.
    ///
    /// Vulkan timeline semaphores are monotonic by specification: signaling
    /// a value below the current one is invalid, so this is a no-op in that
    /// case, exactly like [`CpuTimeline::signal`](crate::CpuTimeline::signal).
    ///
    /// # Errors
    ///
    /// [`Error::TransportUnavailable`] when the signal fails.
    pub fn signal(&self, value: u64) -> Result<()> {
        if self.value()? >= value {
            Ok(())
        } else {
            let info = vk::SemaphoreSignalInfo::default()
                .semaphore(self.semaphore)
                .value(value);

            // SAFETY: `info` is alive across the call and names a timeline
            // semaphore of this device.
            unsafe { self.context.device.signal_semaphore(&info) }
                .map_err(|error| unavailable(format!("signal failed: {error}")))
        }
    }

    /// Wait until the timeline reaches `value`.
    ///
    /// # Errors
    ///
    /// [`Error::WaitTimeout`] carrying `value` when `timeout_nanoseconds`
    /// expires first; [`Error::TransportUnavailable`] for a device error.
    pub fn wait(&self, value: u64, timeout_nanoseconds: u64) -> Result<()> {
        let semaphores = [self.semaphore];
        let values = [value];
        let info = vk::SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&values);

        // SAFETY: `info` borrows the arrays above, both alive across the
        // call.
        unsafe {
            self.context
                .device
                .wait_semaphores(&info, timeout_nanoseconds)
        }
        .map_err(|error| {
            if vk::Result::TIMEOUT == error {
                Error::WaitTimeout { serial: value }
            } else {
                unavailable(format!("wait failed: {error}"))
            }
        })
    }

    /// Export the timeline as an OS handle for the client.
    ///
    /// # Errors
    ///
    /// [`Error::TransportUnavailable`] when the export fails.
    pub fn export_fd(&self) -> Result<OwnedFd> {
        let loader = khr::external_semaphore_fd::Device::new(
            &self.context.instance.instance,
            &self.context.device,
        );
        let info = vk::SemaphoreGetFdInfoKHR::default()
            .semaphore(self.semaphore)
            .handle_type(SEMAPHORE_HANDLE_TYPE);

        // SAFETY: the semaphore was created with
        // `ExportSemaphoreCreateInfo` naming the same handle type.
        let raw =
            unsafe { loader.get_semaphore_fd(&info) }.map_err(|error| {
                unavailable(format!("semaphore export failed: {error}"))
            })?;

        // SAFETY: `get_semaphore_fd` returns a fresh, owned descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(raw) })
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_formatting_is_canonical() {
        let uuid = [
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
            0x67, 0x89, 0xab, 0xcd, 0xef,
        ];

        assert_eq!(format_uuid(&uuid), "01234567-89ab-cdef-0123-456789abcdef");
        assert!(uuid_matches(
            &format_uuid(&uuid),
            "0123456789ABCDEF0123456789ABCDEF"
        ));
    }

    #[test]
    fn formats_map_to_vulkan() {
        assert_eq!(
            vulkan_format(LayerFormat::RgbaF16),
            vk::Format::R16G16B16A16_SFLOAT
        );
        assert_eq!(
            vulkan_format(LayerFormat::RgbaF32),
            vk::Format::R32G32B32A32_SFLOAT
        );
    }

    #[test]
    fn missing_loader_is_a_typed_error() {
        // On a machine with a Vulkan ICD this simply succeeds; on one
        // without, it must be a clean `TransportUnavailable` rather than a
        // panic.
        match probe() {
            Ok(()) => println!("vulkan loader present"),
            Err(error) => {
                assert!(matches!(error, Error::TransportUnavailable { .. }))
            }
        }
    }
}
