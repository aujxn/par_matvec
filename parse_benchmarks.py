#!/usr/bin/env python3
"""
Parse Criterion benchmark results and generate a markdown summary table.
(Clean version without custom implementation)
"""

import re
import sys
from pathlib import Path
from dataclasses import dataclass, field
from typing import Dict, List, Optional
import statistics

@dataclass
class BenchmarkResult:
    matrix_name: str
    implementation: str
    dimensions: str
    nnz: int
    median_ns: float
    lower_bound_ns: float
    upper_bound_ns: float
    
    @property
    def std_dev_ns(self) -> float:
        # Rough approximation of std dev from confidence interval
        # This is approximate since Criterion uses bootstrap confidence intervals
        return (self.upper_bound_ns - self.lower_bound_ns) / 4.0
    
    @property
    def median_us(self) -> float:
        return self.median_ns / 1000.0 if self.median_ns < 10000 else self.median_ns / 1000.0
    
    @property
    def std_dev_us(self) -> float:
        return self.std_dev_ns / 1000.0 if self.median_ns < 10000 else self.std_dev_ns / 1000.0

def parse_time_to_ns(time_str: str) -> float:
    """Parse time strings like '1.0552 ms', '29.783 µs', '61.938 ns' to nanoseconds."""
    time_str = time_str.strip()
    
    # Use regex to extract number and unit
    import re
    match = re.match(r'([0-9.]+)\s*(ms|µs|us|ns)', time_str)
    if not match:
        raise ValueError(f"Could not parse time string: {time_str}")
    
    value = float(match.group(1))
    unit = match.group(2)
    
    if unit == 'ms':
        return value * 1_000_000  # ms to ns
    elif unit in ['µs', 'us']:
        return value * 1_000  # µs to ns
    elif unit == 'ns':
        return value  # already ns
    else:
        raise ValueError(f"Unknown time unit: {unit}")

def parse_benchmark_file(file_path: Path) -> List[BenchmarkResult]:
    """Parse the benchmark results file and extract performance data."""
    results = []
    
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Pattern to match benchmark results
    # Example: sequential_matvec_0/faer/18x18_nnz68
    #          time:   [59.882 ns 61.938 ns 64.625 ns]
    
    benchmark_pattern = r'sequential_matvec_(\w+)/([^/]+)/(\d+x\d+_nnz\d+)\s*\n.*?time:\s*\[([^\]]+)\]'
    
    matches = re.findall(benchmark_pattern, content, re.MULTILINE | re.DOTALL)
    
    for match in matches:
        matrix_name = match[0]
        implementation = match[1]
        dimensions_nnz = match[2]
        time_data = match[3]
        
        # Extract dimensions and nnz
        dim_match = re.match(r'(\d+x\d+)_nnz(\d+)', dimensions_nnz)
        if dim_match:
            dimensions = dim_match.group(1)
            nnz = int(dim_match.group(2))
        else:
            continue
        
        # Parse time values [lower median upper unit]
        # Example: "59.882 ns 61.938 ns 64.625 ns" -> extract unit from any value
        time_parts = time_data.strip().split()
        if len(time_parts) >= 6:  # Should be: value1 unit1 value2 unit2 value3 unit3
            try:
                lower_bound_ns = parse_time_to_ns(f"{time_parts[0]} {time_parts[1]}")
                median_ns = parse_time_to_ns(f"{time_parts[2]} {time_parts[3]}")
                upper_bound_ns = parse_time_to_ns(f"{time_parts[4]} {time_parts[5]}")
                
                result = BenchmarkResult(
                    matrix_name=matrix_name,
                    implementation=implementation,
                    dimensions=dimensions,
                    nnz=nnz,
                    median_ns=median_ns,
                    lower_bound_ns=lower_bound_ns,
                    upper_bound_ns=upper_bound_ns
                )
                results.append(result)
                
            except ValueError as e:
                print(f"Warning: Could not parse time data for {match}: {e}", file=sys.stderr)
                continue
    
    return results

def group_results_by_matrix(results: List[BenchmarkResult]) -> Dict[str, Dict[str, BenchmarkResult]]:
    """Group results by matrix, then by implementation."""
    grouped = {}
    
    for result in results:
        if result.matrix_name not in grouped:
            grouped[result.matrix_name] = {}
        
        # Use implementation names as-is since they're now clean
        impl_name = result.implementation
        
        grouped[result.matrix_name][impl_name] = result
    
    return grouped

