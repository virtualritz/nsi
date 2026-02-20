# Changelog

## 0.8.0

### `nsi-core`

- `Context::render_control()` now takes `nsi::Action` as fist
  parameter.
  This change was made to reflect the fact that the action cannot
  be omitted. The 2nd parameter is the familiar `Option<&ArgSlice>`.

  I.e. this:

  ```rust
  ctx.render_control(&[nsi::string!("action", "start")]);
  ```

  changes to:

  ```rust
  ctx.render_control(nsi::Action::Start, None);
  ```

- All `Arg` types now implement `Clone`.

- `Pointer` & `Pointers` have been deprecated. You should be able to do
  everything via `Reference` & `Refererences`.

- The following types are now `Send` & `Sync`:
  - Callbacks with static lifetimes (`Callback<'static>`).

  - References with static lifetimes (`Reference<'static>`).

  - `Strings`

## 0.7.0

### `nsi-core`

- `Context` is now `Send`, `Sync`, `Copy` & `Clone`.

- All `Context` methods that have optional arguments now take `Option<&ArgSlice>` (instead of `&ArgSlice`).

  I.e. this:

  ```rust
  let ctx = nsi::Context::new(&[]).unwrap();
  ```

  changes to:

  ```rust
  let ctx = nsi::Context::new(None).unwrap();
  ```
