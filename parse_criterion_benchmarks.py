#!/usr/bin/env python3
"""
Parse Criterion JSON benchmark results and generate markdown tables and plots.
This script reads from target/criterion/<group>/<function>/<parameter>/new/*.json
"""

import json
import sys
import subprocess
import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path
from dataclasses import dataclass
from typing import Dict, List, Optional, Tuple
import re

@dataclass
class BenchmarkResult:
    group_id: str
    function_id: str
    parameter: str
    median_ns: float
    median_lower_ns: float
    median_upper_ns: float
    matrix_name: str = ""
    dimensions: str = ""
    nnz: int = 0
    threads: Optional[int] = None
    algorithm: Optional[str] = None  # For parallel algorithms: simple, merge, buffer_foreign
    
    @property
    def std_dev_ns(self) -> float:
        # Approximate standard deviation from median confidence interval
        return (self.median_upper_ns - self.median_lower_ns) / 4.0

def parse_matrix_info(parameter: str) -> Tuple[str, str, int]:
    """Parse matrix info from parameter string like '18x18_nnz68' or '1_threads'."""
    if "_threads" in parameter:
        return "", "", 0
    
    # Extract dimensions and nnz from parameter
    if "_nnz" in parameter:
        parts = parameter.split("_nnz")
        dimensions = parts[0]
        nnz = int(parts[1])
        return "", dimensions, nnz
    
    return parameter, "", 0

def parse_group_id(group_id: str) -> Tuple[str, str, Optional[str]]:
    """Parse group_id to extract benchmark type, matrix name, and algorithm."""
    # Examples: 
    # "sequential_sparse_dense_0-18x18_nnz68" -> ("sequential_sparse_dense", "0", None)
    # "sequential_dense_sparse_0-18x18_nnz68" -> ("sequential_dense_sparse", "0", None)
    # "thread_scaling_anisotropy_3d_1r-84315x84315_nnz1394367" -> ("parallel", "anisotropy_3d_1r", "simple")
    
    if group_id.startswith("sequential_sparse_dense_"):
        parts = group_id.split("-")[0].split("_")
        matrix_name = parts[3] if len(parts) > 3 else "unknown"
        return "sequential_sparse_dense", matrix_name, None
    elif group_id.startswith("sequential_dense_sparse_"):
        parts = group_id.split("-")[0].split("_")
        matrix_name = parts[3] if len(parts) > 3 else "unknown"
        return "sequential_dense_sparse", matrix_name, None
    elif group_id.startswith("thread_scaling_"):
        # Extract matrix name from before the first dash
        matrix_part = group_id.replace("thread_scaling_", "").split("-")[0]
        
        # For synthetic matrices, include dimensions to make them unique
        if matrix_part == "synthetic" and "-" in group_id:
            remaining = group_id.split("-", 1)[1]
            if "x" in remaining and "_nnz" in remaining:
                dims_part = remaining.split("_nnz")[0].split("-")[-1]
                matrix_part = f"synthetic_{dims_part}"
        
        return "parallel", matrix_part, None  # Algorithm will be determined from function_id
    
    return "unknown", "unknown", None

def parse_algorithm_from_function_id(function_id: str) -> Optional[str]:
    """Extract algorithm name from parallel function ID."""
    if function_id.startswith("sparse_dense_"):
        return function_id.replace("sparse_dense_", "")
    elif function_id == "dense_sparse":
        return "dense_sparse"
    return None

