# Sequential Sparse Matrix-Vector Multiplication Benchmark Results

| Matrix | Dimensions | Non-zeros | faer | nalgebra | sprs |
|--------|------------|-----------|------|----------|------|
| **0** | 18x18 | 68 | 70.07 ns ± 0.09 ns | 97.44 ns ± 0.08 ns | 55.63 ns ± 0.06 ns |
| **1** | 51x51 | 215 | 170.94 ns ± 0.25 ns | 266.13 ns ± 0.20 ns | 167.50 ns ± 0.22 ns |
| **2** | 165x165 | 749 | 701.35 ns ± 1.77 ns | 871.81 ns ± 1.53 ns | 551.25 ns ± 0.42 ns |
| **3** | 585x585 | 2,777 | 2.07 µs ± 4.76 ns | 2.87 µs ± 3.82 ns | 2.07 µs ± 1.68 ns |
| **synthetic** | 1000x1000 | 10,000 | 6.16 µs ± 5.85 ns | 8.17 µs ± 5.87 ns | 7.71 µs ± 6.59 ns |
| **4** | 2193x2193 | 10,673 | 8.26 µs ± 12.80 ns | 11.47 µs ± 8.31 ns | 8.46 µs ± 7.10 ns |
| **5** | 8481x8481 | 41,825 | 31.99 µs ± 51.19 ns | 42.89 µs ± 56.39 ns | 31.19 µs ± 19.98 ns |

# Parallel Thread Scaling Results

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 57.8 ns | 12.01 µs | 13.07 µs | 13.72 µs |
| **synthetic_100x100** | 100x100 | 100 | 172.8 ns | 13.74 µs | 14.50 µs | 14.12 µs |
| **1** | 51x51 | 215 | 167.5 ns | 13.97 µs | 15.11 µs | 15.10 µs |
| **2** | 165x165 | 749 | 523.1 ns | 17.75 µs | 15.87 µs | 14.18 µs |
| **3** | 585x585 | 2,777 | 2.16 µs | 20.84 µs | 21.20 µs | 18.14 µs |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 6.19 µs | 25.12 µs | 30.63 µs | 23.74 µs |
| **4** | 2193x2193 | 10,673 | 8.00 µs | 29.75 µs | 39.55 µs | 31.99 µs |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 23.30 µs | 48.29 µs | 52.78 µs | 43.57 µs |
| **5** | 8481x8481 | 41,825 | 32.97 µs | 69.72 µs | 73.41 µs | 73.56 µs |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 766.30 µs | 1.185 ms | 705.51 µs | 499.72 µs |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 1.124 ms | 1.606 ms | 1.289 ms | 1.015 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 3.618 ms | 4.662 ms | 2.747 ms | 1.939 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 10.039 ms | 14.610 ms | 11.387 ms | 10.645 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 13.140 ms | 20.689 ms | 17.093 ms | 17.159 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 24.541 ms | 35.067 ms | 24.481 ms | 23.341 ms |

## Performance Analysis

### Small Matrices
- **0**: sprs wins (55.63 ns), faer is 1.3x slower, nalgebra is 1.8x slower
- **1**: sprs wins (167.50 ns), faer is 1.0x slower, nalgebra is 1.6x slower
- **2**: sprs wins (551.25 ns), faer is 1.3x slower, nalgebra is 1.6x slower

**Small category winner**: sprs (3/3 matrices)

### Medium Matrices
- **3**: sprs wins (2.07 µs), faer is 1.0x slower, nalgebra is 1.4x slower
- **synthetic**: faer wins (6.16 µs), sprs is 1.3x slower, nalgebra is 1.3x slower
- **4**: faer wins (8.26 µs), sprs is 1.0x slower, nalgebra is 1.4x slower
- **5**: sprs wins (31.19 µs), faer is 1.0x slower, nalgebra is 1.4x slower

**Medium category winner**: sprs (2/4 matrices)

## Notes

- Times shown are median ± approximate standard deviation from Criterion benchmarks
- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication
- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication
- `sprs` = sprs CSR matrix-vector multiplication
- Thread scaling shows parallel implementation performance across different thread counts
- All measurements taken on the same system with consistent methodology