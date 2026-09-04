//! `Nsi` implementation for the 3Delight-backed [`Context`].
//!
//! Every method delegates to the inherent method of the same name. The
//! signatures already match; the only adaptation is wrapping the unit
//! return in `Ok`.
//!
//! The ɴsɪ C API surfaces no per-call error. Failures are delivered to
//! the `errorhandler` callback installed by [`Context::new`], so there
//! is nothing to return and the error type is [`Infallible`].

use crate::{Arg, Context};
use ::nsi_trait::{Action, Nsi};
use core::convert::Infallible;

impl<'a> Nsi for Context<'a> {
    /// `'call` is the transient borrow; `'a` is pinned to this context,
    /// which is what `ArgData`'s second lifetime means.
    type Arg<'call> = Arg<'call, 'a>;
    type Error = Infallible;

    #[inline]
    fn create(
        &self,
        handle: &str,
        node_type: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Context::create(self, handle, node_type, args);
        Ok(())
    }

    #[inline]
    fn delete(
        &self,
        handle: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Context::delete(self, handle, args);
        Ok(())
    }

    #[inline]
    fn set_attribute(
        &self,
        handle: &str,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        Context::set_attribute(self, handle, args);
        Ok(())
    }

    #[inline]
    fn set_attribute_at_time(
        &self,
        handle: &str,
        time: f64,
        args: &[Self::Arg<'_>],
    ) -> Result<(), Self::Error> {
        Context::set_attribute_at_time(self, handle, time, args);
        Ok(())
    }

    #[inline]
    fn delete_attribute(
        &self,
        handle: &str,
        name: &str,
    ) -> Result<(), Self::Error> {
        Context::delete_attribute(self, handle, name);
        Ok(())
    }

    #[inline]
    fn connect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Context::connect(self, from, from_attr, to, to_attr, args);
        Ok(())
    }

    #[inline]
    fn disconnect(
        &self,
        from: &str,
        from_attr: Option<&str>,
        to: &str,
        to_attr: &str,
    ) -> Result<(), Self::Error> {
        Context::disconnect(self, from, from_attr, to, to_attr);
        Ok(())
    }

    #[inline]
    fn evaluate(&self, args: &[Self::Arg<'_>]) -> Result<(), Self::Error> {
        Context::evaluate(self, args);
        Ok(())
    }

    #[inline]
    fn render_control(
        &self,
        action: Action,
        args: Option<&[Self::Arg<'_>]>,
    ) -> Result<(), Self::Error> {
        Context::render_control(self, action, args);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Context, Nsi};

    /// Compiles only if `Context` satisfies the full `Nsi` bound,
    /// including `Send + Sync` and the GAT. This is the real assertion;
    /// it needs no renderer.
    fn assert_is_nsi<T: Nsi>() {}

    #[test]
    fn context_implements_nsi() {
        assert_is_nsi::<Context<'static>>();
    }

    /// The GAT is pinned to the context lifetime, not to the call, so a
    /// borrowed `Reference` argument is bounded by the context. This
    /// only has to compile.
    #[test]
    fn arg_gat_is_pinned_to_the_context_lifetime() {
        fn takes_args<'ctx, T>(_: &T, _: &[T::Arg<'ctx>])
        where
            T: Nsi<Arg<'ctx> = crate::Arg<'ctx, 'ctx>> + 'ctx,
        {
        }
        let _ = takes_args::<Context<'static>>;
    }
}
