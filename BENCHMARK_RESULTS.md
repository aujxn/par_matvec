# Sequential Sparse Matrix-Vector Multiplication Benchmark Results

| Matrix | Dimensions | Non-zeros | faer | nalgebra | sprs |
|--------|------------|-----------|------|----------|------|
| **0** | 18x18 | 68 | 68.53 ns ± 0.07 ns | 89.23 ns ± 0.03 ns | 52.04 ns ± 0.02 ns |
| **1** | 51x51 | 215 | 169.91 ns ± 0.08 ns | 226.11 ns ± 0.03 ns | 155.71 ns ± 0.05 ns |
| **2** | 165x165 | 749 | 542.06 ns ± 0.17 ns | 745.08 ns ± 0.09 ns | 534.65 ns ± 0.04 ns |
| **3** | 585x585 | 2,777 | 2.15 µs ± 0.35 ns | 2.67 µs ± 0.18 ns | 2.01 µs ± 0.08 ns |
| **synthetic** | 1000x1000 | 10,000 | 6.17 µs ± 0.25 ns | 7.62 µs ± 0.74 ns | 13.43 µs ± 0.75 ns |
| **4** | 2193x2193 | 10,673 | 8.77 µs ± 1.64 ns | 10.20 µs ± 0.72 ns | 7.82 µs ± 0.55 ns |
| **5** | 8481x8481 | 41,825 | 34.23 µs ± 1.20 ns | 39.98 µs ± 1.40 ns | 30.71 µs ± 2.75 ns |

# Parallel Thread Scaling Results - Sparse-Dense Multiplication

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads | 16 Threads | 32 Threads | 64 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 72.8 ns | 51.73 µs | 100.08 µs | 193.69 µs | 376.51 µs | 1.049 ms | 2.860 ms |
| **synthetic_100x100** | 100x100 | 100 | 137.2 ns | 52.60 µs | 101.07 µs | 193.10 µs | 394.61 µs | 1.105 ms | 2.756 ms |
| **1** | 51x51 | 215 | 178.7 ns | 52.22 µs | 103.60 µs | 193.23 µs | 403.11 µs | 1.072 ms | 2.919 ms |
| **2** | 165x165 | 749 | 547.9 ns | 53.67 µs | 113.90 µs | 199.72 µs | 385.35 µs | 1.107 ms | 2.827 ms |
| **3** | 585x585 | 2,777 | 2.47 µs | 57.25 µs | 104.16 µs | 198.26 µs | 431.06 µs | 1.109 ms | 2.959 ms |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 7.58 µs | 56.33 µs | 104.86 µs | 196.97 µs | 377.50 µs | 1.023 ms | 2.889 ms |
| **4** | 2193x2193 | 10,673 | 8.44 µs | 58.64 µs | 140.40 µs | 207.75 µs | 425.45 µs | 1.331 ms | 3.195 ms |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 25.35 µs | 69.77 µs | 135.07 µs | 196.48 µs | 376.97 µs | 1.113 ms | 3.020 ms |
| **5** | 8481x8481 | 41,825 | 33.20 µs | 86.35 µs | 191.21 µs | 297.40 µs | 648.64 µs | 1.860 ms | 4.025 ms |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 803.46 µs | 756.33 µs | 553.04 µs | 395.05 µs | 514.74 µs | 1.351 ms | 3.389 ms |
| **boneS01_M** | 127224x127224 | 1,182,804 | 877.38 µs | 2.889 ms | 3.707 ms | 6.195 ms | 17.279 ms | 29.340 ms | 53.498 ms |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 966.66 µs | 5.254 ms | 8.590 ms | 12.329 ms | 19.154 ms | 30.402 ms | 53.768 ms |
| **Ga3As3H12** | 61349x61349 | 3,016,148 | 2.009 ms | 6.054 ms | 10.000 ms | 18.974 ms | 25.993 ms | 40.562 ms | 61.238 ms |
| **boneS01** | 127224x127224 | 3,421,188 | 2.362 ms | 5.323 ms | 5.261 ms | 7.479 ms | 24.794 ms | 41.328 ms | 63.716 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 3.875 ms | 2.627 ms | 1.545 ms | 1.020 ms | 845.89 µs | 1.563 ms | 3.658 ms |
| **m_t1** | 97578x97578 | 4,925,574 | 3.205 ms | 7.595 ms | 14.734 ms | 17.497 ms | 25.543 ms | 42.068 ms | 68.504 ms |
| **SiO2** | 155331x155331 | 5,719,417 | 4.232 ms | 16.224 ms | 18.259 ms | 21.694 ms | 28.490 ms | 46.039 ms | 72.744 ms |
| **pwtk** | 217918x217918 | 5,926,171 | 4.011 ms | 11.214 ms | 13.291 ms | 16.809 ms | 29.873 ms | 46.447 ms | 73.324 ms |
| **rajat30** | 643994x643994 | 6,175,377 | 5.745 ms | 21.896 ms | 16.801 ms | 25.657 ms | 36.198 ms | 50.760 ms | 79.564 ms |
| **crankseg_2** | 63838x63838 | 7,106,348 | 4.641 ms | 20.657 ms | 21.066 ms | 22.934 ms | 31.456 ms | 44.982 ms | 75.051 ms |
| **kkt_power** | 2063494x2063494 | 8,130,343 | 9.204 ms | 49.361 ms | 36.234 ms | 30.296 ms | 36.724 ms | 52.578 ms | 82.952 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 8.580 ms | 35.471 ms | 26.321 ms | 23.472 ms | 33.681 ms | 56.555 ms | 89.540 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 10.417 ms | 43.055 ms | 31.458 ms | 24.961 ms | 35.340 ms | 54.895 ms | 88.077 ms |
| **Freescale2** | 2999349x2999349 | 23,042,677 | 24.680 ms | 85.635 ms | 57.094 ms | 35.486 ms | 45.636 ms | 63.300 ms | 101.234 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 22.159 ms | 41.019 ms | 23.713 ms | 19.433 ms | 43.368 ms | 72.700 ms | 100.005 ms |

