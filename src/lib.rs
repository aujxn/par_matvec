use std::{sync::Arc, thread};

use faer::{
    Accum, ColMut, ColRef, Index, MatMut, MatRef, Par, RowMut, RowRef,
    dyn_stack::{MemStack, StackReq},
    linalg::{temp_mat_scratch, temp_mat_zeroed},
    mat::AsMatMut,
    prelude::Reborrow,
    sparse::{
        SparseColMatRef, SymbolicSparseColMatRef,
        linalg::matmul::{
            dense_sparse_matmul as seq_dense_sparse, sparse_dense_matmul as seq_sparse_dense,
        },
    },
    traits::{ComplexField, math_utils::zero},
};

pub mod test_utils;

pub struct SparseDenseStrategy {
    thread_cols: Vec<usize>,
    thread_indptrs: Vec<usize>,
}

impl SparseDenseStrategy {
    pub fn new<I: Index>(mat: SymbolicSparseColMatRef<'_, I>, par: Par) -> Self {
        let (thread_cols, thread_indptrs) = match par {
            Par::Seq => (Vec::new(), Vec::new()),
            Par::Rayon(n_threads) => {
                let n_threads = n_threads.get();

                let mut thread_cols = Vec::with_capacity(n_threads);
                let mut thread_indptrs = Vec::with_capacity(n_threads);

                let nnz = mat.compute_nnz();
                // TODO probably don't assert here
                assert!(nnz > n_threads);
                let per_thread = nnz / n_threads;
                let col_ptrs = mat.col_ptr();
                let ncols = mat.ncols();

                match mat.col_nnz() {
                    None => {
                        thread_indptrs
                            .extend((0..n_threads).map(|thread_id| thread_id * per_thread));
                        thread_indptrs.push(col_ptrs[ncols].zx());
                        thread_cols.push(0);

                        let mut nnz_counter = 0;
                        let mut thread_id = 1;
                        for (col, (start, end)) in
                            col_ptrs.iter().zip(col_ptrs.iter().skip(1)).enumerate()
                        {
                            let col_nnz = end.zx() - start.zx();
                            nnz_counter += col_nnz;

                            while thread_id < n_threads && nnz_counter > thread_indptrs[thread_id] {
                                thread_cols.push(col);
                                thread_id += 1;
                            }
                        }
                        thread_cols.push(ncols - 1);
                        assert_eq!(thread_cols.len(), n_threads + 1);
                    }
                    Some(_nnz_per_col) => {
                        unimplemented!();
                    }
                }
                (thread_cols, thread_indptrs)
            }
        };

        Self {
            thread_cols,
            thread_indptrs,
        }
    }
}

