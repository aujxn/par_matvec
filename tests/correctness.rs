use std::num::NonZero;

use faer::{Mat, Par};
use nalgebra::DVector;

use par_matvec::{
    dense_sparse_impl::{dense_sparse_scratch, par_dense_sparse},
    sparse_dense_impl::{buffer_foreign, merge, simple},
    spmv_drivers::{SpMvStrategy, dense_sparse_matmul, sparse_dense_matmul},
    test_utils::{TestMatrices, small_matrix_paths},
};

/// Tolerance for floating point comparisons
const RELATIVE_TOLERANCE: f64 = 1e-12;
const ABSOLUTE_TOLERANCE: f64 = 1e-8;

/// Convert different vector types to a common Vec<f64> for comparison
trait ToVecF64 {
    fn to_vec_f64(&self) -> Vec<f64>;
}

impl ToVecF64 for Mat<f64> {
    fn to_vec_f64(&self) -> Vec<f64> {
        self.col(0).iter().copied().collect()
    }
}

impl ToVecF64 for nalgebra::DVector<f64> {
    fn to_vec_f64(&self) -> Vec<f64> {
        self.as_slice().to_vec()
    }
}

impl ToVecF64 for Vec<f64> {
    fn to_vec_f64(&self) -> Vec<f64> {
        self.clone()
    }
}

/// Compare two vectors with relative and absolute tolerance
fn vectors_are_equal<T1: ToVecF64, T2: ToVecF64>(
    a: &T1,
    b: &T2,
    relative_tol: f64,
    absolute_tol: f64,
) -> bool {
    let a_vec = a.to_vec_f64();
    let b_vec = b.to_vec_f64();

    if a_vec.len() != b_vec.len() {
        eprintln!("Vector lengths differ: {} vs {}", a_vec.len(), b_vec.len());
        return false;
    }

    for (i, (a_val, b_val)) in a_vec.iter().zip(b_vec.iter()).enumerate() {
        let diff = (a_val - b_val).abs();
        let max_val = a_val.abs().max(b_val.abs());

        if diff > absolute_tol || diff > relative_tol * max_val {
            eprintln!(
                "Vectors differ at index {}: {} vs {}, diff: {}, relative: {}",
                i,
                a_val,
                b_val,
                diff,
                if max_val > 0.0 { diff / max_val } else { 0.0 }
            );
            /*
            eprintln!("i: (correct, incorrect)");
            for (i, (l, r)) in a_vec.iter().zip(b_vec.iter()).enumerate() {
                println!("{}: {:.2}, {:.2}", i, l, r);
            }
            */
            return false;
        }
    }

    true
}

/// Test all sequential implementations against each other
fn test_sequential_implementations(
    matrices: &TestMatrices,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Testing sequential implementations on matrix '{}' ({}x{}, {} nnz)",
        matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
    );

    let mut faer_builtin_output = Mat::zeros(matrices.nrows, 1);
    faer::sparse::linalg::matmul::sparse_dense_matmul(
        faer_builtin_output.as_mut(),
        faer::Accum::Replace,
        matrices.faer_csc.as_ref(),
        matrices.rhs_vector.as_ref(),
        1.0,
        Par::Seq,
    );

    let nalgebra_rhs = DVector::from_iterator(
        matrices.rhs_vector.nrows(),
        matrices.rhs_vector.col(0).iter().copied(),
    );
    let nalgebra_result = &matrices.nalgebra_csc * &nalgebra_rhs;

    assert!(
        vectors_are_equal(
            &faer_builtin_output,
            &nalgebra_result,
            RELATIVE_TOLERANCE,
            ABSOLUTE_TOLERANCE
        ),
        "nalgebra-sparse implementation differs from reference"
    );
    println!("  ✓ nalgebra-sparse matches reference");

    let sprs_rhs: Vec<f64> = matrices.rhs_vector.col(0).iter().copied().collect();
    let mut sprs_output = vec![0.0; matrices.nrows];
    sprs::prod::mul_acc_mat_vec_csc(
        matrices.sprs_csc.view(),
        &sprs_rhs[..],
        &mut sprs_output[..],
    );

    assert!(
        vectors_are_equal(
            &faer_builtin_output,
            &sprs_output,
            RELATIVE_TOLERANCE,
            ABSOLUTE_TOLERANCE
        ),
        "sprs implementation differs from reference"
    );
    println!("  ✓ sprs matches reference");

    Ok(())
}