def collect_benchmark_results(criterion_dir: Path) -> List[BenchmarkResult]:
    """Collect all benchmark results from Criterion JSON files."""
    results = []
    
    if not criterion_dir.exists():
        print(f"Error: {criterion_dir} not found. Run benchmarks first.", file=sys.stderr)
        return results
    
    # Walk through all benchmark directories
    for group_dir in criterion_dir.iterdir():
        if not group_dir.is_dir() or group_dir.name == "report":
            continue
            
        group_id = group_dir.name
        benchmark_type, matrix_name, _ = parse_group_id(group_id)
        
        for function_dir in group_dir.iterdir():
            if not function_dir.is_dir() or function_dir.name == "report":
                continue
                
            function_id = function_dir.name
            algorithm = parse_algorithm_from_function_id(function_id) if benchmark_type == "parallel" else None
            
            for param_dir in function_dir.iterdir():
                if not param_dir.is_dir() or param_dir.name == "report":
                    continue
                    
                parameter = param_dir.name
                new_dir = param_dir / "new"
                
                if not new_dir.exists():
                    continue
                    
                estimates_file = new_dir / "estimates.json"
                benchmark_file = new_dir / "benchmark.json"
                
                if not (estimates_file.exists() and benchmark_file.exists()):
                    continue
                
                try:
                    # Read estimates for timing data
                    with open(estimates_file) as f:
                        estimates = json.load(f)
                    
                    # Read benchmark metadata
                    with open(benchmark_file) as f:
                        benchmark_meta = json.load(f)
                    
                    median_data = estimates["median"]
                    median_ns = median_data["point_estimate"]
                    median_lower_ns = median_data["confidence_interval"]["lower_bound"]
                    median_upper_ns = median_data["confidence_interval"]["upper_bound"]
                    
                    # Parse additional info
                    _, dimensions, nnz = parse_matrix_info(parameter)
                    
                    # Extract thread count for parallel benchmarks
                    threads = None
                    if benchmark_type == "parallel" and "_threads" in parameter:
                        threads = int(parameter.split("_threads")[0])
                    
                    # Extract matrix dimensions and nnz from group_id for both types
                    if "-" in group_id and not dimensions:
                        # For complex synthetic names like "thread_scaling_synthetic-10000x10000_0.00-10000x10000_nnz1000000"
                        if "synthetic" in group_id and group_id.count("-") >= 2:
                            # Extract from the final part after the last dash
                            parts = group_id.split("-")
                            matrix_info = parts[-1]  # Should be like "10000x10000_nnz1000000"
                        else:
                            matrix_info = group_id.split("-")[1]
                        
                        _, extracted_dims, extracted_nnz = parse_matrix_info(matrix_info)
                        if extracted_dims:
                            dimensions = extracted_dims
                            nnz = extracted_nnz
                    
                    result = BenchmarkResult(
                        group_id=group_id,
                        function_id=function_id,
                        parameter=parameter,
                        median_ns=median_ns,
                        median_lower_ns=median_lower_ns,
                        median_upper_ns=median_upper_ns,
                        matrix_name=matrix_name,
                        dimensions=dimensions,
                        nnz=nnz,
                        threads=threads,
                        algorithm=algorithm
                    )
                    
                    results.append(result)
                    
                except (json.JSONDecodeError, KeyError, ValueError) as e:
                    print(f"Warning: Could not parse {estimates_file}: {e}", file=sys.stderr)
                    continue
    
    return results

def format_time_with_unit(time_ns: float) -> Tuple[float, str]:
    """Format time with appropriate unit (ns, µs, ms)."""
    if time_ns < 1000:
        return time_ns, 'ns'
    elif time_ns < 1_000_000:
        return time_ns / 1000, 'µs'
    else:
        return time_ns / 1_000_000, 'ms'

def generate_sequential_table(results: List[BenchmarkResult], operation: str) -> str:
    """Generate sequential performance table for sparse_dense or dense_sparse."""
    # Filter sequential results for the specific operation
    sequential_results = [r for r in results if r.group_id.startswith(f"sequential_{operation}_")]
    
    # Group by matrix
    matrix_groups = {}
    for result in sequential_results:
        if result.matrix_name not in matrix_groups:
            matrix_groups[result.matrix_name] = {}
        matrix_groups[result.matrix_name][result.function_id] = result
    
    # Sort matrices by nnz
    sorted_matrices = sorted(matrix_groups.items(), key=lambda x: next(iter(x[1].values())).nnz)
    
    lines = []
    operation_title = operation.replace("_", "-").title()
    lines.append(f"# Sequential {operation_title} Matrix-Vector Multiplication Benchmark Results\n")
    lines.append("| Matrix | Dimensions | Non-zeros | faer | nalgebra | sprs |")
    lines.append("|--------|------------|-----------|------|----------|------|")
    
    for matrix_name, implementations in sorted_matrices:
        # Get matrix info from any implementation
        first_impl = next(iter(implementations.values()))
        row = [f"**{matrix_name}**", first_impl.dimensions, f"{first_impl.nnz:,}"]
        
        for impl in ['faer', 'nalgebra', 'sprs']:
            if impl in implementations:
                result = implementations[impl]
                time_val, t_unit = format_time_with_unit(result.median_ns)
                std_val, std_unit = format_time_with_unit(result.std_dev_ns)
                
                cell = f"{time_val:.2f} {t_unit} ± {std_val:.2f} {std_unit}"
                
                row.append(cell)
            else:
                row.append("—")
        
        lines.append("| " + " | ".join(row) + " |")
    
    return "\n".join(lines)

