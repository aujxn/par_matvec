# par_matvec

A Rust library for benchmarking and testing parallel sparse matrix-vector multiplication (SpMV) operations.

## Purpose

This crate focuses on evaluating the performance of different sparse matrix by dense vector products (commonly referred to as "matvecs"). These operations are fundamental building blocks in numerical algorithms for solving partial differential equations (PDEs), where they often dominate the computational cost. Having efficient sequential and parallel Rust implementations is crucial for advancing scientific computing in the Rust ecosystem.

## Quick Start

```bash
# Run benchmarks
cargo bench --bench sparse_matvec

# Run correctness tests
cargo test

# View results
cat BENCHMARK_RESULTS.md
```

See `BENCHMARKS.md` and `CORRECTNESS_TESTS.md` for detailed usage instructions.