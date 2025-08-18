//! Algorithm based on merging all the column contributions with k-way merge resulting in a
//! sparse vec for each thread. Summed at end sequentially but could be parallel, doesn't
//! matter since this one is much worse.
use std::cmp::Ordering;
use std::thread;

use faer::{
    Accum, ColMut, ColRef, Index, MatRef, Par,
    dyn_stack::{MemStack, StackReq},
    sparse::SparseColMatRef,
    traits::{ComplexField, math_utils::zero},
};

use crate::spmv_drivers::SpMvStrategy;

pub fn sparse_dense_scratch<I: Index, T: ComplexField>(
    lhs: SparseColMatRef<'_, I, T>,
    rhs: MatRef<'_, T>,
    strategy: &SpMvStrategy,
    par: Par,
) -> StackReq {
    let _ = lhs;
    match par {
        Par::Seq => StackReq::empty(),
        Par::Rayon(n_threads) => {
            let dim = rhs.ncols();
            let n_threads = n_threads.get();
            if dim >= n_threads * 4 {
                StackReq::empty()
            } else {
                let mut total_base_size = 0;
                let mut total_losers_size = 0;

                for tid in 0..n_threads {
                    let col_start = strategy.thread_cols[tid];
                    let col_end = strategy.thread_cols[tid + 1];
                    let k = 1 + col_end - col_start;
                    let tree_size = k.next_power_of_two();

                    total_base_size += tree_size;
                    total_losers_size += tree_size;
                }

                let base_req = StackReq::new::<Option<Contender<T>>>(total_base_size);
                let losers_req = StackReq::new::<usize>(total_losers_size);

                base_req.and(losers_req)
            }
        }
    }
}

#[derive(Clone)]
struct Contender<T: ComplexField> {
    row: usize,
    val: T,
    local_col: usize,
}

impl<T: ComplexField> Ord for Contender<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // Backwards impl of Ord to make min heap
        other.row.cmp(&self.row)
    }
}

impl<T: ComplexField> PartialOrd for Contender<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(other.row.cmp(&self.row))
    }
}

impl<T: ComplexField> PartialEq for Contender<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.row == other.row
    }
}

impl<T: ComplexField> Eq for Contender<T> {}

struct LoserTree<'a, T: ComplexField> {
    base: &'a mut [Option<Contender<T>>],
    losers: &'a mut [usize],
    size: usize,
}

impl<'a, T: ComplexField> LoserTree<'a, T> {
    fn new(base: &'a mut [Option<Contender<T>>], losers: &'a mut [usize]) -> Self {
        let size = base.len();

        let mut tree = Self { base, losers, size };

        tree.build_tournament();
        tree
    }

    fn build_tournament(&mut self) {
        let mut winners = vec![0; self.size];

        for (i, pair) in self.base.chunks(2).enumerate() {
            if pair[0] > pair[1] {
                self.losers[i] = 1 + i * 2;
                winners[i] = i * 2;
            } else {
                self.losers[i] = i * 2;
                winners[i] = 1 + i * 2;
            }
        }

        let mut current_level_size = self.size / 2;
        let mut current_level_start = 0;

        while current_level_size > 1 {
            let next_level_size = current_level_size / 2;
            let next_level_start = current_level_start + current_level_size;

            for i in 0..next_level_size {
                let left_idx = winners[current_level_start + 2 * i];
                let right_idx = winners[current_level_start + 2 * i + 1];

                let winner_idx = if self.base[left_idx] > self.base[right_idx] {
                    self.losers[next_level_start + i] = right_idx;
                    left_idx
                } else {
                    self.losers[next_level_start + i] = left_idx;
                    right_idx
                };

                winners[next_level_start + i] = winner_idx;
            }

            current_level_size = next_level_size;
            current_level_start = next_level_start;
        }

        assert_eq!(current_level_size, 1);
        self.losers[self.size - 1] = winners[current_level_start];
    }

    // This function needs more optimizing but I'm not sure how
    #[inline]
    fn adjust_from_leaf(&mut self, idx: usize) {
        let mut parent_idx = idx / 2;
        let mut winner_idx = idx;
        let mut level_size = self.size / 2;
        let mut offset = 0;

        while level_size > 0 {
            let old_loser = self.losers[parent_idx];
            if self.base[winner_idx] < self.base[old_loser] {
                self.losers[parent_idx] = winner_idx;
                winner_idx = old_loser;
            }
            let next_level_idx = (parent_idx - offset) / 2;
            offset += level_size;
            level_size /= 2;
            parent_idx = offset + next_level_idx;
        }

        self.losers[self.size - 1] = winner_idx;
    }

