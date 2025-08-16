use std::env;
use std::path::Path;
use std::time::{Duration, Instant};

use faer::{Accum, Par};

use par_matvec::dense_sparse_impl::{dense_sparse_scratch, par_dense_sparse};
use par_matvec::sparse_dense_impl::{buffer_foreign, merge, simple};
use par_matvec::spmv_drivers::{SpMvStrategy, dense_sparse_matmul, sparse_dense_matmul};
use par_matvec::test_utils::FaerLoader;

macro_rules! generate_sparse_dense_profiler {
    ($name:ident, $mod_name:ident) => {
        fn $name(
            loader: &FaerLoader,
            par: Par,
            strategy: &SpMvStrategy,
            start_time: Instant,
        ) -> usize {
            let matrix = loader.faer_csc.as_ref();
            let rhs = loader.rhs_vector.as_ref();
            let stack_req = $mod_name::sparse_dense_scratch(matrix, rhs, strategy, par);
            let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req).unwrap();
            let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);
            let mut result = faer::Mat::zeros(loader.nrows, 1);
            let mut iterations = 0;

            while start_time.elapsed() < Duration::from_secs(10) {
                sparse_dense_matmul(
                    result.as_mut(),
                    Accum::Replace,
                    matrix,
                    rhs,
                    1.0,
                    par,
                    strategy,
                    &mut stack,
                    Some($mod_name::par_sparse_dense),
                );
                iterations += 1;
            }
            iterations
        }
    };
}

generate_sparse_dense_profiler!(profile_sparse_dense_simple, simple);
generate_sparse_dense_profiler!(profile_sparse_dense_merge, merge);
generate_sparse_dense_profiler!(profile_sparse_dense_buffer, buffer_foreign);

fn profile_dense_sparse(
    loader: &FaerLoader,
    par: Par,
    strategy: &SpMvStrategy,
    start_time: Instant,
) -> usize {
    let matrix = loader.faer_csc.as_ref();
    let rhs = loader.rhs_vector.as_ref();
    let lhs = rhs.transpose();
    let stack_req = dense_sparse_scratch(lhs.as_ref(), matrix, strategy, par);
    let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req).unwrap();
    let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);
    let mut result = faer::Mat::zeros(lhs.nrows(), loader.ncols);
    let mut iterations = 0;

    while start_time.elapsed() < Duration::from_secs(10) {
        dense_sparse_matmul(
            result.as_mut(),
            Accum::Replace,
            lhs.as_ref(),
            matrix,
            1.0,
            par,
            strategy,
            &mut stack,
            Some(par_dense_sparse),
        );
        iterations += 1;
    }
    iterations
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <matrix_market_file> <num_threads> <algorithm>", args[0]);
        eprintln!("Example: {} test_matrices/bcsstk14.mtx 4 sparse_dense_simple", args[0]);
        eprintln!("Algorithms:");
        eprintln!("  dense_sparse            - Dense-sparse multiplication");
        eprintln!("  sparse_dense_simple     - Sparse-dense simple algorithm");
        eprintln!("  sparse_dense_merge      - Sparse-dense merge algorithm");
        eprintln!("  sparse_dense_buffer     - Sparse-dense buffer_foreign algorithm");
        std::process::exit(1);
    }

    let matrix_path = &args[1];
    let num_threads: usize = args[2]
        .parse()
        .map_err(|_| "Number of threads must be a positive integer")?;
    let algorithm = &args[3];

    if num_threads == 0 {
        return Err("Number of threads must be greater than 0".into());
    }

    // Validate algorithm choice
    match algorithm.as_str() {
        "dense_sparse" | "sparse_dense_simple" | "sparse_dense_merge" | "sparse_dense_buffer" => {}
        _ => {
            return Err(format!(
                "Unknown algorithm '{}'. Valid options: dense_sparse, sparse_dense_simple, sparse_dense_merge, sparse_dense_buffer",
                algorithm
            ).into());
        }
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
    println!("Running {} algorithm with {} threads", algorithm, num_threads);

    let par = if num_threads == 1 {
        Par::Seq
    } else {
        Par::Rayon(std::num::NonZeroUsize::new(num_threads).unwrap())
    };
    let strategy = SpMvStrategy::new(loader.faer_csc.symbolic(), par);

    println!("Starting 10-second profiling loop...");
    let start_time = Instant::now();

    let iterations = match algorithm.as_str() {
        "dense_sparse" => profile_dense_sparse(&loader, par, &strategy, start_time),
        "sparse_dense_simple" => profile_sparse_dense_simple(&loader, par, &strategy, start_time),
        "sparse_dense_merge" => profile_sparse_dense_merge(&loader, par, &strategy, start_time),
        "sparse_dense_buffer" => profile_sparse_dense_buffer(&loader, par, &strategy, start_time),
        _ => unreachable!(), // Already validated above
    };

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

    // Note: Result norm output removed since each algorithm has its own result vector

    Ok(())
}