def format_time_with_unit(time_ns: float) -> tuple[float, str]:
    """Format time with appropriate unit (ns, µs, ms)."""
    if time_ns < 1000:
        return time_ns, 'ns'
    elif time_ns < 1_000_000:
        return time_ns / 1000, 'µs'
    else:
        return time_ns / 1_000_000, 'ms'

def generate_markdown_table(grouped_results: Dict[str, Dict[str, BenchmarkResult]]) -> str:
    """Generate a markdown table summary."""
    
    # Sort matrices by nnz for logical ordering
    matrix_info = []
    for matrix_name, implementations in grouped_results.items():
        # Get nnz from any implementation (they should all be the same)
        first_impl = next(iter(implementations.values()))
        matrix_info.append((matrix_name, first_impl.nnz, first_impl.dimensions, implementations))
    
    matrix_info.sort(key=lambda x: x[1])  # Sort by nnz
    
    # Generate table
    lines = []
    lines.append("# Sequential Sparse Matrix-Vector Multiplication Benchmark Results\n")
    lines.append("| Matrix | Dimensions | Non-zeros | faer | nalgebra | sprs |")
    lines.append("|--------|------------|-----------|------|----------|------|")
    
    for matrix_name, nnz, dimensions, implementations in matrix_info:
        row = [f"**{matrix_name}**", dimensions, f"{nnz:,}"]
        
        for impl in ['faer', 'nalgebra', 'sprs']:
            if impl in implementations:
                result = implementations[impl]
                time_val, unit = format_time_with_unit(result.median_ns)
                std_val, _ = format_time_with_unit(result.std_dev_ns)
                
                if unit == 'ns':
                    cell = f"{time_val:.1f} ± {std_val:.1f} ns"
                elif unit == 'µs':
                    cell = f"{time_val:.2f} ± {std_val:.2f} µs"
                else:  # ms
                    cell = f"{time_val:.3f} ± {std_val:.3f} ms"
                
                row.append(cell)
            else:
                row.append("—")
        
        lines.append("| " + " | ".join(row) + " |")
    
    # Add performance analysis
    lines.append("\n## Performance Analysis\n")
    
    # Find fastest implementation for each matrix size category
    small_matrices = [(name, impls) for name, nnz, _, impls in matrix_info if nnz < 1000]
    medium_matrices = [(name, impls) for name, nnz, _, impls in matrix_info if 1000 <= nnz < 100000]
    large_matrices = [(name, impls) for name, nnz, _, impls in matrix_info if nnz >= 100000]
    
    def analyze_category(category_name: str, matrices: List[tuple]) -> None:
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
    
    # Add notes
    lines.append("## Notes\n")
    lines.append("- Times shown are median ± approximate standard deviation from 100 samples")
    lines.append("- `faer` = faer built-in sequential sparse-dense matrix-vector multiplication")
    lines.append("- `nalgebra` = nalgebra-sparse CSR matrix-vector multiplication")
    lines.append("- `sprs` = sprs CSR matrix-vector multiplication")
    lines.append("- All measurements taken on the same system with consistent methodology")
    
    return "\n".join(lines)

def main():
    benchmark_file = Path("benchmark_results_clean.txt")
    
    if not benchmark_file.exists():
        print(f"Error: {benchmark_file} not found. Run clean benchmarks first.", file=sys.stderr)
        sys.exit(1)
    
    print("Parsing clean benchmark results...")
    results = parse_benchmark_file(benchmark_file)
    
    if not results:
        print("Error: No benchmark results found in file", file=sys.stderr)
        sys.exit(1)
    
    print(f"Found {len(results)} benchmark results")
    
    grouped_results = group_results_by_matrix(results)
    markdown_table = generate_markdown_table(grouped_results)
    
    output_file = Path("BENCHMARK_RESULTS.md")
    with open(output_file, 'w') as f:
        f.write(markdown_table)
    
    print(f"Generated clean markdown table: {output_file}")
    print("\nPreview:")
    print("=" * 50)
    print(markdown_table[:1500] + "..." if len(markdown_table) > 1500 else markdown_table)

if __name__ == "__main__":
    main()