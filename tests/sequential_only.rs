use std::path::Path;

use faer::{
    sparse::{SparseColMat, Triplet}, 
    Mat, Par
};
use nalgebra_sparse::{CooMatrix, CsrMatrix};
use nalgebra_sparse::io::load_coo_from_matrix_market_file;
use sprs::{TriMat, CsMatBase};

use par_matvec::{SparseDenseStrategy, sparse_dense_scratch, sparse_dense_matmul};

/// Tolerance for floating point comparisons
const RELATIVE_TOLERANCE: f64 = 1e-10;
const ABSOLUTE_TOLERANCE: f64 = 1e-12;

/// Test matrices struct with all different format representations
struct TestMatrices {
    faer_csc: SparseColMat<usize, f64>,
    nalgebra_csr: CsrMatrix<f64>,
    sprs_csr: CsMatBase<f64, usize, Vec<usize>, Vec<usize>, Vec<f64>>,
    rhs_vector: Mat<f64>,
    matrix_name: String,
    nrows: usize,
    ncols: usize,
    nnz: usize,
}

impl TestMatrices {
    fn load_from_matrix_market<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let matrix_name = path.file_stem().unwrap().to_string_lossy().to_string();
        
        // Load using nalgebra-sparse (most reliable for matrix-market format)
        let coo_matrix: CooMatrix<f64> = load_coo_from_matrix_market_file(path)?;
        
        let nrows = coo_matrix.nrows();
        let ncols = coo_matrix.ncols();
        let nnz = coo_matrix.nnz();
        
        // Convert to different formats
        let nalgebra_csr = CsrMatrix::from(&coo_matrix);
        
        // Convert to sprs format
        let (row_indices, col_indices, values) = coo_matrix.disassemble();
        let mut sprs_triplet = TriMat::new((nrows, ncols));
        for ((row, col), val) in row_indices.iter().zip(col_indices.iter()).zip(values.iter()) {
            sprs_triplet.add_triplet(*row, *col, *val);
        }
        let sprs_csr = sprs_triplet.to_csr();
        
        // Convert to faer format
        let faer_triplets: Vec<Triplet<usize, usize, f64>> = row_indices.iter()
            .zip(col_indices.iter())
            .zip(values.iter())
            .map(|((row, col), val)| Triplet::new(*row, *col, *val))
            .collect();
        
        let faer_csc = SparseColMat::<usize, f64>::try_new_from_triplets(
            nrows, ncols, &faer_triplets
        )?;
        
        // Create deterministic right-hand side vector for reproducible testing
        let mut rhs_vector = Mat::zeros(ncols, 1);
        for i in 0..ncols {
            // Use a simple deterministic pattern instead of random values
            rhs_vector[(i, 0)] = (i as f64 * 0.1 + 1.0) % 10.0;
        }
        