def generate_parallel_table(results: List[BenchmarkResult], algorithm: str, operation: str) -> str:
    """Generate parallel performance table for a specific algorithm and operation."""
    # Filter parallel results for the specific algorithm
    if operation == "dense_sparse":
        parallel_results = [r for r in results if r.group_id.startswith("thread_scaling_") and 
                          r.threads is not None and r.algorithm == algorithm]
        title = f"Parallel Thread Scaling Results - Dense-Sparse Multiplication ({algorithm.title()})"
    else:
        parallel_results = [r for r in results if r.group_id.startswith("thread_scaling_") and 
                          r.threads is not None and r.algorithm == algorithm]
        title = f"Parallel Thread Scaling Results - Sparse-Dense Multiplication ({algorithm.title()})"
    
    # Group by matrix name, then by thread count
    matrix_groups = {}
    thread_counts = set()
    
    for result in parallel_results:
        if result.matrix_name not in matrix_groups:
            matrix_groups[result.matrix_name] = {}
        matrix_groups[result.matrix_name][result.threads] = result
        thread_counts.add(result.threads)
    
    # Sort matrices by nnz and thread counts
    sorted_matrices = sorted(matrix_groups.items(), key=lambda x: next(iter(x[1].values())).nnz)
    sorted_threads = sorted(thread_counts)
    
    if not sorted_matrices:
        return f"\n# {title}\n\nNo results found for {algorithm} algorithm.\n"
    
    lines = []
    lines.append(f"\n# {title}\n")
    
    # Header
    header = ["| Matrix | Dimensions | Non-zeros |"] + [f" {t} Thread{'s' if t > 1 else ''} |" for t in sorted_threads]
    lines.append("".join(header))
    
    # Separator
    separator = ["|--------|------------|-----------|"] + ["-----------:|" for _ in sorted_threads]
    lines.append("".join(separator))
    
    for matrix_name, thread_results in sorted_matrices:
        if not thread_results:
            continue
            
        # Get matrix info from any thread result
        first_result = next(iter(thread_results.values()))
        row = [f"**{matrix_name}**", first_result.dimensions, f"{first_result.nnz:,}"]
        
        for thread_count in sorted_threads:
            if thread_count in thread_results:
                result = thread_results[thread_count]
                time_val, unit = format_time_with_unit(result.median_ns)
                
                if unit == 'ns':
                    cell = f"{time_val:.1f} ns"
                elif unit == 'µs':
                    cell = f"{time_val:.2f} µs"
                else:  # ms
                    cell = f"{time_val:.3f} ms"
                
                row.append(cell)
            else:
                row.append("—")
        
        lines.append("| " + " | ".join(row) + " |")
    
    return "\n".join(lines)

