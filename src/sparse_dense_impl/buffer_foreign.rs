use crossbeam_channel::{Receiver, Sender, unbounded};
use crossbeam_queue::ArrayQueue;
use std::cmp::min;
use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::{sync::Arc, thread};

use faer::{
    Accum, ColMut, ColRef, Index, MatRef, Par,
    dyn_stack::{MemStack, StackReq},
    prelude::{Reborrow, ReborrowMut},
    sparse::SparseColMatRef,
    traits::{ComplexField, math_utils::zero},
};

use crate::spmv_drivers::SpMvStrategy;

// Cache dependent constants
//const B_ROWS: usize = 32 * 1024; // 32k rows ~ 256KB of y at f64
//const K_CAP: usize = 2048; // ~24KB chunk payload
const B_ROWS: usize = 16 * 1024;
const K_CAP: usize = 1024;

// Communication management constants
const WS_CHUNKS_PER_THREAD: usize = 1000; // how many chunks to put in the chunk pool
const CHUNK_BLOCK: usize = 10; // how many chunks to buffer in communication

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
                // TODO: probably should scale `WS_CHUNKS_PER_THREAD` using some function of nnz,
                // density, and dimensions?
                StackReq::new::<(usize, T)>(K_CAP * n_threads * WS_CHUNKS_PER_THREAD)
            }
        }
    }
}
/// A spill chunk. We batch contributions targeting a single row-block.
#[derive(Debug)]
struct Chunk<'a, T: ComplexField> {
    block_id: usize, // which row-block these pairs belong to
    storage: &'a mut [(usize, T)],
    len: usize,
}

impl<'a, T: ComplexField> Chunk<'a, T> {
    fn new(block_id: usize, storage: &'a mut [(usize, T)]) -> Self {
        // keep lengths in lockstep
        Self {
            block_id,
            storage,
            len: 0,
        }
    }
    #[inline]
    fn push(&mut self, row: usize, v: T) -> bool {
        self.storage[self.len] = (row, v);
        self.len += 1;
        self.len == K_CAP
    }
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
    #[inline]
    fn clear_reuse(&mut self, new_block: usize) {
        self.block_id = new_block;
        self.len = 0;
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

fn collect_chunks<'a, T: ComplexField>(
    chunk_recv: &Receiver<Vec<Box<Chunk<'a, T>>>>,
    owner_of_block: &Vec<usize>,
    scratch: &mut BlockScratch<T>,
    empty_chunks: &mut Vec<Chunk<'a, T>>,
    chunk_queue: &Arc<ArrayQueue<Chunk<'a, T>>>,
    mut dst_owned: ColMut<T>,
    row_start: usize,
    row_end: usize,
    tid: usize,
    m: usize,
    finished: bool,
) {
    let mut by_block: HashMap<usize, Vec<Box<Chunk<T>>>> = HashMap::new();
    if !finished {
        for chunks in chunk_recv.try_iter() {
            for ch in chunks {
                by_block.entry(ch.block_id).or_default().push(ch);
            }
        }
    } else {
        for chunks in chunk_recv.iter() {
            for ch in chunks {
                by_block.entry(ch.block_id).or_default().push(ch);
            }
        }
    }

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
            for (r, v) in chunk.storage.iter().take(chunk.len()) {
                let local_idx = r - base_row;
                debug_assert!(local_idx < block_len);
                scratch.add(local_idx, v.clone());
            }
            let chunk = *chunk;
            if empty_chunks.len() < CHUNK_BLOCK {
                empty_chunks.push(chunk);
            } else {
                // TODO: add chunks in block
                chunk_queue.push(chunk).unwrap();
            }
        }

        // Scatter reduced sums into y
        scratch.flush_into(dst_owned.rb_mut(), base_local);
    }
}

fn get_fresh_chunk<'a, T: ComplexField>(
    empty_chunks: &mut Vec<Chunk<'a, T>>,
    block_id: usize,
    chunk_queue: &Arc<ArrayQueue<Chunk<'a, T>>>,
) -> Box<Chunk<'a, T>> {
    match empty_chunks.pop() {
        Some(mut chunk) => {
            chunk.clear_reuse(block_id);
            Box::new(chunk)
        }
        None => {
            let mut maybe_chunk = None;
            while maybe_chunk.is_none() {
                // TODO: rm chunks in block
                maybe_chunk = chunk_queue.pop()
            }
            let mut chunk = maybe_chunk.unwrap();
            chunk.clear_reuse(block_id);
            Box::new(chunk)
        }
    }
}

