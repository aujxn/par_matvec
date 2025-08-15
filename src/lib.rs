use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::thread;

use faer::{
    Accum, ColMut, ColRef, Index, MatMut, MatRef, Par, RowMut, RowRef,
    dyn_stack::{MemStack, StackReq},
    linalg::temp_mat_scratch,
    prelude::{Reborrow, ReborrowMut},
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
                //StackReq::empty()
                temp_mat_scratch::<T>(rhs.nrows(), n_threads)
            }
        }
    }
}

pub fn dense_sparse_scratch<I: Index, T: ComplexField>(
    lhs: MatRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    _strategy: &SparseDenseStrategy,
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
                StackReq::new::<T>(n_threads * 2)
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
                    let mut dst = dst;
                    simple_impl::par_sparse_dense(
                        dst.rb_mut(),
                        beta,
                        lhs,
                        rhs,
                        &alpha,
                        n_threads,
                        strategy,
                        stack,
                    );
                }
            }
        }
    }
}

mod merge_impl {
    //! Algorithm based on merging all the column contributions with k-way merge resulting in a
    //! sparse vec for each thread. Summed at end sequentially but could be parallel, doesn't
    //! matter since this one is much worse because merging takes 10x as long as anything else with
    //! this min heap algorithm.
    use super::*;
    // (row, val, local column index)
    struct HeapEntry<T: ComplexField>(usize, T, usize);

    // Backwards impl of Ord to make min heap
    impl<T: ComplexField> Ord for HeapEntry<T> {
        fn cmp(&self, other: &Self) -> Ordering {
            other.0.cmp(&self.0)
        }
    }