pub fn sparse_dense_scratch<I: Index, T: ComplexField>(
    lhs: SparseColMatRef<'_, I, T>,
    rhs: MatRef<'_, T>,
    strategy: &SparseDenseStrategy,
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

pub fn dense_sparse_scratch<I: Index, T: ComplexField>(
    lhs: MatRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    strategy: &SparseDenseStrategy,
    par: Par,
) -> StackReq {
    let _ = rhs;
    match par {
        Par::Seq => StackReq::empty(),
        Par::Rayon(n_threads) => {
            let dim = lhs.nrows();
            let n_threads = n_threads.get();
            if dim >= n_threads * 4 {
                StackReq::empty()
            } else {
                let counter = strategy
                    .thread_cols
                    .iter()
                    .zip(strategy.thread_cols.iter().skip(1))
                    .map(|(start, end)| 1 + end - start)
                    .sum();
                StackReq::new::<T>(counter)
            }
        }
    }
}

/// Strategy:
/// - When `Par::Seq` call out to existing impl for matvec
/// - When `Par::Rayon`
///   - If output has more than 4 times `n_threads` columns, then split thread work by output
///   columns. Should be balanced enough? requires no workspace and less syncronization
///   - Otherwise, split the thread work by input columns and iterate over output columns.
///   One workspace vector per thread and synchronization after each matvec to sum workspaces
pub fn sparse_dense_matmul<I: Index, T: ComplexField>(
    dst: MatMut<'_, T>,
    beta: Accum,
    lhs: SparseColMatRef<'_, I, T>,
    rhs: MatRef<'_, T>,
    alpha: T,
    par: Par,
    strategy: &SparseDenseStrategy,
    stack: &mut MemStack,
) {
    match par {
        Par::Seq => seq_sparse_dense(dst, beta, lhs, rhs, alpha, par),
        Par::Rayon(n_threads) => {
            let dim = rhs.ncols();
            let n_threads = n_threads.get();
            if dim >= n_threads * 4 {
                unimplemented!();
            } else {
                for (dst, rhs) in dst.col_iter_mut().zip(rhs.col_iter()) {
                    par_sparse_dense(dst, beta, lhs, rhs, &alpha, n_threads, strategy, stack);
                }
            }
        }
    }
}

fn par_sparse_dense<I: Index, T: ComplexField>(
    dst: ColMut<'_, T>,
    beta: Accum,
    lhs: SparseColMatRef<'_, I, T>,
    rhs: ColRef<'_, T>,
    alpha: &T,
    n_threads: usize,
    strategy: &SparseDenseStrategy,
    stack: &mut MemStack,
) {
    let m = lhs.nrows();

    let (mut work, _) = temp_mat_zeroed::<T, _, _>(m, n_threads, stack);
    let work = work.as_mat_mut();
    let work = work.rb();
    let (lhs_symbolic, lhs_values) = lhs.parts();
    let row_indices = lhs_symbolic.row_idx();

    let arc_dst = Arc::new(spin::Mutex::new(dst));

    thread::scope(|s| {
        // lock the mutex so spawning thread doesn't wipe any finished values...
        let mut dst = arc_dst.lock();

        for tid in 0..n_threads {
            let arc_dst = arc_dst.clone();
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
                    for idx in col_range {
                        let i = row_indices[idx].zx();
                        let lhs_ik = &lhs_values[idx];
                        work[i] = work[i].add_by_ref(&lhs_ik.mul_by_ref(&rhs_k));
                    }
                }

                let mut dst = arc_dst.lock();
                for (i, src) in work.iter().enumerate() {
                    dst[i] = dst[i].add_by_ref(src);
                }
            });
        }

        if let Accum::Replace = beta {
            dst.fill(zero());
        }
    });

    /*
    (0..n_threads).into_par_iter().for_each(|tid| {
        job(tid);
    });
    */

    /*
    let mut rows_per_thread = work.nrows() / n_threads;
    if rows_per_thread == 0 {
        rows_per_thread = 1;
    }

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
        })
    */
    /*
    for (i, dst) in dst.iter_mut().enumerate() {
        let work_row = work.row(i);
        *dst = dst.add_by_ref(&work_row.sum());
    }
    */
}

pub fn dense_sparse_matmul<I: Index, T: ComplexField>(
    dst: MatMut<'_, T>,
    beta: Accum,
    lhs: MatRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    alpha: T,
    par: Par,
    strategy: &SparseDenseStrategy,
    stack: &mut MemStack,
) {
    match par {
        Par::Seq => seq_dense_sparse(dst, beta, lhs, rhs, alpha, par),
        Par::Rayon(n_threads) => {
            let dim = lhs.nrows();
            let n_threads = n_threads.get();
            if dim >= n_threads * 4 {
                unimplemented!();
            } else {
                for (dst, lhs) in dst.row_iter_mut().zip(lhs.row_iter()) {
                    par_dense_sparse(dst, beta, lhs, rhs, &alpha, n_threads, strategy, stack);
                }
            }
        }
    }
}

