//! C API adapter for [`Nsi`] trait implementations.
//!
//! The adapter takes a factory closure that constructs a fresh [`Nsi`] impl
//! on each `NSIBegin` from C. Each constructed instance is stored under a
//! generated integer ID; subsequent C calls dispatch through the trait
//! methods of the right instance. `NSIEnd` removes the instance from the
//! map; its `Drop` then releases any renderer-side state.
//!
//! There is **no** separate FFI-shape backend trait -- the canonical
//! [`Nsi`] (`self` _is_ the context) is the only NSI trait, and the handle
//! mapping needed by the C API lives entirely inside this adapter.

use crate::{Arg, ArgSlice};
use ::nsi_trait::{Action, NodeType, Nsi};
use std::{
    collections::HashMap,
    ffi::c_int,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI32, Ordering},
    },
};

/// Adapter that exposes an [`Nsi`] implementation through the C API.
///
/// The adapter is constrained to backends whose `Arg<'a>` is
/// [`Arg<'a>`](crate::Arg) -- the FFI-friendly parameter type that
/// `nsi-ffi-wrap` produces from C `NSIParam` arrays. Pure-Rust callers
/// that don't need to be exposed via C-FFI implement [`Nsi`] freely with
/// any `Arg` type and don't go through this adapter.
///
/// # Example
///
/// ```ignore
/// use nsi_ffi_wrap::{c_adapter::FfiApiAdapter, NodeType};
///
/// struct MyRenderer { /* ... */ }
/// impl ::nsi_trait::Nsi for MyRenderer {
///     type Arg<'call> = nsi_ffi_wrap::Arg<'call> where Self: 'call;
///     /* ... method impls ... */
/// }
///
/// let adapter = FfiApiAdapter::<MyRenderer>::new(|| MyRenderer::new());
/// let ctx = adapter.begin(None);
/// adapter.create(ctx, "mesh1", NodeType::Mesh, None);
/// adapter.end(ctx);
/// ```
pub struct FfiApiAdapter<T>
where
    T: Nsi + 'static,
    for<'a> T: Nsi<Arg<'a> = Arg<'a, 'a>>,
{
    /// Constructs a fresh `T` for each `NSIBegin` from C.
    factory: Box<dyn Fn() -> T + Send + Sync>,
    /// Map from C-side integer context IDs to live renderer instances.
    contexts: Mutex<HashMap<c_int, Arc<T>>>,
    /// Counter for generating unique context IDs.
    next_id: AtomicI32,
}

impl<T> FfiApiAdapter<T>
where
    T: Nsi + 'static,
    for<'a> T: Nsi<Arg<'a> = Arg<'a, 'a>>,
{
    /// Create a new adapter with the given factory closure.
    pub fn new<F>(factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            factory: Box::new(factory),
            contexts: Mutex::new(HashMap::new()),
            // Start at 1; 0 is typically NSI_BAD_CONTEXT.
            next_id: AtomicI32::new(1),
        }
    }

    /// Look up a live `T` by its C API context ID, releasing the map lock.
    #[inline]
    fn lookup(&self, ctx: c_int) -> Option<Arc<T>> {
        self.contexts.lock().ok()?.get(&ctx).cloned()
    }

    // ─── C API equivalents ───────────────────────────────────────────────

    /// `NSIBegin` -- construct a new `T` and return its integer ID.
    /// Returns 0 (`NSI_BAD_CONTEXT`) if the context map lock is poisoned.
    pub fn begin(&self, _args: Option<&ArgSlice>) -> c_int {
        // The canonical Nsi trait has no begin() -- `self` is the context, so
        // construction happens here via the factory. Begin args from C are
        // ignored (3Delight does the same on its end).
        let nsi = (self.factory)();
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut contexts) = self.contexts.lock() {
            contexts.insert(id, Arc::new(nsi));
            id
        } else {
            0
        }
    }

    /// `NSIEnd` -- remove the `T` from the map. Drop runs on last Arc release.
    pub fn end(&self, ctx: c_int) {
        if let Ok(mut contexts) = self.contexts.lock() {
            contexts.remove(&ctx);
        }
    }

    /// `NSICreate` -- create a node in the addressed context.
    pub fn create(
        &self,
        ctx: c_int,
        handle: &str,
        node_type: NodeType,
        args: Option<&ArgSlice>,
    ) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.create(handle, node_type.as_str(), args);
        }
    }

    /// `NSIDelete` -- delete a node.
    pub fn delete(&self, ctx: c_int, handle: &str, args: Option<&ArgSlice>) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.delete(handle, args);
        }
    }

    /// `NSISetAttribute` -- set attributes on a node.
    pub fn set_attribute(&self, ctx: c_int, handle: &str, args: &ArgSlice) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.set_attribute(handle, args);
        }
    }

    /// `NSISetAttributeAtTime` -- set time-sampled attributes on a node.
    pub fn set_attribute_at_time(
        &self,
        ctx: c_int,
        handle: &str,
        time: f64,
        args: &ArgSlice,
    ) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.set_attribute_at_time(handle, time, args);
        }
    }

    /// `NSIDeleteAttribute` -- remove a single attribute by name.
    pub fn delete_attribute(&self, ctx: c_int, handle: &str, name: &str) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.delete_attribute(handle, name);
        }
    }

    /// `NSIConnect` -- connect two nodes.
    pub fn connect(
        &self,
        ctx: c_int,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Option<&ArgSlice>,
    ) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.connect(from, from_attr, to, to_attr, args);
        }
    }

    /// `NSIDisconnect` -- disconnect two nodes.
    pub fn disconnect(
        &self,
        ctx: c_int,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.disconnect(from, from_attr, to, to_attr);
        }
    }

    /// `NSIEvaluate` -- evaluate a procedural/Lua block.
    pub fn evaluate(&self, ctx: c_int, args: Option<&ArgSlice>) {
        if let Some(nsi) = self.lookup(ctx) {
            // canonical Nsi::evaluate takes &[Self::Arg] not Option;
            // pass an empty slice for None.
            let empty: &[Arg<'_, '_>] = &[];
            let a = args.unwrap_or(empty);
            let _ = nsi.evaluate(a);
        }
    }

    /// `NSIRenderControl` -- start/stop/wait/etc.
    pub fn render_control(
        &self,
        ctx: c_int,
        action: Action,
        args: Option<&ArgSlice>,
    ) {
        if let Some(nsi) = self.lookup(ctx) {
            let _ = nsi.render_control(action, args);
        }
    }
}

// Safety: every field is independently Send + Sync:
// - `factory` is a Box<dyn Fn() + Send + Sync>
// - `contexts` is a Mutex<HashMap<c_int, Arc<T>>>; T: Send + Sync via Nsi
// - `next_id` is AtomicI32
// The auto-derive would also work; declared explicitly for clarity.
unsafe impl<T> Send for FfiApiAdapter<T>
where
    T: Nsi + 'static,
    for<'a> T: Nsi<Arg<'a> = Arg<'a, 'a>>,
{
}
unsafe impl<T> Sync for FfiApiAdapter<T>
where
    T: Nsi + 'static,
    for<'a> T: Nsi<Arg<'a> = Arg<'a, 'a>>,
{
}
