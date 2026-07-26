//! The `stream.*` attribute vocabulary -- version 1.
//!
//! # Wire Format (frozen at `stream.version` 1)
//!
//! The vocabulary is the client → renderer half of the contract. It is set
//! with plain `NSISetAttribute` calls on a standard `outputdriver` node
//! (`drivername "nsi-stream"`) and forwarded verbatim to the driver by any
//! conforming ɴsɪ implementation (R1). No new API entry point exists.
//!
//! | Attribute | ɴsɪ type | Req. | Meaning |
//! | --- | --- | --- | --- |
//! | `stream.version` | `int` | yes | Vocabulary version. Only `1` is supported. |
//! | `stream.transport` | `string` | no | `"auto"` (default), `"gpu"`, `"shm"`, `"callback"`. |
//! | `stream.publish` | `string` | no | `"commit"` (default) or `"continuous"`. |
//! | `stream.ring` | `int` | no | Ring size, default `3`, minimum `2`. |
//! | `stream.channel` | `string` | no | Rendezvous endpoint name (local socket). |
//! | `stream.device.uuid` | `string` | no | Adapter UUID the client renders on. |
//! | `stream.callback.open` | `pointer` | no | In-process open notification. |
//! | `stream.callback.publish` | `pointer` | no | In-process publication notification. |
//! | `stream.callback.close` | `pointer` | no | In-process close notification. |
//! | `stream.onclientloss` | `string` | no | `"continue"` (default) or `"stop"`. |
//!
//! # Compatibility And Versioning
//!
//! - `stream.version` is mandatory. A missing version is
//!   [`Error::MissingAttribute`], an unknown version is
//!   [`Error::UnsupportedVersion`] (R7). There is no default and no silent
//!   downgrade.
//! - Adding an attribute to this table is a version bump.
//! - An **unknown** `stream.*` attribute is not an error: it is reported as
//!   a [`Warning`] so a version-1 driver can consume a scene authored for a
//!   later vocabulary without aborting the render.
//! - An attribute outside the `stream.` namespace is ignored entirely and
//!   without a warning -- it belongs to the driver's other consumers.
//! - A known attribute carrying the wrong ɴsɪ type is
//!   [`Error::MalformedAttribute`], never a warning: a mistyped required
//!   identifier must fail loudly.

use crate::{
    Error, Result,
    transport::{Transport, TransportRequest},
};
use core::ffi::c_void;
use nsi_trait::Type;

/// The vocabulary version this build implements.
pub const SUPPORTED_VERSION: i32 = 1;

/// The `drivername` that addresses this driver.
pub const DRIVER_NAME: &str = "nsi-stream";

/// The attribute namespace owned by this contract.
pub const NAMESPACE: &str = "stream.";

// ─── Attribute ──────────────────────────────────────────────────────────────

/// An opaque pointer attribute value.
///
/// The pointer is never dereferenced by this crate; it is handed back to the
/// in-process callback transport, whose FFI glue owns its meaning. Wrapping
/// it keeps [`StreamConfig`] `Send`/`Sync` without weakening anything: this
/// crate only ever compares and copies the address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallbackPointer(*const c_void);

// SAFETY: `CallbackPointer` is an opaque address. This crate never
// dereferences it and never derives a reference from it, so moving one
// across threads cannot create a data race. Dereferencing happens only in
// the FFI glue that produced the pointer, under its own contract.
unsafe impl Send for CallbackPointer {}
// SAFETY: see the `Send` impl -- shared access exposes the address only.
unsafe impl Sync for CallbackPointer {}

impl CallbackPointer {
    /// Wrap a raw pointer.
    #[inline]
    pub const fn new(pointer: *const c_void) -> Self {
        Self(pointer)
    }

    /// The wrapped address.
    #[inline]
    pub const fn as_ptr(self) -> *const c_void {
        self.0
    }

    /// Whether the address is null.
    #[inline]
    pub fn is_null(self) -> bool {
        self.0.is_null()
    }
}

