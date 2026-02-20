NSI Crate Safety Audit Report

    Executive Summary

    After a comprehensive analysis of the NSI crate, I've identified several critical memory safety issues, particularly around unsafe FFI boundaries and callback handling. The crate interfaces with
    the 3Delight renderer through FFI, requiring careful management of raw pointers and lifetimes.

    Critical Issues Found

    1. Memory Leaks in Callback Handling

    - Location: crates/nsi-core/src/output/mod.rs:770-778
    - Issue: Callbacks are intentionally leaked with Box::leak() to avoid double-free crashes
    - Risk: Permanent memory leak for every render operation using callbacks

    2. Unsafe String Handling

    - Location: crates/nsi-core/src/context.rs:813-815
    - Issue: Uses from_cstr_unchecked() without validation
    - Risk: Potential undefined behavior if C API passes invalid UTF-8

    3. Raw Pointer Lifetime Issues

    - Location: crates/nsi-core/src/argument.rs:340-344
    - Issue: Reference type stores raw pointers with only phantom lifetime markers
    - Risk: Use-after-free if referenced data is dropped while NSI context still holds pointer

    4. Triple-Boxing Pattern

    - Location: crates/nsi-core/src/output/mod.rs:465-476
    - Issue: Uses Box<Box<Box<T>>> without clear justification
    - Risk: Indicates underlying memory management issues

    5. Unsafe Transmutation

    - Location: crates/nsi-core/src/output/mod.rs:587-589
    - Issue: Const to mut transmute for callback parameters
    - Risk: Violates Rust's aliasing rules

    6. Missing Null Checks

    - Location: Multiple locations in linked/dynamic modules
    - Issue: FFI calls pass raw pointers without validation
    - Risk: Null pointer dereferences

    Improvement Plan

    Phase 1: Critical Safety Fixes

    1. Fix Callback Memory Management
      - Replace triple-boxing with proper RAII pattern
      - Use Arc<Mutex<>> for shared callback state
      - Implement proper cleanup in drop handlers
    2. Add String Validation
      - Replace from_cstr_unchecked with safe alternatives
      - Add UTF-8 validation for all C strings
      - Handle invalid strings gracefully
    3. Improve Pointer Safety
      - Add null checks before all pointer dereferences
      - Use NonNull<T> for non-null guarantees
      - Document lifetime requirements clearly

    Phase 2: Architecture Improvements

    1. Refactor Reference Types
      - Use Pin<Box<T>> for stable addresses
      - Add runtime lifetime tracking
      - Consider using typed arena allocators
    2. Improve Error Handling
      - Add Result types to FFI boundaries
      - Implement panic catching at FFI boundaries
      - Add proper error propagation
    3. Add Safety Documentation
      - Document all unsafe invariants
      - Add safety comments to all unsafe blocks
      - Create FFI safety guidelines

    Phase 3: Testing & Validation

    1. Add Miri Testing
      - Test all unsafe code paths with Miri
      - Add regression tests for fixed issues
      - Validate pointer arithmetic
    2. Fuzzing
      - Fuzz test FFI boundaries
      - Test with malformed input data
      - Validate error handling paths
    3. Static Analysis
      - Run additional linters (clippy with pedantic)
      - Use cargo-audit for dependencies
      - Add CI/CD safety checks

    Recommended Immediate Actions

    1. Fix the callback memory leaks (highest priority)
    2. Add null pointer validation
    3. Replace unsafe string operations
    4. Document all safety invariants
    5. Add comprehensive tests
