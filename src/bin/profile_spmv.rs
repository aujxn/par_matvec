use std::env;
use std::path::Path;
use std::time::{Duration, Instant};

use faer::{Accum, Par};

use par_matvec::sparse_dense_impl::buffer_foreign::{par_sparse_dense, sparse_dense_scratch};
use par_matvec::spmv_drivers::{SpMvStrategy, sparse_dense_matmul};
use par_matvec::test_utils::FaerLoader;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage: {} <matrix_market_file> <num_threads>", args[0]);
        eprintln!("Example: {} test_matrices/bcsstk14.mtx 4", args[0]);
        std::process::exit(1);
    }

    let matrix_path = &args[1];
    let num_threads: usize = args[2]
        .parse()
        .map_err(|_| "Number of threads must be a positive integer")?;

    if num_threads == 0 {
        return Err("Number of threads must be greater than 0".into());
    }

    if !Path::new(matrix_path).exists() {
        return Err(format!("Matrix file not found: {}", matrix_path).into());
    }

    println!("Loading matrix from {}...", matrix_path);
    let loader = FaerLoader::load_from_matrix_market(matrix_path, 1)?;

    println!(
        "Matrix: {} ({}x{}, {} non-zeros)",
        loader.matrix_name, loader.nrows, loader.ncols, loader.nnz
    );
    println!("Running with {} threads", num_threads);

    let matrix = loader.faer_csc.as_ref();
    let rhs = loader.rhs_vector.as_ref();

    let par = if num_threads == 1 {
        Par::Seq
    } else {
        Par::Rayon(std::num::NonZeroUsize::new(num_threads).unwrap())
    };
    let strategy = SpMvStrategy::new(matrix.symbolic(), par);

    let stack_req = sparse_dense_scratch(matrix, rhs, &strategy, par);
    let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req).unwrap();
    let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);

    let mut result = faer::Mat::zeros(loader.nrows, 1);

    println!("Starting 10-second profiling loop...");
    let start_time = Instant::now();
    let mut iterations = 0;

    while start_time.elapsed() < Duration::from_secs(10) {
        sparse_dense_matmul(
            result.as_mut(),
            Accum::Replace,
            matrix,
            rhs,
            1.0,
            par,
            &strategy,
            &mut stack,
            Some(par_sparse_dense),
        );
        iterations += 1;
    }

    let elapsed = start_time.elapsed();
    println!(
        "Completed {} iterations in {:.2}s",
        iterations,
        elapsed.as_secs_f64()
    );
    println!(
        "Average time per SPMV: {:.3}ms",
        elapsed.as_millis() as f64 / iterations as f64
    );

    let result_norm = result.norm_l2();
    println!("Result L2 norm: {:.6e}", result_norm);

    Ok(())
}
