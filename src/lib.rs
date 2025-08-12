use crossbeam_channel::{Receiver, Sender, unbounded};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::{sync::Arc, thread};

use faer::{
    Accum, ColMut, ColRef, Index, MatMut, MatRef, Par, RowMut, RowRef,
    dyn_stack::{MemStack, StackReq},
    linalg::{temp_mat_scratch, temp_mat_zeroed},
    mat::AsMatMut,
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

/*
struct MergeBuffer<T: ComplexField> {
    pub old_buffer: Vec<(usize, T)>,
    pub work_buffer: Vec<(usize, T)>,
    pub i: usize,
}

impl<T: ComplexField> MergeBuffer<T> {
    // TODO: figure out how to construct from MemStack
    fn new(capacity: usize) -> Self {
        Self {
            old_buffer: Vec::with_capacity(capacity),
            work_buffer: Vec::with_capacity(capacity),
            i: 0,
        }
    }

    //#[inline]
    fn push(&mut self, idx: usize, val: T) {
        // Move elements from old_buffer into work_buffer while they're smaller than idx
        while self.i < self.old_buffer.len() && self.old_buffer[self.i].0 < idx {
            self.work_buffer.push(self.old_buffer[self.i].clone());
            self.i += 1;
        }

        // Handle the case where old_buffer has the same index
        if self.i < self.old_buffer.len() && self.old_buffer[self.i].0 == idx {
            let combined_val = self.old_buffer[self.i].1.add_by_ref(&val);
            self.work_buffer.push((idx, combined_val));
            self.i += 1;
        } else {
            // Just add the current element (no duplicates possible)
            self.work_buffer.push((idx, val));
        }
    }

    //#[inline]
    fn reset(&mut self) {
        // Finish moving any remaining elements from old_buffer
        while self.i < self.old_buffer.len() {
            self.work_buffer.push(self.old_buffer[self.i].clone());
            self.i += 1;
        }

        // Swap buffers and reset index
        std::mem::swap(&mut self.old_buffer, &mut self.work_buffer);
        self.work_buffer.clear();
        self.i = 0;
    }
}
*/

/// A spill chunk. We batch contributions targeting a single row-block.
struct Chunk<T: ComplexField> {
    block_id: usize,  // which row-block these pairs belong to
    rows: Vec<usize>, // absolute row indices
    vals: Vec<T>,
}

impl<T: ComplexField> Chunk<T> {
    fn with_capacity(block_id: usize, cap: usize) -> Self {
        let mut rows = Vec::with_capacity(cap);
        let mut vals = Vec::with_capacity(cap);
        // keep lengths in lockstep
        rows.clear();
        vals.clear();
        Self {
            block_id,
            rows,
            vals,
        }
    }
    #[inline]
    fn push(&mut self, row: usize, v: T) -> bool {
        self.rows.push(row);
        self.vals.push(v);
        self.rows.len() == self.rows.capacity()
    }
    #[inline]
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn clear_reuse(&mut self, new_block: usize) {
        self.block_id = new_block;
        // keep capacity; just reset length
        unsafe {
            self.rows.set_len(0);
            self.vals.set_len(0);
        }
    }
}

/// Assign contiguous blocks to owners (threads) as evenly as possible.
/// Returns: owner_of_block[b] and for each owner a (row_start,row_end) pair to slice `y`.
fn assign_blocks(
    nrows: usize,
    block_rows: usize,
    threads: usize,
) -> (Vec<usize>, Vec<(usize, usize)>) {
    let num_blocks = (nrows + block_rows - 1) / block_rows;
    let mut owner_of_block = vec![0usize; num_blocks];

    let blocks_per_owner = (num_blocks + threads - 1) / threads;
    let mut row_ranges = Vec::with_capacity(threads);

    for t in 0..threads {
        let b0 = t * blocks_per_owner;
        let b1 = min(num_blocks, (t + 1) * blocks_per_owner);
        for b in b0..b1 {
            owner_of_block[b] = t;
        }
        let row_start = min(nrows, b0 * block_rows);
        let row_end = min(nrows, b1 * block_rows);
        row_ranges.push((row_start, row_end));
    }
    (owner_of_block, row_ranges)
}

/// Owner-side reducer scratch for a single block, using a versioned-visited trick
/// to avoid clearing O(B) memory between blocks.
///
/// acc: accumulated sums for indices within the block (0..B)
/// seen: marks indices that are live under current epoch
struct BlockScratch<T: ComplexField> {
    acc: Vec<T>,
    seen_epoch: Vec<u32>,
    touched: Vec<usize>,
    epoch: u32,
}

impl<T: ComplexField> BlockScratch<T> {
    fn new(block_rows: usize) -> Self {
        Self {
            acc: vec![T::zero_impl(); block_rows],
            seen_epoch: vec![0u32; block_rows],
            touched: Vec::with_capacity(block_rows / 16),
            epoch: 1,
        }
    }
    #[inline]
    fn start_block(&mut self) {
        // bump epoch; if wraps, reset seen_epoch (rare)
        self.epoch = self.epoch.wrapping_add(1);
        if self.epoch == 0 {
            self.seen_epoch.fill(0);
            self.epoch = 1;
        }
        self.touched.clear();
    }
    #[inline]
    fn add(&mut self, local_idx: usize, val: T) {
        if self.seen_epoch[local_idx] != self.epoch {
            self.seen_epoch[local_idx] = self.epoch;
            self.acc[local_idx] = val;
            self.touched.push(local_idx);
        } else {
            self.acc[local_idx] = self.acc[local_idx].add_by_ref(&val);
        }
    }
    #[inline]
    fn flush_into(&mut self, mut y_owned: ColMut<T>, base_local: usize) {
        // y_owned corresponds to the owner’s full row range; base_local is the
        // offset within y_owned where this block begins.
        for &idx in &self.touched {
            y_owned[base_local + idx] = y_owned[base_local + idx].add_by_ref(&self.acc[idx]);
        }
    }
}

const B_ROWS: usize = 32 * 1024; // 32k rows ~ 256KB of y at f64
const K_CAP: usize = 2048; // ~24KB chunk payload
//
// NOTE: This merge based algorithm requires row indices to be in sorted order over columns, a soft
// invariant in faer
fn par_sparse_dense<I: Index, T: ComplexField>(
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
    let (owner_of_block, row_ranges) = assign_blocks(m, B_ROWS, n_threads);
    let n_blocks = (m + B_ROWS - 1) / B_ROWS;

    // Per-owner inbox: MPSC of spill chunks
    let mut txs: Vec<Sender<Box<Chunk<T>>>> = Vec::with_capacity(n_threads);
    let mut rxs: Vec<Receiver<Box<Chunk<T>>>> = Vec::with_capacity(n_threads);
    for _ in 0..n_threads {
        let (tx, rx) = unbounded::<Box<Chunk<T>>>();
        txs.push(tx);
        rxs.push(rx);
    }

    let (lhs_symbolic, lhs_values) = lhs.parts();
    let row_indices = lhs_symbolic.row_idx();

    thread::scope(|s| {
        for tid in 0..n_threads {
            let txs_local: Vec<Sender<Box<Chunk<T>>>> = txs.iter().cloned().collect();
            let rx_owned = rxs[tid].clone();

            let (row_start, row_end) = row_ranges[tid];
            let owned_rows = row_end - row_start;

            let owner_of_block = owner_of_block.clone();
            let dst = dst.rb();

            let _handle = s.spawn(move || {
                // SAFETY: non-overlapping thread ownership of dst slice
                let mut dst_owned = unsafe { dst.subrows(row_start, owned_rows).const_cast() };
                if let Accum::Replace = beta {
                    dst_owned.fill(zero());
                }

                let mut open: Vec<Option<Box<Chunk<T>>>> = (0..n_blocks).map(|_| None).collect();

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
                        let contrib = lhs_ik.mul_by_ref(&rhs_k);
                        let block_id = i / B_ROWS;
                        let owner = owner_of_block[block_id];

                        if owner == tid {
                            let local_idx = i - row_start;
                            debug_assert!(local_idx < owned_rows);
                            dst_owned[local_idx] = dst_owned[local_idx].add_by_ref(&contrib);
                        } else {
                            // Foreign: append to the chunk for block b
                            match open[block_id] {
                                Some(ref mut chunk) => {
                                    if chunk.push(i, contrib) {
                                        // full -> send and remove
                                        // NOTE: ch consumed; replace with fresh
                                        let mut fresh =
                                            Box::new(Chunk::with_capacity(block_id, K_CAP));
                                        fresh.rows.reserve_exact(K_CAP);
                                        fresh.vals.reserve_exact(K_CAP);
                                        std::mem::swap(&mut fresh, chunk);
                                        txs_local[owner].send(fresh).unwrap();
                                    }
                                }
                                None => {
                                    let mut chunk = Box::new(Chunk::with_capacity(block_id, K_CAP));
                                    chunk.rows.reserve_exact(K_CAP);
                                    chunk.vals.reserve_exact(K_CAP);
                                    let _cant_be_full = chunk.push(i, contrib);
                                    open[block_id] = Some(chunk);
                                }
                            }
                        }
                    }
                }

                // Flush partial (non-full) chunks
                for (block_id, maybe_chunk) in open.into_iter().enumerate() {
                    if let Some(chunk) = maybe_chunk {
                        if chunk.len() > 0 {
                            let owner = owner_of_block[block_id];
                            txs_local[owner].send(chunk).unwrap();
                        }
                    }
                }
                drop(txs_local);

                //barrier.wait();

                let mut by_block: HashMap<usize, Vec<Box<Chunk<T>>>> = HashMap::new();
                for ch in rx_owned.iter() {
                    by_block.entry(ch.block_id).or_default().push(ch);
                }

                let mut scratch = BlockScratch::new(B_ROWS);

                // Process each owned block
                for (block_id, chunks) in by_block.into_iter() {
                    // This block must be owned by tid.
                    debug_assert_eq!(owner_of_block[block_id], tid);
                    scratch.start_block();

                    let base_row = block_id * B_ROWS;
                    let block_len = if base_row + B_ROWS <= row_end {
                        B_ROWS
                    } else {
                        // tail block may be shorter
                        m - base_row
                    };
                    let base_local = base_row - row_start; // offset into y_owned

                    // Accumulate all contributions for this block
                    for chunk in chunks {
                        debug_assert_eq!(chunk.block_id, block_id);
                        for i in 0..chunk.rows.len() {
                            let r = chunk.rows[i] as usize;
                            let v = chunk.vals[i].clone();
                            let local_idx = r - base_row;
                            debug_assert!(local_idx < block_len);
                            scratch.add(local_idx, v);
                        }
                        // Box<Chunk> drops here; memory returned to the allocator.
                    }

                    // Scatter reduced sums into y
                    scratch.flush_into(dst_owned.rb_mut(), base_local);
                }
            });
        }
        drop(txs);
    });
}

/*
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

    let mut dst = dst;

    thread::scope(|s| {
        for tid in 0..n_threads {
            let arc_dst = arc_dst.clone();
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
    });
}
*/
