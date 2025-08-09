# Benchmark Suite for par_matvec

This benchmark suite compares different sparse matrix-vector multiplication implementations and evaluates thread scaling performance.

## Overview

The benchmark suite tests:

1. **Sequential Implementations**:
   - `faer` built-in sparse-dense multiplication (sequential)
   - `nalgebra-sparse` CSR multiplication
   - `sprs` CSR multiplication

2. **Parallel Thread Scaling**:
   - Custom parallel implementation with 1, 2, 4, 8, threads
   - Tests using `SparseDenseStrategy` and `sparse_dense_matmul`

## Quick Start

### Using Matrix Market Files

The benchmark suite is configured to use matrices from the `test_matrices/` directory. The following files are currently configured for benchmarking:
- `test_matrices/0.mtx`
- `test_matrices/1.mtx`
- `test_matrices/2.mtx` 
- `test_matrices/anisotropy_2d.mtx`
- `test_matrices/anisotropy_3d_1r.mtx`
- `test_matrices/spe10_0.mtx`

To benchmark additional matrices, edit `benches/sparse_matvec.rs` and add file paths to the `matrix_files` array.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench --bench sparse_matvec

# Run specific benchmark groups
cargo bench --bench sparse_matvec -- sequential
cargo bench --bench sparse_matvec -- thread_scaling
```

## Benchmark Structure

### Sequential Comparisons
- **`faer`**: Uses `faer::sparse::linalg::matmul::sparse_dense_matmul` with `Par::Seq`
- **`nalgebra_sparse`**: Uses nalgebra-sparse CSR matrix multiplication
- **`sprs`**: Uses sprs CSR matrix-vector multiplication

### Thread Scaling Tests
- Tests a parallel implementation with 1, 2, 4, 8 threads
- Uses `SparseDenseStrategy` to distribute work across threads
- Measures performance scaling and parallel efficiency

## Output and Results

### Console Output
The benchmarks print:
- Matrix dimensions and non-zero count for each test matrix
- Performance measurements in iterations per second
- Thread scaling results

### HTML Reports
Detailed results are saved to `target/criterion/`:
- Open `target/criterion/report/index.html` for interactive reports
- Includes plots showing performance comparisons and thread scaling

## Matrix Requirements

Your Matrix Market files should:
- Be in standard Matrix Market coordinate format (`.mtx`)
- Contain real-valued sparse matrices
- Have sufficient non-zeros for meaningful parallel testing (> 1000 recommended)