        Ok(TestMatrices {
            faer_csc,
            nalgebra_csr,
            sprs_csr,
            rhs_vector,
            matrix_name,
            nrows,
            ncols,
            nnz,
        })
    }
    
    fn create_synthetic(nrows: usize, ncols: usize, density: f64) -> Self {
        // Create deterministic synthetic matrix for reproducible testing
        let mut triplets = Vec::new();
        
        // Use a simple pattern to generate deterministic non-zeros
        for i in 0..nrows {
            for j in 0..ncols {
                // Use deterministic pattern instead of random
                if ((i * 7 + j * 11) % 100) as f64 / 100.0 < density {
                    let value = (i as f64 + j as f64 * 0.1 + 1.0) % 5.0 + 0.1; // Avoid zeros
                    triplets.push(Triplet::new(i, j, value));
                }
            }
        }
        
        // Create faer matrix
        let faer_csc = SparseColMat::<usize, f64>::try_new_from_triplets(nrows, ncols, &triplets).unwrap();
        
        // Convert to other formats for testing
        let tuple_triplets: Vec<(usize, usize, f64)> = triplets.iter()
            .map(|t| (t.row, t.col, t.val))
            .collect();
        
        // Create COO matrix for conversion
        let row_indices: Vec<usize> = tuple_triplets.iter().map(|&(r, _, _)| r).collect();
        let col_indices: Vec<usize> = tuple_triplets.iter().map(|&(_, c, _)| c).collect();
        let values: Vec<f64> = tuple_triplets.iter().map(|&(_, _, v)| v).collect();
        
        let coo_matrix = CooMatrix::try_from_triplets(nrows, ncols, row_indices, col_indices, values).unwrap();
        let nalgebra_csr = CsrMatrix::from(&coo_matrix);
        
        // Create sprs format
        let mut sprs_triplet = TriMat::new((nrows, ncols));
        for &(row, col, val) in &tuple_triplets {
            sprs_triplet.add_triplet(row, col, val);
        }
        let sprs_csr = sprs_triplet.to_csr();
        
        // Create deterministic rhs vector
        let mut rhs_vector = Mat::zeros(ncols, 1);
        for i in 0..ncols {
            rhs_vector[(i, 0)] = (i as f64 * 0.1 + 1.0) % 10.0;
        }
        
        TestMatrices {
            faer_csc,
            nalgebra_csr,
            sprs_csr,
            rhs_vector,
            matrix_name: "synthetic".to_string(),
            nrows,
            ncols,
            nnz: triplets.len(),
        }
    }
}

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
fn vectors_are_equal<T1: ToVecF64, T2: ToVecF64>(a: &T1, b: &T2, relative_tol: f64, absolute_tol: f64) -> bool {
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
            eprintln!("Vectors differ at index {}: {} vs {}, diff: {}, relative: {}", 
                     i, a_val, b_val, diff, if max_val > 0.0 { diff / max_val } else { 0.0 });
            return false;
        }
    }
    
    true
}

/// Test ONLY sequential implementations against each other (no parallel)
fn test_sequential_implementations_only(matrices: &TestMatrices) -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing SEQUENTIAL ONLY implementations on matrix '{}' ({}x{}, {} nnz)", 
             matrices.matrix_name, matrices.nrows, matrices.ncols, matrices.nnz);
    
    // Compute reference result using faer built-in sequential
    let mut faer_builtin_output = Mat::zeros(matrices.nrows, 1);
    faer::sparse::linalg::matmul::sparse_dense_matmul(
        faer_builtin_output.as_mut(),
        faer::Accum::Replace,
        matrices.faer_csc.as_ref(),
        matrices.rhs_vector.as_ref(),
        1.0,
        Par::Seq,
    );
    
    // Test custom sequential implementation (Par::Seq only)
    let mut custom_output = Mat::zeros(matrices.nrows, 1);
    let strategy = SparseDenseStrategy::new(matrices.faer_csc.symbolic(), Par::Seq);
    let stack_req = sparse_dense_scratch(
        matrices.faer_csc.as_ref(),
        matrices.rhs_vector.as_ref(),
        Par::Seq,
    );
    let mut stack_buffer = faer::dyn_stack::MemBuffer::try_new(stack_req)?;
    let mut stack = faer::dyn_stack::MemStack::new(&mut stack_buffer);
    
    sparse_dense_matmul(
        custom_output.as_mut(),
        faer::Accum::Replace,
        matrices.faer_csc.as_ref(),
        matrices.rhs_vector.as_ref(),
        1.0,
        Par::Seq,
        &strategy,
        &mut stack,
    );
    
    assert!(
        vectors_are_equal(&faer_builtin_output, &custom_output, RELATIVE_TOLERANCE, ABSOLUTE_TOLERANCE),
        "Custom sequential implementation differs from faer built-in"
    );
    println!("  ✅ Custom sequential matches faer built-in");
    
    // Test nalgebra-sparse
    let nalgebra_rhs = nalgebra::DVector::from_iterator(
        matrices.rhs_vector.nrows(), 
        matrices.rhs_vector.col(0).iter().copied()
    );
    let nalgebra_result = &matrices.nalgebra_csr * &nalgebra_rhs;
    
    assert!(
        vectors_are_equal(&faer_builtin_output, &nalgebra_result, RELATIVE_TOLERANCE, ABSOLUTE_TOLERANCE),
        "nalgebra-sparse implementation differs from reference"
    );
    println!("  ✅ nalgebra-sparse matches reference");
    
    // Test sprs
    let sprs_rhs: Vec<f64> = matrices.rhs_vector.col(0).iter().copied().collect();
    let mut sprs_output = vec![0.0; matrices.nrows];
    sprs::prod::mul_acc_mat_vec_csr(
        matrices.sprs_csr.view(),
        &sprs_rhs[..],
        &mut sprs_output[..],
    );
    
    assert!(
        vectors_are_equal(&faer_builtin_output, &sprs_output, RELATIVE_TOLERANCE, ABSOLUTE_TOLERANCE),
        "sprs implementation differs from reference"
    );
    println!("  ✅ sprs matches reference");
    
    Ok(())
}

