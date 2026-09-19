# Tasks: Type Names And `hpoint`

- [x] T1 `nsi-sys` binds the 2.9.210 header. *Evidence:* `NSIType::HPoint = 10`.
- [x] T2 `Type` renamed, `Point4F32` added, old names deprecated.
- [x] T3 `nsi-ffi-wrap` wrappers, `ArgData`, macros renamed; deprecated
      aliases and forwarders. *Evidence:* `tests/deprecated_names.rs`.
- [x] T4 Every crate rewritten. *Evidence:* clippy `-D warnings`.
- [x] T5 `Point4F32` sends `NSITypeHPoint`. *Evidence:* `tests/hpoint.rs`, falsified.
- [x] T6 `hpoint` in the parser and the writers. *Evidence:* `an_hpoint_round_trips`.
- [x] T7 The design draft on nsi.readthedocs.io matches (docs c744c34).