/// Test parallel implementations against sequential reference
fn test_parallel_implementations(
    matrices: &TestMatrices,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "Testing parallel implementations on matrix '{}' ({}x{}, {} nnz)",
        matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz
    );

    let mut sparse_dense_reference_output = Mat::zeros(matrices.nrows, 1);
    faer::sparse::linalg::matmul::sparse_dense_matmul(
        sparse_dense_reference_output.as_mut(),
        faer::Accum::Replace,
        matrices.faer_csc.as_ref(),
        matrices.rhs_vector.as_ref(),
        1.0,
        Par::Seq,
    );

    let mut dense_sparse_reference_output = Mat::zeros(1, matrices.ncols);
    let lhs_vector = matrices.rhs_vector.transpose();
    faer::sparse::linalg::matmul::dense_sparse_matmul(
        dense_sparse_reference_output.as_mut(),
        faer::Accum::Replace,
        lhs_vector.as_ref(),
        matrices.faer_csc.as_ref(),
        1.0,
        Par::Seq,
    );

    let cpus = num_cpus::get();
    let mut thread_counts = Vec::new();
    let mut n_threads = 2;
    while n_threads <= cpus {
        thread_counts.push(n_threads);
        n_threads *= 2;
    }

    for &num_threads in &thread_counts {
        if let Some(n_threads) = NonZero::new(num_threads) {
            let par = if num_threads == 1 {
                Par::Seq
            } else {
                Par::Rayon(n_threads)
            };

            let mut parallel_output = Mat::zeros(matrices.nrows, 1);
            let strategy = SpMvStrategy::new(matrices.faer_csc.symbolic(), par);

            let names = ["simple", "merge", "buffer_foreign"];
            let scratch_fns = [
                simple::sparse_dense_scratch,
                merge::sparse_dense_scratch,
                buffer_foreign::sparse_dense_scratch,
            ];
            let par_matvec_fns = [
                simple::par_sparse_dense,
                merge::par_sparse_dense,
                buffer_foreign::par_sparse_dense,
            ];

            for (name, (sparse_dense_scratch, par_impl)) in names
                .iter()
                .zip(scratch_fns.iter().zip(par_matvec_fns.iter()))
            {
                let stack_req = sparse_dense_scratch(
                    matrices.faer_csc.as_ref(),
                    matrices.rhs_vector.as_ref(),
                    &strategy,
                    par,
                );
                let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req)?;
                let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);

                sparse_dense_matmul(
                    parallel_output.as_mut(),
                    faer::Accum::Replace,
                    matrices.faer_csc.as_ref(),
                    matrices.rhs_vector.as_ref(),
                    1.0,
                    par,
                    &strategy,
                    &mut stack,
                    Some(*par_impl),
                );

                assert!(
                    vectors_are_equal(
                        &sparse_dense_reference_output,
                        &parallel_output,
                        RELATIVE_TOLERANCE,
                        ABSOLUTE_TOLERANCE
                    ),
                    "Parallel sparse-dense {} implementation with {} threads differs from reference",
                    name,
                    num_threads
                );
            }

            let stack_req =
                dense_sparse_scratch(lhs_vector, matrices.faer_csc.as_ref(), &strategy, par);
            let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req)?;
            let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);

            let mut parallel_output = Mat::zeros(1, matrices.nrows);
            dense_sparse_matmul(
                parallel_output.as_mut(),
                faer::Accum::Replace,
                lhs_vector,
                matrices.faer_csc.as_ref(),
                1.0,
                par,
                &strategy,
                &mut stack,
                Some(par_dense_sparse),
            );

            assert!(
                vectors_are_equal(
                    &dense_sparse_reference_output,
                    &parallel_output,
                    RELATIVE_TOLERANCE,
                    ABSOLUTE_TOLERANCE
                ),
                "Parallel dense-sparse implementation with {} threads differs from reference\n{:?}\n{:?}",
                num_threads,
                dense_sparse_reference_output,
                parallel_output
            );

            println!(
                "  ✓ Custom parallel with {} threads matches reference",
                num_threads
            );
        }
    }

    Ok(())
}

#[test]
fn test_synthetic_matrices() {
    let test_cases = [
        (10, 10, 0.5),
        (50, 50, 0.1),
        (100, 100, 0.05),
        (2000, 2000, 0.05),
    ];

    for &(nrows, ncols, density) in &test_cases {
        println!(
            "\nTesting synthetic matrix {}x{} with density {}",
            nrows, ncols, density
        );
        let matrices = TestMatrices::create_synthetic(nrows, ncols, density);

        test_sequential_implementations(&matrices).expect("Sequential tests failed");
        if matrices.nnz > 1000 {
            test_parallel_implementations(&matrices).expect("Parallel tests failed");
        }
    }

    println!("All synthetic matrix tests passed!");
}

#[test]
fn test_matrix_market_files() {
    let matrix_paths: Vec<_> = small_matrix_paths().collect();

    for matrix_path in matrix_paths {
        if matrix_path.exists() {
            println!("\nTesting matrix file: {}", matrix_path.display());
            match TestMatrices::load_from_matrix_market(&matrix_path, 1) {
                Ok(matrices) => {
                    test_sequential_implementations(&matrices).expect(&format!(
                        "Sequential tests failed on {}",
                        matrix_path.display()
                    ));

                    // Only test parallel if matrix has enough non-zeros
                    if matrices.nnz > 32 {
                        test_parallel_implementations(&matrices).expect(&format!(
                            "Parallel tests failed on {}",
                            matrix_path.display()
                        ));
                    } else {
                        println!(
                            "  Skipping parallel test (too few non-zeros: {})",
                            matrices.nnz
                        );
                    }
                }
                Err(e) => {
                    panic!(
                        "Failed to load matrix market file {}: {}",
                        matrix_path.display(),
                        e
                    );
                }
            }
        } else {
            println!("Matrix file {} not found, skipping", matrix_path.display());
        }
    }

    println!("All matrix market file tests passed!");
}

#[test]
fn test_edge_cases() {
    println!("\nTesting dense matrix");
    let dense_matrices = TestMatrices::create_synthetic(20, 20, 1.0);

    test_sequential_implementations(&dense_matrices)
        .expect("Sequential tests failed on dense matrix");
    let dense_matrices = TestMatrices::create_synthetic(200, 200, 1.0);
    test_parallel_implementations(&dense_matrices).expect("Parallel tests failed on dense matrix");

    println!("Edge case tests passed!");
}