    #[inline]
    fn winner(&mut self) -> Option<Contender<T>> {
        let winner_idx = self.losers[self.size - 1];
        let result = self.base[winner_idx].take();
        result
    }

    #[inline]
    fn push(&mut self, replacement: Option<Contender<T>>) {
        let winner_idx = *self.losers.last().unwrap();
        self.base[winner_idx] = replacement;
        self.adjust_from_leaf(winner_idx);
    }
}

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
    stack: &mut MemStack,
) {
    let m = lhs.nrows();

    let (lhs_symbolic, lhs_values) = lhs.parts();
    let row_indices = lhs_symbolic.row_idx();

    // Calculate total workspace sizes needed
    let mut total_base_size = 0;
    let mut total_losers_size = 0;
    let mut thread_sizes = Vec::with_capacity(n_threads);

    for tid in 0..n_threads {
        let col_start = strategy.thread_cols[tid];
        let col_end = strategy.thread_cols[tid + 1];
        let k = 1 + col_end - col_start;
        let tree_size = k.next_power_of_two();

        thread_sizes.push(tree_size);
        total_base_size += tree_size;
        total_losers_size += tree_size;
    }

    let (mut all_base_workspace, stack) =
        stack.make_with::<Option<Contender<T>>>(total_base_size, |_| None);
    let (mut all_losers_workspace, _stack) = stack.make_with::<usize>(total_losers_size, |_| 0);

    let mut all_base_slice: &mut [Option<Contender<T>>] = all_base_workspace.as_mut();
    let mut all_losers_slice: &mut [usize] = all_losers_workspace.as_mut();

    thread::scope(|s| {
        let mut handles = Vec::with_capacity(n_threads);
        let core_ids = core_affinity::get_core_ids().unwrap();
        debug_assert!(core_ids.len() >= n_threads);

        for (tid, core_id) in (0..n_threads).zip(core_ids.into_iter()) {
            let tree_size = thread_sizes[tid];

            let (base_workspace, remaining_base) = all_base_slice.split_at_mut(tree_size);
            let (losers_workspace, remaining_losers) = all_losers_slice.split_at_mut(tree_size);

            all_base_slice = remaining_base;
            all_losers_slice = remaining_losers;

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

                // Initialize base array with first elements from each column
                for (local_col, (indices, values, _)) in slices.iter().enumerate() {
                    if indices.len() > 0 {
                        base_workspace[local_col] = Some(Contender {
                            row: indices[0].zx(),
                            val: values[0].clone(),
                            local_col,
                        });
                    }
                }

                let mut loser_tree = LoserTree::new(base_workspace, losers_workspace);

                let mut merge_ptrs: Vec<usize> = vec![1; k];
                // probably don't need this much capacity... but in pathological case we do
                let mut merged: Vec<(usize, T)> = Vec::with_capacity(m);

                // buffer the first entry so we can unwrap in hot loop
                if let Some(first_contender) = loser_tree.winner() {
                    let local_col = first_contender.local_col;

                    merged.push((
                        first_contender.row,
                        first_contender.val.mul_by_ref(&slices[local_col].2),
                    ));
                    merge_ptrs[local_col] += 1;
                    if slices[local_col].0.len() > 1 {
                        let replacement = Contender {
                            row: slices[local_col].0[1].zx(),
                            val: slices[local_col].1[1].clone(),
                            local_col,
                        };
                        loser_tree.push(Some(replacement));
                    } else {
                        loser_tree.push(None);
                    }

                    loop {
                        if let Some(contender) = loser_tree.winner() {
                            let local_col = contender.local_col;
                            let last = merged.last_mut().unwrap();
                            let val = contender.val.mul_by_ref(&slices[local_col].2);

                            if last.0 == contender.row {
                                last.1 = last.1.add_by_ref(&val);
                            } else {
                                merged.push((contender.row, val));
                            }

                            let current_idx = merge_ptrs[local_col];
                            if slices[local_col].0.len() > current_idx {
                                let replacement = Contender {
                                    row: slices[local_col].0[current_idx].zx(),
                                    val: slices[local_col].1[current_idx].clone(),
                                    local_col,
                                };
                                loser_tree.push(Some(replacement));
                            } else {
                                loser_tree.push(None);
                            }
                            merge_ptrs[local_col] += 1;
                        } else {
                            break;
                        }
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