fn hot_loop<'a, I: Index, T: ComplexField>(
    col_range: Range<usize>,
    row_indices: &[I],
    lhs_values: &[T],
    rhs_k: &T,
    owner_of_block: &Vec<usize>,
    tid: usize,
    row_start: usize,
    owned_rows: usize,
    mut dst_owned: ColMut<T>,
    open: &mut Vec<Option<Box<Chunk<'a, T>>>>,
    chunk_queue: &Arc<ArrayQueue<Chunk<'a, T>>>,
    full_chunks: &mut Vec<Vec<Box<Chunk<'a, T>>>>,
    empty_chunks: &mut Vec<Chunk<'a, T>>,
    txs_local: &Vec<Sender<Vec<Box<Chunk<'a, T>>>>>,
) {
    for idx in col_range {
        let i = row_indices[idx].zx();
        let lhs_ik = &lhs_values[idx];
        let contrib = lhs_ik.mul_by_ref(rhs_k);
        let block_id = i / B_ROWS;
        let owner = owner_of_block[block_id];

        if owner == tid {
            let local_idx = i - row_start;
            debug_assert!(local_idx < owned_rows);
            dst_owned[local_idx] = dst_owned[local_idx].add_by_ref(&contrib);
        } else {
            buffer_foreign(
                block_id,
                owner,
                open,
                i,
                contrib,
                chunk_queue,
                full_chunks,
                empty_chunks,
                txs_local,
            );
        }
    }
}

fn buffer_foreign<'a, T: ComplexField>(
    block_id: usize,
    owner: usize,
    open: &mut Vec<Option<Box<Chunk<'a, T>>>>,
    i: usize,
    contrib: T,
    chunk_queue: &Arc<ArrayQueue<Chunk<'a, T>>>,
    full_chunks: &mut Vec<Vec<Box<Chunk<'a, T>>>>,
    empty_chunks: &mut Vec<Chunk<'a, T>>,
    txs_local: &Vec<Sender<Vec<Box<Chunk<'a, T>>>>>,
) {
    match open[block_id] {
        Some(ref mut chunk) => {
            if chunk.push(i, contrib) {
                // full -> send and remove
                // NOTE: chunk consumed; replace with fresh
                let mut fresh = get_fresh_chunk(empty_chunks, block_id, &chunk_queue);

                std::mem::swap(&mut fresh, chunk);
                full_chunks[owner].push(fresh);
                if full_chunks[owner].len() == CHUNK_BLOCK {
                    let mut swap = Vec::with_capacity(CHUNK_BLOCK);
                    std::mem::swap(&mut swap, &mut full_chunks[owner]);
                    txs_local[owner].send(swap).unwrap();
                }
            }
        }
        None => {
            let mut chunk = get_fresh_chunk(empty_chunks, block_id, &chunk_queue);
            let _cant_be_full = chunk.push(i, contrib);
            open[block_id] = Some(chunk);
        }
    }
}

