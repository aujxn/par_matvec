# Correctness Test Suite for par_matvec

This test suite verifies that all sparse matrix-vector multiplication implementations produce equivalent results, handling the challenge of different output types across libraries.

## Overview

The correctness tests ensure that:

1. **Sequential implementations match**: faer built-in, custom sequential, nalgebra-sparse, and sprs
2. **Parallel implementations are correct**: Custom parallel implementation with 1, 2, 4, 8 threads matches the sequential reference
3. **Floating-point tolerance**: Uses appropriate relative and absolute tolerances for numerical comparisons

## Test Structure

### Core Components

- **`TestMatrices`**: Unified struct holding matrix representations in all tested formats (faer CSC, nalgebra CSR, sprs CSR)
- **`ToVecF64`**: Trait for converting different vector types to a common `Vec<f64>` for comparison
- **`vectors_are_equal()`**: Robust floating-point comparison with configurable tolerances

### Tolerances
- **Relative tolerance**: `1e-10` (for comparing values relative to their magnitude)
- **Absolute tolerance**: `1e-12` (for comparing very small values)

## Running Tests

```bash
# Run all standard tests
cargo test

# Run specific test groups
cargo test test_synthetic_matrices    # Synthetic matrices only
cargo test test_matrix_market_files   # Matrix market files only  
cargo test test_edge_cases           # Edge cases (diagonal, etc.)
```

## Test Categories

### 1. Synthetic Matrix Tests
- **Small dense**: 10x10 with 50% density
- **Medium sparse**: 50x50 with 10% density  
- **Large sparse**: 100x100 with 5% density
- Uses deterministic patterns for reproducible testing

### 2. Matrix Market File Tests
Tests using your real-world matrices from `test_matrices/`:
- `test_matrices/0.mtx` through `test_matrices/7.mtx`
- `test_matrices/anisotropy_2d.mtx`
- `test_matrices/anisotropy_3d_*r.mtx`
- `test_matrices/spe10_0.mtx`

### 3. Edge Case Tests
- **Diagonal matrices**: Identity-like patterns
- **Empty matrices**: No non-zero entries
- **Single-element matrices**: Minimal test cases

## What Gets Tested

### Sequential Implementation Verification
Each matrix is tested against all implementations:
1. **faer built-in sequential** (reference implementation)
3. **nalgebra-sparse CSR** multiplication
4. **sprs CSR** multiplication

All results must match the reference within tolerance.

### Parallel Implementation Verification
For each matrix, tests your custom parallel implementation with:
- 1 thread (should match sequential)
- 2 threads
- 4 threads  
- 8 threads

All parallel results must match the sequential reference.

## Implementation Details

### Type Conversion Challenges Solved
- **faer**: Uses `Mat<f64>` output
- **nalgebra**: Uses `DVector<f64>` output
- **sprs**: Uses `Vec<f64>` output
- **Solution**: `ToVecF64` trait converts all to common `Vec<f64>`

### Matrix Format Conversions
- Loads Matrix Market files via nalgebra-sparse (most robust parser)
- Converts to all required formats: faer CSC, nalgebra CSR, sprs CSR
- Uses faer `Triplet` format for matrix construction
