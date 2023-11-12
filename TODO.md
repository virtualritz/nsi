# `nsi` -- To Do

* Change the signatures of array argument methods that accept tuples to take
  slices of arrays. These can be cast to flat slices e.g. using the `slice_of_array` crate.

* Create a `nsi-traits` crate defining `Context` and `Argument` traits. This
  way the whole implemetation for renderer exposing the C-API can be turned
  into an `nsi-3delight` crate, based on a `nsi-ffi` template that any renderer
  implemeting the C FFI can just clone to get a Rust wrapper.
