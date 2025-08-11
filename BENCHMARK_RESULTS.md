# Sequential Sparse Matrix-Vector Multiplication Benchmark Results

| Matrix | Dimensions | Non-zeros | faer | nalgebra | sprs |
|--------|------------|-----------|------|----------|------|
| **0** | 18x18 | 68 | 93.25 ns ± 0.03 ns | 87.65 ns ± 0.01 ns | 50.59 ns ± 0.02 ns |
| **1** | 51x51 | 215 | 171.45 ns ± 0.11 ns | 225.32 ns ± 0.04 ns | 152.94 ns ± 0.03 ns |
| **2** | 165x165 | 749 | 585.53 ns ± 0.08 ns | 748.18 ns ± 0.07 ns | 533.54 ns ± 0.04 ns |
| **3** | 585x585 | 2,777 | 2.06 µs ± 0.09 ns | 2.70 µs ± 0.09 ns | 2.01 µs ± 0.05 ns |
| **synthetic** | 1000x1000 | 10,000 | 6.15 µs ± 0.44 ns | 7.12 µs ± 0.29 ns | 13.36 µs ± 0.93 ns |
| **4** | 2193x2193 | 10,673 | 8.24 µs ± 0.94 ns | 12.75 µs ± 0.68 ns | 7.94 µs ± 0.26 ns |
| **5** | 8481x8481 | 41,825 | 33.28 µs ± 2.64 ns | 39.79 µs ± 1.08 ns | 31.25 µs ± 0.94 ns |

# Parallel Thread Scaling Results - Sparse-Dense Multiplication

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads | 16 Threads | 32 Threads | 64 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 72.6 ns | 36.40 µs | 72.75 µs | 104.71 µs | 127.97 µs | 125.64 µs | 96.96 µs |
| **synthetic_100x100** | 100x100 | 100 | 159.7 ns | 43.09 µs | 74.29 µs | 113.94 µs | 143.37 µs | 157.81 µs | 126.58 µs |
| **1** | 51x51 | 215 | 198.9 ns | 51.49 µs | 86.72 µs | 110.31 µs | 137.71 µs | 112.71 µs | 139.60 µs |
| **2** | 165x165 | 749 | 554.4 ns | 59.35 µs | 88.99 µs | 121.03 µs | 157.85 µs | 176.43 µs | 185.46 µs |
| **3** | 585x585 | 2,777 | 2.08 µs | 65.79 µs | 100.41 µs | 143.94 µs | 205.78 µs | 261.08 µs | 360.56 µs |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 6.30 µs | 59.32 µs | 100.96 µs | 159.32 µs | 226.64 µs | 309.11 µs | 476.10 µs |
| **4** | 2193x2193 | 10,673 | 8.20 µs | 79.69 µs | 134.40 µs | 208.85 µs | 301.50 µs | 484.61 µs | 746.34 µs |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 26.01 µs | 81.37 µs | 124.43 µs | 200.59 µs | 288.22 µs | 468.67 µs | 719.40 µs |
| **5** | 8481x8481 | 41,825 | 33.78 µs | 147.24 µs | 216.18 µs | 401.59 µs | 700.75 µs | 1.210 ms | 2.125 ms |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 806.56 µs | 820.83 µs | 612.29 µs | 634.19 µs | 903.32 µs | 1.449 ms | 2.485 ms |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 962.23 µs | 1.447 ms | 1.648 ms | 2.647 ms | 4.392 ms | 8.661 ms | 16.568 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 3.755 ms | 2.759 ms | 1.930 ms | 1.421 ms | 1.808 ms | 2.699 ms | 4.713 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 8.587 ms | 9.879 ms | 12.689 ms | 17.714 ms | 31.439 ms | 55.028 ms | 91.139 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 10.197 ms | 13.426 ms | 17.830 ms | 32.625 ms | 53.022 ms | 82.438 ms | 116.182 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 21.851 ms | 17.042 ms | 19.070 ms | 29.870 ms | 51.691 ms | 84.108 ms | 125.696 ms |

