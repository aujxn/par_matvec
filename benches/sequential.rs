use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use faer::{Mat, Par};
use nalgebra::DVector;
use par_matvec::test_utils::{TestMatrices, small_matrix_paths};

fn sequential_benchmarks(c: &mut Criterion) {
    println!("Running sequential implementations benchmark...");

    let matrix_paths: Vec<_> = small_matrix_paths().collect();
    for matrix_path in matrix_paths {
        match TestMatrices::load_from_matrix_market(&matrix_path, 1) {
            Ok(matrices) => {
                println!(
                    "Loaded matrix '{}': {}x{} with {} non-zeros",
                    matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
                );

                bench_sequential_implementations(c, &matrices);
            }
            Err(e) => {
                eprintln!("Failed to load matrix '{}': {}", matrix_path.display(), e);
            }
        }
    }

    create_synthetic_benchmark_sequential(c);
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

criterion_group!(all, sequential_benchmarks);
criterion_main!(all);