def generate_plots(results: List[BenchmarkResult], figures_dir: Path) -> List[str]:
    """Generate matplotlib plots for thread scaling."""
    # Create figures directory
    figures_dir.mkdir(exist_ok=True)
    
    generated_plots = []
    
    # Get sequential results for baseline lines
    seq_sparse_dense = [r for r in results if r.group_id.startswith("sequential_sparse_dense_")]
    seq_dense_sparse = [r for r in results if r.group_id.startswith("sequential_dense_sparse_")]
    
    # Get parallel results
    parallel_results = [r for r in results if r.group_id.startswith("thread_scaling_") and r.threads is not None]
    
    # Group by matrix for plotting
    matrices = {}
    for result in parallel_results:
        if result.matrix_name not in matrices:
            matrices[result.matrix_name] = {
                'sparse_dense': {},
                'dense_sparse': {}
            }
        
        operation = 'dense_sparse' if result.algorithm == 'dense_sparse' else 'sparse_dense'
        algorithm = result.algorithm if result.algorithm != 'dense_sparse' else 'dense_sparse'
        
        if algorithm not in matrices[result.matrix_name][operation]:
            matrices[result.matrix_name][operation][algorithm] = {}
        
        matrices[result.matrix_name][operation][algorithm][result.threads] = result
    
    # Create sequential lookup for baseline lines
    seq_lookup = {}
    for result in seq_sparse_dense + seq_dense_sparse:
        operation = 'sparse_dense' if 'sparse_dense' in result.group_id else 'dense_sparse'
        if result.matrix_name not in seq_lookup:
            seq_lookup[result.matrix_name] = {}
        if operation not in seq_lookup[result.matrix_name]:
            seq_lookup[result.matrix_name][operation] = {}
        seq_lookup[result.matrix_name][operation][result.function_id] = result
    
    # Debug: Print available sequential data
    print("Available sequential data:")
    for matrix, ops in seq_lookup.items():
        for op, libs in ops.items():
            print(f"  {matrix} -> {op}: {list(libs.keys())}")
    
    colors = {'simple': 'blue', 'merge': 'red', 'buffer_foreign': 'green', 'dense_sparse': 'purple'}
    seq_colors = {'faer': 'orange', 'nalgebra': 'gray', 'sprs': 'brown'}
    
    for matrix_name, operations in matrices.items():
        for operation, algorithms in operations.items():
            if not algorithms:
                continue
                
            plt.figure(figsize=(10, 6))
            
            # Debug: Print what we're looking for
            print(f"Looking for {matrix_name} in {operation}")
            print(f"Available matrices: {list(seq_lookup.keys())}")
            
            # Plot sequential baselines as horizontal dotted lines
            # Try both exact match and fuzzy matching
            seq_data = None
            if matrix_name in seq_lookup and operation in seq_lookup[matrix_name]:
                seq_data = seq_lookup[matrix_name][operation]
            else:
                # Try fuzzy matching - look for matrices that contain the parallel matrix name
                # or vice versa (handles cases where naming might be slightly different)
                for seq_matrix in seq_lookup.keys():
                    if (matrix_name in seq_matrix or seq_matrix in matrix_name or 
                        seq_matrix.replace('_', '').replace('-', '') == matrix_name.replace('_', '').replace('-', '')):
                        if operation in seq_lookup[seq_matrix]:
                            seq_data = seq_lookup[seq_matrix][operation]
                            print(f"  Found fuzzy match: {seq_matrix} -> {matrix_name}")
                            break
            
            if seq_data:
                for lib_name, seq_result in seq_data.items():
                    time_ms = seq_result.median_ns / 1_000_000
                    color = seq_colors.get(lib_name, 'black')
                    plt.axhline(y=time_ms, linestyle='--', alpha=0.7, color=color,
                              label=f'{lib_name} sequential', linewidth=1)
            
            # Plot parallel scaling for each algorithm
            for algorithm, thread_data in algorithms.items():
                if not thread_data:
                    continue
                
                threads = sorted(thread_data.keys())
                times_ms = []
                lower_bounds = []
                upper_bounds = []
                
                for t in threads:
                    result = thread_data[t]
                    time_ms = result.median_ns / 1_000_000
                    lower_ms = result.median_lower_ns / 1_000_000
                    upper_ms = result.median_upper_ns / 1_000_000
                    
                    times_ms.append(time_ms)
                    lower_bounds.append(lower_ms)
                    upper_bounds.append(upper_ms)
                
                color = colors.get(algorithm, 'black')
                label = algorithm.replace('_', ' ').title()
                
                # Plot main line
                plt.plot(threads, times_ms, 'o-', color=color, label=label, linewidth=2, markersize=6)
                
                # Plot confidence interval as alpha band
                plt.fill_between(threads, lower_bounds, upper_bounds, 
                               color=color, alpha=0.2)
            
            plt.xlabel('Number of Threads')
            plt.ylabel('Time (ms)')
            plt.title(f'{matrix_name} - {operation.replace("_", "-").title()} Thread Scaling')
            plt.legend()
            plt.grid(True, alpha=0.3)
            plt.yscale('log')
            
            # Save plot
            filename = f'{matrix_name}_{operation}_thread_scaling.png'
            filepath = figures_dir / filename
            plt.savefig(filepath, dpi=150, bbox_inches='tight')
            plt.close()
            
            generated_plots.append(filename)
            print(f"Generated plot: {filepath}")
    
    return generated_plots

def get_cpu_info() -> str:
    """Get CPU information using lscpu."""
    try:
        result = subprocess.run(['lscpu'], capture_output=True, text=True, check=True)
        return result.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError):
        # Fallback if lscpu is not available
        try:
            # Try reading /proc/cpuinfo as fallback
            with open('/proc/cpuinfo', 'r') as f:
                cpuinfo = f.read()
            return cpuinfo.strip()
        except:
            return "CPU information not available"