// NOTE: This merge based algorithm requires row indices to be in sorted order over columns, a soft
// invariant in faer.
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
    let (owner_of_block, row_ranges) = assign_blocks(m, B_ROWS, n_threads);
    let n_blocks = (m + B_ROWS - 1) / B_ROWS;

    let (mut array, _) = stack.make_with(K_CAP * n_threads * WS_CHUNKS_PER_THREAD, |_| {
        (0usize, T::zero_impl())
    });
    // TODO: add chunks in block
    let chunk_queue = Arc::new(ArrayQueue::new(n_threads * WS_CHUNKS_PER_THREAD));
    for storage in array.chunks_exact_mut(K_CAP) {
        let chunk = Chunk::new(0, storage);
        chunk_queue.push(chunk).expect("error building chunk store");
    }

    // Per-owner inbox: MPSC of spill chunks
    let mut txs = Vec::with_capacity(n_threads);
    let mut rxs = VecDeque::with_capacity(n_threads);
    for _ in 0..n_threads {
        let (tx, rx) = unbounded::<Vec<Box<Chunk<T>>>>();
        txs.push(tx);
        rxs.push_back(rx);
    }

    let (lhs_symbolic, lhs_values) = lhs.parts();
    let row_indices = lhs_symbolic.row_idx();

    thread::scope(|s| {
        let mut handles = Vec::with_capacity(n_threads);
        //let start_time = Instant::now();
        let core_ids = core_affinity::get_core_ids().unwrap();
        debug_assert!(core_ids.len() >= n_threads);
        for (tid, core_id) in (0..n_threads).zip(core_ids.into_iter()) {
            //for tid in 0..n_threads {
            let txs_local: Vec<Sender<Vec<Box<Chunk<T>>>>> = txs.iter().cloned().collect();
            let rx_owned = rxs.pop_front().unwrap();

            let (row_start, row_end) = row_ranges[tid];
            let owned_rows = row_end - row_start;

            let owner_of_block = owner_of_block.clone();
            let chunk_queue = chunk_queue.clone();
            let dst = dst.rb();
            let mut scratch = BlockScratch::new(B_ROWS);

            let handle = s.spawn(move || {
                let res = core_affinity::set_for_current(core_id);
                debug_assert!(res);
                // SAFETY: non-overlapping thread ownership of dst slice
                let mut dst_owned = unsafe { dst.subrows(row_start, owned_rows).const_cast() };
                if let Accum::Replace = beta {
                    dst_owned.fill(zero());
                }

                let mut empty_chunks = Vec::with_capacity(CHUNK_BLOCK);
                let mut full_chunks: Vec<Vec<Box<Chunk<T>>>> = (0..n_threads)
                    .map(|_| Vec::with_capacity(CHUNK_BLOCK))
                    .collect();
                // TODO: don't pull these off queue just initialize in loop
                for _ in 0..CHUNK_BLOCK {
                    let chunk = chunk_queue.pop().unwrap();
                    empty_chunks.push(chunk);
                }

                let mut open: Vec<Option<Box<Chunk<T>>>> = (0..n_blocks).map(|_| None).collect();

                let col_start = strategy.thread_cols[tid];
                let col_end = strategy.thread_cols[tid + 1];
                let idx_start = strategy.thread_indptrs[tid];
                let idx_end = strategy.thread_indptrs[tid + 1];

                for (iter, depth) in (col_start..=col_end).enumerate() {
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
                        &owner_of_block,
                        tid,
                        row_start,
                        owned_rows,
                        dst_owned.rb_mut(),
                        &mut open,
                        &chunk_queue,
                        &mut full_chunks,
                        &mut empty_chunks,
                        &txs_local,
                    );

                    if (iter + 1) * n_threads % WS_CHUNKS_PER_THREAD == 0 {
                        collect_chunks(
                            &rx_owned,
                            &owner_of_block,
                            &mut scratch,
                            &mut empty_chunks,
                            &chunk_queue,
                            dst_owned.rb_mut(),
                            row_start,
                            row_end,
                            tid,
                            m,
                            false,
                        );
                    }
                }

                // Flush partial (non-full) chunks
                for (block_id, maybe_chunk) in open.into_iter().enumerate() {
                    if let Some(chunk) = maybe_chunk {
                        if chunk.len() > 0 {
                            let owner = owner_of_block[block_id];
                            full_chunks[owner].push(chunk);
                        }
                    }
                }
                for (owner, chunks) in full_chunks.into_iter().enumerate() {
                    if !chunks.is_empty() {
                        txs_local[owner].send(chunks).unwrap();
                    }
                }
                drop(txs_local);

                collect_chunks(
                    &rx_owned,
                    &owner_of_block,
                    &mut scratch,
                    &mut empty_chunks,
                    &chunk_queue,
                    dst_owned.rb_mut(),
                    row_start,
                    row_end,
                    tid,
                    m,
                    true,
                );

                //Instant::now()
            });

            handles.push(handle);
        }
        drop(txs);

        /*
        let mut finish_times = Vec::with_capacity(n_threads);
        for handle in handles {
            let thread_finish_time = handle.join().expect("Thread panicked");
            println!("{:?}", thread_finish_time);
            finish_times.push(thread_finish_time);
        }
        let min_finish_duration = finish_times
            .iter()
            .map(|t| t.duration_since(start_time))
            .min()
            .unwrap();
        let max_finish_duration = finish_times
            .iter()
            .map(|t| t.duration_since(start_time))
            .max()
            .unwrap();

        let variation = max_finish_duration - min_finish_duration;
        println!("Min finish duration: {:?}", min_finish_duration);
        println!("Max finish duration: {:?}", max_finish_duration);
        println!("Variation (range) in finish times: {:?}", variation);
        */
    });
}