/// The value of one attribute, in the three ɴsɪ types this vocabulary uses.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AttrValue {
    /// [`Type::I32`].
    Int(i32),
    /// [`Type::String`].
    String(String),
    /// [`Type::Reference`] -- called "pointer" in the C API.
    Pointer(CallbackPointer),
}

impl AttrValue {
    /// The ɴsɪ type discriminant of this value.
    #[inline]
    pub const fn type_tag(&self) -> Type {
        match self {
            Self::Int(_) => Type::I32,
            Self::String(_) => Type::String,
            Self::Pointer(_) => Type::Reference,
        }
    }

    /// Human-readable type name, for diagnostics.
    #[inline]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Int(_) => "int",
            Self::String(_) => "string",
            Self::Pointer(_) => "pointer",
        }
    }
}

impl From<i32> for AttrValue {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<&str> for AttrValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<String> for AttrValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<*const c_void> for AttrValue {
    fn from(value: *const c_void) -> Self {
        Self::Pointer(CallbackPointer::new(value))
    }
}

impl From<CallbackPointer> for AttrValue {
    fn from(value: CallbackPointer) -> Self {
        Self::Pointer(value)
    }
}

/// One attribute as an output driver receives it: a name plus a typed value.
///
/// This is the owned, allocation-friendly shape the driver decodes. The FFI
/// glue that turns `NSIParam_t` into [`Attr`] lives with the renderer
/// bridge, not here.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attr {
    /// Attribute name.
    pub name: String,
    /// Attribute value.
    pub value: AttrValue,
}

impl Attr {
    /// Construct an attribute from anything that converts into an
    /// [`AttrValue`].
    pub fn new(name: impl Into<String>, value: impl Into<AttrValue>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Construct an `int` attribute.
    pub fn int(name: impl Into<String>, value: i32) -> Self {
        Self::new(name, value)
    }

    /// Construct a `string` attribute.
    pub fn string(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(name, value.into())
    }

    /// Construct a `pointer` attribute.
    pub fn pointer(name: impl Into<String>, value: *const c_void) -> Self {
        Self::new(name, value)
    }
}

// ─── Warning ────────────────────────────────────────────────────────────────

/// A non-fatal vocabulary diagnostic.
///
/// Warnings are returned alongside the [`StreamConfig`]; they never abort
/// the parse.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Warning {
    /// The attribute the warning is about.
    pub attribute: String,
    /// Why it was not consumed.
    pub reason: String,
}

impl Warning {
    fn unknown(attribute: &str) -> Self {
        Self {
            attribute: attribute.to_string(),
            reason: format!(
                "unknown `{NAMESPACE}*` attribute for `stream.version` \
                 {SUPPORTED_VERSION}; ignored"
            ),
        }
    }
}

impl core::fmt::Display for Warning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "`{}`: {}", self.attribute, self.reason)
    }
}

// ─── Enumerations ───────────────────────────────────────────────────────────

/// The `stream.publish` mode (R5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PublishMode {
    /// One atomic publication per applied `synchronize`. Progressive
    /// refinement is invisible to the client.
    #[default]
    Commit,
    /// Progressive accumulation may be published between commits. Every
    /// publication still carries exactly one scene generation.
    Continuous,
}

impl PublishMode {
    /// The `stream.publish` spelling.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Continuous => "continuous",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "commit" => Ok(Self::Commit),
            "continuous" => Ok(Self::Continuous),
            other => Err(Error::malformed(
                "stream.publish",
                format!("expected `commit` or `continuous`, got `{other}`"),
            )),
        }
    }
}

impl core::fmt::Display for PublishMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The `stream.onclientloss` policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClientLoss {
    /// Keep rendering; publications are dropped and counted.
    #[default]
    Continue,
    /// Raise the stop flag so the renderer can abort the render.
    Stop,
}

impl ClientLoss {
    /// The `stream.onclientloss` spelling.
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Stop => "stop",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "continue" => Ok(Self::Continue),
            "stop" => Ok(Self::Stop),
            other => Err(Error::malformed(
                "stream.onclientloss",
                format!("expected `continue` or `stop`, got `{other}`"),
            )),
        }
    }
}