fn par_dense_sparse<I: Index, T: ComplexField>(
    dst: RowMut<'_, T>,
    beta: Accum,
    lhs: RowRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    alpha: &T,
    n_threads: usize,
    strategy: &SparseDenseStrategy,
    stack: &mut MemStack,
) {
    let global_counter: usize = strategy
        .thread_cols
        .iter()
        .zip(strategy.thread_cols.iter().skip(1))
        .map(|(start, end)| 1 + end - start)
        .sum();

    let (mut work, _) = temp_mat_zeroed::<T, _, _>(global_counter, 1, stack);
    let work = work.as_mat_mut();
    let work = work.rb();
    let (rhs_symbolic, rhs_values) = rhs.parts();
    let row_indices = rhs_symbolic.row_idx();

    let arc_dst = Arc::new(spin::Mutex::new(dst));

    thread::scope(|s| {
        // lock the mutex so spawning thread doesn't wipe any finished values...
        let mut dst = arc_dst.lock();
        //let (tx, rx) = sync_channel(n_threads);
        for tid in 0..n_threads {
            let arc_dst = arc_dst.clone();
            //let tx = tx.clone();
            let _handle = s.spawn(move || {
                // find the starting index of this thread's workspace
                // TODO probably want to just save this info (along with total) in `strategy` because it's
                // recomputed a lot
                let counter: usize = strategy
                    .thread_cols
                    .iter()
                    .take(tid)
                    .zip(strategy.thread_cols.iter().skip(1))
                    .map(|(start, end)| 1 + end - start)
                    .sum();

                let col_start = strategy.thread_cols[tid];
                let col_end = strategy.thread_cols[tid + 1];
                let idx_start = strategy.thread_indptrs[tid];
                let idx_end = strategy.thread_indptrs[tid + 1];

                let thread_workspace_size = 1 + col_end - col_start;

                // SAFETY each thread gets its own (non-overlapping) workspace subvector based on row partitions
                let mut work = unsafe {
                    work.col(0)
                        .const_cast()
                        .try_as_col_major_mut()
                        .unwrap()
                        .subrows_mut(counter, thread_workspace_size)
                };

                for (work_idx, j) in (col_start..=col_end).enumerate() {
                    let mut col_range = rhs_symbolic.col_range(j);
                    if j == col_start {
                        col_range.start = idx_start;
                    }
                    if j == col_end {
                        col_range.end = idx_end;
                    }
                    for idx in col_range {
                        let k = row_indices[idx].zx();
                        let lhs_k = lhs[k].mul_by_ref(alpha);
                        let rhs_kj = &rhs_values[idx];
                        work[work_idx] = work[work_idx].add_by_ref(&lhs_k.mul_by_ref(&rhs_kj));
                    }
                }
                //tx.send((counter, thread_workspace_size, col_start)).unwrap();

                let mut dst = arc_dst.lock();
                for (offset, val) in work.iter().enumerate() {
                    let dst_col = col_start + offset;
                    dst[dst_col] = dst[dst_col].add_by_ref(val);
                }
            });
        }

        if let Accum::Replace = beta {
            dst.fill(zero());
        }

        /*
        let work = work.col(0);
        let mut finished_counter = 0;
        for (ws_start, ws_size, col_start) in rx.iter() {
            for offset in 0..ws_size {
                let dst_col = col_start + offset;
                let work_idx = ws_start + offset;
                dst[dst_col] = dst[dst_col].add_by_ref(&work[work_idx]);
            }
            finished_counter += 1;
            if finished_counter == n_threads {
                break;
            }
        }
        */
    });

    /*
    let work = work.col(0);
    let mut global_idx = 0;
    for (col_start, col_end) in strategy
        .thread_cols
        .iter()
        .zip(strategy.thread_cols.iter().skip(1))
    {
        let workspace_size = 1 + col_end - col_start;
        for offset in 0..workspace_size {
            let dst_col = col_start + offset;
            let work_idx = global_idx + offset;
            dst[dst_col] = dst[dst_col].add_by_ref(&work[work_idx]);
        }
        global_idx += workspace_size;
    }
    */
}
