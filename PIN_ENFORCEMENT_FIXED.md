# Pin Enforcement Fix for NSI Reference Type

## Problem Summary

The `Reference` type claimed to require pinning but didn't actually enforce it. Users could pass `Pin::new(&data)` which doesn't guarantee the data won't move.

## Solution Implemented

### 1. Trait-Based Type Safety

Created a `StableDeref` trait that is only implemented for types that guarantee stable addresses:

```rust
pub trait StableDeref<'a> {
    fn stable_deref(&self) -> *const c_void;
}

// Only implemented for:
impl<'a, T: ?Sized> StableDeref<'a> for &'a Box<T> { ... }
impl<'a, T: ?Sized> StableDeref<'a> for &'a Arc<T> { ... }
impl<'a, T: ?Sized> StableDeref<'a> for &'a Pin<Box<T>> { ... }
```

### 2. Updated Reference API

The `Reference::new()` method now only accepts types that implement `StableDeref`:

```rust
impl<'a> Reference<'a> {
    pub fn new<S: StableDeref<'a>>(data: S) -> Self {
        let ptr = data.stable_deref();
        Self { data: ptr, _marker: PhantomData }
    }
}
```

### 3. Safe Macro Usage

The `reference!` macro now requires heap-allocated data:

```rust
// Before (unsafe - data could move):
let data = vec![1, 2, 3];
let reference = nsi::reference!("data", Pin::new(&data));

// After (safe - Box ensures stable address):
let data = Box::new(vec![1, 2, 3]);
let reference = nsi::reference!("data", &data);
```

### 4. Escape Hatch for Advanced Users

Added `reference_stable!` macro for users who need to reference other stable data:

```rust
// For static data or other guaranteed-stable addresses
static DATA: [u8; 3] = [1, 2, 3];
let reference = nsi::reference_stable!("data", &DATA);
```

## Benefits

1. **Compile-time safety**: Invalid references are now caught at compile time
2. **Clear API**: Users must explicitly choose heap allocation or use the unsafe variant
3. **No false guarantees**: The API now accurately reflects what it enforces
4. **Backwards compatible**: Existing code using Box/Arc continues to work

## Migration Guide

For existing code:

```rust
// Old (unsafe):
let data = MyStruct { ... };
nsi::reference!("data", Pin::new(&data))

// New (safe):
let data = Box::new(MyStruct { ... });
nsi::reference!("data", &data)

// Or use Arc for shared ownership:
let data = Arc::new(MyStruct { ... });
nsi::reference!("data", &data)

// Or explicitly use unsafe for stack data:
let data = MyStruct { ... };
nsi::reference_stable!("data", &data) // Must ensure data doesn't move!
```

## Conclusion

The Pin enforcement is now properly implemented through the type system. Users cannot accidentally create invalid references, and the API clearly communicates its requirements.
