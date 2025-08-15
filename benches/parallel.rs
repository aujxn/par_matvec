use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::num::NonZero;

use faer::{Mat, Par};

use par_matvec::{
    dense_sparse_impl::{dense_sparse_scratch, par_dense_sparse},
    sparse_dense_impl::{buffer_foreign, merge, simple},
    spmv_drivers::{SpMvStrategy, dense_sparse_matmul, sparse_dense_matmul},
    test_utils::{FaerLoader, large_matrix_paths},
};

macro_rules! generate_sparse_dense_bench {
    ($name:ident, $mod_name:ident, $mod_name_str:expr) => {
        fn $name(c: &mut Criterion, loader: &FaerLoader) {
            let mut group = c.benchmark_group(format!(
                "thread_scaling_{}-{}x{}_nnz{}",
                loader.matrix_name, loader.nrows, loader.ncols, loader.nnz
            ));
            group.sample_size(100);

            let cpus = num_cpus::get();
            let mut thread_counts = Vec::new();
            let mut n_threads = 2;
            while n_threads <= cpus {
                thread_counts.push(n_threads);
                n_threads *= 2;
            }

            let rhs_cols = loader.rhs_vector.ncols();

            for &num_threads in &thread_counts {
                if let Some(n_threads) = NonZero::new(num_threads) {
                    let par = if num_threads == 1 {
                        Par::Seq
                    } else {
                        Par::Rayon(n_threads)
                    };
                    let mut output = Mat::zeros(loader.nrows, rhs_cols);
                    let strategy = SpMvStrategy::new(loader.faer_csc.symbolic(), par);

                    let stack_req = $mod_name::sparse_dense_scratch(
                        loader.faer_csc.as_ref(),
                        loader.rhs_vector.as_ref(),
                        &strategy,
                        par,
                    );
                    let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req).unwrap();
                    let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);

                    group.bench_with_input(
                        BenchmarkId::new(
                            format!("sparse_dense_{}", $mod_name_str),
                            format!("{}_threads", num_threads),
                        ),
                        &(loader, par, &strategy),
                        |b, (loader, par, strategy)| {
                            b.iter(|| {
                                sparse_dense_matmul(
                                    output.as_mut(),
                                    faer::Accum::Replace,
                                    loader.faer_csc.as_ref(),
                                    loader.rhs_vector.as_ref(),
                                    1.0,
                                    *par,
                                    strategy,
                                    &mut stack,
                                    Some($mod_name::par_sparse_dense),
                                );
                            })
                        },
                    );
                }
            }

            group.finish();
        }
    };
}

generate_sparse_dense_bench!(bench_sparse_dense_simple, simple, "simple");
generate_sparse_dense_bench!(bench_sparse_dense_merge, merge, "merge");
generate_sparse_dense_bench!(bench_sparse_dense_buffer_foreign, buffer_foreign, "buffer_foreign");


fn bench_dense_sparse(c: &mut Criterion, loader: &FaerLoader) {
    let mut group = c.benchmark_group(format!(
        "thread_scaling_{}-{}x{}_nnz{}",
        loader.matrix_name, loader.nrows, loader.ncols, loader.nnz
    ));
    group.sample_size(100);

    let cpus = num_cpus::get();
    let mut thread_counts = Vec::new();
    let mut n_threads = 2;
    while n_threads <= cpus {
        thread_counts.push(n_threads);
        n_threads *= 2;
    }

    let lhs_vector = loader.rhs_vector.transpose();
    let lhs_rows = lhs_vector.nrows();

    for &num_threads in &thread_counts {
        if let Some(n_threads) = NonZero::new(num_threads) {
            let par = if num_threads == 1 {
                Par::Seq
            } else {
                Par::Rayon(n_threads)
            };
            let strategy = SpMvStrategy::new(loader.faer_csc.symbolic(), par);

            let stack_req = dense_sparse_scratch(
                lhs_vector.as_ref(),
                loader.faer_csc.as_ref(),
                &strategy,
                par,
            );
            let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req).unwrap();
            let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);

            let mut output = Mat::zeros(lhs_rows, loader.ncols);
            group.bench_with_input(
                BenchmarkId::new("dense_sparse", format!("{}_threads", num_threads)),
                &(loader, par, &strategy),
                |b, (loader, par, strategy)| {
                    b.iter(|| {
                        dense_sparse_matmul(
                            output.as_mut(),
                            faer::Accum::Replace,
                            lhs_vector.as_ref(),
                            loader.faer_csc.as_ref(),
                            1.0,
                            *par,
                            strategy,
                            &mut stack,
                            Some(par_dense_sparse),
                        );
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_parallel_thread_scaling(c: &mut Criterion, loader: &FaerLoader) {
    bench_sparse_dense_simple(c, loader);
    bench_sparse_dense_merge(c, loader);
    bench_sparse_dense_buffer_foreign(c, loader);

    bench_dense_sparse(c, loader);
}

fn parallel_scaling_benchmarks(c: &mut Criterion) {
    println!("Running parallel thread scaling benchmark...");

    let matrix_paths: Vec<_> = large_matrix_paths().collect();
    for matrix_path in matrix_paths {
        match FaerLoader::load_from_matrix_market(&matrix_path, 1) {
            Ok(loader) => {
                println!(
                    "Loaded matrix '{}': {}x{} with {} non-zeros",
                    loader.matrix_name, loader.nrows, loader.ncols, loader.nnz
                );

                bench_parallel_thread_scaling(c, &loader);
            }
            Err(e) => {
                eprintln!("Failed to load matrix '{}': {}", matrix_path.display(), e);
            }
        }
    }

    //create_synthetic_benchmark_parallel(c);
}

fn create_synthetic_benchmark_parallel(c: &mut Criterion) {
    let matrix_params = [
        (100, 0.01),
        (1000, 0.01),
        (2000, 0.01),
        (10000, 0.001),
        (20000, 0.0005),
    ];
    for (size, density) in matrix_params {
        let synthetic_loader = FaerLoader::create_synthetic(size, size, density);

        println!(
            "Created synthetic matrix for parallel: {}x{} with {} non-zeros ({:.3}% density)",
            synthetic_loader.nrows,
            synthetic_loader.ncols,
            synthetic_loader.nnz,
            (synthetic_loader.nnz as f64)
                / (synthetic_loader.nrows * synthetic_loader.ncols) as f64
                * 100.0
        );

        bench_parallel_thread_scaling(c, &synthetic_loader);
    }
}

criterion_group!(all, parallel_scaling_benchmarks);
criterion_main!(all);
