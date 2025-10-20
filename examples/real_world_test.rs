use ida_rs::Ida;
use std::sync::Arc;
use std::thread;

fn main() {
    println!("Simulating a real-world scenario with multiple threads...");

    let allocator = Arc::new(Ida::new());
    let mut handles = vec![];

    // Spawn 5 threads. Each will try to allocate 3 IDs.
    for i in 0..5 {
        let allocator_clone = Arc::clone(&allocator);
        let handle = thread::spawn(move || {
            let mut allocated_ids = vec![];
            for _ in 0..3 {
                match allocator_clone.alloc() {
                    Some(id) => {
                        println!("Thread {i} allocated ID: {id}");
                        allocated_ids.push(id);
                    }
                    None => {
                        println!("Thread {i} failed to allocate an ID.");
                    }
                }
                // Simulate some work
                thread::sleep(std::time::Duration::from_millis(10));
            }
            allocated_ids
        });
        handles.push(handle);
    }

    // Wait for all threads to finish and collect the results
    let mut all_ids = vec![];
    for handle in handles {
        all_ids.extend(handle.join().unwrap());
    }

    println!("\nAll threads finished.");
    println!("Total IDs allocated: {}", all_ids.len());
    println!("Allocated IDs: {all_ids:?}");

    // Verify correctness: sort and check for duplicates
    all_ids.sort();
    let mut unique_ids = all_ids.clone();
    unique_ids.dedup();

    assert_eq!(
        all_ids.len(),
        unique_ids.len(),
        "Error: Duplicate IDs were allocated!"
    );

    println!("\nVerification successful: No duplicate IDs were issued.");
}
