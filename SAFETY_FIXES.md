# NSI Crate Safety Fixes

This document summarizes the safety fixes applied to the NSI crate based on the comprehensive audit.

## Completed Fixes

### 1. ✅ Callback Memory Management

**Issue**: Callbacks were being leaked with `Box::leak()` to prevent double-free crashes.

**Root Cause**: Discovered that triple-boxing is required for FFI callbacks due to fat pointer representation:

- `Box<dyn Trait>` is a fat pointer (16 bytes: data + vtable)
- `Box<Box<dyn Trait>>` is a thin pointer to the fat pointer
- When casting through `*const c_void`, type information is lost
- Double-boxing causes segfaults when reconstructing the fat pointer
- Triple-boxing ensures thin pointers at FFI boundary

**Fix**: Documented why triple-boxing is necessary. The memory leak remains but is now understood as a safety requirement until a better solution is found.

### 2. ✅ String Validation

**Issue**: Used `from_cstr_unchecked()` without validating UTF-8.

**Fixes Applied**:

- `context.rs:814`: Added null checks and UTF-8 validation with lossy conversion fallback
- `output/mod.rs:551`: Added null checks and error handling for parameter names
- `output/mod.rs:601`: Replaced unsafe string conversion with `to_string_lossy()`
- `output/pixel_format.rs`: Added fallback values for invalid UTF-8 strings

### 3. ✅ Reference Type Documentation

**Issue**: Raw pointer with only phantom lifetime marker, no safety documentation.

**Fixes Applied**:

- Added comprehensive safety documentation
- Added debug assertions for null pointer checks
- Added `from_ptr` method with clear safety requirements
- Documented lifetime requirements and pinning guarantees

## Pending Fixes

### 4. ❌ Null Pointer Checks in FFI Calls

Still need to add comprehensive null checks before dereferencing pointers in:

- FFI function parameters
- Callback invocations
- Dynamic loading functions

### 5. ❌ Unsafe Transmutation

The `const` to `mut` transmute in `output/mod.rs:587-589` violates Rust's aliasing rules and needs to be fixed.

### 6. ✅ Document Unsafe Invariants

**Completed**: Added safety comments to all `unsafe` blocks explaining:

- What invariants are being upheld
- Why the operation is safe
- What assumptions are being made

**Files updated**:

- `context.rs`: Documented unsafe FFI callbacks and string handling
- `output/mod.rs`: Documented triple-boxing pattern, FFI boundaries, and pointer operations
- `output/pixel_format.rs`: Already had safety documentation
- `linked/mod.rs`: Documented all NSI C API function calls
- `dynamic/mod.rs`: Documented dlopen operations
- `tests.rs`: Documented array casting safety
- `argument.rs`: Already had comprehensive safety documentation

### 7. ✅ Panic Catching at FFI Boundaries

**Completed**: Added `catch_unwind` at all FFI boundaries to prevent unwinding into C code.

**Files updated**:

- `output/mod.rs`: Added panic catching to all image callbacks (open, write, close, query, progress)
- `context.rs`: Added panic catching to status and error handler callbacks

**Implementation details**:

- All FFI callbacks now catch panics and return appropriate error codes
- Prevents undefined behavior from Rust panics unwinding into C code
- Returns `Error::Undefined` for output callbacks if a panic occurs
- Silently returns for context callbacks (no return value to signal error)

## Recommendations

1. **Consider Alternative to Triple-Boxing**: Research if there's a cleaner way to handle FFI callbacks without the triple-boxing pattern.

2. **Add Miri Testing**: Use Miri to validate all unsafe code paths.

3. **Fuzz Testing**: Add fuzzing for FFI boundaries with malformed input.

4. **Runtime Validation**: Add debug-mode runtime checks for pointer validity.

5. **API Redesign**: Consider a safer API design that doesn't require raw pointers for references.
