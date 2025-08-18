#!/bin/bash
#SBATCH --job-name=par_matvec_bench
#SBATCH --nodes=1
#SBATCH --cpus-per-task=64
#SBATCH --gres=gpu:a30:4
#SBATCH --mem=0
#SBATCH --exclusive
#SBATCH --partition=main
#SBATCH --output=logs/%x_%j.out
#SBATCH --error=logs/%x_%j.err
#SBATCH --time=1-00:00:00

mkdir -p logs

export RUST_BACKTRACE=FULL 
export RUST_LOG=trace

# These shouldn't change very much so no need to rebench
#cargo bench --bench sequential
cargo bench --bench parallel

module load python
source venv/bin/activate
./venv/bin/python3 parse_criterion_benchmarks.py
