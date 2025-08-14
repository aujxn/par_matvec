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

# Parallel Thread Scaling Results - Sparse-Dense Multiplication

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads | 16 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 75.2 ns | 114.44 µs | 68.04 µs | 210.40 µs | 235.91 µs |
| **synthetic_100x100** | 100x100 | 100 | 167.5 ns | 141.53 µs | 68.29 µs | 147.53 µs | 236.04 µs |
| **1** | 51x51 | 215 | 190.7 ns | 149.45 µs | 68.05 µs | 136.84 µs | 235.82 µs |
| **2** | 165x165 | 749 | 587.0 ns | 150.06 µs | 68.22 µs | 148.54 µs | 238.02 µs |
| **3** | 585x585 | 2,777 | 2.16 µs | 146.33 µs | 75.94 µs | 273.88 µs | 241.04 µs |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 6.17 µs | 235.12 µs | 92.67 µs | 348.10 µs | 237.53 µs |
| **4** | 2193x2193 | 10,673 | 8.72 µs | 95.27 µs | 93.89 µs | 348.74 µs | 243.64 µs |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 24.02 µs | 364.99 µs | 174.81 µs | 182.60 µs | 279.60 µs |
| **5** | 8481x8481 | 41,825 | 34.47 µs | 725.19 µs | 198.51 µs | 205.20 µs | 345.39 µs |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 967.03 µs | 4.130 ms | 3.336 ms | 3.001 ms | 3.113 ms |
| **boneS01_M** | 127224x127224 | 1,182,804 | 1.057 ms | 5.139 ms | 3.702 ms | 4.509 ms | 8.195 ms |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 1.308 ms | 6.904 ms | 6.024 ms | 5.894 ms | 8.931 ms |
| **Ga3As3H12** | 61349x61349 | 3,016,148 | 2.368 ms | 9.123 ms | 6.837 ms | 9.433 ms | 12.339 ms |
| **boneS01** | 127224x127224 | 3,421,188 | 3.038 ms | 10.484 ms | 6.543 ms | 5.975 ms | 11.587 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 4.268 ms | 14.882 ms | 12.503 ms | 12.457 ms | 13.637 ms |
| **m_t1** | 97578x97578 | 4,925,574 | 4.297 ms | 13.799 ms | 11.095 ms | 10.563 ms | 15.288 ms |
| **SiO2** | 155331x155331 | 5,719,417 | 4.831 ms | 18.703 ms | 13.779 ms | 15.175 ms | 16.866 ms |
| **pwtk** | 217918x217918 | 5,926,171 | 5.071 ms | 18.847 ms | 13.014 ms | 11.936 ms | 13.204 ms |
| **rajat30** | 643994x643994 | 6,175,377 | 7.091 ms | 26.716 ms | 17.710 ms | 17.405 ms | 18.424 ms |
| **crankseg_2** | 63838x63838 | 7,106,348 | 5.582 ms | 24.031 ms | 17.846 ms | 17.544 ms | 21.048 ms |
| **kkt_power** | 2063494x2063494 | 8,130,343 | 12.381 ms | 62.782 ms | 43.513 ms | 28.351 ms | 26.690 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 10.356 ms | 46.433 ms | 29.707 ms | 21.343 ms | 24.403 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 12.549 ms | 52.283 ms | 33.967 ms | 23.488 ms | 31.829 ms |
| **JP** | 87616x67320 | 13,734,559 | 13.596 ms | — | — | — | — |
| **Freescale2** | 2999349x2999349 | 23,042,677 | 30.052 ms | 123.182 ms | 69.520 ms | 46.205 ms | 47.112 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 27.924 ms | 87.981 ms | 46.172 ms | 29.697 ms | 34.264 ms |