# Parallel Thread Scaling Results - Dense-Sparse Multiplication

| Matrix | Dimensions | Non-zeros | 1 Thread | 2 Threads | 4 Threads | 8 Threads | 16 Threads | 32 Threads | 64 Threads |
|--------|------------|-----------|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|-----------:|
| **0** | 18x18 | 68 | 54.4 ns | 51.54 µs | 100.28 µs | 192.95 µs | 380.16 µs | 1.061 ms | 2.825 ms |
| **synthetic_100x100** | 100x100 | 100 | 143.9 ns | 52.40 µs | 120.56 µs | 215.51 µs | 380.61 µs | 1.019 ms | 2.847 ms |
| **1** | 51x51 | 215 | 144.8 ns | 52.54 µs | 100.71 µs | 191.74 µs | 381.68 µs | 1.039 ms | 2.818 ms |
| **2** | 165x165 | 749 | 502.9 ns | 52.90 µs | 102.59 µs | 197.95 µs | 385.22 µs | 1.031 ms | 2.774 ms |
| **3** | 585x585 | 2,777 | 1.98 µs | 54.83 µs | 101.87 µs | 193.74 µs | 381.55 µs | 1.018 ms | 2.839 ms |
| **synthetic_1000x1000** | 1000x1000 | 10,000 | 5.41 µs | 56.71 µs | 105.27 µs | 194.91 µs | 386.99 µs | 1.054 ms | 2.791 ms |
| **4** | 2193x2193 | 10,673 | 7.56 µs | 81.67 µs | 96.34 µs | 170.23 µs | 387.59 µs | 1.053 ms | 2.865 ms |
| **synthetic_2000x2000** | 2000x2000 | 40,000 | 21.60 µs | 88.49 µs | 138.59 µs | 178.20 µs | 391.95 µs | 1.028 ms | 2.866 ms |
| **5** | 8481x8481 | 41,825 | 30.74 µs | 90.90 µs | 140.16 µs | 190.28 µs | 402.16 µs | 1.010 ms | 2.891 ms |
| **synthetic_10000x10000** | 10000x10000 | 1,000,000 | 771.49 µs | 1.496 ms | 820.92 µs | 552.23 µs | 551.52 µs | 981.31 µs | 2.994 ms |
| **boneS01_M** | 127224x127224 | 1,182,804 | 771.48 µs | 1.162 ms | 752.57 µs | 584.49 µs | 650.14 µs | 1.197 ms | 2.570 ms |
| **anisotropy_3d_1r** | 84315x84315 | 1,394,367 | 836.67 µs | 1.490 ms | 998.73 µs | 629.78 µs | 641.94 µs | 1.184 ms | 2.525 ms |
| **Ga3As3H12** | 61349x61349 | 3,016,148 | 2.295 ms | 4.353 ms | 2.402 ms | 1.335 ms | 979.55 µs | 1.314 ms | 2.592 ms |
| **boneS01** | 127224x127224 | 3,421,188 | 2.406 ms | 3.636 ms | 1.962 ms | 1.194 ms | 1.030 ms | 1.446 ms | 2.736 ms |
| **synthetic_20000x20000** | 20000x20000 | 4,000,000 | 3.871 ms | 6.319 ms | 3.227 ms | 1.777 ms | 1.132 ms | 1.435 ms | 3.271 ms |
| **m_t1** | 97578x97578 | 4,925,574 | 3.232 ms | 6.017 ms | 3.245 ms | 1.854 ms | 1.273 ms | 1.729 ms | 2.778 ms |
| **SiO2** | 155331x155331 | 5,719,417 | 4.643 ms | 7.879 ms | 4.843 ms | 2.501 ms | 1.713 ms | 1.917 ms | 2.861 ms |
| **pwtk** | 217918x217918 | 5,926,171 | 4.118 ms | 6.313 ms | 3.337 ms | 1.918 ms | 1.619 ms | 2.034 ms | 3.059 ms |
| **rajat30** | 643994x643994 | 6,175,377 | 5.583 ms | 8.078 ms | 5.548 ms | 3.351 ms | 3.282 ms | 3.491 ms | 4.236 ms |
| **crankseg_2** | 63838x63838 | 7,106,348 | 5.246 ms | 10.966 ms | 5.734 ms | 2.964 ms | 1.683 ms | 1.621 ms | 3.067 ms |
| **kkt_power** | 2063494x2063494 | 8,130,343 | 11.370 ms | 15.466 ms | 11.652 ms | 9.658 ms | 9.525 ms | 10.842 ms | 9.456 ms |
| **anisotropy_3d_2r** | 650621x650621 | 10,978,101 | 8.364 ms | 12.359 ms | 6.894 ms | 4.385 ms | 4.351 ms | 4.198 ms | 5.122 ms |
| **anisotropy_2d** | 1313281x1313281 | 11,804,161 | 10.043 ms | 13.644 ms | 8.810 ms | 7.685 ms | 7.129 ms | 7.373 ms | 7.873 ms |
| **Freescale2** | 2999349x2999349 | 23,042,677 | 24.221 ms | 32.757 ms | 20.764 ms | 17.841 ms | 16.684 ms | 20.472 ms | 17.823 ms |
| **spe10_0** | 1159366x1159366 | 30,628,096 | 21.518 ms | 32.713 ms | 17.962 ms | 10.371 ms | 7.784 ms | 8.558 ms | 10.227 ms |

