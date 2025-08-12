use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::num::NonZero;

use faer::{Mat, Par};
use nalgebra::DVector;

use par_matvec::test_utils::{LARGE_MATRICES, SMALL_MATRICES, SimpleMatrixLoader, TestMatrices};
use par_matvec::{SparseDenseStrategy, sparse_dense_matmul, sparse_dense_scratch};

fn bench_sequential_implementations(c: &mut Criterion, matrices: &TestMatrices) {
    let mut group = c.benchmark_group(format!(
        "sequential_matvec_{}-{}x{}_nnz{}",
        matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
    ));
    group.sample_size(100);

    let mut faer_output = Mat::zeros(matrices.nrows, matrices.rhs_vector.ncols());
    let rhs_col = matrices.rhs_vector.col(0);

    let nalgebra_rhs = DVector::from_iterator(matrices.rhs_vector.nrows(), rhs_col.iter().copied());

    let sprs_rhs: Vec<f64> = rhs_col.iter().copied().collect();
    let mut sprs_output = vec![0.0; matrices.nrows];

    group.bench_with_input(
        BenchmarkId::new(
            "faer",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        matrices,
        |b, matrices| {
            b.iter(|| {
                faer::sparse::linalg::matmul::sparse_dense_matmul(
                    faer_output.as_mut(),
                    faer::Accum::Replace,
                    matrices.faer_csc.as_ref(),
                    matrices.rhs_vector.as_ref(),
                    1.0,
                    Par::Seq,
                );
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new(
            "nalgebra",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        matrices,
        |b, matrices| {
            b.iter(|| {
                let _result = &matrices.nalgebra_csr * &nalgebra_rhs;
            })
        },
    );

    group.bench_with_input(
        BenchmarkId::new(
            "sprs",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        matrices,
        |b, matrices| {
            b.iter(|| {
                sprs::prod::mul_acc_mat_vec_csr(
                    matrices.sprs_csr.view(),
                    &sprs_rhs[..],
                    &mut sprs_output[..],
                );
            })
        },
    );

    group.finish();
}

fn bench_parallel_thread_scaling(c: &mut Criterion, loader: &SimpleMatrixLoader) {
    let mut group = c.benchmark_group(format!(
        "thread_scaling_{}-{}x{}_nnz{}",
        loader.matrix_name, loader.nrows, loader.ncols, loader.nnz
    ));
    group.sample_size(100);

    let cpus = num_cpus::get();
    let mut thread_counts = Vec::new();
    let mut n_threads = 8;
    while n_threads <= cpus {
        thread_counts.push(n_threads);
        n_threads *= 2;
    }

    let rhs_cols = loader.rhs_vector.ncols();
    //let lhs = loader.rhs_vector.transpose();
    //let lhs_rows = lhs.nrows();

    for &num_threads in &thread_counts {
        if let Some(n_threads) = NonZero::new(num_threads) {
            let par = if num_threads == 1 {
                Par::Seq
            } else {
                Par::Rayon(n_threads)
            };
            let mut output = Mat::zeros(loader.nrows, rhs_cols);
            let strategy = SparseDenseStrategy::new(loader.faer_csc.symbolic(), par);

            let stack_req = sparse_dense_scratch(
                loader.faer_csc.as_ref(),
                loader.rhs_vector.as_ref(),
                &strategy,
                par,
            );
            let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req).unwrap();
            let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);

            group.bench_with_input(
                BenchmarkId::new("sparse_dense", format!("{}_threads", num_threads)),
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
                        );
                    })
                },
            );
            break;

            /*
            let mut output = Mat::zeros(lhs_rows, loader.ncols);
            group.bench_with_input(
                BenchmarkId::new("dense_sparse", format!("{}_threads", num_threads)),
                &(loader, par, &strategy),
                |b, (loader, par, strategy)| {
                    b.iter(|| {
                        dense_sparse_matmul(
                            output.as_mut(),
                            faer::Accum::Replace,
                            lhs.as_ref(),
                            loader.faer_csc.as_ref(),
                            1.0,
                            *par,
                            strategy,
                            &mut stack,
                        );
                    })
                },
            );
            */
        }
    }

    group.finish();
}

fn sequential_benchmarks(c: &mut Criterion) {
    println!("Running sequential implementations benchmark...");

    for matrix_file in SMALL_MATRICES {
        match TestMatrices::load_from_matrix_market(matrix_file, 1) {
            Ok(matrices) => {
                println!(
                    "Loaded matrix '{}': {}x{} with {} non-zeros",
                    matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
                );

                bench_sequential_implementations(c, &matrices);
            }
            Err(e) => {
                eprintln!("Failed to load matrix '{}': {}", matrix_file, e);
            }
        }
    }

    create_synthetic_benchmark_sequential(c);
}

fn parallel_scaling_benchmarks(c: &mut Criterion) {
    println!("Running parallel thread scaling benchmark...");

    for matrix_file in LARGE_MATRICES.iter() {
        match SimpleMatrixLoader::load_from_matrix_market(matrix_file, 1) {
            Ok(loader) => {
                println!(
                    "Loaded matrix '{}': {}x{} with {} non-zeros",
                    loader.matrix_name, loader.nrows, loader.ncols, loader.nnz
                );

                bench_parallel_thread_scaling(c, &loader);
            }
            Err(e) => {
                eprintln!("Failed to load matrix '{}': {}", matrix_file, e);
            }
        }
    }

    //create_synthetic_benchmark_parallel(c);
}

fn sparse_matvec_benchmarks(c: &mut Criterion) {
    sequential_benchmarks(c);
    parallel_scaling_benchmarks(c);
}

fn create_synthetic_benchmark_sequential(c: &mut Criterion) {
    let synthetic_matrices = TestMatrices::create_synthetic(1000, 1000, 0.01);

    println!(
        "Created synthetic matrix for sequential: {}x{} with {} non-zeros ({:.3}% density)",
        synthetic_matrices.nrows,
        synthetic_matrices.ncols,
        synthetic_matrices.nnz,
        (synthetic_matrices.nnz as f64)
            / (synthetic_matrices.nrows * synthetic_matrices.ncols) as f64
            * 100.0
    );

    bench_sequential_implementations(c, &synthetic_matrices);
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
        let synthetic_loader = SimpleMatrixLoader::create_synthetic(size, size, density);

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

criterion_group!(sequential, sequential_benchmarks);
criterion_group!(parallel, parallel_scaling_benchmarks);

//criterion_group!(all, parallel_scaling_benchmarks, sequential_benchmarks);
criterion_group!(all, parallel_scaling_benchmarks);
criterion_main!(all);
