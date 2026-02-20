//! Test to investigate Box sizes for FFI callbacks

use std::mem;

#[test]
fn box_sizes() {
    // Let's check the size and alignment of different Box levels

    // A simple closure
    let closure = |x: i32| x + 1;

    println!("Size of closure: {}", mem::size_of_val(&closure));
    println!(
        "Size of Box<dyn Fn>: {}",
        mem::size_of::<Box<dyn Fn(i32) -> i32>>()
    );
    println!(
        "Size of Box<Box<dyn Fn>>: {}",
        mem::size_of::<Box<Box<dyn Fn(i32) -> i32>>>()
    );
    println!(
        "Size of Box<Box<Box<dyn Fn>>>: {}",
        mem::size_of::<Box<Box<Box<dyn Fn(i32) -> i32>>>>()
    );

    println!("\nAlignment:");
    println!(
        "Align of Box<dyn Fn>: {}",
        mem::align_of::<Box<dyn Fn(i32) -> i32>>()
    );
    println!(
        "Align of Box<Box<dyn Fn>>: {}",
        mem::align_of::<Box<Box<dyn Fn(i32) -> i32>>>()
    );
    println!(
        "Align of Box<Box<Box<dyn Fn>>>: {}",
        mem::align_of::<Box<Box<Box<dyn Fn(i32) -> i32>>>>()
    );

    // Check raw pointer representation for a properly boxed trait object
    let boxed: Box<Box<Box<dyn Fn(i32) -> i32>>> =
        Box::new(Box::new(Box::new(closure)));
    let ptr = Box::into_raw(boxed);
    println!("\nRaw pointer: {:?}", ptr);

    // Clean up - use the same type for from_raw as we used for into_raw
    let _cleaned = unsafe { Box::from_raw(ptr) };
}

#[test]
fn ffi_pattern() {
    // This simulates the correct FFI pattern: always use trait objects from the start
    trait MyCallback: FnMut(i32) -> i32 {}
    impl<T: FnMut(i32) -> i32> MyCallback for T {}

    let mut counter = 0i32;
    let callback = move |x: i32| {
        counter += 1;
        x + counter
    };

    // Triple Box with trait object from the start
    // This is the correct pattern for FFI where we need a thin pointer
    let triple: Box<Box<Box<dyn MyCallback>>> =
        Box::new(Box::new(Box::new(callback)));
    let triple_ptr = Box::into_raw(triple);
    println!("Triple Box ptr: {:?}", triple_ptr);

    // Pointer is thin (single pointer size) because the outer Box is a concrete type
    assert_eq!(
        mem::size_of_val(&triple_ptr),
        mem::size_of::<*const ()>(),
        "Outer pointer should be thin"
    );

    // Reconstruct with the same type
    let mut reconstructed: Box<Box<Box<dyn MyCallback>>> =
        unsafe { Box::from_raw(triple_ptr) };

    // Call the callback through all the layers
    let result = reconstructed(10);
    println!("Callback result: {}", result);
    assert_eq!(result, 11); // 10 + 1 (first call increments counter to 1)

    println!("Triple box FFI pattern works correctly");
}