## Performance Analysis

### Small Matrices
- **0**: sprs wins (52.04 ns), faer is 1.3x slower, nalgebra is 1.7x slower
- **1**: sprs wins (155.71 ns), faer is 1.1x slower, nalgebra is 1.5x slower
- **2**: sprs wins (534.65 ns), faer is 1.0x slower, nalgebra is 1.4x slower

**Small category winner**: sprs (3/3 matrices)

### Medium Matrices
- **3**: sprs wins (2.01 µs), faer is 1.1x slower, nalgebra is 1.3x slower
- **synthetic**: faer wins (6.17 µs), nalgebra is 1.2x slower, sprs is 2.2x slower
- **4**: sprs wins (7.82 µs), faer is 1.1x slower, nalgebra is 1.3x slower
- **5**: sprs wins (30.71 µs), faer is 1.1x slower, nalgebra is 1.3x slower

**Medium category winner**: sprs (3/4 matrices)

## Notes

- Times shown are median ± approximate standard deviation from Criterion benchmarks
- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication
- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication
- `sprs` = sprs CSR matrix-vector multiplication
- `sparse_dense` = parallel sparse-dense matrix-vector multiplication implementation
- `dense_sparse` = parallel dense-sparse matrix-vector multiplication implementation
- Thread scaling shows parallel implementation performance across different thread counts
- All measurements taken on the same system with consistent methodologyArchitecture:                         x86_64
CPU op-mode(s):                       32-bit, 64-bit
Address sizes:                        45 bits physical, 48 bits virtual
Byte Order:                           Little Endian
CPU(s):                               4
On-line CPU(s) list:                  0-3
Vendor ID:                            GenuineIntel
Model name:                           Intel(R) Xeon(R) Gold 6342 CPU @ 2.80GHz
CPU family:                           6
Model:                                85
Thread(s) per core:                   1
Core(s) per socket:                   1
Socket(s):                            4
Stepping:                             7
BogoMIPS:                             5586.87
Flags:                                fpu vme de pse tsc msr pae mce cx8 apic sep mtrr pge mca cmov pat pse36 clflush mmx fxsr sse sse2 ss syscall nx pdpe1gb rdtscp lm constant_tsc arch_perfmon nopl xtopology tsc_reliable nonstop_tsc cpuid tsc_known_freq pni pclmulqdq ssse3 fma cx16 pcid sse4_1 sse4_2 x2apic movbe popcnt tsc_deadline_timer aes xsave avx f16c rdrand hypervisor lahf_lm abm 3dnowprefetch ssbd ibrs ibpb stibp ibrs_enhanced fsgsbase tsc_adjust bmi1 avx2 smep bmi2 invpcid avx512f avx512dq rdseed adx smap clflushopt clwb avx512cd avx512bw avx512vl xsaveopt xsavec xgetbv1 xsaves arat pku ospke avx512_vnni md_clear flush_l1d arch_capabilities
Hypervisor vendor:                    VMware
Virtualization type:                  full
L1d cache:                            192 KiB (4 instances)
L1i cache:                            128 KiB (4 instances)
L2 cache:                             5 MiB (4 instances)
L3 cache:                             144 MiB (4 instances)
NUMA node(s):                         1
NUMA node0 CPU(s):                    0-3
Vulnerability Gather data sampling:   Unknown: Dependent on hypervisor status
Vulnerability Itlb multihit:          KVM: Mitigation: VMX unsupported
Vulnerability L1tf:                   Not affected
Vulnerability Mds:                    Not affected
Vulnerability Meltdown:               Not affected
Vulnerability Mmio stale data:        Vulnerable: Clear CPU buffers attempted, no microcode; SMT Host state unknown
Vulnerability Reg file data sampling: Not affected
Vulnerability Retbleed:               Mitigation; Enhanced IBRS
Vulnerability Spec rstack overflow:   Not affected
Vulnerability Spec store bypass:      Mitigation; Speculative Store Bypass disabled via prctl
Vulnerability Spectre v1:             Mitigation; usercopy/swapgs barriers and __user pointer sanitization
Vulnerability Spectre v2:             Mitigation; Enhanced / Automatic IBRS; IBPB conditional; RSB filling; PBRSB-eIBRS SW sequence; BHI SW loop, KVM SW loop
Vulnerability Srbds:                  Not affected
Vulnerability Tsx async abort:        Not affected