impl core::fmt::Display for ClientLoss {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Callback Pointers ──────────────────────────────────────────────────────

/// The three `stream.callback.*` pointer attributes.
///
/// Pointer-typed attributes are legal only for the in-process callback
/// transport (R2, `attribute-vocabulary.md` invariants).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CallbackPointers {
    /// `stream.callback.open`.
    pub open: Option<CallbackPointer>,
    /// `stream.callback.publish`.
    pub publish: Option<CallbackPointer>,
    /// `stream.callback.close`.
    pub close: Option<CallbackPointer>,
}

impl CallbackPointers {
    /// Whether any pointer was set.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.open.is_none() && self.publish.is_none() && self.close.is_none()
    }
}

// ─── StreamConfig ───────────────────────────────────────────────────────────

/// The decoded `stream.*` attribute set.
///
/// Produced by [`StreamConfig::parse`]; consumed by
/// [`StreamDriver::open`](crate::StreamDriver::open).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamConfig {
    /// `stream.version`.
    pub version: i32,
    /// `stream.transport`.
    pub transport: TransportRequest,
    /// `stream.publish`.
    pub publish: PublishMode,
    /// `stream.ring`.
    pub ring: usize,
    /// `stream.channel`.
    pub channel: Option<String>,
    /// `stream.device.uuid`.
    pub device_uuid: Option<String>,
    /// `stream.onclientloss`.
    pub on_client_loss: ClientLoss,
    /// `stream.callback.*`.
    pub callbacks: CallbackPointers,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            version: SUPPORTED_VERSION,
            transport: TransportRequest::default(),
            publish: PublishMode::default(),
            ring: Self::DEFAULT_RING,
            channel: None,
            device_uuid: None,
            on_client_loss: ClientLoss::default(),
            callbacks: CallbackPointers::default(),
        }
    }
}

impl StreamConfig {
    /// Default ring size when `stream.ring` is not set.
    pub const DEFAULT_RING: usize = 3;

    /// Smallest legal ring size (R3).
    pub const MIN_RING: usize = 2;

    /// Decode a version-1 `stream.*` attribute set.
    ///
    /// Returns the configuration plus the [`Warning`]s collected for
    /// unknown `stream.*` attributes. Attributes outside the `stream.`
    /// namespace are ignored silently -- they are forwarded to the driver
    /// for other purposes and are none of this parser's business.
    ///
    /// # Errors
    ///
    /// - [`Error::MissingAttribute`] -- `stream.version` was not set.
    /// - [`Error::UnsupportedVersion`] -- `stream.version` is not
    ///   [`SUPPORTED_VERSION`].
    /// - [`Error::MalformedAttribute`] -- a known attribute has the wrong
    ///   ɴsɪ type or an out-of-range value.
    pub fn parse(attributes: &[Attr]) -> Result<(Self, Vec<Warning>)> {
        // The version gate runs before anything else is decoded: a driver
        // must not act on a table it does not understand.
        let version = attributes
            .iter()
            .rfind(|attr| attr.name == "stream.version")
            .ok_or_else(|| Error::MissingAttribute {
                name: "stream.version".to_string(),
            })
            .and_then(|attr| int(attr).copied())?;

        if SUPPORTED_VERSION != version {
            Err(Error::UnsupportedVersion {
                requested: version,
                supported: SUPPORTED_VERSION,
            })?;
        }

        let mut config = Self::default();
        let mut warnings = Vec::new();

        attributes
            .iter()
            .filter(|attr| attr.name.starts_with(NAMESPACE))
            .try_for_each(|attr| config.apply(attr, &mut warnings))?;

        Ok((config, warnings))
    }

    /// Whether the configuration asks for the callback transport in any
    /// form.
    pub fn wants_callbacks(&self) -> bool {
        !self.callbacks.is_empty()
            || self.transport == TransportRequest::Explicit(Transport::Callback)
    }

