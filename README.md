# ida: An ID Allocator for Sparse ID Spaces

`ida` provides a thread-safe, `no_std` compatible ID allocator suitable for
systems-level programming, such as in OS kernels or embedded environments.

It is implemented as a radix tree, which makes it highly memory-efficient
when dealing with sparse ID allocations (e.g., allocating ID 5 and ID 5,000,000
without allocating the space in between).

## Features
- **`no_std` compatible:** Usable in bare-metal environments.
- **Thread-Safe:** All public methods are thread-safe, using a spinlock for synchronization.
- **Memory-Efficient for Sparse Sets:** Ideal when allocated IDs are far apart.

## Example
```rust
use ida::Ida;

let ida = Ida::new();

// Allocate new IDs
let id1 = ida.alloc().unwrap();
let id2 = ida.alloc().unwrap();

assert_eq!(id1, 0);
assert_eq!(id2, 1);

// Free an ID
ida.free(id1);

// The next allocation reuses the freed ID
let id3 = ida.alloc().unwrap();
assert_eq!(id3, 0);
```

## License

This project is licensed under either of

 * Apache License, Version 2.0, (`LICENSE-APACHE` or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license (`LICENSE-MIT` or http://opensource.org/licenses/MIT)

at your option.
