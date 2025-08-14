use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};

use faer::{
    Mat,
    sparse::{SparseColMat, Triplet},
};
use nalgebra_sparse::{CooMatrix, CsrMatrix};
use sprs::{CsMatBase, TriMat};

/// Size thresholds for matrix categorization
const SMALL_NNZ_THRESHOLD: usize = 100_000;
const MEDIUM_NNZ_THRESHOLD: usize = 1_000_000;

/// Iterator over small matrix files (< 100k nnz)
pub fn small_matrix_paths() -> impl Iterator<Item = PathBuf> {
    matrix_paths_in_range(0, SMALL_NNZ_THRESHOLD)
}

/// Iterator over medium matrix files (100k - 1M nnz)
pub fn medium_matrix_paths() -> impl Iterator<Item = PathBuf> {
    matrix_paths_in_range(SMALL_NNZ_THRESHOLD, MEDIUM_NNZ_THRESHOLD)
}

/// Iterator over large matrix files (>= 1M nnz)
pub fn large_matrix_paths() -> impl Iterator<Item = PathBuf> {
    matrix_paths_in_range(MEDIUM_NNZ_THRESHOLD, usize::MAX)
}

/// Helper function to get matrix paths within a nnz range
fn matrix_paths_in_range(min_nnz: usize, max_nnz: usize) -> impl Iterator<Item = PathBuf> {
    get_all_matrix_paths()
        .into_iter()
        .filter(move |path| {
            if let Ok(nnz) = get_matrix_nnz(path) {
                nnz >= min_nnz && nnz < max_nnz
            } else {
                false
            }
        })
}

/// Get all matrix file paths from test_matrices directory
fn get_all_matrix_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    // Add files from main test_matrices directory
    if let Ok(entries) = fs::read_dir("test_matrices") {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("mtx") {
                    paths.push(path);
                }
            }
        }
    }
    
    // Add files from suitesparse subdirectory
    if let Ok(entries) = fs::read_dir("test_matrices/suitesparse") {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("mtx") {
                    paths.push(path);
                }
            }
        }
    }
    
    paths.sort();
    paths
}

/// Get the number of non-zeros for a matrix file by reading only the header
fn get_matrix_nnz(path: &Path) -> Result<usize, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    
    // Skip comment lines starting with %
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        if !line.starts_with('%') {
            break;
        }
    }
    
    // Parse the header line: rows cols nnz
    let parts: Vec<&str> = line.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return Err(format!("Invalid Matrix Market header: {}", line.trim()).into());
    }
    
    let nnz = parts[2].parse::<usize>()?;
    Ok(nnz)
}

/// Convert matrix_market_rs MtxData to nalgebra CooMatrix
fn mtx_data_to_nalgebra_coo(mtx_data: matrix_market_rs::MtxData<f64, 2>) -> Result<CooMatrix<f64>, Box<dyn std::error::Error>> {
    let (nrows, ncols, coord, val) = match mtx_data {
        matrix_market_rs::MtxData::Sparse([nrows, ncols], coord, val, _symmetry) => {
            (nrows, ncols, coord, val)
        },
        _ => {
            return Err("Only sparse Matrix Market files are supported".into());
        }
    };
    
    let row_indices: Vec<usize> = coord.iter().map(|&[row, _col]| row).collect();
    let col_indices: Vec<usize> = coord.iter().map(|&[_row, col]| col).collect();
    
    Ok(CooMatrix::try_from_triplets(nrows, ncols, row_indices, col_indices, val)?)
}

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

        let coo_matrix: CooMatrix<f64> = mtx_data_to_nalgebra_coo(matrix_market_rs::MtxData::<f64, 2>::from_file(path)?)?;

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

/// Matrix loader for parallel benchmarks that only needs faer_csc format
pub struct FaerLoader {
    pub faer_csc: SparseColMat<usize, f64>,
    pub rhs_vector: Mat<f64>,
    pub matrix_name: String,
    pub nrows: usize,
    pub ncols: usize,
    pub nnz: usize,
}

impl FaerLoader {
    /// Load matrix from Matrix Market file for parallel-only benchmarks
    pub fn load_from_matrix_market<P: AsRef<Path>>(
        path: P,
        rhs_cols: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        let matrix_name = path.file_stem().unwrap().to_string_lossy().to_string();

        let coo_matrix: CooMatrix<f64> = mtx_data_to_nalgebra_coo(matrix_market_rs::MtxData::<f64, 2>::from_file(path)?)?;

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

        Ok(FaerLoader {
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

        FaerLoader {
            faer_csc,
            rhs_vector,
            matrix_name: format!("synthetic-{}x{}_{:.2}", nrows, ncols, density),
            nrows,
            ncols,
            nnz: triplets.len(),
        }
    }
}
