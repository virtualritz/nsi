# Quickstart

```rust
use nsi_intermediate::{Recorder, Scene};

// An exporter writing today's names.
let recorder = Recorder::new();
recorder.create("surface", "nurbs", None)?;
recorder.set_attribute("surface", &[nsi::integer_i32!("nu", 4)])?;
let scene = recorder.into_scene();

// The scene speaks the draft's vocabulary.
let node = scene.node("surface").unwrap();
assert!(node.attribute("u.count").is_some());
assert!(node.attribute("nu").is_none());

// The log carries, once per name:
//   `nu` on a `nurbs` node is deprecated; use `u.count`
```

Writing it back out gives the stream 3Delight reads today, `nu` and
all.
