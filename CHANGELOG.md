# Changelog

## 0.7.0

### `nsi-core`

* `Context` is now `Send`, `Sync`, `Copy` & `Clone`.
* All `Context` methods that have optional arguments now take `Option<&ArgSlice>` (instead of `&ArgSlice`).

  I.e. this:

  ```rust
  let ctx = nsi::Context::new(&[]).unwrap();
  ```

  changes to:

  ```rust
  let ctx = nsi::Context::new(None).unwrap();
  ```
