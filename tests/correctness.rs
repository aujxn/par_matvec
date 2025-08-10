use std::num::NonZero;

use faer::{Mat, Par};
use nalgebra::DVector;

use par_matvec::test_utils::{SMALL_MATRICES, TestMatrices};
use par_matvec::{
    SparseDenseStrategy, dense_sparse_matmul, sparse_dense_matmul, sparse_dense_scratch,
};

/// Tolerance for floating point comparisons
const RELATIVE_TOLERANCE: f64 = 1e-10;
const ABSOLUTE_TOLERANCE: f64 = 1e-12;

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

        if diff > absolute_tol && diff > relative_tol * max_val {
            eprintln!(
                "Vectors differ at index {}: {} vs {}, diff: {}, relative: {}",
                i,
                a_val,
                b_val,
                diff,
                if max_val > 0.0 { diff / max_val } else { 0.0 }
            );
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
    let nalgebra_result = &matrices.nalgebra_csr * &nalgebra_rhs;

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
    sprs::prod::mul_acc_mat_vec_csr(
        matrices.sprs_csr.view(),
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
            let strategy = SparseDenseStrategy::new(matrices.faer_csc.symbolic(), par);

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
            );

            assert!(
                vectors_are_equal(
                    &sparse_dense_reference_output,
                    &parallel_output,
                    RELATIVE_TOLERANCE,
                    ABSOLUTE_TOLERANCE
                ),
                "Parallel sparse-dense implementation with {} threads differs from reference",
                num_threads
            );

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
        (10, 10, 0.5),    // Small dense
        (50, 50, 0.1),    // Medium sparse
        (100, 100, 0.05), // Larger sparser
    ];

    for &(nrows, ncols, density) in &test_cases {
        println!(
            "\nTesting synthetic matrix {}x{} with density {}",
            nrows, ncols, density
        );
        let matrices = TestMatrices::create_synthetic(nrows, ncols, density);

        test_sequential_implementations(&matrices).expect("Sequential tests failed");
        test_parallel_implementations(&matrices).expect("Parallel tests failed");
    }

    println!("All synthetic matrix tests passed!");
}

#[test]
fn test_matrix_market_files() {
    let matrix_files = &SMALL_MATRICES[0..4];

    for matrix_file in matrix_files {
        if std::path::Path::new(matrix_file).exists() {
            println!("\nTesting matrix file: {}", matrix_file);
            match TestMatrices::load_from_matrix_market(matrix_file, 1) {
                Ok(matrices) => {
                    test_sequential_implementations(&matrices)
                        .expect(&format!("Sequential tests failed on {}", matrix_file));

                    // Only test parallel if matrix has enough non-zeros
                    if matrices.nnz > 100 {
                        test_parallel_implementations(&matrices)
                            .expect(&format!("Parallel tests failed on {}", matrix_file));
                    } else {
                        println!(
                            "  Skipping parallel test (too few non-zeros: {})",
                            matrices.nnz
                        );
                    }
                }
                Err(e) => {
                    panic!("Failed to load matrix market file {}: {}", matrix_file, e);
                }
            }
        } else {
            println!("Matrix file {} not found, skipping", matrix_file);
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
    test_parallel_implementations(&dense_matrices).expect("Parallel tests failed on dense matrix");

    println!("Edge case tests passed!");
}

fn run_all_matrix_tests() -> Result<(), Box<dyn std::error::Error>> {
    let matrix_files = SMALL_MATRICES;

    println!("Running correctness tests on all available matrices...\n");

    for matrix_file in matrix_files {
        if std::path::Path::new(matrix_file).exists() {
            println!("Testing matrix file: {}", matrix_file);
            match TestMatrices::load_from_matrix_market(matrix_file, 1) {
                Ok(matrices) => {
                    test_sequential_implementations(&matrices)?;

                    // Only test parallel if matrix has sufficient work
                    if matrices.nnz > 50 {
                        test_parallel_implementations(&matrices)?;
                    } else {
                        println!(
                            "  Skipping parallel test (too few non-zeros: {})",
                            matrices.nnz
                        );
                    }
                    println!("  ✅ All tests passed for {}\n", matrix_file);
                }
                Err(e) => {
                    eprintln!("  ❌ Failed to load {}: {}\n", matrix_file, e);
                }
            }
        } else {
            println!("Matrix file {} not found, skipping\n", matrix_file);
        }
    }

    Ok(())
}

#[test]
#[ignore]
fn test_all_matrices() {
    run_all_matrix_tests().expect("Comprehensive matrix tests failed");
    println!("🎉 All comprehensive matrix tests passed!");
}
