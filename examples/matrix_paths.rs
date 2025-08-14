use par_matvec::test_utils::{small_matrix_paths, medium_matrix_paths, large_matrix_paths};

fn main() {
    println!("=== Small Matrix Paths (< 100k nnz) ===");
    let small_paths: Vec<_> = small_matrix_paths().collect();
    if small_paths.is_empty() {
        println!("No small matrices found");
    } else {
        for (i, path) in small_paths.iter().enumerate() {
            println!("{}. {}", i + 1, path.display());
        }
    }
    
    println!("\n=== Medium Matrix Paths (100k - 1M nnz) ===");
    let medium_paths: Vec<_> = medium_matrix_paths().collect();
    if medium_paths.is_empty() {
        println!("No medium matrices found");
    } else {
        for (i, path) in medium_paths.iter().enumerate() {
            println!("{}. {}", i + 1, path.display());
        }
    }
    
    println!("\n=== Large Matrix Paths (>= 1M nnz) ===");
    let large_paths: Vec<_> = large_matrix_paths().collect();
    if large_paths.is_empty() {
        println!("No large matrices found");
    } else {
        for (i, path) in large_paths.iter().enumerate() {
            println!("{}. {}", i + 1, path.display());
        }
    }
    
    println!("\n=== Summary ===");
    println!("Small matrices: {}", small_paths.len());
    println!("Medium matrices: {}", medium_paths.len());
    println!("Large matrices: {}", large_paths.len());
    println!("Total matrices: {}", small_paths.len() + medium_paths.len() + large_paths.len());
}