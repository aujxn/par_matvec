use std::num::NonZero;
use faer::{
    dyn_stack::{MemBuffer, MemStack, StackReq}, linalg::{temp_mat_scratch, temp_mat_zeroed}, mat::AsMatMut, prelude::{Reborrow, ReborrowMut}, sparse::{linalg::matmul::sparse_dense_matmul as seq_sparse_dense, SparseColMatRef, SymbolicSparseColMatRef}, traits::{ComplexField, math_utils::zero}, Accum, ColMut, ColRef, Index, MatMut, MatRef, Par
};

// could probably replace with existing `SparseMatMulInfo` but may be less efficient since cannot
// store the associated columns (`log(ncols)` extra lookup complexity per thread each matvec)
pub struct SparseDenseStrategy {
    thread_cols: Vec<usize>,
    thread_indptrs : Vec<usize>
}

impl SparseDenseStrategy {
    fn new<I: Index>(mat: SymbolicSparseColMatRef<'_, I>, par: Par) -> Self {
        let (thread_cols, thread_indptrs) =
        match par {
            Par::Seq => (Vec::new(), Vec::new()),
            Par::Rayon(n_threads) => {
                let n_threads = n_threads.get();
                // use this instead?
                // let n_threads = par.degree();

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
                        thread_indptrs.extend((0..n_threads).map(|thread_id| thread_id * per_thread));
                        thread_indptrs.push(col_ptrs[ncols + 1].zx());
                        thread_cols.push(0);

                        let mut nnz_counter = 0;
                        let mut thread_id = 1;
                        for (col, (start, end)) in col_ptrs.iter().zip(col_ptrs.iter().skip(1)).enumerate() {
                            let col_nnz = end.zx() - start.zx();
                            nnz_counter += col_nnz;

                            while thread_id < n_threads && nnz_counter > thread_indptrs[thread_id] {
                                thread_cols.push(col);
                                thread_id += 1;
                            }
                        }
                        thread_cols.push(ncols);
                        assert_eq!(thread_cols.len(), n_threads + 1);
                    },
                    Some(_nnz_per_col) => {
                        unimplemented!();
                    }
                }
                (thread_cols, thread_indptrs)
            },
        };

        Self { thread_cols, thread_indptrs}
    }
}

fn sparse_dense_scratch<
    I: Index,
    T: ComplexField,
>(
    lhs: SparseColMatRef<'_, I, T>,
    rhs: MatRef<'_, T>,
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
                temp_mat_scratch::<T>(rhs.nrows(), n_threads)
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
fn sparse_dense_matmul<
    I: Index,
    T: ComplexField,
>(
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
                    par_sparse_matvec(dst, beta, lhs, rhs, &alpha, n_threads, strategy, stack);
                }
            }
        }
    }
}

//#[cfg(feature = "rayon")]
fn par_sparse_matvec<
    I: Index,
    T: ComplexField,
>(
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

    let job = &|tid: usize| {
        assert!(tid < n_threads);

        // SAFETY each thread gets its own workspace vector to be summed when all complete
        let mut work = unsafe { work.col(tid).const_cast().try_as_col_major_mut().unwrap() };
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
    };

    use rayon::prelude::*;

    (0..n_threads).into_par_iter().for_each(|tid| {
        job(tid);
    });

    if let Accum::Replace = beta {
        dst.fill(zero());
    }

    // TODO sum in parallel also
    //for (i, dst) in dst.par_iter_mut().enumerate() {
    for (i, dst) in dst.iter_mut().enumerate() {
        let work_row = work.row(i);
        *dst = dst.add_by_ref(&work_row.sum());
    }

}

fn bench() {
    let n_threads = NonZero::new(32).unwrap();
    let par = Par::Rayon(n_threads);

    let lhs_symbolic = ;
    let lhs = ;
    let rhs = ;
    let dst = ;

    let stack_req = sparse_dense_scratch(lhs, rhs, par);
    let stack = MemStack::new(&mut MemBuffer::try_new(stack_req).unwrap());
    let strategy = SparseDenseStrategy::new(lhs_symbolic, par);

    sparse_dense_matmul(dst, beta, lhs, rhs, alpha, par, &strategy, stack);
}
