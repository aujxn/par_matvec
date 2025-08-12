use std::path::Path;

use faer::{
    Mat,
    sparse::{SparseColMat, Triplet},
};
use nalgebra_sparse::io::load_coo_from_matrix_market_file;
use nalgebra_sparse::{CooMatrix, CsrMatrix};
use sprs::{CsMatBase, TriMat};

/// Small matrices (< 1M nnz) for correctness tests and sequential benchmarks
/// (2d FEM using continuous linear elements for laplacian problem)
pub const SMALL_MATRICES: &[&str] = &[
    "test_matrices/0.mtx", // 18x18, 68 nnz
    "test_matrices/1.mtx", // 51x51, 215 nnz
    "test_matrices/2.mtx", // 165x165, 749 nnz
    "test_matrices/3.mtx", // 585x585, 2,777 nnz
    "test_matrices/4.mtx", // 2,193x2,193, 10,673 nnz
    "test_matrices/5.mtx", // 8,481x8,481, 41,825 nnz
];

/// Large matrices (>= 1M nnz) for parallel scaling benchmarks
/// (2d and 3d FEM using continuous linear elements for laplacian problem)
pub const LARGE_MATRICES: &[&str] = &[
    //"test_matrices/anisotropy_3d_1r.mtx", // 84k x 84k, 1.4M nnz
    //"test_matrices/anisotropy_3d_2r.mtx", // 650k x 650k, 11M nnz
    //"test_matrices/anisotropy_2d.mtx",    // 1.3M x 1.3M, 12M nnz
    "test_matrices/spe10_0.mtx", // 1.2M x 1.2M, 31M nnz
];

/// Test matrices struct with all different format representations
pub struct TestMatrices {
    pub faer_csc: SparseColMat<usize, f64>,
    pub nalgebra_csr: CsrMatrix<f64>,
    pub sprs_csr: CsMatBase<f64, usize, Vec<usize>, Vec<usize>, Vec<f64>>,
    pub rhs_vector: Mat<f64>,
    pub matrix_name: String,
    pub nrows: usize,
    pub ncols: usize,
    pub nnz: usize,
}

