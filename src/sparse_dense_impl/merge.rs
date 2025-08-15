//! Algorithm based on merging all the column contributions with k-way merge resulting in a
//! sparse vec for each thread. Summed at end sequentially but could be parallel, doesn't
//! matter since this one is much worse because merging takes 10x as long as anything else with
//! this min heap algorithm.
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::thread;

use faer::{
    Accum, ColMut, ColRef, Index,
    dyn_stack::MemStack,
    sparse::SparseColMatRef,
    traits::{ComplexField, math_utils::zero},
};

use crate::spmv_drivers::SpMvStrategy;

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
    strategy: &SpMvStrategy,
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
