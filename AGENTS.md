# AGENTS.md

This file provides guidance to AI agents when working with code in this repository.

## Project Overview

This is the Rust bindings for the Nodal Scene Interface (NSI) -- a scene description API for 3D renderers. The primary (and currently only) renderer implementing NSI is 3Delight. The crate provides a high-level, type-safe Rust wrapper over the C FFI.

## Build Commands

```bash
# Build the workspace
cargo build

# IMPORTANT: NEVER use --release flag for builds or tests!
# Only use --release when the user EXPLICITLY requests it.
# Debug builds are faster and sufficient for all development and testing.

# Build with specific features
cargo build --features output,toolbelt,delight

# Run tests (requires 3Delight installed, DELIGHT env var set)
cargo test --package nsi-core --features output

# Run a specific test
cargo test --package nsi-core test_sphere -- --nocapture

# Update expected test images (when making intentional render changes)
RUST_TEST_UPDATE=1 cargo test --package nsi-core --features output

# Run examples
cargo run --example interactive
cargo run --example output --features output
cargo run --example volume

# Format code
cargo fmt

# Run clippy linter
cargo clippy --fix --allow-dirty

# Check code without building
cargo check

# Generate and view documentation
cargo doc --open
```

## Architecture

### Workspace Crates

- **`nsi`** (root) -- Re-exports from nsi-core with feature-gated extensions.
- **`nsi-core`** -- Core implementation: Context, Argument system, API bindings.
- **`nsi-sys`** -- Low-level C FFI bindings generated via bindgen.
- **`nsi-3delight`** -- 3Delight-specific nodes and shaders.
- **`nsi-toolbelt`** -- Convenience methods for Context (scene setup helpers).
- **`nsi-jupyter`** -- Jupyter notebook integration for rendering.

### Key Components in nsi-core

**Context** (`context.rs`) -- Thread-safe handle for renderer communication. Uses `Arc<InnerContext>` for safe cloning. Lifetime parameter `'a` allows storing callbacks/references.

**Arguments** (`argument.rs`) -- Type-safe parameter passing to NSI API:

- `Arg<'a, 'b>` -- Single argument with name, data, array length, flags.
- `ArgData` -- Enum dispatch over all supported data types.
- Macros like `nsi::string!`, `nsi::integer!`, `nsi::floats!`, `nsi::points!` for ergonomic construction.

**Node Types** (`node.rs`) -- Constants for all NSI node types (MESH, SHADER, TRANSFORM, etc.).

**API Loading** (`dynamic/` vs `linked/`) -- By default lib3delight is loaded at runtime via dlopen2. With `link_lib3delight` feature, it's linked at compile time.

**Output Module** (`output/`) -- Generic callback-based pixel streaming during/after render:

- `PixelType` marker trait for zero-cost type handling (f32, u16, u8, etc.).
- `FnOpen`, `FnWrite<T>`, `FnFinish` closures for receiving pixel data.
- `WriteCallback<T>`, `FinishCallback` wrappers for type-safe callback registration.
- `AccumulatingCallbacks<T>` helper for users who need full image accumulation.
- `PixelFormat` describes buffer layout.
- Typed drivers: `FERRIS_F32`, `FERRIS_U16`, `FERRIS_U8`, etc.

### Linking Modes

- **Default (runtime loading)**: lib3delight loaded dynamically -- app runs without renderer, shows error.
- **`link_lib3delight`**: Static linking -- lib3delight required at runtime.
- **`download_lib3delight`**: Downloads lib3delight during build (for CI).

## Testing

Tests are in `crates/nsi-core/tests/`. Image-based regression tests render at 320x240 and compare against expected images. The `output` feature is required for most tests.

Key test files:

- `geometry.rs` -- Geometric primitive tests.
- `materials.rs` -- Material/shader tests.
- `safety.rs` -- FFI boundary and unsafe code tests.
- `common/mod.rs` -- Shared scene setup utilities.

## Prerequisites

3Delight renderer must be installed. Set `DELIGHT` environment variable to the install location. Free version renders with up to 12 cores.

---

## Git Safety

**NEVER use `git reset --hard`, `git checkout --`, `git clean`, or any destructive git command without FIRST running `git stash`!**

Uncommitted working tree changes CANNOT be recovered after a hard reset. Always stash first:

```bash
git stash push -m "backup before reset"
git reset --hard <target>
# If something went wrong:
git stash pop
```

---

## Code Style

- Write idiomatic and canonical Rust code. Avoid patterns common in imperative languages like C/C++/JS/TS that can be expressed more elegantly in Rust.

- PREFER functional style over imperative style. Use `for_each` or `map` instead of for loops, use `collect` instead of pre-allocating a Vec and using `push`.

- PREFER direct initialization of collections. Use `BTreeMap::from([...])` or `vec![...]` instead of `new()` followed by `insert()`/`push()`.

- USE rayon to parallelize whenever larger amounts of data are being processed.

- AVOID unnecessary allocations, conversions, copies.

- AVOID using `unsafe` code unless absolutely necessary.

- AVOID return statements; structure functions with if ... if else ... else blocks instead.

- Prefer using the stack, use SmallVec whenever it makes sense.