impl TestMatrices {
    /// Load a test matrix from a Matrix Market file with specified number of RHS columns
    pub fn load_from_matrix_market<P: AsRef<Path>>(
        path: P,
        rhs_cols: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let matrix_name = path.file_stem().unwrap().to_string_lossy().to_string();

        let coo_matrix: CooMatrix<f64> = load_coo_from_matrix_market_file(path)?;

        let nrows = coo_matrix.nrows();
        let ncols = coo_matrix.ncols();
        let nnz = coo_matrix.nnz();

        let nalgebra_csr = CsrMatrix::from(&coo_matrix);

        let (row_indices, col_indices, values) = coo_matrix.disassemble();
        let mut sprs_triplet = TriMat::new((nrows, ncols));
        for ((row, col), val) in row_indices
            .iter()
            .zip(col_indices.iter())
            .zip(values.iter())
        {
            sprs_triplet.add_triplet(*row, *col, *val);
        }
        let sprs_csr = sprs_triplet.to_csr();

        let faer_triplets: Vec<Triplet<usize, usize, f64>> = row_indices
            .iter()
            .zip(col_indices.iter())
            .zip(values.iter())
            .map(|((row, col), val)| Triplet::new(*row, *col, *val))
            .collect();

        let faer_csc =
            SparseColMat::<usize, f64>::try_new_from_triplets(nrows, ncols, &faer_triplets)?;

        let mut rhs_vector = Mat::zeros(ncols, rhs_cols);
        for j in 0..rhs_cols {
            for i in 0..ncols {
                if rhs_cols == 1 {
                    rhs_vector[(i, j)] = (i as f64 * 0.1 + 1.0) % 10.0;
                } else {
                    rhs_vector[(i, j)] = ((i + j * 1000) as f64 * 0.0001) % 1.0;
                }
            }
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

    /// Create a synthetic test matrix with deterministic pattern
    pub fn create_synthetic(nrows: usize, ncols: usize, density: f64) -> Self {
        let mut triplets = Vec::new();

        for i in 0..nrows {
            for j in 0..ncols {
                if ((i * 7 + j * 11) % 100) as f64 / 100.0 < density {
                    let value = (i as f64 + j as f64 * 0.1 + 1.0) % 5.0 + 0.1; // Avoid zeros
                    triplets.push(Triplet::new(i, j, value));
                }
            }
        }

        let faer_csc =
            SparseColMat::<usize, f64>::try_new_from_triplets(nrows, ncols, &triplets).unwrap();

        let tuple_triplets: Vec<(usize, usize, f64)> =
            triplets.iter().map(|t| (t.row, t.col, t.val)).collect();

        let row_indices: Vec<usize> = tuple_triplets.iter().map(|&(r, _, _)| r).collect();
        let col_indices: Vec<usize> = tuple_triplets.iter().map(|&(_, c, _)| c).collect();
        let values: Vec<f64> = tuple_triplets.iter().map(|&(_, _, v)| v).collect();

        let coo_matrix =
            CooMatrix::try_from_triplets(nrows, ncols, row_indices, col_indices, values).unwrap();
        let nalgebra_csr = CsrMatrix::from(&coo_matrix);

        let mut sprs_triplet = TriMat::new((nrows, ncols));
        for &(row, col, val) in &tuple_triplets {
            sprs_triplet.add_triplet(row, col, val);
        }
        let sprs_csr = sprs_triplet.to_csr();

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

/// Simple matrix loader for parallel benchmarks that only needs faer_csc format
pub struct SimpleMatrixLoader {
    pub faer_csc: SparseColMat<usize, f64>,
    pub rhs_vector: Mat<f64>,
    pub matrix_name: String,
    pub nrows: usize,
    pub ncols: usize,
    pub nnz: usize,
}

impl SimpleMatrixLoader {
    /// Load matrix from Matrix Market file for parallel-only benchmarks
    pub fn load_from_matrix_market<P: AsRef<Path>>(
        path: P,
        rhs_cols: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let matrix_name = path.file_stem().unwrap().to_string_lossy().to_string();

        let coo_matrix: CooMatrix<f64> = load_coo_from_matrix_market_file(path)?;

        let nrows = coo_matrix.nrows();
        let ncols = coo_matrix.ncols();
        let nnz = coo_matrix.nnz();

        let (row_indices, col_indices, values) = coo_matrix.disassemble();
        let faer_triplets: Vec<Triplet<usize, usize, f64>> = row_indices
            .iter()
            .zip(col_indices.iter())
            .zip(values.iter())
            .map(|((row, col), val)| Triplet::new(*row, *col, *val))
            .collect();

        let faer_csc =
            SparseColMat::<usize, f64>::try_new_from_triplets(nrows, ncols, &faer_triplets)?;

        let mut rhs_vector = Mat::zeros(ncols, rhs_cols);
        for j in 0..rhs_cols {
            for i in 0..ncols {
                rhs_vector[(i, j)] = ((i + j * 1000) as f64 * 0.0001) % 1.0;
            }
        }

        Ok(SimpleMatrixLoader {
            faer_csc,
            rhs_vector,
            matrix_name,
            nrows,
            ncols,
            nnz,
        })
    }

    /// Create a synthetic test matrix for parallel benchmarks only
    pub fn create_synthetic(nrows: usize, ncols: usize, density: f64) -> Self {
        let mut triplets = Vec::new();

        for i in 0..nrows {
            for j in 0..ncols {
                if ((i * 7 + j * 11) % 100) as f64 / 100.0 < density {
                    let value = (i as f64 + j as f64 * 0.1 + 1.0) % 5.0 + 0.1; // Avoid zeros
                    triplets.push(Triplet::new(i, j, value));
                }
            }
        }

        let faer_csc =
            SparseColMat::<usize, f64>::try_new_from_triplets(nrows, ncols, &triplets).unwrap();

        let mut rhs_vector = Mat::zeros(ncols, 1);
        for i in 0..ncols {
            rhs_vector[(i, 0)] = ((i * 17) as f64 * 0.0001) % 1.0;
        }

        SimpleMatrixLoader {
            faer_csc,
            rhs_vector,
            matrix_name: format!("synthetic-{}x{}_{:.2}", nrows, ncols, density),
            nrows,
            ncols,
            nnz: triplets.len(),
        }
    }
}
