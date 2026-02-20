# Pin Enforcement Issue in NSI Reference Type

## Current Problem

The `Reference` type claims to require pinning but doesn't actually enforce it:

```rust
pub struct Reference<'a> {
    data: *const c_void,
    _marker: PhantomData<&'a ()>,
}

impl<'a> Reference<'a> {
    pub fn new<T: Sized>(data: Pin<&'a T>) -> Self {
        // This extracts the pointer but doesn't maintain pinning!
        let ptr = data.get_ref() as *const T as *const c_void;
        Self { data: ptr, _marker: PhantomData }
    }
}
```

## Why This Is Broken

1. **`Pin::new(&T)` doesn't pin anything** - it's just a wrapper that can be created for any reference
2. **No actual pinning guarantee** - the original data can still be moved after Reference is created
3. **False sense of security** - users think their data is pinned but it's not

## Example of the Problem

```rust
let data = vec![1, 2, 3];
let reference = nsi::reference!("data", Pin::new(&data));

// This compiles but invalidates the reference!
let moved_data = data; // data is moved, reference now points to invalid memory
```

## Proper Solutions

### Option 1: Require Heap-Pinned Data

```rust
impl<'a> Reference<'a> {
    pub fn new<T: Sized>(data: Pin<Box<T>>) -> Self {
        // Box::pin() actually guarantees the data won't move
        let ptr = Pin::into_inner(data) as *const T as *const c_void;
        Self { data: ptr, _marker: PhantomData }
    }
}
```

### Option 2: Use Stable Memory Addresses

```rust
impl<'a> Reference<'a> {
    pub fn new<T: StableAddress>(data: &'a T) -> Self {
        // Only accept types that guarantee stable addresses
        let ptr = data as *const T as *const c_void;
        Self { data: ptr, _marker: PhantomData }
    }
}

// Types like Box<T>, Arc<T>, etc. implement StableAddress
```

### Option 3: Remove False Pin Requirement

```rust
impl<'a> Reference<'a> {
    pub fn new<T: Sized>(data: &'a T) -> Self {
        // Be honest: we just need the data to outlive 'a
        let ptr = data as *const T as *const c_void;
        Self { data: ptr, _marker: PhantomData }
    }
}
```

## Current Workaround

Users must manually ensure their data doesn't move:

```rust
// Heap allocate to ensure stable address
let data = Box::new(vec![1, 2, 3]);
let reference = nsi::reference!("data", Pin::new(data.as_ref()));
// data won't be moved because it's in a Box

// Or use Box::pin
let data = Box::pin(vec![1, 2, 3]);
let reference = nsi::reference!("data", data.as_ref());
```

## Recommendation

The current API is misleading. Either:

1. Properly enforce pinning by requiring `Pin<Box<T>>` or similar
2. Remove the Pin requirement and document that users must ensure stable addresses
3. Create a safe API that handles the allocation internally

The current middle ground provides no actual safety while giving a false impression of safety.