#[test]
fn test_synthetic_sequential_only() {
    let test_cases = [
        (10, 10, 0.5),   // Small dense
        (50, 50, 0.1),   // Medium sparse
        (100, 100, 0.05), // Larger sparser
    ];
    
    for &(nrows, ncols, density) in &test_cases {
        println!("\nTesting synthetic matrix {}x{} with density {}", nrows, ncols, density);
        let matrices = TestMatrices::create_synthetic(nrows, ncols, density);
        
        test_sequential_implementations_only(&matrices).expect("Sequential tests failed");
    }
    
    println!("\n🎉 All synthetic sequential-only tests passed!");
}

#[test] 
fn test_matrix_market_sequential_only() {
    let matrix_files = [
        "test_matrices/0.mtx",
        "test_matrices/1.mtx",
        "test_matrices/2.mtx",
        "test_matrices/anisotropy_2d.mtx",
    ];
    
    for matrix_file in &matrix_files {
        if std::path::Path::new(matrix_file).exists() {
            println!("\nTesting matrix file: {}", matrix_file);
            match TestMatrices::load_from_matrix_market(matrix_file) {
                Ok(matrices) => {
                    test_sequential_implementations_only(&matrices)
                        .expect(&format!("Sequential tests failed on {}", matrix_file));
                    println!("  ✅ Sequential tests passed for {}", matrix_file);
                }
                Err(e) => {
                    panic!("Failed to load matrix market file {}: {}", matrix_file, e);
                }
            }
        } else {
            println!("Matrix file {} not found, skipping", matrix_file);
        }
    }
    
    println!("\n🎉 All matrix market sequential-only tests passed!");
}

#[test]
#[ignore] // Use `cargo test test_all_matrices_sequential_only -- --ignored` to run
fn test_all_matrices_sequential_only() {
    let matrix_files = [
        "test_matrices/0.mtx",
        "test_matrices/1.mtx", 
        "test_matrices/2.mtx",
        "test_matrices/3.mtx",
        "test_matrices/4.mtx",
        "test_matrices/5.mtx",
        "test_matrices/6.mtx", 
        "test_matrices/7.mtx",
        "test_matrices/anisotropy_2d.mtx",
        "test_matrices/anisotropy_3d_1r.mtx",
        "test_matrices/anisotropy_3d_2r.mtx",
        "test_matrices/anisotropy_3d_3r.mtx",
        "test_matrices/anisotropy_3d_4r.mtx", 
        "test_matrices/anisotropy_3d_5r.mtx",
        "test_matrices/spe10_0.mtx",
    ];
    
    println!("Running sequential-only correctness tests on all available matrices...\n");
    
    let mut passed = 0;
    let mut failed = 0;
    
    for matrix_file in &matrix_files {
        if std::path::Path::new(matrix_file).exists() {
            println!("Testing matrix file: {}", matrix_file);
            match TestMatrices::load_from_matrix_market(matrix_file) {
                Ok(matrices) => {
                    match test_sequential_implementations_only(&matrices) {
                        Ok(_) => {
                            println!("  ✅ All sequential tests passed for {}\n", matrix_file);
                            passed += 1;
                        }
                        Err(e) => {
                            eprintln!("  ❌ Sequential tests failed for {}: {}\n", matrix_file, e);
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("  ❌ Failed to load {}: {}\n", matrix_file, e);
                    failed += 1;
                }
            }
        } else {
            println!("Matrix file {} not found, skipping\n", matrix_file);
        }
    }
    
    println!("Sequential-only test results: {} passed, {} failed", passed, failed);
    
    if failed == 0 {
        println!("🎉 All sequential-only tests passed!");
    } else {
        panic!("Some sequential tests failed!");
    }
}