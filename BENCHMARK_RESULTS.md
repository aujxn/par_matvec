# Sequential Sparse Matrix-Vector Multiplication Benchmark Results

| Matrix | Dimensions | Non-zeros | faer | nalgebra | sprs |
|--------|------------|-----------|------|----------|------|
| **0** | 18x18 | 68 | 60.4 ± 0.0 ns | 92.6 ± 0.2 ns | 53.4 ± 0.0 ns |
| **1** | 51x51 | 215 | 162.2 ± 0.2 ns | 239.4 ± 0.1 ns | 163.2 ± 0.1 ns |
| **2** | 165x165 | 749 | 534.1 ± 0.4 ns | 800.6 ± 1.0 ns | 555.5 ± 0.5 ns |
| **3** | 585x585 | 2,777 | 2.15 ± 1.16 µs | 2.83 ± 0.59 µs | 2.03 ± 1.36 µs |
| **synthetic** | 1000x1000 | 10,000 | 6.20 ± 6.06 µs | 8.23 ± 3.78 µs | 7.69 ± 3.55 µs |
| **4** | 2193x2193 | 10,673 | 8.16 ± 211.49 µs | 11.28 ± 18.23 µs | 8.03 ± 6.33 µs |
| **5** | 8481x8481 | 41,825 | 32.68 ± 141.01 µs | 43.51 ± 5.38 µs | 32.97 ± 18.55 µs |

# Parallel Thread Scaling Results

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 116.1 ns | 25.61 µs | 29.00 µs | 28.23 µs |
| **synthetic_100x100** | 100x100 | 100 | 193.1 ns | 28.89 µs | 30.42 µs | 29.82 µs |
| **1** | 51x51 | 215 | 217.9 ns | 28.30 µs | 30.37 µs | 29.01 µs |
| **2** | 165x165 | 749 | 627.2 ns | 30.07 µs | 32.03 µs | 29.43 µs |
| **3** | 585x585 | 2,777 | 2.50 µs | 37.38 µs | 36.62 µs | 30.70 µs |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 7.25 µs | 47.06 µs | 47.54 µs | 41.76 µs |
| **4** | 2193x2193 | 10,673 | 9.52 µs | 52.52 µs | 55.93 µs | 48.26 µs |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 28.99 µs | 63.56 µs | 66.27 µs | 62.59 µs |
| **5** | 8481x8481 | 41,825 | 36.78 µs | 87.08 µs | 107.85 µs | 109.76 µs |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 1.022 ms | 719.79 µs | 547.47 µs | 450.43 µs |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 1.269 ms | 1.027 ms | 735.22 µs | 771.58 µs |
| **anisotropy_3d_4r** | 193968x193968 | 2,793,232 | 3.006 ms | 2.470 ms | 1.858 ms | 2.324 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 4.765 ms | 2.751 ms | 2.233 ms | 1.630 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 10.507 ms | 8.378 ms | 7.818 ms | 9.330 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 12.708 ms | 12.104 ms | 11.065 ms | 14.368 ms |
| **anisotropy_3d_5r** | 1493600x1493600 | 21,944,096 | 24.236 ms | 19.557 ms | 17.183 ms | 20.822 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 26.818 ms | 20.270 ms | 16.152 ms | 18.454 ms |
| **anisotropy_3d_3r** | 5111001x5111001 | 87,122,313 | 96.292 ms | 79.360 ms | 72.902 ms | 82.753 ms |

## Performance Analysis

### Small Matrices
- **0**: sprs wins (53.37 ns), faer is 1.1x slower, nalgebra is 1.7x slower
- **1**: faer wins (162.16 ns), sprs is 1.0x slower, nalgebra is 1.5x slower
- **2**: faer wins (534.13 ns), sprs is 1.0x slower, nalgebra is 1.5x slower

**Small category winner**: faer (2/3 matrices)

### Medium Matrices
- **3**: sprs wins (2.03 µs), faer is 1.1x slower, nalgebra is 1.4x slower
- **synthetic**: faer wins (6.20 µs), sprs is 1.2x slower, nalgebra is 1.3x slower
- **4**: sprs wins (8.03 µs), faer is 1.0x slower, nalgebra is 1.4x slower
- **5**: faer wins (32.68 µs), sprs is 1.0x slower, nalgebra is 1.3x slower

**Medium category winner**: faer (2/4 matrices)

## Notes

- Times shown are median ± approximate standard deviation from Criterion benchmarks
- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication
- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication
- `sprs` = sprs CSR matrix-vector multiplication
- Thread scaling shows parallel implementation performance across different thread counts
- All measurements taken on the same system with consistent methodology