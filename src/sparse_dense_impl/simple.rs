//! Most simple algorithm which creates a `dst` workspace for each thread and then sums them
//! all in parallel at the end.
use std::thread;

use faer::{
    Accum, ColMut, ColRef, Index, MatRef, Par,
    col::AsColMut,
    dyn_stack::{MemStack, StackReq},
    linalg::{temp_mat_scratch, temp_mat_zeroed},
    mat::AsMatMut,
    prelude::{Reborrow, ReborrowMut},
    sparse::SparseColMatRef,
    traits::{ComplexField, math_utils::zero},
};

use rayon::iter::{IndexedParallelIterator, ParallelIterator};

use crate::spmv_drivers::SpMvStrategy;

pub fn sparse_dense_scratch<I: Index, T: ComplexField>(
    lhs: SparseColMatRef<'_, I, T>,
    rhs: MatRef<'_, T>,
    strategy: &SpMvStrategy,
    par: Par,
) -> StackReq {
    let _ = lhs;
    let _ = strategy;
    match par {
        Par::Seq => StackReq::empty(),
        Par::Rayon(n_threads) => {
            let dim = rhs.ncols();
            let n_threads = n_threads.get();
            if dim >= n_threads * 4 {
                StackReq::empty()
            } else {
                temp_mat_scratch::<T>(rhs.nrows(), n_threads)
            }
        }
    }
}

#[inline]
fn hot_loop<I: Index, T: ComplexField>(
    col_range: std::ops::Range<usize>,
    row_indices: &[I],
    lhs_values: &[T],
    rhs_k: &T,
    mut work: ColMut<'_, T>,
) {
    for idx in col_range {
        let i = row_indices[idx].zx();
        let lhs_ik = &lhs_values[idx];
        work[i] = work[i].add_by_ref(&lhs_ik.mul_by_ref(rhs_k));
    }
}

pub fn par_sparse_dense<I: Index, T: ComplexField>(
    dst: ColMut<'_, T>,
    beta: Accum,
    lhs: SparseColMatRef<'_, I, T>,
    rhs: ColRef<'_, T>,
    alpha: &T,
    n_threads: usize,
    strategy: &SpMvStrategy,
    stack: &mut MemStack,
) {
    let m = lhs.nrows();

    let (mut work, _) = temp_mat_zeroed::<T, _, _>(m, n_threads, stack);
    let work = work.as_mat_mut();
    let work = work.rb();
    let (lhs_symbolic, lhs_values) = lhs.parts();
    let row_indices = lhs_symbolic.row_idx();
    let mut dst = dst;

    thread::scope(|s| {
        for tid in 0..n_threads {
            let _handle = s.spawn(move || {
                // SAFETY each thread gets its own workspace vector to be summed when all complete
                let mut work =
                    unsafe { work.col(tid).const_cast().try_as_col_major_mut().unwrap() };

                let col_start = strategy.thread_cols[tid];
                let col_end = strategy.thread_cols[tid + 1];
                let idx_start = strategy.thread_indptrs[tid];
                let idx_end = strategy.thread_indptrs[tid + 1];

                for depth in col_start..=col_end {
                    let rhs_k = rhs[depth].mul_by_ref(alpha);
                    let mut col_range = lhs_symbolic.col_range(depth);
                    if depth == col_start {
                        col_range.start = idx_start;
                    }
                    if depth == col_end {
                        col_range.end = idx_end;
                    }
                    hot_loop(
                        col_range,
                        row_indices,
                        lhs_values,
                        &rhs_k,
                        work.as_col_mut(),
                    );
                }
            });
        }

        let mut dst = dst.rb_mut();
        if let Accum::Replace = beta {
            dst.fill(zero());
        }
    });

    //reduce_workspaces_threaded(n_threads, work, dst);
    reduce_workspaces_rayon(n_threads, work, dst);
}

/// somehow this is slower than `reduce_workspaces_rayon` variant
#[allow(dead_code)]
fn reduce_workspaces_threaded<T: ComplexField>(n_threads: usize, work: MatRef<T>, dst: ColMut<T>) {
    let rows_per_thread = (work.nrows() + (n_threads - 1)) / n_threads;
    thread::scope(|s| {
        for tid in 0..n_threads {
            let dst = dst.rb();
            let work = work.rb();
            let _handle = s.spawn(move || {
                let mut rows_per_thread = rows_per_thread;
                let start_row = tid * rows_per_thread;
                let mut end_row = (tid + 1) * rows_per_thread;
                if tid == n_threads - 1 {
                    end_row = work.nrows();
                    rows_per_thread = end_row - start_row;
                }

                let mut dst = unsafe { dst.subrows(start_row, rows_per_thread).const_cast() };
                for col in work.col_iter() {
                    for (local_row, i) in (start_row..end_row).enumerate() {
                        dst[local_row] = dst[local_row].add_by_ref(&col[i]);
                    }
                }
            });
        }
    });
}

/// somehow this is faster than `reduce_workspaces_threaded` variant
#[allow(dead_code)]
fn reduce_workspaces_rayon<T: ComplexField>(n_threads: usize, work: MatRef<T>, dst: ColMut<T>) {
    let rows_per_thread = (work.nrows() + (n_threads - 1)) / n_threads;
    // This seems janky could probably improve lots
    dst.as_mat_mut()
        .par_row_chunks_mut(rows_per_thread)
        .zip(work.par_row_chunks(rows_per_thread))
        .for_each(|(dst_chunk, work_chunk)| {
            let dst_chunk = dst_chunk.col_mut(0);
            for (i, dst) in dst_chunk.iter_mut().enumerate() {
                let work_row = work_chunk.row(i);
                *dst = dst.add_by_ref(&work_row.sum());
            }
        });
}
