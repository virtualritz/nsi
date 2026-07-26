//! Transport selection.
//!
//! A transport is *how* published pixels reach the client. The client-facing
//! acquire/release API is identical across all three (US3): GPU residency is
//! an optimization, not a protocol fork.
//!
//! # Negotiation Rules (frozen at `stream.version` 1)
//!
//! - `"auto"` tries [`Transport::GpuShared`], then [`Transport::Shm`], then
//!   [`Transport::Callback`]; the first viable one wins. A
//!   `stream.device.uuid` that does not match the driver's adapter makes the
//!   GPU transport non-viable *under `"auto"` only*, so negotiation falls
//!   through to the next candidate.
//! - An **explicit** transport never falls back. If it is not viable, open()
//!   fails with [`Error::TransportUnavailable`] (R8).
//! - An explicit `"gpu"` with a mismatching `stream.device.uuid` fails with
//!   [`Error::DeviceMismatch`] -- a required identifier is never silently
//!   ignored (constitution principle V).

pub mod callback;
#[cfg(feature = "vulkan")]
pub mod gpu;
#[cfg(all(unix, feature = "shm"))]
pub mod shm;

use crate::{Error, Result, config::StreamConfig};

/// The selected pixel transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Transport {
    /// GPU-resident images shared through exported OS handles (R2).
    GpuShared,
    /// Shared memory, driven by the generation counter in the shared header.
    Shm,
    /// In-process typed closures.
    Callback,
}

impl Transport {
    /// Negotiation order for `stream.transport "auto"`.
    pub const AUTO_ORDER: [Self; 3] =
        [Self::GpuShared, Self::Shm, Self::Callback];

    /// The `stream.transport` spelling.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuShared => "gpu",
            Self::Shm => "shm",
            Self::Callback => "callback",
        }
    }

    /// The version-1 wire discriminant, as sent in the `Open` message.
    #[inline]
    pub const fn as_wire(self) -> u8 {
        match self {
            Self::GpuShared => 1,
            Self::Shm => 2,
            Self::Callback => 3,
        }
    }

    /// Decode a version-1 wire discriminant.
    #[inline]
    pub const fn from_wire(wire: u8) -> Option<Self> {
        match wire {
            1 => Some(Self::GpuShared),
            2 => Some(Self::Shm),
            3 => Some(Self::Callback),
            _ => None,
        }
    }
}

impl core::fmt::Display for Transport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What `stream.transport` asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TransportRequest {
    /// `"auto"` -- negotiate in [`Transport::AUTO_ORDER`].
    #[default]
    Auto,
    /// A named transport. Never falls back.
    Explicit(Transport),
}

impl TransportRequest {
    /// The `stream.transport` spelling.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Explicit(transport) => transport.as_str(),
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "auto" => Ok(Self::Auto),
            "gpu" => Ok(Self::Explicit(Transport::GpuShared)),
            "shm" => Ok(Self::Explicit(Transport::Shm)),
            "callback" => Ok(Self::Explicit(Transport::Callback)),
            other => Err(Error::malformed(
                "stream.transport",
                format!(
                    "expected `auto`, `gpu`, `shm` or `callback`, got \
                     `{other}`"
                ),
            )),
        }
    }
}

impl core::fmt::Display for TransportRequest {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Probe ──────────────────────────────────────────────────────────────────

/// Reports which transports the driver side can actually provide.
///
/// The probe is injected so that negotiation is testable without a GPU, a
/// second process, or a renderer: a fixture reports a transport unviable and
/// the negotiation contract is exercised exactly as it would be at runtime.
pub trait TransportProbe {
    /// Whether `transport` can be opened right now.
    ///
    /// The `Err` payload is the human-readable reason, which negotiation
    /// forwards into [`Error::TransportUnavailable`].
    fn viability(
        &self,
        transport: Transport,
    ) -> core::result::Result<(), String>;

    /// UUID of the adapter the driver renders on, if it renders on one.
    ///
    /// `None` means "no GPU adapter is involved", which can never match a
    /// requested [`StreamConfig::device_uuid`].
    fn device_uuid(&self) -> Option<String> {
        None
    }
}

/// A probe with fixed answers.
///
/// Used as the default driver-side probe (built from compile-time features
/// and the Vulkan loader state) and as the negotiation test fixture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct StaticProbe {
    gpu: Option<String>,
    shm: Option<String>,
    callback: Option<String>,
    device_uuid: Option<String>,
}

impl StaticProbe {
    /// A probe on which every transport is viable.
    pub fn all_viable() -> Self {
        Self::default()
    }