- **NO INLINE PATHS**: Always import types at the top of the file using `use` statements. Never use inline paths like `crate::foo::Bar` in function bodies. Instead, add `use crate::foo::Bar;` at the top. The only exception is macros, which must use `$crate::` prefix for hygiene.

- **NO REDUNDANT TYPE WRAPPERS**: Never create wrapper enums/structs that duplicate types from dependencies. If an imported type does everything needed, re-export it with `pub use` instead of creating a new type. Only wrap when adding functionality or adapting interfaces.

### Naming Conventions

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html):

- **Casing**: `UpperCamelCase` for types/traits/variants; `snake_case` for functions/methods/modules/variables; `SCREAMING_SNAKE_CASE` for constants/statics.
- **Conversions**: `as_` for cheap borrowed→borrowed; `to_` for expensive conversions; `into_` for ownership-consuming conversions.
- **Getters**: No `get_` prefix (use `width()` not `get_width()`), except for unsafe variants like `get_unchecked()`.
- **Iterators**: `iter()` for &T, `iter_mut()` for &mut T, `into_iter()` for T by value.

### Traits

All public-facing types must implement `Debug`, `Clone`, `Hash`, `PartialEq`, and `Eq` and `Copy` if directly derivable.

---

## Documentation

- All code comments MUST end with a period.

- All doc comments should also end with a period unless they're headlines. This includes list items.

- All comments must be on their own line. Never put comments at the end of a line of code.

- ENSURE an en-dash is expressed as two dashes like so: `--`. En-dashes are not used for connecting words, e.g. "compile-time".

- All references to types, keywords, symbols etc. MUST be enclosed in backticks: `struct` `Foo`.

- For each part of the docs, every first reference to a type, keyword, symbol etc. that is NOT the item itself that is being described MUST be linked to the relevant section in the docs like so: [`Foo`].

---

## Writing Instructions

These instructions apply to any communication (e.g. feedback you print to the user) as well as any documentation you write:

- Be concise.

- Use simple sentences. But feel free to use technical jargon.

- Do NOT overexplain basic concepts. Assume the user is technically proficient.

- AVOID flattering, corporate-ish or marketing language. Maintain a neutral viewpoint.

- AVOID vague and/or generic claims which may seem correct but are not substantiated by the context.

- AVOID weasel words.

---

## Guidelines

- **Test Naming Convention**: Test functions should NOT be prefixed with `test_`. The `#[test]` attribute already indicates it's a test. Use descriptive names without the prefix.

- **CRITICAL: ALWAYS run `cargo test` and ensure the code compiles and tests pass WITHOUT ANY WARNINGS BEFORE committing!** Never commit code that doesn't build, has failing tests, or produces warnings.
  - First run: `cargo test` to ensure everything compiles and passes without warnings.
  - Also run: `cargo build --all-targets` to check examples and benches are warning-free.
  - Then run: `cargo fmt` to format the code.
  - Then run: `cargo clippy --all-targets -- -W warnings` and fix any issues.
  - Finally run: `cargo test` one more time to verify everything is clean.
  - Only then commit the changes when there are ZERO warnings and all tests pass.

- **CRITICAL: Address ALL warnings before EVERY commit!** This includes:
  - Unused imports, variables, and functions.
  - Dead code warnings.
  - Deprecated API usage.
  - Type inference ambiguities.
  - Any clippy warnings or suggestions.
  - Never use `#[allow(warnings)]` or similar suppressions without explicit user approval.

- ALWAYS run `cargo clippy --fix` before committing. If clippy brings up any issues, fix them, then repeat until there are no more issues brought up by clippy. Finally run `cargo fmt`, then commit.

- **CRITICAL: Fix ALL workspace warnings before declaring any task/plan finished.** The final `cargo clippy --all-targets` check covers the entire workspace -- if it surfaces warnings from other crates or prior changes, fix those too. A task is not done until the workspace is warning-free.

- **CRITICAL: Never modify test files** -- tests encode human intent.
- **CRITICAL: Never change expected test outputs** -- these are the ground truth for tests.

---

## Commit & Pull Request Guidelines

Adopt conventional commit prefixes (`feat:`, `fix:`, `chore:`). Keep messages concise and describe the user-facing effect. Every pull request should summarize intent, list key changes, and document how you validated them (tests run, examples exercised).

---

## Performance Best Practices

### Memory Management

- Avoid unnecessary cloning in hot paths.
- Use `Arc`/`Rc` for shared immutable data.
- Prefer borrowing over ownership transfer when possible.
- Use `SmallVec` for collections that are usually small.

### String Handling

- Use `&str` instead of `String` where ownership isn't needed.
- Avoid `.to_string()` for temporary values.
- Use string slices for function parameters when possible.

---

## Code Organization Best Practices

1. **Module Structure**: Keep modules focused on a single responsibility.
2. **Module Size**: Keep individual files reasonably sized (~300-500 lines). Split larger modules into submodules rather than having monolithic files.
3. **Public API**: Minimize public surface area; use `pub(crate)` liberally.
4. **Error Types**: Define domain-specific error types with `thiserror`.
5. **Tests**: Co-locate unit tests; integration tests in `tests/`.
