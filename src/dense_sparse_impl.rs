use std::thread;

use faer::{
    Accum, Index, MatRef, Par, RowMut, RowRef,
    dyn_stack::{MemStack, StackReq},
    prelude::Reborrow,
    sparse::SparseColMatRef,
    traits::{ComplexField, math_utils::zero},
};

use crate::spmv_drivers::SpMvStrategy;

pub fn dense_sparse_scratch<I: Index, T: ComplexField>(
    lhs: MatRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    strategy: &SpMvStrategy,
    par: Par,
) -> StackReq {
    let _ = rhs;
    let _ = strategy;
    match par {
        Par::Seq => StackReq::empty(),
        Par::Rayon(n_threads) => {
            let dim = lhs.nrows();
            let n_threads = n_threads.get();
            if dim >= n_threads * 4 {
                StackReq::empty()
            } else {
                // TODO: actually use ws in impl...
                StackReq::new::<T>(n_threads * 2)
            }
        }
    }
}

pub fn par_dense_sparse<I: Index, T: ComplexField>(
    dst: RowMut<'_, T>,
    beta: Accum,
    lhs: RowRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    alpha: &T,
    n_threads: usize,
    strategy: &SpMvStrategy,
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