# Parallel Thread Scaling Results - Dense-Sparse Multiplication

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads | 16 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 54.5 ns | 26.59 µs | 49.00 µs | 93.22 µs | 220.60 µs |
| **synthetic_100x100** | 100x100 | 100 | 192.8 ns | 29.33 µs | 49.54 µs | 96.90 µs | 250.61 µs |
| **1** | 51x51 | 215 | 153.4 ns | 26.81 µs | 48.99 µs | 98.45 µs | 221.39 µs |
| **2** | 165x165 | 749 | 491.7 ns | 27.08 µs | 49.68 µs | 98.83 µs | 222.04 µs |
| **3** | 585x585 | 2,777 | 2.02 µs | 31.79 µs | 52.88 µs | 96.30 µs | 226.93 µs |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 6.14 µs | 34.56 µs | 48.93 µs | 99.16 µs | 233.31 µs |
| **4** | 2193x2193 | 10,673 | 7.42 µs | 37.60 µs | 51.00 µs | 101.23 µs | 243.30 µs |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 24.36 µs | 66.72 µs | 60.26 µs | 102.25 µs | 274.81 µs |
| **5** | 8481x8481 | 41,825 | 32.96 µs | 83.15 µs | 84.42 µs | 121.25 µs | 276.51 µs |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 851.90 µs | 1.215 ms | 658.72 µs | 398.17 µs | 415.22 µs |
| **boneS01_M** | 127224x127224 | 1,182,804 | 1.092 ms | 2.514 ms | 819.50 µs | 634.42 µs | 661.35 µs |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 1.142 ms | 1.439 ms | 812.76 µs | 578.74 µs | 947.52 µs |
| **Ga3As3H12** | 61349x61349 | 3,016,148 | 2.748 ms | 3.031 ms | 1.860 ms | 1.193 ms | 1.130 ms |
| **boneS01** | 127224x127224 | 3,421,188 | 2.997 ms | 3.052 ms | 1.940 ms | 1.615 ms | 1.607 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 3.762 ms | 4.266 ms | 2.622 ms | 1.705 ms | 1.399 ms |
| **m_t1** | 97578x97578 | 4,925,574 | 4.052 ms | 4.347 ms | 2.667 ms | 2.051 ms | 2.087 ms |
| **SiO2** | 155331x155331 | 5,719,417 | 5.364 ms | 5.556 ms | 3.698 ms | 2.792 ms | 2.740 ms |
| **pwtk** | 217918x217918 | 5,926,171 | 4.864 ms | 5.202 ms | 3.417 ms | 3.139 ms | 3.194 ms |
| **rajat30** | 643994x643994 | 6,175,377 | 7.287 ms | 8.556 ms | 7.239 ms | 7.093 ms | 7.993 ms |
| **crankseg_2** | 63838x63838 | 7,106,348 | 6.291 ms | 7.244 ms | 4.246 ms | 2.781 ms | 2.777 ms |
| **kkt_power** | 2063494x2063494 | 8,130,343 | 15.194 ms | 18.965 ms | 18.060 ms | 17.813 ms | 18.182 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 10.118 ms | 10.879 ms | 9.230 ms | 9.016 ms | 9.324 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 13.340 ms | 15.752 ms | 15.143 ms | 16.531 ms | 18.289 ms |
| **Freescale2** | 2999349x2999349 | 23,042,677 | 31.089 ms | 36.444 ms | 32.452 ms | 32.601 ms | 33.844 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 25.624 ms | 28.100 ms | 21.197 ms | 20.433 ms | 21.273 ms |

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

**Medium category winner**: faer (2/4 matrices)

## Notes

- Times shown are median ± approximate standard deviation from Criterion benchmarks
- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication
- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication
- `sprs` = sprs CSR matrix-vector multiplication
- `sparse_dense` = parallel sparse-dense matrix-vector multiplication implementation
- `dense_sparse` = parallel dense-sparse matrix-vector multiplication implementation
- Thread scaling shows parallel implementation performance across different thread counts
- All measurements taken on the same system with consistent methodology