    /// Mark `transport` unviable, with a reason.
    #[must_use]
    pub fn unviable(
        mut self,
        transport: Transport,
        reason: impl Into<String>,
    ) -> Self {
        let reason = Some(reason.into());
        match transport {
            Transport::GpuShared => self.gpu = reason,
            Transport::Shm => self.shm = reason,
            Transport::Callback => self.callback = reason,
        }
        self
    }

    /// Set the adapter UUID the driver renders on.
    #[must_use]
    pub fn with_device_uuid(mut self, uuid: impl Into<String>) -> Self {
        self.device_uuid = Some(uuid.into());
        self
    }

    /// The probe describing this build: the GPU transport needs the
    /// `vulkan` feature and a working loader, the shared-memory transport
    /// needs the `shm` feature on a Unix host, the callback transport is
    /// always available in-process.
    pub fn for_this_build() -> Self {
        let mut probe = Self::default();

        #[cfg(not(feature = "vulkan"))]
        {
            probe = probe.unviable(
                Transport::GpuShared,
                "built without the `vulkan` feature",
            );
        }
        #[cfg(feature = "vulkan")]
        if let Err(error) = gpu::probe() {
            probe = probe.unviable(Transport::GpuShared, error.to_string());
        }

        #[cfg(not(all(unix, feature = "shm")))]
        {
            probe = probe.unviable(
                Transport::Shm,
                "built without the `shm` feature, or not a Unix host",
            );
        }

        probe
    }
}

impl TransportProbe for StaticProbe {
    fn viability(
        &self,
        transport: Transport,
    ) -> core::result::Result<(), String> {
        let reason = match transport {
            Transport::GpuShared => &self.gpu,
            Transport::Shm => &self.shm,
            Transport::Callback => &self.callback,
        };

        reason.clone().map_or(Ok(()), Err)
    }

    fn device_uuid(&self) -> Option<String> {
        self.device_uuid.clone()
    }
}

// ─── Negotiation ────────────────────────────────────────────────────────────

/// Select the transport for `config`.
///
/// See the module documentation for the frozen rules.
///
/// # Errors
///
/// - [`Error::DeviceMismatch`] -- explicit `"gpu"` with a
///   `stream.device.uuid` the driver does not render on.
/// - [`Error::TransportUnavailable`] -- an explicit transport that is not
///   viable, or `"auto"` with no viable candidate at all.
pub fn negotiate(
    config: &StreamConfig,
    probe: &dyn TransportProbe,
) -> Result<Transport> {
    match config.transport {
        TransportRequest::Explicit(transport) => {
            probe.viability(transport).map_err(|reason| {
                Error::unavailable(transport.as_str(), reason)
            })?;

            if Transport::GpuShared == transport {
                device_match(config, probe).map_err(|actual| {
                    Error::DeviceMismatch {
                        requested: config
                            .device_uuid
                            .clone()
                            .unwrap_or_default(),
                        actual,
                    }
                })?;
            }

            Ok(transport)
        }
        TransportRequest::Auto => Transport::AUTO_ORDER
            .into_iter()
            .find(|transport| {
                probe.viability(*transport).is_ok()
                    && (Transport::GpuShared != *transport
                        || device_match(config, probe).is_ok())
            })
            .ok_or_else(|| {
                Error::unavailable(
                    "auto",
                    "no viable transport: gpu, shm and callback all \
                     reported unavailable",
                )
            }),
    }
}

/// `Ok(())` when the driver's adapter satisfies `stream.device.uuid`;
/// `Err(actual)` otherwise.
fn device_match(
    config: &StreamConfig,
    probe: &dyn TransportProbe,
) -> core::result::Result<(), String> {
    config.device_uuid.as_ref().map_or(Ok(()), |requested| {
        match probe.device_uuid() {
            Some(actual) if &actual == requested => Ok(()),
            Some(actual) => Err(actual),
            None => Err("<no gpu adapter>".to_string()),
        }
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_wire_round_trip() {
        Transport::AUTO_ORDER.into_iter().for_each(|transport| {
            assert_eq!(
                Transport::from_wire(transport.as_wire()),
                Some(transport)
            );
        });

        assert_eq!(Transport::from_wire(0), None);
    }

    #[test]
    fn auto_prefers_gpu() {
        let config = StreamConfig::default();
        let probe = StaticProbe::all_viable();

        assert_eq!(
            negotiate(&config, &probe).expect("a viable transport"),
            Transport::GpuShared
        );
    }
}
