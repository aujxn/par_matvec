use faer::{
    Accum, ColMut, ColRef, Index, MatMut, MatRef, Par, RowMut, RowRef,
    dyn_stack::MemStack,
    sparse::{
        SparseColMatRef, SymbolicSparseColMatRef,
        linalg::matmul::{
            dense_sparse_matmul as seq_dense_sparse, sparse_dense_matmul as seq_sparse_dense,
        },
    },
    traits::ComplexField,
};

pub struct SpMvStrategy {
    pub thread_cols: Vec<usize>,
    pub thread_indptrs: Vec<usize>,
}

impl SpMvStrategy {
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

type SparseDenseImpl<I, T> = fn(
    dst: ColMut<'_, T>,
    beta: Accum,
    lhs: SparseColMatRef<'_, I, T>,
    rhs: ColRef<'_, T>,
    alpha: &T,
    n_threads: usize,
    strategy: &SpMvStrategy,
    stack: &mut MemStack,
);

type DenseSparseImpl<I, T> = fn(
    dst: RowMut<'_, T>,
    beta: Accum,
    lhs: RowRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    alpha: &T,
    n_threads: usize,
    strategy: &SpMvStrategy,
    stack: &mut MemStack,
);

pub fn sparse_dense_matmul<I: Index, T: ComplexField>(
    dst: MatMut<'_, T>,
    beta: Accum,
    lhs: SparseColMatRef<'_, I, T>,
    rhs: MatRef<'_, T>,
    alpha: T,
    par: Par,
    strategy: &SpMvStrategy,
    stack: &mut MemStack,
    par_impl: Option<SparseDenseImpl<I, T>>,
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
                    let par_spmv =
                        par_impl.expect("Can't do parallel SpMV without providing an impl");
                    par_spmv(dst, beta, lhs, rhs, &alpha, n_threads, strategy, stack);
                }
            }
        }
    }
}

pub fn dense_sparse_matmul<I: Index, T: ComplexField>(
    dst: MatMut<'_, T>,
    beta: Accum,
    lhs: MatRef<'_, T>,
    rhs: SparseColMatRef<'_, I, T>,
    alpha: T,
    par: Par,
    strategy: &SpMvStrategy,
    stack: &mut MemStack,
    par_impl: Option<DenseSparseImpl<I, T>>,
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
                    let par_spmv =
                        par_impl.expect("Can't do parallel SpMV without providing an impl");
                    par_spmv(dst, beta, lhs, rhs, &alpha, n_threads, strategy, stack);
                }
            }
        }
    }
}