    impl<T: ComplexField> PartialOrd for HeapEntry<T> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(other.0.cmp(&self.0))
        }
    }

    impl<T: ComplexField> PartialEq for HeapEntry<T> {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl<T: ComplexField> Eq for HeapEntry<T> {}

    // NOTE: This merge based algorithm requires row indices to be in sorted order over columns, a soft
    // invariant in faer
    pub fn par_sparse_dense<I: Index, T: ComplexField>(
        dst: ColMut<'_, T>,
        beta: Accum,
        lhs: SparseColMatRef<'_, I, T>,
        rhs: ColRef<'_, T>,
        alpha: &T,
        n_threads: usize,
        strategy: &SparseDenseStrategy,
        _stack: &mut MemStack,
    ) {
        let m = lhs.nrows();

        let (lhs_symbolic, lhs_values) = lhs.parts();
        let row_indices = lhs_symbolic.row_idx();

        thread::scope(|s| {
            let mut handles = Vec::with_capacity(n_threads);
            let core_ids = core_affinity::get_core_ids().unwrap();
            debug_assert!(core_ids.len() >= n_threads);
            for (tid, core_id) in (0..n_threads).zip(core_ids.into_iter()) {
                //let dst = dst.rb();

                //let owned_rows = (m + (n_threads - 1)) / n_threads;
                //let row_start = tid * owned_rows;
                //if tid == n_threads - 1 {
                //owned_rows = m - row_start;
                //}
                //let row_end = row_start + owned_rows;

                let handle = s.spawn(move || {
                    let res = core_affinity::set_for_current(core_id);
                    debug_assert!(res);
                    // SAFETY: non-overlapping thread ownership of dst slice
                    //let mut dst_owned = unsafe { dst.subrows(row_start, owned_rows).const_cast() };
                    //if let Accum::Replace = beta {
                    //dst_owned.fill(zero());
                    //}

                    let col_start = strategy.thread_cols[tid];
                    let col_end = strategy.thread_cols[tid + 1];
                    let idx_start = strategy.thread_indptrs[tid];
                    let idx_end = strategy.thread_indptrs[tid + 1];

                    let k = 1 + col_end - col_start;
                    let mut min_heap: BinaryHeap<HeapEntry<T>> = BinaryHeap::with_capacity(k);
                    // (row indices, values, rhs scalar) for each column the thread owns
                    let slices: Vec<(&[I], &[T], T)> = (col_start..=col_end)
                        .map(|depth| {
                            let rhs_k = rhs[depth].mul_by_ref(alpha);
                            let mut col_range = lhs_symbolic.col_range(depth);
                            if depth == col_start {
                                col_range.start = idx_start;
                            }
                            if depth == col_end {
                                col_range.end = idx_end;
                            }

                            let row_indices = &row_indices[col_range.clone()];
                            let values = &lhs_values[col_range];

                            (row_indices, values, rhs_k)
                        })
                        .collect();

                    for (local_col, (indices, values, _)) in slices.iter().enumerate() {
                        if indices.len() > 0 {
                            min_heap.push(HeapEntry(indices[0].zx(), values[0].clone(), local_col));
                        }
                    }

                    let mut merge_ptrs: Vec<usize> = vec![1; k];
                    // probably don't need this much capacity... but in pathological case we do
                    let mut merged: Vec<(usize, T)> = Vec::with_capacity(m);

                    // buffer the first entry so we can unwrap in hot loop
                    let HeapEntry(first_row, first_val, local_col) = min_heap.pop().unwrap();
                    merged.push((first_row, first_val.mul_by_ref(&slices[local_col].2)));
                    merge_ptrs[local_col] += 1;
                    if slices[local_col].0.len() > 1 {
                        min_heap.push(HeapEntry(
                            slices[local_col].0[1].zx(),
                            slices[local_col].1[1].clone(),
                            local_col,
                        ));
                    }

                    loop {
                        match min_heap.pop() {
                            Some(HeapEntry(row_idx, val, local_col)) => {
                                let last = merged.last_mut().unwrap();
                                let val = val.mul_by_ref(&slices[local_col].2);

                                if last.0 == row_idx {
                                    last.1 = last.1.add_by_ref(&val);
                                } else {
                                    merged.push((row_idx, val));
                                }

                                let current_idx = merge_ptrs[local_col];
                                if slices[local_col].0.len() > current_idx {
                                    min_heap.push(HeapEntry(
                                        slices[local_col].0[current_idx].zx(),
                                        slices[local_col].1[current_idx].clone(),
                                        local_col,
                                    ));
                                }
                                merge_ptrs[local_col] += 1;
                            }
                            None => break,
                        }
                    }
                    merged
                });

                handles.push(handle);
            }

            let mut dst = dst;
            if let Accum::Replace = beta {
                dst.fill(zero());
            }
            for handle in handles {
                for (row, val) in handle.join().unwrap() {
                    dst[row] = dst[row].add_by_ref(&val);
                }
            }
        });
    }
}

mod simple_impl {
    //! Most simple algorithm which creates a `dst` workspace for each thread and then sums them
    //! all in parallel at the end.

