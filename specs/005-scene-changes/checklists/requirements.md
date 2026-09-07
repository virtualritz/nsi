# Requirements Checklist

- [x] Every requirement in `spec.md` has a contract row.
- [x] Every `Covered` row names a test that exists. Checked
      mechanically: every module-qualified name in this surface
      resolves to a `fn` in `crates/`.
- [x] Every `Covered` row names a mutation that reddens it, or says
      why the property gate cannot see it (the shader-parameter row).
- [x] Non-goals are stated, and each is a decision in `research.md`
      rather than an omission.
- [x] The measurements carry their command and their conditions
      (release, this machine, best of ten).
- [ ] A consumer has exercised the API. Open, and named in `plan.md` --
      the rows are proven by this crate's tests and by the renderer,
      not by a backend.
- [x] The failure mode that would be silent is named, and the gate that
      exists to catch it says how it failed to catch anything three
      times.
