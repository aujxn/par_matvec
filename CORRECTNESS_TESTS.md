# Correctness Test Suite for par_matvec

This test suite verifies that all sparse matrix-vector multiplication implementations produce equivalent results.

## Overview

The correctness tests ensure that:

1. **Sequential implementations match**: faer built-in, custom sequential, nalgebra-sparse, and sprs
2. **Parallel implementations are correct**: Parallel implementations with varying threads matches the sequential faer reference

## Running Tests

```bash
# Run all standard tests
cargo test

# Run specific test groups
cargo test test_synthetic_matrices
cargo test test_matrix_market_files
cargo test test_edge_cases
```