# Parallel Thread Scaling Results - Dense-Sparse Multiplication

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads | 16 Threads | 32 Threads | 64 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 55.7 ns | 18.96 µs | 33.18 µs | 47.32 µs | 59.95 µs | 49.76 µs | 42.92 µs |
| **synthetic_100x100** | 100x100 | 100 | 198.0 ns | 21.64 µs | 37.11 µs | 51.60 µs | 65.58 µs | 54.25 µs | 45.01 µs |
| **1** | 51x51 | 215 | 156.9 ns | 22.11 µs | 36.76 µs | 51.36 µs | 65.29 µs | 49.89 µs | 43.51 µs |
| **2** | 165x165 | 749 | 469.0 ns | 24.27 µs | 39.49 µs | 54.74 µs | 65.95 µs | 55.05 µs | 44.14 µs |
| **3** | 585x585 | 2,777 | 2.00 µs | 26.82 µs | 44.52 µs | 63.12 µs | 72.24 µs | 56.63 µs | 44.92 µs |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 5.25 µs | 35.94 µs | 51.03 µs | 68.70 µs | 86.00 µs | 60.40 µs | 46.92 µs |
| **4** | 2193x2193 | 10,673 | 7.64 µs | 37.25 µs | 52.65 µs | 70.66 µs | 90.71 µs | 96.58 µs | 50.73 µs |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 21.64 µs | 78.28 µs | 72.70 µs | 86.58 µs | 107.98 µs | 103.61 µs | 52.62 µs |
| **5** | 8481x8481 | 41,825 | 31.74 µs | 75.40 µs | 78.59 µs | 98.80 µs | 123.76 µs | 146.77 µs | 99.53 µs |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 768.35 µs | 1.654 ms | 864.78 µs | 490.20 µs | 342.06 µs | 263.62 µs | 149.49 µs |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 819.88 µs | 1.999 ms | 1.138 ms | 687.42 µs | 501.05 µs | 486.87 µs | 455.84 µs |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 3.760 ms | 6.657 ms | 3.386 ms | 1.773 ms | 1.008 ms | 707.98 µs | 649.35 µs |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 8.215 ms | 17.143 ms | 10.308 ms | 7.019 ms | 5.178 ms | 4.476 ms | 4.183 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 9.985 ms | 21.175 ms | 14.622 ms | 11.585 ms | 9.829 ms | 9.724 ms | 9.517 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 21.460 ms | 49.467 ms | 27.691 ms | 16.777 ms | 12.256 ms | 11.277 ms | 10.476 ms |

## Performance Analysis

### Small Matrices
- **0**: sprs wins (50.59 ns), faer is 1.8x slower, nalgebra is 1.7x slower
- **1**: sprs wins (152.94 ns), faer is 1.1x slower, nalgebra is 1.5x slower
- **2**: sprs wins (533.54 ns), faer is 1.1x slower, nalgebra is 1.4x slower

**Small category winner**: sprs (3/3 matrices)

### Medium Matrices
- **3**: sprs wins (2.01 µs), faer is 1.0x slower, nalgebra is 1.3x slower
- **synthetic**: faer wins (6.15 µs), nalgebra is 1.2x slower, sprs is 2.2x slower
- **4**: sprs wins (7.94 µs), faer is 1.0x slower, nalgebra is 1.6x slower
- **5**: sprs wins (31.25 µs), faer is 1.1x slower, nalgebra is 1.3x slower

**Medium category winner**: sprs (3/4 matrices)

## Notes

- Times shown are median ± approximate standard deviation from Criterion benchmarks
- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication
- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication
- `sprs` = sprs CSR matrix-vector multiplication
- `sparse_dense` = parallel sparse-dense matrix-vector multiplication implementation
- `dense_sparse` = parallel dense-sparse matrix-vector multiplication implementation
- Thread scaling shows parallel implementation performance across different thread counts
- All measurements taken on the same system with consistent methodology

## CPU info
```
$ lscpu
Architecture:             x86_64
  CPU op-mode(s):         32-bit, 64-bit
  Address sizes:          52 bits physical, 57 bits virtual
  Byte Order:             Little Endian
CPU(s):                   64
  On-line CPU(s) list:    0-63
Vendor ID:                AuthenticAMD
  Model name:             AMD EPYC 9534 64-Core Processor
    CPU family:           25
    Model:                17
    Thread(s) per core:   1
    Core(s) per socket:   64
    Socket(s):            1
    Stepping:             1
    Frequency boost:      enabled
    CPU(s) scaling MHz:   80%
    CPU max MHz:          3718.0659
    CPU min MHz:          1500.0000
```
