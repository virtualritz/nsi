//! Core ɴsɪ trait, parameter abstraction, and FFI-compatible types.

use bitflags::bitflags;

// ─── Type Enum ──────────────────────────────────────────────────────────────

/// NSI data type discriminant, binary-compatible with `NSIType_t` from `nsi.h`.
///
/// Values use bit flags from the C header -- they are NOT sequential.
#[repr(i32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Invalid = 0,
    Float = 1,
    Double = 0x11,
    Integer = 2,
    String = 3,
    Color = 4,
    Point = 5,
    Vector = 6,
    Normal = 7,
    Matrix = 8,
    DoubleMatrix = 0x18,
    /// Called "Pointer" in the C API; renamed for clarity.
    Reference = 9,
}

// ─── Flags ──────────────────────────────────────────────────────────────────

bitflags! {
    /// Parameter flags matching the C API constants.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Flags: i32 {
        const IS_ARRAY = 1;
        const PER_FACE = 2;
        const PER_VERTEX = 4;
        const INTERPOLATE_LINEAR = 8;
    }
}

// ─── FfiParam ───────────────────────────────────────────────────────────────

/// C-compatible parameter struct, layout-identical to `NSIParam_t`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FfiParam {
    pub name: *const core::ffi::c_char,
    pub data: *const core::ffi::c_void,
    pub type_: core::ffi::c_int,
    pub arraylength: core::ffi::c_int,
    pub count: usize,
    pub flags: core::ffi::c_int,
}

// SAFETY: FfiParam is a POD struct with raw pointers that are only
// dereferenced by the C API on the same thread that calls it.
unsafe impl Send for FfiParam {}
unsafe impl Sync for FfiParam {}

// ─── Name Type ──────────────────────────────────────────────────────────────

/// Interned string type when the `ustr` feature is enabled, otherwise `String`.
#[cfg(feature = "ustr")]
pub type Name = ustr::Ustr;

/// Plain `String` when the `ustr` feature is not enabled.
#[cfg(not(feature = "ustr"))]
pub type Name = String;

// ─── Parameter Trait ────────────────────────────────────────────────────────

/// A single ɴsɪ parameter (name + typed data).
///
/// Lifetime-free -- implementors own or borrow their data as they see fit.
/// Mirrors the published crate's internal `ArgDataMethods` + `Arg` metadata.
pub trait Parameter {
    /// Parameter name.
    fn name(&self) -> &str;

    /// NSI type discriminant.
    fn type_tag(&self) -> Type;

    /// Number of data elements.
    fn len(&self) -> usize;

    /// Whether the parameter carries zero elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Array length (for array-of-arrays; default 1).
    fn array_length(&self) -> usize {
        1
    }

    /// Parameter flags (PerFace, PerVertex, IsArray, InterpolateLinear).
    fn flags(&self) -> i32 {
        0
    }

    /// FFI fast-path: return a C-compatible param struct if the layout
    /// supports it.
    ///
    /// Returns `Some(FfiParam)` when the data is already in C-compatible
    /// layout. Returns `None` when the consumer must construct a temporary
    /// `FfiParam`.
    ///
    /// # Safety
    ///
    /// The returned `FfiParam`'s pointers are valid only while `self` is
    /// alive.
    fn as_c_param(&self) -> Option<FfiParam>;
}

// ─── Action Enum ────────────────────────────────────────────────────────────

/// Actions for render control.
///
/// These control the rendering process after it has been started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Start rendering. Returns immediately, rendering proceeds in parallel.
    Start,
    /// Wait for rendering to complete. Blocks until finished or stopped.
    Wait,
    /// Synchronize scene changes for interactive renders.
    Synchronize,
    /// Temporarily pause the render process.
    Suspend,
    /// Continue a previously suspended render.
    Resume,
    /// Stop rendering without destroying the scene.
    Stop,
}

impl Action {
    /// Returns the string identifier used by the C API.
    #[inline]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Wait => "wait",
            Self::Synchronize => "synchronize",
            Self::Suspend => "suspend",
            Self::Resume => "resume",
            Self::Stop => "stop",
        }
    }

    /// Parse an action from its string identifier.
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "start" => Self::Start,
            "wait" => Self::Wait,
            "synchronize" => Self::Synchronize,
            "suspend" => Self::Suspend,
            "resume" => Self::Resume,
            "stop" => Self::Stop,
            _ => return None,
        })
    }
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ─── Nsi Trait ──────────────────────────────────────────────────────────────