def generate_plots_section(plot_files: List[str], results: List[BenchmarkResult]) -> str:
    """Generate plots section for markdown."""
    lines = []
    lines.append("\n## Thread Scaling Plots\n")
    
    # Create matrix info lookup from results
    matrix_info = {}
    for result in results:
        if result.matrix_name and result.dimensions and result.nnz:
            matrix_info[result.matrix_name] = {
                'dimensions': result.dimensions,
                'nnz': result.nnz
            }
    
    # Group plots by matrix name
    plot_groups = {}
    for plot_file in plot_files:
        # Extract matrix name and operation from filename: matrix_name_operation_thread_scaling.png
        # Expected format: matrix_name_sparse_dense_thread_scaling.png or matrix_name_dense_sparse_thread_scaling.png
        if plot_file.endswith('_thread_scaling.png'):
            base_name = plot_file.replace('_thread_scaling.png', '')
            
            if base_name.endswith('_sparse_dense'):
                operation = 'sparse_dense'
                matrix_name = base_name.replace('_sparse_dense', '')
            elif base_name.endswith('_dense_sparse'):
                operation = 'dense_sparse'
                matrix_name = base_name.replace('_dense_sparse', '')
            else:
                continue  # Skip files that don't match expected pattern
            
            if matrix_name not in plot_groups:
                plot_groups[matrix_name] = {}
            plot_groups[matrix_name][operation] = plot_file
    
    # Sort matrices alphabetically
    for matrix_name in sorted(plot_groups.keys()):
        plots = plot_groups[matrix_name]
        
        # Create header with matrix info
        header = f"### {matrix_name}"
        if matrix_name in matrix_info:
            info = matrix_info[matrix_name]
            dimensions = info['dimensions']
            nnz = info['nnz']
            
            # Calculate density
            if 'x' in dimensions:
                try:
                    rows, cols = dimensions.split('x')
                    total_elements = int(rows) * int(cols)
                    density = (nnz / total_elements) * 100
                    header += f" ({dimensions}, {nnz:,} nnz, {density:.3f}% dense)"
                except (ValueError, ZeroDivisionError):
                    header += f" ({dimensions}, {nnz:,} nnz)"
            else:
                header += f" ({dimensions}, {nnz:,} nnz)"
        
        lines.append(f"{header}\n")
        
        # Try to put plots side by side using HTML table approach
        lines.append("<table><tr>")
        
        if 'sparse_dense' in plots:
            sparse_plot = plots['sparse_dense']
            lines.append(f'<td><img src="figures/{sparse_plot}" alt="{matrix_name} Sparse-Dense Thread Scaling" width="500"></td>')
        
        if 'dense_sparse' in plots:
            dense_plot = plots['dense_sparse']
            lines.append(f'<td><img src="figures/{dense_plot}" alt="{matrix_name} Dense-Sparse Thread Scaling" width="500"></td>')
        
        lines.append("</tr></table>\n")
    
    return "\n".join(lines)

def generate_notes_section() -> str:
    """Generate notes section."""
    lines = []
    lines.append("## Notes\n")
    lines.append("- Times shown are median ± approximate standard deviation from Criterion benchmarks")
    lines.append("- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication")
    lines.append("- `nalgebra` = nalgebra-sparse CSC matrix-vector multiplication")
    lines.append("- `sprs` = sprs CSC matrix-vector multiplication")
    lines.append("- `simple`, `merge`, `buffer_foreign` = different parallel sparse-dense algorithms")
    lines.append("- `dense_sparse` = parallel dense-sparse matrix-vector multiplication implementation")
    lines.append("- Thread scaling shows parallel implementation performance across different thread counts")
    lines.append("- All measurements taken on the same system with consistent methodology")
    lines.append("- Plots show thread scaling with 95% confidence intervals and sequential baselines")
    
    # Add CPU information
    lines.append("\n## System Information\n")
    cpu_info = get_cpu_info()
    lines.append("```")
    lines.append(cpu_info)
    lines.append("```")
    
    return "\n".join(lines)

def main():
    criterion_dir = Path("target/criterion")
    figures_dir = Path("figures")
    
    print("Collecting benchmark results from Criterion JSON files...")
    results = collect_benchmark_results(criterion_dir)
    
    if not results:
        print("Error: No benchmark results found", file=sys.stderr)
        sys.exit(1)
    
    print(f"Found {len(results)} benchmark results")
    
    # Generate sections
    sections = []
    
    # Sequential tables
    sections.append(generate_sequential_table(results, "sparse_dense"))
    sections.append(generate_sequential_table(results, "dense_sparse"))
    
    # Parallel tables for all algorithms
    algorithms = ['simple', 'merge', 'buffer_foreign', 'dense_sparse']
    for algorithm in algorithms:
        if algorithm == 'dense_sparse':
            sections.append(generate_parallel_table(results, algorithm, "dense_sparse"))
        else:
            sections.append(generate_parallel_table(results, algorithm, "sparse_dense"))
    
    # Generate plots first
    print("Generating thread scaling plots...")
    generated_plots = generate_plots(results, figures_dir)
    print("All plots generated!")
    
    # Add plots section
    sections.append(generate_plots_section(generated_plots, results))
    sections.append(generate_notes_section())
    
    # Combine all sections
    full_content = "\n".join(sections)
    
    # Write to file
    output_file = Path("BENCHMARK_RESULTS.md")
    with open(output_file, 'w') as f:
        f.write(full_content)
    
    print(f"Generated benchmark results: {output_file}")

if __name__ == "__main__":
    main()
