use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use faer::{Mat, Par};
use nalgebra::DVector;
use par_matvec::test_utils::{TestMatrices, get_all_matrix_paths};

fn sequential_benchmarks(c: &mut Criterion) {
    println!("Running sequential implementations benchmark...");

    for matrix_path in get_all_matrix_paths() {
        match TestMatrices::load_from_matrix_market(&matrix_path, 1) {
            Ok(matrices) => {
                println!(
                    "Loaded matrix '{}': {}x{} with {} non-zeros",
                    matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
                );

                bench_sequential_sparse_dense_implementations(c, &matrices);
                bench_sequential_dense_sparse_implementations(c, matrices);
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

    bench_sequential_sparse_dense_implementations(c, &synthetic_matrices);
    bench_sequential_dense_sparse_implementations(c, synthetic_matrices);
}

fn bench_sequential_sparse_dense_implementations(c: &mut Criterion, matrices: &TestMatrices) {
    let mut group = c.benchmark_group(format!(
        "sequential_sparse_dense_{}-{}x{}_nnz{}",
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

    //let mut result = nalgebra::DVector::zeros(matrices.nrows);
    group.bench_with_input(
        BenchmarkId::new(
            "nalgebra",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        matrices,
        |b, matrices| {
            b.iter(|| {
                let _result = &matrices.nalgebra_csc * &nalgebra_rhs;
                /* This API may be slightly better since it doesn't have to allocate the result
                * vector in the benchmark unlike the operator version... But looking at the
                * implementation I see that even when the first arg `beta` is 0.0 it still scales
                * the `c` vector by it and accumulates it so there is a lot of wasted operations.
                nalgebra_sparse::ops::serial::spmm_csc_dense(
                    0.0,
                    nalgebra_sparse::ops::Op::NoOp(&matrices.nalgebra_csc),
                    1.0,
                    nalgebra_sparse::ops::Op::NoOp(&nalgebra_rhs),
                    &mut result,
                );
                */
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
                sprs::prod::mul_acc_mat_vec_csc(
                    matrices.sprs_csc.view(),
                    &sprs_rhs[..],
                    &mut sprs_output[..],
                );
            })
        },
    );

    group.finish();
}

fn bench_sequential_dense_sparse_implementations(c: &mut Criterion, matrices: TestMatrices) {
    let mut group = c.benchmark_group(format!(
        "sequential_dense_sparse_{}-{}x{}_nnz{}",
        matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
    ));
    group.sample_size(100);

    let lhs_mat = matrices.rhs_vector.transpose();
    let lhs_row = lhs_mat.row(0);
    let mut faer_output = Mat::zeros(lhs_row.nrows(), matrices.ncols);

    let nalgebra_lhs = DVector::from_iterator(matrices.rhs_vector.nrows(), lhs_row.iter().copied());
    let sprs_lhs: Vec<f64> = lhs_row.iter().copied().collect();
    let mut sprs_output = vec![0.0; lhs_row.ncols()];

    // These transpose APIs simply switch the storage type from CSC to CSR
    // So we bench CSR times dense_vec which is equivalent to faer row dense_vec times CSC
    let nalgebra_csr = &matrices.nalgebra_csc.transpose_as_csr();
    let sprs_csr = &matrices.sprs_csc.transpose_into();
    let faer_csc = &matrices.faer_csc;

    group.bench_function(
        BenchmarkId::new(
            "faer",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        |b| {
            b.iter(|| {
                faer::sparse::linalg::matmul::dense_sparse_matmul(
                    faer_output.as_mut(),
                    faer::Accum::Replace,
                    lhs_mat.as_ref(),
                    faer_csc.as_ref(),
                    1.0,
                    Par::Seq,
                );
            })
        },
    );

    //let mut result = nalgebra::DMatrix::zeros(lhs_vector.nrows(), matrices.ncols);
    group.bench_function(
        BenchmarkId::new(
            "nalgebra",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        |b| {
            b.iter(|| {
                let _result = nalgebra_csr * &nalgebra_lhs;
                /* This API may be slightly better with caveat, see `sparse_dense` bench comment.
                nalgebra_sparse::ops::serial::spmm_csr_dense(
                    1.0,
                    nalgebra_sparse::ops::Op::NoOp(&nalgebra_csr),
                    0.0,
                    nalgebra_sparse::ops::Op::NoOp(&lhs_nalgebra),
                    &mut result,
                );
                */
            })
        },
    );

    group.bench_function(
        BenchmarkId::new(
            "sprs",
            format!("{}x{}_nnz{}", matrices.nrows, matrices.ncols, matrices.nnz),
        ),
        |b| {
            b.iter(|| {
                sprs::prod::mul_acc_mat_vec_csr(
                    sprs_csr.view(),
                    &sprs_lhs[..],
                    &mut sprs_output[..],
                );
            })
        },
    );

    group.finish();
}

criterion_group!(all, sequential_benchmarks);
criterion_main!(all);