/// Core ɴsɪ interface trait.
///
/// `Self` IS the rendering context. There is no separate handle type --
/// construction is implementation-specific, destruction is via `Drop`.
///
/// # Generic Associated Type
///
/// `Arg<'call>` uses a GAT with a transient borrow lifetime (`'call`).
/// The context-bound lifetime (for References/Callbacks that must outlive
/// the context) is baked into the implementor's concrete `Arg` type.
///
/// # Thread Safety
///
/// All methods take `&self`. Implementors use interior mutability where
/// needed.
///
/// # String-Based Node Types
///
/// Methods accept `&str` for node types to allow custom types beyond the
/// standard specification. Use constants like [`MESH`](crate::MESH),
/// [`SHADER`](crate::SHADER), etc. for standard types.
pub trait Nsi: Send + Sync {
    /// Argument type -- each implementor picks its own.
    ///
    /// `'call` is the transient borrow lifetime (data copied by C side).
    /// The context-bound lifetime (for References/Callbacks) is baked
    /// into the implementor's concrete Arg type.
    type Arg<'call>: Parameter
    where
        Self: 'call;

    /// Error type for fallible operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Create a new node in the scene graph.
    fn create(
        &self,
        handle: &str,
        node_type: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error>;

    /// Delete a node from the scene graph.
    fn delete(
        &self,
        handle: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error>;

    /// Set attributes on a node.
    fn set_attribute(
        &self,
        handle: &str,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error>;

    /// Set attributes on a node at a specific time (for motion blur).
    fn set_attribute_at_time(
        &self,
        handle: &str,
        time: f64,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error>;

    /// Delete an attribute from a node.
    fn delete_attribute(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<(), Self::Error>;

    /// Connect two nodes in the scene graph.
    fn connect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error>;

    /// Disconnect two nodes in the scene graph.
    fn disconnect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), Self::Error>;

    /// Evaluate procedural nodes or Lua scripts.
    fn evaluate(&self, args: &[Self::Arg<'_>]) -> Result<(), Self::Error>;

    /// Control the rendering process.
    fn render_control(
        &self,
        action: Action,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error>;
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_round_trip() {
        let actions = [
            Action::Start,
            Action::Wait,
            Action::Synchronize,
            Action::Suspend,
            Action::Resume,
            Action::Stop,
        ];

        for action in actions {
            let s = action.as_str();
            let parsed = Action::from_name(s).expect("should parse");
            assert_eq!(action, parsed);
        }
    }

    #[test]
    fn type_values_match_c_header() {
        assert_eq!(Type::Invalid as i32, 0);
        assert_eq!(Type::Float as i32, 1);
        assert_eq!(Type::Double as i32, 0x11);
        assert_eq!(Type::Integer as i32, 2);
        assert_eq!(Type::String as i32, 3);
        assert_eq!(Type::Color as i32, 4);
        assert_eq!(Type::Point as i32, 5);
        assert_eq!(Type::Vector as i32, 6);
        assert_eq!(Type::Normal as i32, 7);
        assert_eq!(Type::Matrix as i32, 8);
        assert_eq!(Type::DoubleMatrix as i32, 0x18);
        assert_eq!(Type::Reference as i32, 9);
    }

    #[test]
    fn flags_combine() {
        let f = Flags::PER_VERTEX | Flags::IS_ARRAY;
        assert!(f.contains(Flags::PER_VERTEX));
        assert!(f.contains(Flags::IS_ARRAY));
        assert!(!f.contains(Flags::PER_FACE));
        // 4 | 1.
        assert_eq!(f.bits(), 5);
    }

    #[test]
    fn ffi_param_layout() {
        // Verify field count / size is reasonable for a 6-field repr(C) struct.
        // Exact size depends on platform, but must be at least 6 * pointer-size
        // on 64-bit or a mix of i32 + pointer on 32-bit.
        assert!(std::mem::size_of::<FfiParam>() >= 6 * 4);
    }
}