    fn apply(
        &mut self,
        attr: &Attr,
        warnings: &mut Vec<Warning>,
    ) -> Result<()> {
        match attr.name.as_str() {
            // Already validated above; re-reading it here keeps the match
            // exhaustive over the frozen table.
            "stream.version" => {
                self.version = *int(attr)?;
            }
            "stream.transport" => {
                self.transport = TransportRequest::parse(string(attr)?)?;
            }
            "stream.publish" => {
                self.publish = PublishMode::parse(string(attr)?)?;
            }
            "stream.ring" => {
                let ring = *int(attr)?;
                if ring < Self::MIN_RING as i32 {
                    Err(Error::malformed(
                        "stream.ring",
                        format!(
                            "ring size must be at least {}, got {ring}",
                            Self::MIN_RING
                        ),
                    ))?;
                }
                self.ring = ring as usize;
            }
            "stream.channel" => {
                self.channel = Some(non_empty(attr)?.to_string());
            }
            "stream.device.uuid" => {
                self.device_uuid = Some(non_empty(attr)?.to_string());
            }
            "stream.onclientloss" => {
                self.on_client_loss = ClientLoss::parse(string(attr)?)?;
            }
            "stream.callback.open" => {
                self.callbacks.open = Some(*pointer(attr)?);
            }
            "stream.callback.publish" => {
                self.callbacks.publish = Some(*pointer(attr)?);
            }
            "stream.callback.close" => {
                self.callbacks.close = Some(*pointer(attr)?);
            }
            unknown => warnings.push(Warning::unknown(unknown)),
        }

        Ok(())
    }
}

// ─── Typed Accessors ────────────────────────────────────────────────────────

fn wrong_type(attr: &Attr, expected: &str) -> Error {
    Error::malformed(
        &attr.name,
        format!(
            "expected ɴsɪ type `{expected}`, got `{}`",
            attr.value.type_name()
        ),
    )
}

fn int(attr: &Attr) -> Result<&i32> {
    match &attr.value {
        AttrValue::Int(value) => Ok(value),
        _ => Err(wrong_type(attr, "int")),
    }
}

fn string(attr: &Attr) -> Result<&str> {
    match &attr.value {
        AttrValue::String(value) => Ok(value.as_str()),
        _ => Err(wrong_type(attr, "string")),
    }
}

fn non_empty(attr: &Attr) -> Result<&str> {
    string(attr).and_then(|value| {
        if value.is_empty() {
            Err(Error::malformed(&attr.name, "must not be empty"))
        } else {
            Ok(value)
        }
    })
}

fn pointer(attr: &Attr) -> Result<&CallbackPointer> {
    match &attr.value {
        AttrValue::Pointer(value) => Ok(value),
        _ => Err(wrong_type(attr, "pointer")),
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_documented_ones() {
        let (config, warnings) =
            StreamConfig::parse(&[Attr::int("stream.version", 1)])
                .expect("version 1 parses");

        assert!(warnings.is_empty());
        assert_eq!(config.transport, TransportRequest::Auto);
        assert_eq!(config.publish, PublishMode::Commit);
        assert_eq!(config.ring, StreamConfig::DEFAULT_RING);
        assert_eq!(config.on_client_loss, ClientLoss::Continue);
        assert_eq!(config.channel, None);
        assert_eq!(config.device_uuid, None);
    }

    #[test]
    fn ring_below_minimum_is_malformed() {
        let error = StreamConfig::parse(&[
            Attr::int("stream.version", 1),
            Attr::int("stream.ring", 1),
        ])
        .expect_err("ring 1 is illegal");

        assert!(matches!(
            error,
            Error::MalformedAttribute { ref name, .. } if name == "stream.ring"
        ));
    }

    #[test]
    fn wrong_type_is_malformed() {
        let error = StreamConfig::parse(&[
            Attr::int("stream.version", 1),
            Attr::string("stream.ring", "3"),
        ])
        .expect_err("string ring is illegal");

        assert!(matches!(
            error,
            Error::MalformedAttribute { ref name, .. } if name == "stream.ring"
        ));
    }
}
