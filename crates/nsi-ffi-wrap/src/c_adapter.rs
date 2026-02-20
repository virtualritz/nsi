//! C API adapter for [`Nsi`] trait implementations.
//!
//! This module provides [`FfiApiAdapter`], which wraps an [`Nsi`] implementation
//! and manages the mapping between C API integer context handles and Rust
//! context handles.

use crate::{
    ArgSlice,
    nsi_trait::{Action, NodeType, Nsi},
};
use std::{
    collections::HashMap,
    ffi::c_int,
    sync::{
        Mutex,
        atomic::{AtomicI32, Ordering},
    },
};

/// Adapter that exposes an [`Nsi`] implementation through a C-compatible interface.
///
/// The adapter manages a mapping from integer context IDs (as used by the C API)
/// to the renderer's native handle type.
///
/// # Thread Safety
///
/// The adapter is thread-safe and can be used from multiple threads concurrently.
/// It uses interior mutability to manage the context map.
///
/// # Example
///
/// ```ignore
/// use nsi_ffi_wrap::c_adapter::FfiApiAdapter;
///
/// // Create adapter wrapping a renderer implementation
/// let adapter = FfiApiAdapter::new(MyRenderer::new());
///
/// // Create a new context (returns integer ID for C API)
/// let ctx_id = adapter.begin(None)?;
///
/// // Use the context...
/// adapter.create(ctx_id, "mesh1", NodeType::Mesh, None)?;
///
/// // Clean up
/// adapter.end(ctx_id)?;
/// ```
pub struct FfiApiAdapter<T: Nsi> {
    /// The underlying renderer implementation.
    renderer: T,
    /// Map from C API integer context IDs to renderer handles.
    contexts: Mutex<HashMap<c_int, T::Handle>>,
    /// Counter for generating unique context IDs.
    next_id: AtomicI32,
}

impl<T: Nsi> FfiApiAdapter<T> {
    /// Create a new adapter wrapping the given renderer.
    pub fn new(renderer: T) -> Self {
        Self {
            renderer,
            contexts: Mutex::new(HashMap::new()),
            // Start at 1; 0 is typically NSI_BAD_CONTEXT.
            next_id: AtomicI32::new(1),
        }
    }

    /// Get a reference to the underlying renderer.
    #[inline]
    pub fn renderer(&self) -> &T {
        &self.renderer
    }

    /// Look up a renderer handle by its C API context ID.
    fn lookup_handle(&self, ctx: c_int) -> Option<T::Handle> {
        self.contexts.lock().ok()?.get(&ctx).cloned()
    }

    // ─── C API Equivalents ───────────────────────────────────────────────

    /// Create a new rendering context.
    ///
    /// Returns a context ID for use with other C API functions.
    /// Returns 0 (NSI_BAD_CONTEXT) on failure.
    pub fn begin(&self, args: Option<&ArgSlice>) -> c_int {
        match self.renderer.begin(args) {
            Ok(handle) => {
                let id = self.next_id.fetch_add(1, Ordering::SeqCst);
                if let Ok(mut contexts) = self.contexts.lock() {
                    contexts.insert(id, handle);
                    id
                } else {
                    // Lock poisoned.
                    0
                }
            }
            Err(_) => 0,
        }
    }

    /// Destroy a rendering context.
    pub fn end(&self, ctx: c_int) {
        if let Ok(mut contexts) = self.contexts.lock()
            && let Some(handle) = contexts.remove(&ctx)
        {
            let _ = self.renderer.end(&handle);
        }
    }

    /// Create a new node in the scene graph.
    pub fn create(
        &self,
        ctx: c_int,
        handle: &str,
        node_type: NodeType,
        args: Option<&ArgSlice>,
    ) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.create(&ctx_handle, handle, node_type, args);
        }
    }

    /// Delete a node from the scene graph.
    pub fn delete(&self, ctx: c_int, handle: &str, args: Option<&ArgSlice>) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.delete(&ctx_handle, handle, args);
        }
    }

    /// Set attributes on a node.
    pub fn set_attribute(&self, ctx: c_int, handle: &str, args: &ArgSlice) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.set_attribute(&ctx_handle, handle, args);
        }
    }

    /// Set attributes on a node at a specific time.
    pub fn set_attribute_at_time(
        &self,
        ctx: c_int,
        handle: &str,
        time: f64,
        args: &ArgSlice,
    ) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.set_attribute_at_time(
                &ctx_handle,
                handle,
                time,
                args,
            );
        }
    }

    /// Delete an attribute from a node.
    pub fn delete_attribute(&self, ctx: c_int, handle: &str, name: &str) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.delete_attribute(&ctx_handle, handle, name);
        }
    }

    /// Connect two nodes in the scene graph.
    pub fn connect(
        &self,
        ctx: c_int,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Option<&ArgSlice>,
    ) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.connect(
                &ctx_handle,
                from,
                from_attr,
                to,
                to_attr,
                args,
            );
        }
    }

    /// Disconnect two nodes in the scene graph.
    pub fn disconnect(
        &self,
        ctx: c_int,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.disconnect(
                &ctx_handle,
                from,
                from_attr,
                to,
                to_attr,
            );
        }
    }

    /// Evaluate procedural nodes or Lua scripts.
    pub fn evaluate(&self, ctx: c_int, args: Option<&ArgSlice>) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.evaluate(&ctx_handle, args);
        }
    }

    /// Control the rendering process.
    pub fn render_control(
        &self,
        ctx: c_int,
        action: Action,
        args: Option<&ArgSlice>,
    ) {
        if let Some(ctx_handle) = self.lookup_handle(ctx) {
            let _ = self.renderer.render_control(&ctx_handle, action, args);
        }
    }
}

// Safety: FfiApiAdapter is Send + Sync because:
// - T: Nsi requires Send + Sync
// - contexts uses Mutex for interior mutability
// - next_id uses AtomicI32
unsafe impl<T: Nsi> Send for FfiApiAdapter<T> {}
unsafe impl<T: Nsi> Sync for FfiApiAdapter<T> {}
