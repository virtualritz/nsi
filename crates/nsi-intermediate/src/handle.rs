//! How a node handle is stored.
//!
//! A handle appears many times in a recorded scene: as the key in
//! [`Scene::nodes`](crate::Scene::nodes), on both ends of every edge,
//! and in each of the three edge indexes. Measured on a scene of
//! 200 000 nodes, the whole recording costs 879 bytes a node, and the
//! handles are a large part of that.
//!
//! With `ustr_handles` a handle is an interned pointer -- eight bytes,
//! `Copy`, hashed and compared as a pointer. Without it, a `String`, as
//! before. **The switch is invisible from outside**: every public
//! signature takes and returns `&str`, so a consumer cannot tell, and
//! two crates in one build that disagree about the feature still
//! compile.
//!
//! The cost of interning is a global table that is never freed. For a
//! recorder that is bounded by the scene; for a host that invents
//! millions of distinct handles per session it is a leak, which is why
//! this is a feature rather than the default.

#[cfg(feature = "ustr_handles")]
pub(crate) type Handle = ustr::Ustr;
#[cfg(not(feature = "ustr_handles"))]
pub(crate) type Handle = String;

/// Store a handle.
#[cfg(feature = "ustr_handles")]
pub(crate) fn handle(name: &str) -> Handle {
    ustr::ustr(name)
}

/// Store a handle.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn handle(name: &str) -> Handle {
    name.to_string()
}

/// Look one up in a map keyed by [`Handle`], from a `&str`.
///
/// Two bodies because the two representations index differently: a
/// `String` key borrows as `&str`, while an interned key has to be
/// interned first -- `Ustr` hashes as a pointer, so it cannot also
/// hash as its own text.
#[cfg(feature = "ustr_handles")]
pub(crate) fn get<'a, V>(
    map: &'a std::collections::HashMap<Handle, V>,
    name: &str,
) -> Option<&'a V> {
    map.get(&ustr::ustr(name))
}

/// Look one up in a map keyed by [`Handle`], from a `&str`.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn get<'a, V>(
    map: &'a std::collections::HashMap<Handle, V>,
    name: &str,
) -> Option<&'a V> {
    map.get(name)
}

/// The same, for a map keyed by a handle and an attribute name.
#[cfg(feature = "ustr_handles")]
pub(crate) fn get_pair<'a, V>(
    map: &'a std::collections::HashMap<(Handle, Handle), V>,
    name: &str,
    attribute: &str,
) -> Option<&'a V> {
    map.get(&(ustr::ustr(name), ustr::ustr(attribute)))
}

/// The same, for a map keyed by a handle and an attribute name.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn get_pair<'a, V>(
    map: &'a std::collections::HashMap<(Handle, Handle), V>,
    name: &str,
    attribute: &str,
) -> Option<&'a V> {
    map.get(&(name.to_string(), attribute.to_string()))
}

/// Look one up in an `IndexMap` keyed by [`Handle`], from a `&str`.
#[cfg(feature = "ustr_handles")]
pub(crate) fn map_get<'a, V>(
    map: &'a indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<&'a V> {
    map.get(&ustr::ustr(name))
}

/// Look one up in an `IndexMap` keyed by [`Handle`], from a `&str`.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn map_get<'a, V>(
    map: &'a indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<&'a V> {
    map.get(name)
}

/// The mutable twin of [`map_get`].
#[cfg(feature = "ustr_handles")]
pub(crate) fn map_get_mut<'a, V>(
    map: &'a mut indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<&'a mut V> {
    map.get_mut(&ustr::ustr(name))
}

/// The mutable twin of [`map_get`].
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn map_get_mut<'a, V>(
    map: &'a mut indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<&'a mut V> {
    map.get_mut(name)
}

/// Remove one, keeping the order of the rest.
#[cfg(feature = "ustr_handles")]
pub(crate) fn map_remove<V>(
    map: &mut indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<V> {
    map.shift_remove(&ustr::ustr(name))
}

/// Remove one, keeping the order of the rest.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn map_remove<V>(
    map: &mut indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<V> {
    map.shift_remove(name)
}

/// The stored key together with its value.
#[cfg(feature = "ustr_handles")]
pub(crate) fn map_entry<'a, V>(
    map: &'a indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<(&'a str, &'a V)> {
    map.get_key_value(&ustr::ustr(name))
        .map(|(key, value)| (key.as_str(), value))
}

/// The stored key together with its value.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn map_entry<'a, V>(
    map: &'a indexmap::IndexMap<Handle, V>,
    name: &str,
) -> Option<(&'a str, &'a V)> {
    map.get_key_value(name)
        .map(|(key, value)| (key.as_str(), value))
}

/// Whether an `IndexSet` of handles holds one, from a `&str`.
#[cfg(feature = "ustr_handles")]
pub(crate) fn set_contains(
    set: &indexmap::IndexSet<Handle>,
    name: &str,
) -> bool {
    set.contains(&ustr::ustr(name))
}

/// Whether an `IndexSet` of handles holds one, from a `&str`.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn set_contains(
    set: &indexmap::IndexSet<Handle>,
    name: &str,
) -> bool {
    set.contains(name)
}

/// Remove one from an `IndexSet` of handles, from a `&str`.
#[cfg(feature = "ustr_handles")]
pub(crate) fn set_remove(set: &mut indexmap::IndexSet<Handle>, name: &str) {
    set.shift_remove(&ustr::ustr(name));
}

/// Remove one from an `IndexSet` of handles, from a `&str`.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn set_remove(set: &mut indexmap::IndexSet<Handle>, name: &str) {
    set.shift_remove(name);
}

/// Copy a handle.
///
/// `Ustr` is `Copy` and `String` is not, so a bare `clone` is right in
/// one configuration and a clippy error in the other. The choice lives
/// here once.
#[cfg(feature = "ustr_handles")]
pub(crate) fn copy(handle: &Handle) -> Handle {
    *handle
}

/// Copy a handle.
#[cfg(not(feature = "ustr_handles"))]
pub(crate) fn copy(handle: &Handle) -> Handle {
    handle.clone()
}