    use super::*;
    use faer::{col::AsColMut, linalg::temp_mat_zeroed, mat::AsMatMut};
    use rayon::iter::{IndexedParallelIterator, ParallelIterator};

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
        strategy: &SparseDenseStrategy,
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
    fn reduce_workspaces_threaded<T: ComplexField>(
        n_threads: usize,
        work: MatRef<T>,
        dst: ColMut<T>,
    ) {
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
                    let mut dst = dst;
                    par_dense_sparse(
                        dst.rb_mut(),
                        beta,
                        lhs,
                        rhs,
                        &alpha,
                        n_threads,
                        strategy,
                        stack,
                    );
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
    _stack: &mut MemStack,
) {
    /*
    let global_counter: usize = strategy
        .thread_cols
        .iter()
        .zip(strategy.thread_cols.iter().skip(1))
        .map(|(start, end)| 1 + end - start)
        .sum();
    */

    //let (mut work, _) = temp_mat_zeroed::<T, _, _>(global_counter, 1, stack);
    //let work = work.as_mat_mut();
    //let work = work.rb();
    let (rhs_symbolic, rhs_values) = rhs.parts();
    let row_indices = rhs_symbolic.row_idx();

    let mut dst = dst;
    if let Accum::Replace = beta {
        dst.fill(zero());
    }

    //println!();

    thread::scope(|s| {
        let dst = dst.rb();
        let mut handles = Vec::with_capacity(n_threads);
        for tid in 0..n_threads {
            let handle = s.spawn(move || {
                //let start_time = Instant::now();
                let col_start = strategy.thread_cols[tid];
                let col_end = strategy.thread_cols[tid + 1];
                let idx_start = strategy.thread_indptrs[tid];
                let idx_end = strategy.thread_indptrs[tid + 1];

                // SAFETY: each thread gets 2 workspace values. Could probably just return these as
                // tuple...
                /*
                let mut work = unsafe {
                    work.col(0)
                        .const_cast()
                        .try_as_col_major_mut()
                        .unwrap()
                        .subrows_mut(n_threads * 2, 2)
                };
                */
                // SAFETY: the ranges (col_start+1)..col_end are non-overlapping per thread
                let mut dst_owned = unsafe { dst.const_cast() };

                let mut left_contrib = T::zero_impl();
                let mut right_contrib = T::zero_impl();
                if col_start == col_end {
                    for idx in col_start..col_end {
                        let k = row_indices[idx].zx();
                        let lhs_k = lhs[k].mul_by_ref(alpha);
                        let rhs_kj = &rhs_values[idx];
                        //work[0] = work[0].add_by_ref(&lhs_k.mul_by_ref(&rhs_kj));
                        left_contrib = left_contrib.add_by_ref(&lhs_k.mul_by_ref(&rhs_kj));
                    }
                } else {
                    let mut col_range = rhs_symbolic.col_range(col_start);
                    col_range.start = idx_start;
                    for idx in col_range {
                        let k = row_indices[idx].zx();
                        let lhs_k = lhs[k].mul_by_ref(alpha);
                        let rhs_kj = &rhs_values[idx];
                        left_contrib = left_contrib.add_by_ref(&lhs_k.mul_by_ref(&rhs_kj));
                    }

                    for j in col_start + 1..col_end {
                        for idx in rhs_symbolic.col_range(j) {
                            let k = row_indices[idx].zx();
                            let lhs_k = lhs[k].mul_by_ref(alpha);
                            let rhs_kj = &rhs_values[idx];
                            dst_owned[j] = dst_owned[j].add_by_ref(&lhs_k.mul_by_ref(&rhs_kj));
                        }
                    }

                    let mut col_range = rhs_symbolic.col_range(col_end);
                    col_range.end = idx_end;
                    for idx in col_range {
                        let k = row_indices[idx].zx();
                        let lhs_k = lhs[k].mul_by_ref(alpha);
                        let rhs_kj = &rhs_values[idx];
                        right_contrib = right_contrib.add_by_ref(&lhs_k.mul_by_ref(&rhs_kj));
                    }
                }
                //let end_time = Instant::now();
                //println!("{}: {:?}", tid, end_time.duration_since(start_time));
                (left_contrib, right_contrib)
            });
            handles.push(handle);
        }

        let stitch: Vec<(T, T)> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        //let work = work.col(0);
        // SAFETY: all workers have been joined so root thread is only one with access
        let mut dst = unsafe { dst.const_cast() };
        //for tid in 0..n_threads {
        for (tid, (left_v, right_v)) in stitch.iter().enumerate() {
            let left = strategy.thread_cols[tid];
            let right = strategy.thread_cols[tid + 1];
            //let work_idx = tid * 2;
            //let left_v = &work[work_idx];
            //let right_v = &work[work_idx + 1];
            dst[left] = dst[left].add_by_ref(left_v);
            dst[right] = dst[right].add_by_ref(right_v);
        }
    });
}
