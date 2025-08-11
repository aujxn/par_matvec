# par_matvec

A Rust library for benchmarking and testing parallel sparse matrix-vector multiplication (SpMV) operations.

## Purpose

This crate focuses on evaluating the performance of different sparse matrix by dense vector products (commonly referred to as "matvecs"). These operations are fundamental building blocks in numerical algorithms for solving partial differential equations (PDEs), where they often dominate the computational cost. Having efficient sequential and parallel Rust implementations is crucial for advancing scientific computing in the Rust ecosystem.

## Quick Start

```bash
# Run benchmarks
cargo bench

# Run tests
cargo test

# Generate results tables
python3 parse_criterion_benchmarks.py

# View results
cat BENCHMARK_RESULTS.md
```

See `BENCHMARKS.md` and `CORRECTNESS_TESTS.md` for more details.

## Analysis

See `BENCHMARKS_RESULTS.md` for table summary of the benchmarks. 

### Sequential

`sprs` and `faer` seem about the same with `nalgebra` generally taking 1.5 times longer for each matrix SpMV. 

### Parallel

Current implementation is bad. Matrices have to be very large to overcome the parallelism overhead.
For PDE discretization problems where the matrices are very structured scaling is awful; sometimes degrading timings with more threads. 
Scaling is slightly better on synthetic matrices with no structure and higher densities.
