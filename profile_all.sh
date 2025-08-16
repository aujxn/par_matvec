#!/bin/bash

# profile_all.sh - Run flamegraph profiling for all algorithms
# Usage: ./profile_all.sh <matrix_path> <num_threads>

set -e

if [ $# -ne 2 ]; then
    echo "Usage: $0 <matrix_path> <num_threads>"
    echo "Example: $0 test_matrices/bcsstk14.mtx 4"
    exit 1
fi

MATRIX_PATH="$1"
NUM_THREADS="$2"

# Validate inputs
if [ ! -f "$MATRIX_PATH" ]; then
    echo "Error: Matrix file not found: $MATRIX_PATH"
    exit 1
fi

if ! [[ "$NUM_THREADS" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: Number of threads must be a positive integer"
    exit 1
fi

# Create figures directory if it doesn't exist
mkdir -p figures

# Extract matrix name from path for cleaner output names
MATRIX_NAME=$(basename "$MATRIX_PATH" .mtx)

# Define algorithms
ALGORITHMS=("dense_sparse" "sparse_dense_simple" "sparse_dense_merge" "sparse_dense_buffer")

echo "Running flamegraph profiling for matrix: $MATRIX_NAME with $NUM_THREADS threads"
echo "Output directory: figures/"
echo

for algorithm in "${ALGORITHMS[@]}"; do
    output_file="figures/${algorithm}_${NUM_THREADS}-threads_${MATRIX_NAME}_flamegraph.svg"
    
    echo "Profiling $algorithm algorithm..."
    echo "Output: $output_file"
    
    cargo flamegraph \
        --output "$output_file" \
        --release \
        --bin profile_spmv \
        -- "$MATRIX_PATH" "$NUM_THREADS" "$algorithm"
    
    echo "✓ Completed $algorithm"
    echo
done

echo "All flamegraph profiles completed!"
echo "Generated files:"
for algorithm in "${ALGORITHMS[@]}"; do
    echo "  figures/${algorithm}_${NUM_THREADS}-threads_${MATRIX_NAME}_flamegraph.svg"
done