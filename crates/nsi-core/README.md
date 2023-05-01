# `nsi-core`

This crate implements the high level wrapper around the NSI API and
links against the commercial [3Delight](https://www.3delight.com/) renderer.

The low level wrapper around the C FFI is in the `nsi-sys` crate.

> **Note:** This crate will likely be deprecated and split into `nsi-traits` (describing
  the high level API) and `nsi-3delight` (the actual implementation linking
  against 3Delight).
>
> This will allow different implementations to depend on `nsi-traits`.
  For example, there could be an [`nsi-moonray`](https://github.com/dreamworksanimation/openmoonray)
  or [`nsi-kajira`](https://github.com/EmbarkStudios/kajiya)
  crate.
