#!/usr/bin/env python3
"""
Parse Criterion JSON benchmark results and generate markdown tables.
This script reads from target/criterion/<group>/<function>/<parameter>/new/*.json
"""

import json
import sys
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

def parse_group_id(group_id: str) -> Tuple[str, str]:
    """Parse group_id to extract matrix name and benchmark type."""
    # Examples: 
    # "sequential_matvec_0-18x18_nnz68" -> ("sequential", "0")
    # "thread_scaling_anisotropy_3d_1r-84315x84315_nnz1394367" -> ("parallel", "anisotropy_3d_1r")
    # "thread_scaling_synthetic-10000x10000_0.00-10000x10000_nnz1000000" -> ("parallel", "synthetic_10000x10000")
    
    if group_id.startswith("sequential_matvec_"):
        parts = group_id.split("-")[0].split("_")
        matrix_name = parts[2] if len(parts) > 2 else "unknown"
        return "sequential", matrix_name
    elif group_id.startswith("thread_scaling_"):
        # Extract matrix name from before the first dash
        matrix_part = group_id.replace("thread_scaling_", "").split("-")[0]
        
        # For synthetic matrices, include dimensions to make them unique
        if matrix_part == "synthetic" and "-" in group_id:
            # Extract dimensions from the part after the dash
            remaining = group_id.split("-", 1)[1]
            if "x" in remaining and "_nnz" in remaining:
                dims_part = remaining.split("_nnz")[0].split("-")[-1]
                matrix_part = f"synthetic_{dims_part}"
        
        return "parallel", matrix_part
    
    return "unknown", "unknown"

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
        benchmark_type, matrix_name = parse_group_id(group_id)
        
        # For sequential benchmarks: group/function/parameter/new/
        # For parallel benchmarks: group/function/parameter/new/
        for function_dir in group_dir.iterdir():
            if not function_dir.is_dir() or function_dir.name == "report":
                continue
                
            function_id = function_dir.name
            
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
                        threads=threads
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

def generate_sequential_table(results: List[BenchmarkResult]) -> str:
    """Generate sequential performance table."""
    # Filter sequential results
    sequential_results = [r for r in results if r.group_id.startswith("sequential_matvec_")]
    
    # Group by matrix
    matrix_groups = {}
    for result in sequential_results:
        if result.matrix_name not in matrix_groups:
            matrix_groups[result.matrix_name] = {}
        matrix_groups[result.matrix_name][result.function_id] = result
    
    # Sort matrices by nnz
    sorted_matrices = sorted(matrix_groups.items(), key=lambda x: next(iter(x[1].values())).nnz)
    
    lines = []
    lines.append("# Sequential Sparse Matrix-Vector Multiplication Benchmark Results\n")
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

def generate_thread_scaling_table(results: List[BenchmarkResult]) -> str:
    """Generate thread scaling performance table."""
    # Filter parallel results
    parallel_results = [r for r in results if r.group_id.startswith("thread_scaling_") and r.threads is not None]
    
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
    
    lines = []
    lines.append("\n# Parallel Thread Scaling Results\n")
    
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

def generate_performance_analysis(results: List[BenchmarkResult]) -> str:
    """Generate performance analysis section."""
    sequential_results = [r for r in results if r.group_id.startswith("sequential_matvec_")]
    
    # Group by matrix for analysis
    matrix_groups = {}
    for result in sequential_results:
        if result.matrix_name not in matrix_groups:
            matrix_groups[result.matrix_name] = {}
        matrix_groups[result.matrix_name][result.function_id] = result
    
    sorted_matrices = sorted(matrix_groups.items(), key=lambda x: next(iter(x[1].values())).nnz)
    
    # Categorize by size
    small_matrices = [(name, impls) for name, impls in sorted_matrices if next(iter(impls.values())).nnz < 1000]
    medium_matrices = [(name, impls) for name, impls in sorted_matrices if 1000 <= next(iter(impls.values())).nnz < 100000]
    large_matrices = [(name, impls) for name, impls in sorted_matrices if next(iter(impls.values())).nnz >= 100000]
    
    lines = []
    lines.append("\n## Performance Analysis\n")
    
    def analyze_category(category_name: str, matrices: List[Tuple]) -> None:
        if not matrices:
            return
            
        lines.append(f"### {category_name} Matrices")
        winners = []
        
        for matrix_name, implementations in matrices:
            if not implementations:
                continue
                
            fastest_impl = min(implementations.items(), key=lambda x: x[1].median_ns)
            fastest_name, fastest_result = fastest_impl
            fastest_time, fastest_unit = format_time_with_unit(fastest_result.median_ns)
            
            winners.append(fastest_name)
            
            # Show performance comparison
            comparisons = []
            for impl_name, result in implementations.items():
                if impl_name != fastest_name:
                    ratio = result.median_ns / fastest_result.median_ns
                    comparisons.append(f"{impl_name} is {ratio:.1f}x slower")
            
            if comparisons:
                lines.append(f"- **{matrix_name}**: {fastest_name} wins ({fastest_time:.2f} {fastest_unit}), " + ", ".join(comparisons))
            else:
                lines.append(f"- **{matrix_name}**: {fastest_name} only implementation ({fastest_time:.2f} {fastest_unit})")
        
        # Overall winner for category
        if winners:
            winner_counts = {impl: winners.count(impl) for impl in set(winners)}
            category_winner = max(winner_counts.items(), key=lambda x: x[1])
            lines.append(f"\n**{category_name} category winner**: {category_winner[0]} ({category_winner[1]}/{len(matrices)} matrices)")
        
        lines.append("")
    
    analyze_category("Small", small_matrices)
    analyze_category("Medium", medium_matrices) 
    analyze_category("Large", large_matrices)
    
    return "\n".join(lines)

def generate_notes_section() -> str:
    """Generate notes section."""
    lines = []
    lines.append("## Notes\n")
    lines.append("- Times shown are median ± approximate standard deviation from Criterion benchmarks")
    lines.append("- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication")
    lines.append("- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication")
    lines.append("- `sprs` = sprs CSR matrix-vector multiplication")
    lines.append("- Thread scaling shows parallel implementation performance across different thread counts")
    lines.append("- All measurements taken on the same system with consistent methodology")
    
    return "\n".join(lines)

def main():
    criterion_dir = Path("target/criterion")
    
    print("Collecting benchmark results from Criterion JSON files...")
    results = collect_benchmark_results(criterion_dir)
    
    if not results:
        print("Error: No benchmark results found", file=sys.stderr)
        sys.exit(1)
    
    print(f"Found {len(results)} benchmark results")
    
    # Generate sections
    sequential_table = generate_sequential_table(results)
    thread_scaling_table = generate_thread_scaling_table(results)
    performance_analysis = generate_performance_analysis(results)
    notes_section = generate_notes_section()
    
    # Combine all sections
    full_content = "\n".join([
        sequential_table,
        thread_scaling_table,
        performance_analysis,
        notes_section
    ])
    
    # Write to file
    output_file = Path("BENCHMARK_RESULTS.md")
    with open(output_file, 'w') as f:
        f.write(full_content)
    
    print(f"Generated benchmark results: {output_file}")

if __name__ == "__main__":
    main()
