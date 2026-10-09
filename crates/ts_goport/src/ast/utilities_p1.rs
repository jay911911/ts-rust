//! Port of typescript-go `internal/ast/utilities.go` lines 1-905.

use crate::prelude::*;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock, PoisonError};

// Atomic ids

// PORT: Go keeps `nextNodeId`/`nextSymbolId` as process-wide atomics and
// stores the lazily assigned id on the node or symbol. Our AST and symbol
// arena entries have no id slot. Symbol ids come from one process counter,
// as in Go (`next_symbol_id`), and live in id tables (`get_symbol_id`).
// Node ids stay per thread, in thread-local maps keyed by handle: no output
// that we compare holds one.
thread_local! {
    static NEXT_NODE_ID: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    // Forgets the ids of dead file versions (`PerFileMap`, lsshells M3a).
    // Their nodes are not read again, and an id is never given twice: a
    // node of a dead version that has no id here panics (`get_node_id`).
    static NODE_IDS: RefCell<PerFileMap<u64>> = const { RefCell::new(PerFileMap::new()) };
    // The symbol ids that this thread gave (`next_symbol_id`), for
    // `next_ids`.
    static SYMBOL_IDS_GIVEN_HERE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    // The ids of the symbols that checker arenas add, by arena.
    static OWN_SYMBOL_IDS: RefCell<OwnSymbolIds> = const { RefCell::new(OwnSymbolIds::new()) };
}

/// Go `nextSymbolId` (ast/utilities.go:19): one counter for the process.
/// Nothing resets it: not a program, a `tsc -b` project, a language server
/// session, a watch cycle or a test. So with more than one checker, as in
/// Go, the ids that the checkers give race, and the digits of a late-bound
/// name (`__@k@<id>`, which the node builder counts toward truncation)
/// depend on thread timing.
// PERF: alone in its 128 bytes, so no other value shares its cache line.
#[repr(align(128))]
struct SymbolIdCounter(AtomicU64);

static NEXT_SYMBOL_ID: SymbolIdCounter = SymbolIdCounter(AtomicU64::new(0));

/// Go `nextSymbolId.Add(1)`. Relaxed: the id carries no other data, and the
/// ids of one thread still go up.
#[inline(always)]
fn next_symbol_id() -> u64 {
    SYMBOL_IDS_GIVEN_HERE.with(|given| given.set(given.get() + 1));
    NEXT_SYMBOL_ID.0.fetch_add(1, Ordering::Relaxed) + 1
}

/// `GOPORT_SYMCOUNT=1`: the ids given so far, by kind
/// (`print_symbol_id_counts`). Not in Go: the trunc2 counting build of Go
/// prints the same totals.
#[derive(Default)]
struct IdCounts {
    /// Binder lineage symbols (Go: symbols without `SymbolFlagsTransient`).
    lineage: AtomicU64,
    /// Symbols that a checker made (Go: transient symbols).
    own: AtomicU64,
    /// Ids lost to a race on a lineage symbol (Go: a failed
    /// `CompareAndSwap`).
    lost: AtomicU64,
}

static ID_COUNTS: LazyLock<Option<IdCounts>> =
    LazyLock::new(|| std::env::var_os("GOPORT_SYMCOUNT").map(|_| IdCounts::default()));

/// Adds one to the count that `kind` picks, when `GOPORT_SYMCOUNT` is set.
#[inline]
fn count_id(kind: fn(&IdCounts) -> &AtomicU64) {
    if let Some(counts) = &*ID_COUNTS {
        kind(counts).fetch_add(1, Ordering::Relaxed);
    }
}

/// `GOPORT_SYMCOUNT=1`: prints the symbol id counts of the process on
/// stderr, as `SYMDBG exit next=<ids given> lineage=<n> own=<n>
/// casfail=<n>`. The binaries call it as they exit.
pub fn print_symbol_id_counts() {
    if let Some(counts) = &*ID_COUNTS {
        use std::io::Write as _;
        let _ = writeln!(
            std::io::stderr(),
            "SYMDBG exit next={} lineage={} own={} casfail={}",
            NEXT_SYMBOL_ID.0.load(Ordering::Relaxed),
            counts.lineage.load(Ordering::Relaxed),
            counts.own.load(Ordering::Relaxed),
            counts.lost.load(Ordering::Relaxed),
        );
    }
}

/// A symbol id as an own id table keeps it: 0 for no id yet (Go ids start
/// at 1), `BIG_ID` for an id of `BIG_ID` or more (in the table's `big`
/// map), else the id.
// PERF: 4 bytes per symbol. Each checker gives an id to most symbols that it
// reads (Go `GetSymbolId` in `symbolArenaLinkStore`), up to 2 million of its
// own on zod, so a CLI run stays far below `BIG_ID`.
type IdCell = u32;
const BIG_ID: IdCell = IdCell::MAX;

/// The id in `cell`, assigned now when it has none. `big` keeps ids from
/// `BIG_ID` on, by `place`.
// PERF: always inline, as the id reads inlined it before trunc2. Out of
// line (the shared counter made it larger), its call showed in profiles of
// effect --singleThreaded (trunc2 round 2). Ids from `BIG_ID` on go out of
// line.
#[inline(always)]
fn cell_id<K: Eq + std::hash::Hash>(
    cell: &mut IdCell,
    big: &mut FxHashMap<K, u64>,
    place: K,
) -> u64 {
    match *cell {
        0 => {
            let id = next_symbol_id();
            count_id(|counts| &counts.own);
            match IdCell::try_from(id) {
                Ok(small) if small != BIG_ID => *cell = small,
                _ => set_big_id(cell, big, place, id),
            }
            id
        }
        BIG_ID => big_id(big, &place),
        id => u64::from(id),
    }
}

/// `cell_id` of an id from `BIG_ID` on: keeps it in `big`.
#[cold]
#[inline(never)]
fn set_big_id<K: Eq + std::hash::Hash>(
    cell: &mut IdCell,
    big: &mut FxHashMap<K, u64>,
    place: K,
    id: u64,
) {
    *cell = BIG_ID;
    big.insert(place, id);
}

/// `cell_id` of a cell that holds `BIG_ID`: the id in `big`.
#[cold]
#[inline(never)]
fn big_id<K: Eq + std::hash::Hash>(big: &FxHashMap<K, u64>, place: &K) -> u64 {
    big[place]
}

/// The symbols of one `SymbolArena` chunk (core.rs `COW_CHUNK_LEN`), so a
/// freed lineage range frees whole id chunks. A test checks that the two
/// agree (`lineage_id_chunks_are_symbol_chunks`).
// PORT: not shared with core.rs, which is a lib blob key.
const LINEAGE_ID_SHIFT: usize = 8;
const LINEAGE_ID_CHUNK: usize = 1 << LINEAGE_ID_SHIFT;

/// The id cells of the lineage symbols of one symbol chunk.
type IdChunk = [AtomicU64; LINEAGE_ID_CHUNK];

/// `LineageIdTable` pages: a symbol index is a `u32`, so there are 2^24
/// symbol chunks, in 64 pages of 2^18 (1 MiB each, made on first use).
const PAGE_SHIFT: usize = 18;
const PAGE_LEN: usize = 1 << PAGE_SHIFT;
const PAGES: usize = (1 << (32 - LINEAGE_ID_SHIFT)) >> PAGE_SHIFT;

/// `LineageIdTable` chunk buckets: bucket `b` holds `FIRST_BUCKET << b`
/// chunks, so the buckets made are at most twice the chunks used.
const FIRST_BUCKET_SHIFT: usize = 4;
const FIRST_BUCKET: usize = 1 << FIRST_BUCKET_SHIFT;
const BUCKETS: usize = 32 - LINEAGE_ID_SHIFT - FIRST_BUCKET_SHIFT + 1;

/// Go `ast.Symbol.id` of the binder lineage symbols (`program.rs`). Go
/// keeps the id on the `*ast.Symbol`, which every checker shares. Here a
/// lineage index names one binder symbol in every arena that is or copies
/// the lineage (`SymbolArena::id_slot`, key 0), and those arenas share this
/// table (`SymbolArena::lineage_ids`). A cell is 0 until its symbol has an
/// id. The first thread to give one wins a compare-exchange, as in Go
/// `GetSymbolId`, and a thread that loses takes the winner's id (Go: "we
/// burn a few ids").
///
/// The cells are in chunks of the symbols of one symbol chunk, made on the
/// first id in the chunk. When the lineage frees the symbols of a dead file
/// version (`free`), their chunks go to a free list. Lineage indexes are not
/// used again, and an id read of a freed one panics.
pub(crate) struct LineageIdTable {
    /// By symbol chunk (`index >> LINEAGE_ID_SHIFT`), in pages of
    /// `PAGE_LEN`: 0 for no id chunk, else 1 + the place of the id chunk in
    /// `chunks`.
    pages: [OnceLock<Box<[AtomicU32]>>; PAGES],
    /// The id chunks by place, in buckets (`FIRST_BUCKET`).
    chunks: [OnceLock<Box<[IdChunk]>>; BUCKETS],
    /// Makes and frees id chunks. A thread writes `pages` only while it
    /// holds it.
    alloc: Mutex<ChunkAlloc>,
}

/// The id chunks of a `LineageIdTable` that are not in use, and its freed
/// symbol chunks.
#[derive(Default)]
struct ChunkAlloc {
    /// The first place that was never used.
    next: u32,
    /// Places of freed id chunks.
    free: Vec<u32>,
    /// The freed symbol chunks, one bit each.
    freed: Vec<u64>,
    /// The freed ranges, for the panic of a read (`freed_read`).
    ranges: Vec<FreedLineage>,
}

/// A lineage range that `program::Lineage` freed: the symbol chunks from
/// `first` to `end` of file version `file`.
#[derive(Clone, Copy, Debug)]
struct FreedLineage {
    file: usize,
    first: u32,
    end: u32,
}

impl std::fmt::Debug for LineageIdTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let alloc = self.alloc.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_struct("LineageIdTable")
            .field("chunks", &alloc.next)
            .field("free", &alloc.free.len())
            .finish_non_exhaustive()
    }
}

impl LineageIdTable {
    pub(crate) fn new() -> Self {
        LineageIdTable {
            pages: std::array::from_fn(|_| OnceLock::new()),
            chunks: std::array::from_fn(|_| OnceLock::new()),
            alloc: Mutex::default(),
        }
    }

    /// Go `GetSymbolId` of lineage index `index`.
    #[inline]
    pub(crate) fn id(&self, index: usize) -> u64 {
        match self.cell(index) {
            Some(cell) => match cell.load(Ordering::Relaxed) {
                0 => give_lineage_id(cell),
                id => id,
            },
            None => self.id_in_new_chunk(index),
        }
    }

    /// The cell of `index`, when its id chunk is made.
    #[inline]
    fn cell(&self, index: usize) -> Option<&AtomicU64> {
        let chunk = index >> LINEAGE_ID_SHIFT;
        let page = self.pages[chunk >> PAGE_SHIFT].get()?;
        match page[chunk & (PAGE_LEN - 1)].load(Ordering::Acquire) {
            0 => None,
            entry => Some(&self.chunk(entry - 1)[index & (LINEAGE_ID_CHUNK - 1)]),
        }
    }

    /// The id chunk at `place`.
    #[inline]
    fn chunk(&self, place: u32) -> &IdChunk {
        let n = place as usize + FIRST_BUCKET;
        let bucket = n.ilog2() as usize - FIRST_BUCKET_SHIFT;
        let chunks = self.chunks[bucket]
            .get()
            .expect("an id chunk is made before its place is published");
        &chunks[n - (FIRST_BUCKET << bucket)]
    }

    /// `id` when the id chunk of `index` is not made: makes it, or panics
    /// when the lineage freed the symbols of the chunk.
    #[cold]
    #[inline(never)]
    fn id_in_new_chunk(&self, index: usize) -> u64 {
        let chunk = index >> LINEAGE_ID_SHIFT;
        let mut alloc = self.alloc.lock().unwrap_or_else(PoisonError::into_inner);
        if alloc
            .freed
            .get(chunk / 64)
            .is_some_and(|bits| bits & (1 << (chunk % 64)) != 0)
        {
            let file = alloc.freed_file(chunk);
            drop(alloc);
            crate::ast::file_version::released(file);
        }
        let page = self.pages[chunk >> PAGE_SHIFT]
            .get_or_init(|| (0..PAGE_LEN).map(|_| AtomicU32::new(0)).collect());
        let entry = &page[chunk & (PAGE_LEN - 1)];
        // Another thread can have made it since `cell`.
        let mut place = entry.load(Ordering::Relaxed);
        if place == 0 {
            place = self.take_chunk(&mut alloc) + 1;
            entry.store(place, Ordering::Release);
        }
        drop(alloc);
        let cell = &self.chunk(place - 1)[index & (LINEAGE_ID_CHUNK - 1)];
        match cell.load(Ordering::Relaxed) {
            0 => give_lineage_id(cell),
            id => id,
        }
    }

    /// A free id chunk with no ids, and its place.
    fn take_chunk(&self, alloc: &mut ChunkAlloc) -> u32 {
        if let Some(place) = alloc.free.pop() {
            for cell in self.chunk(place) {
                cell.store(0, Ordering::Relaxed);
            }
            return place;
        }
        let place = alloc.next;
        alloc.next = place.checked_add(1).expect("symbol id chunk overflow");
        let n = place as usize + FIRST_BUCKET;
        let bucket = n.ilog2() as usize - FIRST_BUCKET_SHIFT;
        self.chunks[bucket].get_or_init(|| {
            (0..FIRST_BUCKET << bucket)
                .map(|_| std::array::from_fn(|_| AtomicU64::new(0)))
                .collect()
        });
        place
    }

    /// Frees the ids of the symbols at lineage indexes `symbols` of file
    /// version `file`, whose ends are chunk starts
    /// (`SymbolArena::free_range`). Their id chunks go to the free list, and
    /// a later id read of one of them panics.
    // PORT: a thread that read an id chunk before the free can still write
    // an id into it after the chunk is used again. Only a read of a dead
    // version's symbol does that, which nothing does once the lineage frees
    // it, and the id that it writes is still unique.
    pub(crate) fn free(&self, file: usize, symbols: std::ops::Range<usize>) {
        let chunk = |chunk: usize| u32::try_from(chunk).expect("symbol overflow");
        // Only whole chunks, which the range is (`SymbolArena::end_chunk`).
        let first = chunk(symbols.start.div_ceil(LINEAGE_ID_CHUNK));
        let end = chunk(symbols.end >> LINEAGE_ID_SHIFT);
        if first >= end {
            return;
        }
        let mut alloc = self.alloc.lock().unwrap_or_else(PoisonError::into_inner);
        alloc.ranges.push(FreedLineage { file, first, end });
        for chunk in first as usize..end as usize {
            if chunk / 64 >= alloc.freed.len() {
                alloc.freed.resize(chunk / 64 + 1, 0);
            }
            alloc.freed[chunk / 64] |= 1 << (chunk % 64);
            let Some(page) = self.pages[chunk >> PAGE_SHIFT].get() else {
                continue;
            };
            let place = page[chunk & (PAGE_LEN - 1)].swap(0, Ordering::Relaxed);
            if place != 0 {
                alloc.free.push(place - 1);
            }
        }
    }
}

impl ChunkAlloc {
    /// The file version whose freed range holds symbol chunk `chunk`.
    fn freed_file(&self, chunk: usize) -> usize {
        self.ranges
            .iter()
            .find(|range| (range.first as usize..range.end as usize).contains(&chunk))
            .map(|range| range.file)
            .expect("a freed symbol chunk is in a freed range")
    }
}

/// Gives the lineage symbol of `cell` its id: Go `GetSymbolId` after the
/// load, a new id and a compare-exchange. A thread that loses the race takes
/// the winner's id, and its own id is lost.
fn give_lineage_id(cell: &AtomicU64) -> u64 {
    let id = next_symbol_id();
    match cell.compare_exchange(0, id, Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => {
            count_id(|counts| &counts.lineage);
            id
        }
        Err(winner) => {
            count_id(|counts| &counts.lost);
            winner
        }
    }
}

/// The ids of the symbols that checker arenas add on one thread, by arena
/// key. Such a symbol has an index that other checkers and later binds use
/// for other symbols, so its id cannot live in the lineage table. Only the
/// thread that owns a checker arena reads its own symbols. A checker worker
/// has one arena; a thread that runs checkers of several programs (the
/// language server's) switches between them.
struct OwnSymbolIds {
    /// The arena whose ids are in `ids`, or 0.
    key: u32,
    /// By place among the arena's own symbols (`SymbolIds::own_place`), as
    /// `IdCell`s. The places are dense until a catch-up of the arena passes
    /// its own tail. After that, each odd chunk that the lineage passed is a
    /// gap of 256 places (1 KiB) here.
    ids: Vec<IdCell>,
    /// The ids of the other arenas.
    others: FxHashMap<u32, Vec<IdCell>>,
    /// The ids from `BIG_ID` on, by arena and place.
    big: FxHashMap<(u32, usize), u64>,
    /// The arenas whose ids stay after the arena drops
    /// (`keep_own_symbol_ids`).
    kept: Vec<u32>,
}

impl OwnSymbolIds {
    const fn new() -> Self {
        Self {
            key: 0,
            ids: Vec::new(),
            others: FxHashMap::with_hasher(rustc_hash::FxBuildHasher),
            big: FxHashMap::with_hasher(rustc_hash::FxBuildHasher),
            kept: Vec::new(),
        }
    }

    /// The id of the own symbol at `place` of arena `key`, assigned now when
    /// it has none.
    #[inline]
    fn id(&mut self, key: u32, place: usize) -> u64 {
        if self.key != key {
            self.switch_to(key);
        }
        match self.ids.get_mut(place) {
            Some(cell) => cell_id(cell, &mut self.big, (key, place)),
            None => self.id_past_end(key, place),
        }
    }

    /// `id` for a place past the end of `ids`: grows it to twice its length
    /// or more (zero cells have no id), so a run of new symbols grows it
    /// rarely.
    #[cold]
    #[inline(never)]
    fn id_past_end(&mut self, key: u32, place: usize) -> u64 {
        let len = (place + 1).max(self.ids.len() * 2).max(LINEAGE_ID_CHUNK);
        self.ids.resize(len, 0);
        cell_id(&mut self.ids[place], &mut self.big, (key, place))
    }

    /// Makes arena `key` the one in `ids`.
    #[cold]
    #[inline(never)]
    fn switch_to(&mut self, key: u32) {
        let ids = self.others.remove(&key).unwrap_or_default();
        let previous = std::mem::replace(&mut self.ids, ids);
        if self.key != 0 {
            self.others.insert(self.key, previous);
        }
        self.key = key;
    }
}

/// Frees this thread's ids of the own symbols of arena `key`. A checker
/// arena calls it when it drops. The ids of a kept arena stay.
pub(crate) fn forget_own_symbol_ids(key: u32) {
    // The thread may be ending, after its thread-locals.
    let _ = OWN_SYMBOL_IDS.try_with(|own| {
        let mut own = own.borrow_mut();
        if own.kept.contains(&key) {
            return;
        }
        if own.key == key {
            own.key = 0;
            own.ids = Vec::new();
        } else {
            own.others.remove(&key);
        }
        if !own.big.is_empty() {
            own.big.retain(|&(arena, _), _| arena != key);
        }
    });
}

/// Keeps this thread's ids of the own symbols of arena `key` after the
/// arena drops. A shadow of one of those symbols in another arena
/// (`SymbolArena::push_shadow`) keeps its id there, as Go keeps the id on
/// the symbol for as long as any checker holds it.
pub(crate) fn keep_own_symbol_ids(key: u32) {
    OWN_SYMBOL_IDS.with(|own| {
        let mut own = own.borrow_mut();
        if !own.kept.contains(&key) {
            own.kept.push(key);
        }
    });
}

/// The node ids of one thread (see `id_seed`).
#[derive(Clone, Debug, Default)]
pub struct IdSeed {
    next_node_id: u64,
    node_ids: PerFileMap<u64>,
}

/// The node ids assigned on this thread so far. A checker worker starts
/// from the ids of the loading thread (`install_id_seed`), so a node that
/// got an id before the checkers started keeps it on every thread. Symbol
/// ids need no seed: every thread shares the counter (`next_symbol_id`) and
/// the lineage ids (`LineageIdTable`).
#[must_use]
pub fn id_seed() -> IdSeed {
    IdSeed {
        next_node_id: NEXT_NODE_ID.with(std::cell::Cell::get),
        node_ids: NODE_IDS.with(|ids| {
            let mut ids = ids.borrow_mut();
            // The copy has no ids of dead file versions.
            ids.write();
            ids.clone()
        }),
    }
}

/// The last symbol id that the process gave. Tests use it.
#[cfg(test)]
#[must_use]
pub(crate) fn last_symbol_id() -> u64 {
    NEXT_SYMBOL_ID.0.load(Ordering::Relaxed)
}

/// The next node id of this thread, and the number of symbol ids that this
/// thread gave. Work that must give no ids (a bind thread, a parse thread)
/// compares them before and after: ids of other threads do not change them.
#[must_use]
pub fn next_ids() -> (u64, u64) {
    (
        NEXT_NODE_ID.with(std::cell::Cell::get),
        SYMBOL_IDS_GIVEN_HERE.with(std::cell::Cell::get),
    )
}

/// Makes `seed` the node id state of this thread.
pub fn install_id_seed(seed: IdSeed) {
    NEXT_NODE_ID.with(|next| next.set(seed.next_node_id));
    NODE_IDS.with(|ids| *ids.borrow_mut() = seed.node_ids);
    OWN_SYMBOL_IDS.with(|own| *own.borrow_mut() = OwnSymbolIds::new());
}

/// wasm: the node ids of one checker of the inline checker pool
/// (`program::WorkerState`), which a native checker worker keeps in its
/// own thread. A job swaps them in (`swap_id_state`).
// PORT: not in Go.
#[cfg(target_family = "wasm")]
pub struct IdState {
    next_node_id: u64,
    node_ids: PerFileMap<u64>,
}

#[cfg(target_family = "wasm")]
impl From<IdSeed> for IdState {
    /// The ids of a new worker, as `install_id_seed` sets them.
    fn from(seed: IdSeed) -> Self {
        IdState {
            next_node_id: seed.next_node_id,
            node_ids: seed.node_ids,
        }
    }
}

/// wasm: swaps the node ids of this thread with `state`.
// PORT: not in Go.
#[cfg(target_family = "wasm")]
pub fn swap_id_state(state: &mut IdState) {
    NEXT_NODE_ID.with(|next| state.next_node_id = next.replace(state.next_node_id));
    NODE_IDS.with(|ids| std::mem::swap(&mut *ids.borrow_mut(), &mut state.node_ids));
}

// Go: ast/utilities.go:22 GetNodeId
// PORT: Go `ast.NodeId` is a `uint64`; returned here as `u64`.
pub fn get_node_id(node: Node) -> u64 {
    NODE_IDS.with(|ids| {
        let mut ids = ids.borrow_mut();
        if let Some(id) = ids.get(&node) {
            return *id;
        }
        ids.write();
        if ids.is_dead(node) {
            crate::ast::file_version::released(node.file_index());
        }
        let id = NEXT_NODE_ID.with(|next| {
            let id = next.get() + 1;
            next.set(id);
            id
        });
        ids.write().insert(node, id);
        id
    })
}

// Go: ast/utilities.go:34 GetSymbolId
// PORT: Go `ast.SymbolId` is a `uint64`; returned here as `u64`. A symbol
// handle is an index into `symbols`, which gives the slot where the id is
// kept (see `SymbolArena::id_slot`): the lineage table that the arena
// shares, or this thread's ids of a checker's own symbols.
pub fn get_symbol_id(symbols: &SymbolArena, symbol: SymbolId) -> u64 {
    let slot = symbols.id_slot(symbol);
    let place = slot.place as usize;
    if slot.key == 0 {
        symbols.lineage_ids().id(place)
    } else {
        OWN_SYMBOL_IDS.with(|own| own.borrow_mut().id(slot.key, place))
    }
}

// Go: ast/utilities.go:46 GetSymbolTable
// PORT: Go takes `*SymbolTable` and allocates the map in place. The arena
// must be mutable to allocate a table.
pub fn get_symbol_table(symbols: &mut SymbolArena, data: &mut SymbolTable) -> SymbolTable {
    if data.is_nil() {
        *data = symbols.new_table();
    }
    *data
}

// Go: ast/utilities.go:53 GetMembers
pub fn get_members(symbols: &mut SymbolArena, symbol: SymbolId) -> SymbolTable {
    let mut members = symbols.sym(symbol).members;
    let table = get_symbol_table(symbols, &mut members);
    symbols.sym_mut(symbol).members = members;
    table
}

// Go: ast/utilities.go:57 GetExports
pub fn get_exports(symbols: &mut SymbolArena, symbol: SymbolId) -> SymbolTable {
    let mut exports = symbols.sym(symbol).exports;
    let table = get_symbol_table(symbols, &mut exports);
    symbols.sym_mut(symbol).exports = exports;
    table
}

// Go: ast/utilities.go:61 GetLocals
// PORT: Go writes `container.LocalsContainerData().Locals`. Installed binder
// data (`Node::locals`) is not written again, so the binder passes the
// mutable per-node bind data of the file it is binding, indexed by
// `NodeId::index()`.
pub fn get_locals(
    symbols: &mut SymbolArena,
    node_bind: &mut [NodeBindData],
    container: Node,
) -> SymbolTable {
    let index = container.node_id().index();
    let mut locals = node_bind[index].locals;
    let table = get_symbol_table(symbols, &mut locals);
    node_bind[index].locals = locals;
    table
}

// Determines if a node is missing (either `nil` or empty)
// Go: ast/utilities.go:66 NodeIsMissing
pub fn node_is_missing(node: Node) -> bool {
    node.is_nil()
        || node.loc().pos() == node.loc().end()
            && node.loc().pos() >= 0
            && node.kind() != SyntaxKind::EndOfFile
}

// Determines if a node is present
// Go: ast/utilities.go:71 NodeIsPresent
pub fn node_is_present(node: Node) -> bool {
    !node_is_missing(node)
}

// Determines if a node contains synthetic positions
// Go: ast/utilities.go:76 NodeIsSynthesized
pub fn node_is_synthesized(node: Node) -> bool {
    position_is_synthesized(node.loc().pos()) || position_is_synthesized(node.loc().end())
}

// Go: ast/utilities.go:80 RangeIsSynthesized
pub fn range_is_synthesized(loc: TextRange) -> bool {
    position_is_synthesized(loc.pos()) || position_is_synthesized(loc.end())
}

// Determines whether a position is synthetic
// Go: ast/utilities.go:85 PositionIsSynthesized
pub fn position_is_synthesized(pos: i32) -> bool {
    pos < 0
}

// Go: ast/utilities.go:89 FindLastVisibleNode
pub fn find_last_visible_node(nodes: &[Node]) -> Node {
    let mut from_end = 1usize;
    while from_end <= nodes.len()
        && nodes[nodes.len() - from_end]
            .flags()
            .intersects(NodeFlags::REPARSED)
    {
        from_end += 1;
    }
    if from_end <= nodes.len() {
        return nodes[nodes.len() - from_end];
    }
    Node::NIL
}

// Go: ast/utilities.go:100 NodeKindIs
pub fn node_kind_is(node: Node, kinds: &[SyntaxKind]) -> bool {
    kinds.contains(&node.kind())
}

// Go: ast/utilities.go:104 IsModifier
pub fn is_modifier(node: Node) -> bool {
    is_modifier_kind(node.kind())
}

// Go: ast/utilities.go:108 IsModifierLike
pub fn is_modifier_like(node: Node) -> bool {
    is_modifier(node) || is_decorator(node)
}

// Go: ast/utilities.go:112 IsCompoundAssignment
pub fn is_compound_assignment(token: SyntaxKind) -> bool {
    (token as u16) >= (SyntaxKind::FIRST_COMPOUND_ASSIGNMENT as u16)
        && (token as u16) <= (SyntaxKind::LAST_COMPOUND_ASSIGNMENT as u16)
}

/// The fields of a BinaryExpression, read once: Go `node.AsBinaryExpression()`
/// and its fields, plus `OperatorToken.Kind`. The binder reads them through
/// this view (`Binder::binary_view`) in `bind` and passes it on.
// PERF: binderview1 (cliperf1 rank 4a). Each accessor read (`left`,
// `operator_token`, `right`, `type_`) looks the node up again: about 45
// instructions and 2 file block lookups, and the binder made about 7
// `operator_token` reads per BinaryExpression. A view loads the node data
// once and resolves the child ids with no lookup (`FrozenIds`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinaryView {
    pub left: Node,
    pub operator_token: Node,
    /// Go `OperatorToken.Kind`.
    pub operator: SyntaxKind,
    pub right: Node,
    pub type_: Node,
}

impl BinaryView {
    /// The view of BinaryExpression `node`. `ids` is `frozen_store_ids` of
    /// the file of `node` (a caller that reads many nodes of one file reads
    /// it once). A node with no published data (synthetic, unpublished or
    /// of a freeable file version), or no `ids`, reads each field with its
    /// accessor. Panics as the accessors do when `node` is no
    /// BinaryExpression.
    #[inline]
    #[must_use]
    pub fn with_ids(node: Node, ids: Option<FrozenIds>) -> Self {
        let view = match (frozen_store_ast_node(node), ids) {
            (Some(crate::astdata::NodeData::BinaryExpression(d)), Some(ids)) => {
                let operator_token = ids.node(d.operator_token);
                BinaryView {
                    left: ids.node(d.left),
                    operator_token,
                    operator: operator_token.kind(),
                    right: ids.node(d.right),
                    type_: d.type_.map_or(Node::NIL, |id| ids.node(id)),
                }
            }
            _ => Self::read(node),
        };
        debug_assert_eq!(view, Self::read(node), "binary view of {node:?}");
        view
    }

    /// `with_ids` with the ids of the file of `node`.
    #[inline]
    #[must_use]
    pub fn new(node: Node) -> Self {
        Self::with_ids(node, frozen_store_ids(node.file_index()))
    }

    /// The view from the field accessors.
    #[cold]
    #[inline(never)]
    fn read(node: Node) -> Self {
        let operator_token = node.operator_token();
        BinaryView {
            left: node.left(),
            operator_token,
            operator: operator_token.kind(),
            right: node.right(),
            type_: node.type_(),
        }
    }

    /// `is_destructuring_assignment` of the node of this view. An object or
    /// array literal is a left-hand side expression kind, so the kind test
    /// of `left` also gives Go `IsLeftHandSideExpression(Left)`.
    #[must_use]
    pub fn is_destructuring_assignment(&self) -> bool {
        self.operator == SyntaxKind::EqualsToken
            && matches!(
                self.left.kind(),
                SyntaxKind::ObjectLiteralExpression | SyntaxKind::ArrayLiteralExpression
            )
    }
}

// Go: ast/utilities.go:116 IsAssignmentExpression
pub fn is_assignment_expression(node: Node, exclude_compound_assignment: bool) -> bool {
    if node.kind() == SyntaxKind::BinaryExpression {
        let operator = node.operator_token().kind();
        return (operator == SyntaxKind::EqualsToken
            || !exclude_compound_assignment && is_assignment_operator(operator))
            && is_left_hand_side_expression(node.left());
    }
    false
}

// Go: ast/utilities.go:125 GetRightMostAssignedExpression
pub fn get_right_most_assigned_expression(node: Node) -> Node {
    let mut node = node;
    while is_assignment_expression(node, false /*excludeCompoundAssignment*/) {
        node = node.right();
    }
    node
}

// Go: ast/utilities.go:132 IsDestructuringAssignment
pub fn is_destructuring_assignment(node: Node) -> bool {
    if is_assignment_expression(node, true /*excludeCompoundAssignment*/) {
        let kind = node.left().kind();
        return kind == SyntaxKind::ObjectLiteralExpression
            || kind == SyntaxKind::ArrayLiteralExpression;
    }
    false
}

// Go: ast/utilities.go:140 IsObjectBindingOrAssignmentElement
pub fn is_object_binding_or_assignment_element(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::BindingElement
            | SyntaxKind::PropertyAssignment
            | SyntaxKind::ShorthandPropertyAssignment
            | SyntaxKind::SpreadAssignment
    )
}

// Go: ast/utilities.go:151 IsArrayBindingOrAssignmentElement
pub fn is_array_binding_or_assignment_element(node: Node) -> bool {
    match node.kind() {
        SyntaxKind::BindingElement
        | SyntaxKind::OmittedExpression
        | SyntaxKind::SpreadElement
        | SyntaxKind::ArrayLiteralExpression
        | SyntaxKind::ObjectLiteralExpression
        | SyntaxKind::Identifier
        | SyntaxKind::PropertyAccessExpression
        | SyntaxKind::ElementAccessExpression => return true,
        _ => {}
    }
    is_assignment_expression(node, true /*excludeCompoundAssignment*/)
}

// Go: ast/utilities.go:166 IsBindingPattern
pub fn is_binding_pattern(node: Node) -> bool {
    node.kind() == SyntaxKind::ObjectBindingPattern
        || node.kind() == SyntaxKind::ArrayBindingPattern
}

// Go: ast/utilities.go:170 IsForInOrOfStatement
pub fn is_for_in_or_of_statement(node: Node) -> bool {
    node.is_some()
        && (node.kind() == SyntaxKind::ForInStatement || node.kind() == SyntaxKind::ForOfStatement)
}

// A node is an assignment target if it is on the left hand side of an '=' token, if it is parented by a property
// assignment in an object literal that is an assignment target, or if it is parented by an array literal that is
// an assignment target. Examples include 'a = xxx', '{ p: a } = xxx', '[{ a }] = xxx'.
// (Note that `p` is not a target in the above examples, only `a`.)
// Go: ast/utilities.go:178 IsAssignmentTarget
pub fn is_assignment_target(node: Node) -> bool {
    get_assignment_target(node).is_some()
}

// Returns the BinaryExpression, PrefixUnaryExpression, PostfixUnaryExpression, or ForInOrOfStatement that references
// the given node as an assignment target
// Go: ast/utilities.go:184 GetAssignmentTarget
// PERF: U4 (CH7). A step inside a published store reads the parent and its
// kind with one store lookup (`frozen_store_parent_kind`).
pub fn get_assignment_target(node: Node) -> Node {
    let mut node = node;
    loop {
        let (parent, parent_kind) = match frozen_store_parent_kind(node) {
            Some(parent_and_kind) => parent_and_kind,
            None => {
                let parent = node.parent();
                (parent, parent.kind())
            }
        };
        match parent_kind {
            SyntaxKind::BinaryExpression => {
                if is_assignment_operator(parent.operator_token().kind()) && parent.left() == node {
                    return parent;
                }
                return Node::NIL;
            }
            SyntaxKind::PrefixUnaryExpression => {
                if parent.operator() == SyntaxKind::PlusPlusToken
                    || parent.operator() == SyntaxKind::MinusMinusToken
                {
                    return parent;
                }
                return Node::NIL;
            }
            SyntaxKind::PostfixUnaryExpression => {
                if parent.operator() == SyntaxKind::PlusPlusToken
                    || parent.operator() == SyntaxKind::MinusMinusToken
                {
                    return parent;
                }
                return Node::NIL;
            }
            SyntaxKind::ForInStatement | SyntaxKind::ForOfStatement => {
                if parent.initializer() == node {
                    return parent;
                }
                return Node::NIL;
            }
            SyntaxKind::ParenthesizedExpression
            | SyntaxKind::ArrayLiteralExpression
            | SyntaxKind::SpreadElement
            | SyntaxKind::NonNullExpression => {
                node = parent;
            }
            SyntaxKind::SpreadAssignment => {
                node = parent.parent();
            }
            SyntaxKind::ShorthandPropertyAssignment => {
                if parent.name() != node {
                    return Node::NIL;
                }
                node = parent.parent();
            }
            SyntaxKind::PropertyAssignment => {
                if parent.name() == node {
                    return Node::NIL;
                }
                node = parent.parent();
            }
            _ => return Node::NIL,
        }
    }
}

// Go: ast/utilities.go:228 IsLogicalBinaryOperator
pub fn is_logical_binary_operator(token: SyntaxKind) -> bool {
    token == SyntaxKind::BarBarToken || token == SyntaxKind::AmpersandAmpersandToken
}

// Go: ast/utilities.go:232 IsLogicalOrCoalescingBinaryOperator
pub fn is_logical_or_coalescing_binary_operator(token: SyntaxKind) -> bool {
    is_logical_binary_operator(token) || token == SyntaxKind::QuestionQuestionToken
}

// Go: ast/utilities.go:236 IsLogicalOrCoalescingBinaryExpression
pub fn is_logical_or_coalescing_binary_expression(expr: Node) -> bool {
    is_binary_expression(expr)
        && is_logical_or_coalescing_binary_operator(expr.operator_token().kind())
}

// Go: ast/utilities.go:240 IsLogicalOrCoalescingAssignmentExpression
pub fn is_logical_or_coalescing_assignment_expression(expr: Node) -> bool {
    is_binary_expression(expr)
        && is_logical_or_coalescing_assignment_operator(expr.operator_token().kind())
}

// Go: ast/utilities.go:244 IsLogicalExpression
pub fn is_logical_expression(node: Node) -> bool {
    let mut node = node;
    loop {
        if node.kind() == SyntaxKind::ParenthesizedExpression {
            node = node.expression();
        } else if node.kind() == SyntaxKind::PrefixUnaryExpression
            && node.operator() == SyntaxKind::ExclamationToken
        {
            node = node.operand();
        } else {
            return is_logical_or_coalescing_binary_expression(node);
        }
    }
}

// Go: ast/utilities.go:256 IsAccessor
pub fn is_accessor(node: Node) -> bool {
    node.kind() == SyntaxKind::GetAccessor || node.kind() == SyntaxKind::SetAccessor
}

// Go: ast/utilities.go:260 IsPropertyNameLiteral
pub fn is_property_name_literal(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Identifier
            | SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::NumericLiteral
    )
}

// Go: ast/utilities.go:271 IsMemberName
pub fn is_member_name(node: Node) -> bool {
    node.kind() == SyntaxKind::Identifier || node.kind() == SyntaxKind::PrivateIdentifier
}

// Go: ast/utilities.go:275 IsEntityName
pub fn is_entity_name(node: Node) -> bool {
    node.kind() == SyntaxKind::Identifier || node.kind() == SyntaxKind::QualifiedName
}

// Go: ast/utilities.go:279 IsPropertyName
pub fn is_property_name(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Identifier
            | SyntaxKind::PrivateIdentifier
            | SyntaxKind::StringLiteral
            | SyntaxKind::NumericLiteral
            | SyntaxKind::ComputedPropertyName
    )
}

// Return true if the given identifier is classified as an IdentifierName by inspecting the parent of the node
// Go: ast/utilities.go:292 IsIdentifierName
pub fn is_identifier_name(node: Node) -> bool {
    let parent = node.parent();
    match parent.kind() {
        SyntaxKind::PropertyDeclaration
        | SyntaxKind::PropertySignature
        | SyntaxKind::MethodDeclaration
        | SyntaxKind::MethodSignature
        | SyntaxKind::GetAccessor
        | SyntaxKind::SetAccessor
        | SyntaxKind::EnumMember
        | SyntaxKind::PropertyAssignment
        | SyntaxKind::PropertyAccessExpression => parent.name() == node,
        SyntaxKind::QualifiedName => parent.right() == node,
        SyntaxKind::BindingElement => parent.property_name() == node,
        SyntaxKind::ImportSpecifier => parent.property_name() == node,
        SyntaxKind::ExportSpecifier
        | SyntaxKind::JsxAttribute
        | SyntaxKind::JsxSelfClosingElement
        | SyntaxKind::JsxOpeningElement
        | SyntaxKind::JsxClosingElement => true,
        _ => false,
    }
}

// Go: ast/utilities.go:310 IsPushOrUnshiftIdentifier
pub fn is_push_or_unshift_identifier(node: Node) -> bool {
    let text = node.text();
    text == "push" || text == "unshift"
}

// Go: ast/utilities.go:315 IsBooleanLiteral
pub fn is_boolean_literal(node: Node) -> bool {
    node.kind() == SyntaxKind::TrueKeyword || node.kind() == SyntaxKind::FalseKeyword
}

// Go: ast/utilities.go:319 IsLiteralExpression
pub fn is_literal_expression(node: Node) -> bool {
    is_literal_kind(node.kind())
}

// Go: ast/utilities.go:323 IsStringLiteralLike
pub fn is_string_literal_like(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::StringLiteral | SyntaxKind::NoSubstitutionTemplateLiteral
    )
}

// Go: ast/utilities.go:331 IsStringOrNumericLiteralLike
// PERF: binderview1. The kind is read once (see `is_access_expression`).
pub fn is_string_or_numeric_literal_like(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::NumericLiteral
    )
}

// Go: ast/utilities.go:335 IsSignedNumericLiteral
pub fn is_signed_numeric_literal(node: Node) -> bool {
    if node.kind() == SyntaxKind::PrefixUnaryExpression {
        return (node.operator() == SyntaxKind::PlusToken
            || node.operator() == SyntaxKind::MinusToken)
            && is_numeric_literal(node.operand());
    }
    false
}

// Determines if a node is part of an OptionalChain
// Go: ast/utilities.go:344 IsOptionalChain
// PERF: U4 (CH7). The kind test comes first (both tests are pure, so the
// result is the same) and `OPTIONAL_CHAIN` is a parser bit
// (`Node::parser_flags`), so most nodes load neither their flags nor their
// binder data.
pub fn is_optional_chain(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::PropertyAccessExpression
            | SyntaxKind::ElementAccessExpression
            | SyntaxKind::CallExpression
            | SyntaxKind::NonNullExpression
    ) && !node.parser_flags(NodeFlags::OPTIONAL_CHAIN).is_empty()
}

// Go: ast/utilities.go:357 getQuestionDotToken
fn get_question_dot_token(node: Node) -> Node {
    node.question_dot_token()
}

// Determines if node is the root expression of an OptionalChain
// Go: ast/utilities.go:362 IsOptionalChainRoot
pub fn is_optional_chain_root(node: Node) -> bool {
    is_optional_chain(node)
        && !is_non_null_expression(node)
        && get_question_dot_token(node).is_some()
}

// Determines whether a node is the outermost `OptionalChain` in an ECMAScript `OptionalExpression`:
//
//  1. For `a?.b.c`, the outermost chain is `a?.b.c` (`c` is the end of the chain starting at `a?.`)
//  2. For `a?.b!`, the outermost chain is `a?.b` (`b` is the end of the chain starting at `a?.`)
//  3. For `(a?.b.c).d`, the outermost chain is `a?.b.c` (`c` is the end of the chain starting at `a?.` since parens end the chain)
//  4. For `a?.b.c?.d`, both `a?.b.c` and `a?.b.c?.d` are outermost (`c` is the end of the chain starting at `a?.`, and `d` is
//     the end of the chain starting at `c?.`)
//  5. For `a?.(b?.c).d`, both `b?.c` and `a?.(b?.c)d` are outermost (`c` is the end of the chain starting at `b`, and `d` is
//     the end of the chain starting at `a?.`)
// Go: ast/utilities.go:375 IsOutermostOptionalChain
pub fn is_outermost_optional_chain(node: Node) -> bool {
    let parent = node.parent();
    !is_optional_chain(parent) || // cases 1, 2, and 3
        is_optional_chain_root(parent) || // case 4
        node != parent.expression() // case 5
}

// Determines whether a node is the expression preceding an optional chain (i.e. `a` in `a?.b`).
// Go: ast/utilities.go:383 IsExpressionOfOptionalChainRoot
pub fn is_expression_of_optional_chain_root(node: Node) -> bool {
    is_optional_chain_root(node.parent()) && node.parent().expression() == node
}

// Go: ast/utilities.go:387 IsNullishCoalesce
pub fn is_nullish_coalesce(node: Node) -> bool {
    node.kind() == SyntaxKind::BinaryExpression
        && node.operator_token().kind() == SyntaxKind::QuestionQuestionToken
}

// Go: ast/utilities.go:391 IsAssertionExpression
pub fn is_assertion_expression(node: Node) -> bool {
    let kind = node.kind();
    kind == SyntaxKind::TypeAssertionExpression || kind == SyntaxKind::AsExpression
}

// Go: ast/utilities.go:396 isLeftHandSideExpressionKind
fn is_left_hand_side_expression_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::PropertyAccessExpression
            | SyntaxKind::ElementAccessExpression
            | SyntaxKind::NewExpression
            | SyntaxKind::CallExpression
            | SyntaxKind::JsxElement
            | SyntaxKind::JsxSelfClosingElement
            | SyntaxKind::JsxFragment
            | SyntaxKind::TaggedTemplateExpression
            | SyntaxKind::ArrayLiteralExpression
            | SyntaxKind::ParenthesizedExpression
            | SyntaxKind::ObjectLiteralExpression
            | SyntaxKind::ClassExpression
            | SyntaxKind::FunctionExpression
            | SyntaxKind::Identifier
            | SyntaxKind::PrivateIdentifier
            | SyntaxKind::RegularExpressionLiteral
            | SyntaxKind::NumericLiteral
            | SyntaxKind::BigIntLiteral
            | SyntaxKind::StringLiteral
            | SyntaxKind::NoSubstitutionTemplateLiteral
            | SyntaxKind::TemplateExpression
            | SyntaxKind::FalseKeyword
            | SyntaxKind::NullKeyword
            | SyntaxKind::ThisKeyword
            | SyntaxKind::TrueKeyword
            | SyntaxKind::SuperKeyword
            | SyntaxKind::NonNullExpression
            | SyntaxKind::ExpressionWithTypeArguments
            | SyntaxKind::MetaProperty
            | SyntaxKind::ImportKeyword
            | SyntaxKind::MissingDeclaration
    )
}

// Determines whether a node is a LeftHandSideExpression based only on its kind.
// Go: ast/utilities.go:411 IsLeftHandSideExpression
// PERF: binderview1. The kind is read once. Only a PartiallyEmittedExpression
// is an outer expression of `OEK_PARTIALLY_EMITTED_EXPRESSIONS`, so any other
// node is its own `SkipPartiallyEmittedExpressions`.
pub fn is_left_hand_side_expression(node: Node) -> bool {
    let kind = node.kind();
    if kind != SyntaxKind::PartiallyEmittedExpression {
        return is_left_hand_side_expression_kind(kind);
    }
    is_left_hand_side_expression_kind(skip_partially_emitted_expressions(node).kind())
}

// Go: ast/utilities.go:415 isUnaryExpressionKind
fn is_unary_expression_kind(kind: SyntaxKind) -> bool {
    match kind {
        SyntaxKind::PrefixUnaryExpression
        | SyntaxKind::PostfixUnaryExpression
        | SyntaxKind::DeleteExpression
        | SyntaxKind::TypeOfExpression
        | SyntaxKind::VoidExpression
        | SyntaxKind::AwaitExpression
        | SyntaxKind::TypeAssertionExpression => true,
        _ => is_left_hand_side_expression_kind(kind),
    }
}

// Determines whether a node is a UnaryExpression based only on its kind.
// Go: ast/utilities.go:430 IsUnaryExpression
pub fn is_unary_expression(node: Node) -> bool {
    is_unary_expression_kind(skip_partially_emitted_expressions(node).kind())
}

// Go: ast/utilities.go:434 isExpressionKind
fn is_expression_kind(kind: SyntaxKind) -> bool {
    match kind {
        SyntaxKind::ConditionalExpression
        | SyntaxKind::YieldExpression
        | SyntaxKind::ArrowFunction
        | SyntaxKind::BinaryExpression
        | SyntaxKind::SpreadElement
        | SyntaxKind::AsExpression
        | SyntaxKind::OmittedExpression
        | SyntaxKind::PartiallyEmittedExpression
        | SyntaxKind::SatisfiesExpression => true,
        _ => is_unary_expression_kind(kind),
    }
}

// Determines whether a node is an expression based only on its kind.
// Go: ast/utilities.go:451 IsExpression
pub fn is_expression(node: Node) -> bool {
    is_expression_kind(skip_partially_emitted_expressions(node).kind())
}

// Go: ast/utilities.go:455 IsCommaExpression
pub fn is_comma_expression(node: Node) -> bool {
    node.kind() == SyntaxKind::BinaryExpression
        && node.operator_token().kind() == SyntaxKind::CommaToken
}

// Go: ast/utilities.go:459 IsCommaSequence
pub fn is_comma_sequence(node: Node) -> bool {
    is_comma_expression(node)
}

// Go: ast/utilities.go:463 IsIterationStatement
pub fn is_iteration_statement(node: Node, look_in_labeled_statements: bool) -> bool {
    match node.kind() {
        SyntaxKind::ForStatement
        | SyntaxKind::ForInStatement
        | SyntaxKind::ForOfStatement
        | SyntaxKind::DoStatement
        | SyntaxKind::WhileStatement => true,
        SyntaxKind::LabeledStatement => {
            look_in_labeled_statements
                && is_iteration_statement(node.statement(), look_in_labeled_statements)
        }
        _ => false,
    }
}

// Determines if a node is a property or element access expression
// Go: ast/utilities.go:479 IsAccessExpression
// PERF: binderview1. The kind is read once: a kind read is a store lookup
// that the compiler does not merge with a second one.
pub fn is_access_expression(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::PropertyAccessExpression | SyntaxKind::ElementAccessExpression
    )
}

/// A fixed set of `SyntaxKind`s, one bit per kind, built at compile time.
// PERF: the hot kind tests below (`is_statement`, `is_function_like`,
// `is_private_identifier_class_element_declaration`) are one table load and
// one bit test, not a chain of compares. The answers are the same as the Go
// `switch` statements, whose kind lists are kept here in Go order.
#[derive(Clone, Copy)]
struct KindSet([u64; 8]);

// `KindSet::contains` masks the word index with `& 7`, so 8 words (512 bits)
// must cover every kind. The mask removes the bounds check.
const _: () = assert!(SyntaxKind::COUNT <= 512);

impl KindSet {
    const fn of(kinds: &[SyntaxKind]) -> Self {
        Self::EMPTY.with(kinds)
    }

    const EMPTY: Self = Self([0; 8]);

    /// This set plus `kinds`.
    const fn with(mut self, kinds: &[SyntaxKind]) -> Self {
        let mut i = 0;
        while i < kinds.len() {
            let k = kinds[i] as usize;
            self.0[k >> 6] |= 1 << (k & 63);
            i += 1;
        }
        self
    }

    #[inline]
    const fn contains(&self, kind: SyntaxKind) -> bool {
        let k = kind as usize;
        self.0[(k >> 6) & 7] & (1 << (k & 63)) != 0
    }
}

// The kinds of Go `isFunctionLikeDeclarationKind` (ast/utilities.go:483).
const FUNCTION_LIKE_DECLARATION_KIND_LIST: &[SyntaxKind] = &[
    SyntaxKind::FunctionDeclaration,
    SyntaxKind::MethodDeclaration,
    SyntaxKind::Constructor,
    SyntaxKind::GetAccessor,
    SyntaxKind::SetAccessor,
    SyntaxKind::FunctionExpression,
    SyntaxKind::ArrowFunction,
];

static FUNCTION_LIKE_DECLARATION_KINDS: KindSet = KindSet::of(FUNCTION_LIKE_DECLARATION_KIND_LIST);

// The kinds of Go `IsFunctionLikeKind` (ast/utilities.go:503): these kinds,
// then the function-like declaration kinds.
static FUNCTION_LIKE_KINDS: KindSet = KindSet::of(&[
    SyntaxKind::MethodSignature,
    SyntaxKind::CallSignature,
    SyntaxKind::JsDocSignature,
    SyntaxKind::ConstructSignature,
    SyntaxKind::IndexSignature,
    SyntaxKind::FunctionType,
    SyntaxKind::ConstructorType,
])
.with(FUNCTION_LIKE_DECLARATION_KIND_LIST);

// Go: ast/utilities.go:483 isFunctionLikeDeclarationKind
#[inline]
fn is_function_like_declaration_kind(kind: SyntaxKind) -> bool {
    FUNCTION_LIKE_DECLARATION_KINDS.contains(kind)
}

// Determines if a node is function-like (but is not a signature declaration)
// Go: ast/utilities.go:498 IsFunctionLikeDeclaration
pub fn is_function_like_declaration(node: Node) -> bool {
    // TODO(rbuckton): Move `node != nil` test to call sites
    node.is_some() && is_function_like_declaration_kind(node.kind())
}

// Go: ast/utilities.go:503 IsFunctionLikeKind
#[inline]
pub fn is_function_like_kind(kind: SyntaxKind) -> bool {
    FUNCTION_LIKE_KINDS.contains(kind)
}

// Determines if a node is function- or signature-like.
// Go: ast/utilities.go:518 IsFunctionLike
#[inline]
pub fn is_function_like(node: Node) -> bool {
    // TODO(rbuckton): Move `node != nil` test to call sites
    node.is_some() && is_function_like_kind(node.kind())
}

// Go: ast/utilities.go:523 IsFunctionLikeOrClassStaticBlockDeclaration
pub fn is_function_like_or_class_static_block_declaration(node: Node) -> bool {
    node.is_some() && (is_function_like(node) || is_class_static_block_declaration(node))
}

// Go: ast/utilities.go:527 IsFunctionOrSourceFile
pub fn is_function_or_source_file(node: Node) -> bool {
    is_function_like(node) || is_source_file(node)
}

// Go: ast/utilities.go:531 IsClassLike
pub fn is_class_like(node: Node) -> bool {
    node.kind() == SyntaxKind::ClassDeclaration || node.kind() == SyntaxKind::ClassExpression
}

// Go: ast/utilities.go:535 IsClassOrInterfaceLike
pub fn is_class_or_interface_like(node: Node) -> bool {
    node.kind() == SyntaxKind::ClassDeclaration
        || node.kind() == SyntaxKind::ClassExpression
        || node.kind() == SyntaxKind::InterfaceDeclaration
}

// Go: ast/utilities.go:539 IsClassElement
pub fn is_class_element(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::Constructor
            | SyntaxKind::PropertyDeclaration
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::IndexSignature
            | SyntaxKind::ClassStaticBlockDeclaration
            | SyntaxKind::SemicolonClassElement
    )
}

// Go: ast/utilities.go:554 IsMethodOrAccessor
pub fn is_method_or_accessor(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::MethodDeclaration | SyntaxKind::GetAccessor | SyntaxKind::SetAccessor
    )
}

// The kinds of `IsPropertyDeclaration || IsMethodOrAccessor` in Go
// `IsPrivateIdentifierClassElementDeclaration` (ast/utilities.go:562).
static PRIVATE_IDENTIFIER_CLASS_ELEMENT_KINDS: KindSet = KindSet::of(&[
    SyntaxKind::PropertyDeclaration,
    SyntaxKind::MethodDeclaration,
    SyntaxKind::GetAccessor,
    SyntaxKind::SetAccessor,
]);

// Go: ast/utilities.go:562 IsPrivateIdentifierClassElementDeclaration
#[inline]
pub fn is_private_identifier_class_element_declaration(node: Node) -> bool {
    // PERF: one kind lookup for `IsPropertyDeclaration || IsMethodOrAccessor`.
    PRIVATE_IDENTIFIER_CLASS_ELEMENT_KINDS.contains(node.kind())
        && is_private_identifier(node.name())
}

// Go: ast/utilities.go:566 IsObjectLiteralOrClassExpressionMethodOrAccessor
pub fn is_object_literal_or_class_expression_method_or_accessor(node: Node) -> bool {
    let kind = node.kind();
    (kind == SyntaxKind::MethodDeclaration
        || kind == SyntaxKind::GetAccessor
        || kind == SyntaxKind::SetAccessor)
        && (node.parent().kind() == SyntaxKind::ObjectLiteralExpression
            || node.parent().kind() == SyntaxKind::ClassExpression)
}

// Go: ast/utilities.go:572 IsTypeElement
pub fn is_type_element(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::ConstructSignature
            | SyntaxKind::CallSignature
            | SyntaxKind::PropertySignature
            | SyntaxKind::MethodSignature
            | SyntaxKind::IndexSignature
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
            | SyntaxKind::NotEmittedTypeElement
    )
}

// Go: ast/utilities.go:587 IsObjectLiteralElement
pub fn is_object_literal_element(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::PropertyAssignment
            | SyntaxKind::ShorthandPropertyAssignment
            | SyntaxKind::SpreadAssignment
            | SyntaxKind::MethodDeclaration
            | SyntaxKind::GetAccessor
            | SyntaxKind::SetAccessor
    )
}

// Go: ast/utilities.go:600 IsObjectLiteralMethod
pub fn is_object_literal_method(node: Node) -> bool {
    node.is_some()
        && node.kind() == SyntaxKind::MethodDeclaration
        && node.parent().kind() == SyntaxKind::ObjectLiteralExpression
}

// Go: ast/utilities.go:604 IsAutoAccessorPropertyDeclaration
pub fn is_auto_accessor_property_declaration(node: Node) -> bool {
    is_property_declaration(node) && has_accessor_modifier(node)
}

// Go: ast/utilities.go:608 IsParameterPropertyDeclaration
pub fn is_parameter_property_declaration(node: Node, parent: Node) -> bool {
    is_parameter_declaration(node)
        && has_syntactic_modifier(node, ModifierFlags::PARAMETER_PROPERTY_MODIFIER)
        && parent.kind() == SyntaxKind::Constructor
}

// Go: ast/utilities.go:612 IsJsxChild
pub fn is_jsx_child(node: Node) -> bool {
    matches!(
        node.kind(),
        SyntaxKind::JsxElement
            | SyntaxKind::JsxExpression
            | SyntaxKind::JsxSelfClosingElement
            | SyntaxKind::JsxText
            | SyntaxKind::JsxFragment
    )
}

// Go: ast/utilities.go:624 IsJsxAttributeLike
pub fn is_jsx_attribute_like(node: Node) -> bool {
    is_jsx_attribute(node) || is_jsx_spread_attribute(node)
}

// The kinds of Go `isDeclarationStatementKind` (ast/utilities.go:628).
const DECLARATION_STATEMENT_KIND_LIST: &[SyntaxKind] = &[
    SyntaxKind::FunctionDeclaration,
    SyntaxKind::MissingDeclaration,
    SyntaxKind::ClassDeclaration,
    SyntaxKind::InterfaceDeclaration,
    SyntaxKind::TypeAliasDeclaration,
    SyntaxKind::JsTypeAliasDeclaration,
    SyntaxKind::EnumDeclaration,
    SyntaxKind::ModuleDeclaration,
    SyntaxKind::ImportDeclaration,
    SyntaxKind::JsImportDeclaration,
    SyntaxKind::ImportEqualsDeclaration,
    SyntaxKind::ExportDeclaration,
    SyntaxKind::ExportAssignment,
    SyntaxKind::NamespaceExportDeclaration,
];

static DECLARATION_STATEMENT_KINDS: KindSet = KindSet::of(DECLARATION_STATEMENT_KIND_LIST);

// Go: ast/utilities.go:628 isDeclarationStatementKind
#[inline]
fn is_declaration_statement_kind(kind: SyntaxKind) -> bool {
    DECLARATION_STATEMENT_KINDS.contains(kind)
}

// Determines whether a node is a DeclarationStatement. Ideally this does not use Parent pointers, but it may use them
// to rule out a Block node that is part of `try` or `catch` or is the Block-like body of a function.
//
// NOTE: ECMA262 would just call this a Declaration
// Go: ast/utilities.go:653 IsDeclarationStatement
pub fn is_declaration_statement(node: Node) -> bool {
    is_declaration_statement_kind(node.kind())
}

// The kinds of Go `isStatementKindButNotDeclarationKind` (ast/utilities.go:657).
const STATEMENT_BUT_NOT_DECLARATION_KIND_LIST: &[SyntaxKind] = &[
    SyntaxKind::BreakStatement,
    SyntaxKind::ContinueStatement,
    SyntaxKind::DebuggerStatement,
    SyntaxKind::DoStatement,
    SyntaxKind::ExpressionStatement,
    SyntaxKind::EmptyStatement,
    SyntaxKind::ForInStatement,
    SyntaxKind::ForOfStatement,
    SyntaxKind::ForStatement,
    SyntaxKind::IfStatement,
    SyntaxKind::LabeledStatement,
    SyntaxKind::ReturnStatement,
    SyntaxKind::SwitchStatement,
    SyntaxKind::ThrowStatement,
    SyntaxKind::TryStatement,
    SyntaxKind::VariableStatement,
    SyntaxKind::WhileStatement,
    SyntaxKind::WithStatement,
    SyntaxKind::NotEmittedStatement,
];

static STATEMENT_BUT_NOT_DECLARATION_KINDS: KindSet =
    KindSet::of(STATEMENT_BUT_NOT_DECLARATION_KIND_LIST);

// The two statement kind lists together, for `is_statement`.
static STATEMENT_KINDS: KindSet =
    KindSet::of(STATEMENT_BUT_NOT_DECLARATION_KIND_LIST).with(DECLARATION_STATEMENT_KIND_LIST);

// Go: ast/utilities.go:657 isStatementKindButNotDeclarationKind
#[inline]
fn is_statement_kind_but_not_declaration_kind(kind: SyntaxKind) -> bool {
    STATEMENT_BUT_NOT_DECLARATION_KINDS.contains(kind)
}

// Determines whether a node is a Statement that is not also a Declaration. Ideally this does not use Parent pointers,
// but it may use them to rule out a Block node that is part of `try` or `catch` or is the Block-like body of a function.
//
// NOTE: ECMA262 would just call this a Statement
// Go: ast/utilities.go:687 IsStatementButNotDeclaration
pub fn is_statement_but_not_declaration(node: Node) -> bool {
    is_statement_kind_but_not_declaration_kind(node.kind())
}

// Determines whether a node is a Statement. Ideally this does not use Parent pointers, but it may use
// them to rule out a Block node that is part of `try` or `catch` or is the Block-like body of a function.
//
// NOTE: ECMA262 would call this either a StatementListItem or ModuleListItem
// Go: ast/utilities.go:695 IsStatement
#[inline]
pub fn is_statement(node: Node) -> bool {
    // PERF: one lookup for `isStatementKindButNotDeclarationKind(kind) ||
    // isDeclarationStatementKind(kind)`. Neither list holds `Block`.
    STATEMENT_KINDS.contains(node.kind()) || is_block_statement(node)
}

// Determines whether a node is a BlockStatement. If parents are available, this ensures the Block is
// not part of a `try` statement, `catch` clause, or the Block-like body of a function
// Go: ast/utilities.go:702 isBlockStatement
fn is_block_statement(node: Node) -> bool {
    if node.kind() != SyntaxKind::Block {
        return false;
    }
    if node.parent().is_some()
        && (node.parent().kind() == SyntaxKind::TryStatement
            || node.parent().kind() == SyntaxKind::CatchClause)
    {
        return false;
    }
    !is_function_block(node)
}

// Determines whether a node is the Block-like body of a function by walking the parent of the node
// Go: ast/utilities.go:713 IsFunctionBlock
pub fn is_function_block(node: Node) -> bool {
    node.is_some()
        && node.kind() == SyntaxKind::Block
        && node.parent().is_some()
        && is_function_like(node.parent())
}

// Go: ast/utilities.go:717 IsBlockOrCatchScoped
// PERF: lsshells M3 repair. `BLOCK_SCOPED` has no binder bit, so the walk
// reads no binder data (`get_combined_parser_flags`).
pub fn is_block_or_catch_scoped(declaration: Node) -> bool {
    !get_combined_parser_flags(declaration, NodeFlags::BLOCK_SCOPED).is_empty()
        || is_catch_clause_variable_declaration_or_binding_element(declaration)
}

// Go: ast/utilities.go:721 IsCatchClauseVariableDeclarationOrBindingElement
pub fn is_catch_clause_variable_declaration_or_binding_element(declaration: Node) -> bool {
    let node = get_root_declaration(declaration);
    node.kind() == SyntaxKind::VariableDeclaration
        && node.parent().kind() == SyntaxKind::CatchClause
}

// Go: ast/utilities.go:726 IsTypeNodeKind
pub fn is_type_node_kind(kind: SyntaxKind) -> bool {
    match kind {
        SyntaxKind::AnyKeyword
        | SyntaxKind::UnknownKeyword
        | SyntaxKind::NumberKeyword
        | SyntaxKind::BigIntKeyword
        | SyntaxKind::ObjectKeyword
        | SyntaxKind::BooleanKeyword
        | SyntaxKind::StringKeyword
        | SyntaxKind::SymbolKeyword
        | SyntaxKind::VoidKeyword
        | SyntaxKind::UndefinedKeyword
        | SyntaxKind::NeverKeyword
        | SyntaxKind::IntrinsicKeyword
        | SyntaxKind::ExpressionWithTypeArguments
        | SyntaxKind::JsDocAllType
        | SyntaxKind::JsDocNullableType
        | SyntaxKind::JsDocNonNullableType
        | SyntaxKind::JsDocOptionalType
        | SyntaxKind::JsDocVariadicType => return true,
        _ => {}
    }
    (kind as u16) >= (SyntaxKind::FIRST_TYPE_NODE as u16)
        && (kind as u16) <= (SyntaxKind::LAST_TYPE_NODE as u16)
}

// Go: ast/utilities.go:751 IsTypeNode
pub fn is_type_node(node: Node) -> bool {
    is_type_node_kind(node.kind())
}

// Go: ast/utilities.go:755 IsJSDocKind
pub fn is_js_doc_kind(kind: SyntaxKind) -> bool {
    (SyntaxKind::FIRST_JS_DOC_NODE as u16) <= (kind as u16)
        && (kind as u16) <= (SyntaxKind::LAST_JS_DOC_NODE as u16)
}

// Go: ast/utilities.go:759 IsJSDocTypeAssertion
pub fn is_js_doc_type_assertion(node: Node) -> bool {
    if node.is_nil() || !is_parenthesized_expression(node) || !is_in_js_file(node) {
        return false;
    }
    let expr = node.expression();
    is_as_expression(expr)
        && expr.type_().is_some()
        && expr.type_().flags().intersects(NodeFlags::REPARSED)
}

// Go: ast/utilities.go:767 IsPrologueDirective
pub fn is_prologue_directive(node: Node) -> bool {
    node.kind() == SyntaxKind::ExpressionStatement
        && node.expression().kind() == SyntaxKind::StringLiteral
}

// PORT: `crate::flags` defines the single-bit `OEK*` consts. The composite
// consts from the same Go const block are defined here.
impl OuterExpressionKinds {
    pub const OEK_ASSERTIONS: Self =
        Self(Self::OEK_TYPE_ASSERTIONS.0 | Self::OEK_NON_NULL_ASSERTIONS.0 | Self::OEK_SATISFIES.0);
    pub const OEK_ALL: Self = Self(
        Self::OEK_PARENTHESES.0
            | Self::OEK_ASSERTIONS.0
            | Self::OEK_PARTIALLY_EMITTED_EXPRESSIONS.0
            | Self::OEK_EXPRESSIONS_WITH_TYPE_ARGUMENTS.0,
    );
    pub const OEK_ALL_EXCEPT_ASSERTIONS_OR_EXPRESSIONS_WITH_TYPE_ARGUMENTS: Self = Self(
        Self::OEK_ALL.0 & !Self::OEK_ASSERTIONS.0 & !Self::OEK_EXPRESSIONS_WITH_TYPE_ARGUMENTS.0,
    );
    pub const OEK_EXPRESSION_TYPE_PASSTHROUGH: Self =
        Self(Self::OEK_PARENTHESES.0 | Self::OEK_ASSIGNMENTS.0 | Self::OEK_COMMA.0);
}

// Determines whether node is an "outer expression" of the provided kinds
// Go: ast/utilities.go:791 IsOuterExpression
pub fn is_outer_expression(node: Node, kinds: OuterExpressionKinds) -> bool {
    match node.kind() {
        SyntaxKind::ParenthesizedExpression => {
            return kinds.intersects(OuterExpressionKinds::OEK_PARENTHESES)
                && !(kinds.intersects(OuterExpressionKinds::OEK_EXCLUDE_JS_DOC_TYPE_ASSERTION)
                    && is_js_doc_type_assertion(node));
        }
        SyntaxKind::TypeAssertionExpression | SyntaxKind::AsExpression => {
            return kinds.intersects(OuterExpressionKinds::OEK_TYPE_ASSERTIONS);
        }
        SyntaxKind::SatisfiesExpression => {
            return kinds.intersects(
                OuterExpressionKinds::OEK_EXPRESSIONS_WITH_TYPE_ARGUMENTS
                    | OuterExpressionKinds::OEK_SATISFIES,
            );
        }
        SyntaxKind::ExpressionWithTypeArguments => {
            return kinds.intersects(OuterExpressionKinds::OEK_EXPRESSIONS_WITH_TYPE_ARGUMENTS);
        }
        SyntaxKind::NonNullExpression => {
            return kinds.intersects(OuterExpressionKinds::OEK_NON_NULL_ASSERTIONS);
        }
        SyntaxKind::PartiallyEmittedExpression => {
            return kinds.intersects(OuterExpressionKinds::OEK_PARTIALLY_EMITTED_EXPRESSIONS);
        }
        // PERF: binderview1. With neither the assignment nor the comma bit in
        // `kinds` the answer is false for every operator, so the operator
        // token is not read (`skip_partially_emitted_expressions`).
        SyntaxKind::BinaryExpression
            if kinds.intersects(
                OuterExpressionKinds::OEK_ASSIGNMENTS | OuterExpressionKinds::OEK_COMMA,
            ) =>
        {
            match node.operator_token().kind() {
                SyntaxKind::EqualsToken => {
                    return kinds.intersects(OuterExpressionKinds::OEK_ASSIGNMENTS);
                }
                SyntaxKind::CommaToken => {
                    return kinds.intersects(OuterExpressionKinds::OEK_COMMA);
                }
                _ => {}
            }
        }
        _ => {}
    }
    false
}

// Descends into an expression, skipping past "outer expressions" of the provided kinds
// Go: ast/utilities.go:817 SkipOuterExpressions
pub fn skip_outer_expressions(node: Node, kinds: OuterExpressionKinds) -> Node {
    let mut node = node;
    while is_outer_expression(node, kinds) {
        if is_binary_expression(node) {
            node = node.right();
        } else {
            node = node.expression();
        }
    }
    node
}

// Skips past the parentheses of an expression
// Go: ast/utilities.go:829 SkipParentheses
pub fn skip_parentheses(node: Node) -> Node {
    skip_outer_expressions(node, OuterExpressionKinds::OEK_PARENTHESES)
}

// Go: ast/utilities.go:833 SkipTypeParentheses
pub fn skip_type_parentheses(node: Node) -> Node {
    let mut node = node;
    while is_parenthesized_type_node(node) {
        node = node.type_();
    }
    node
}

// Go: ast/utilities.go:840 SkipPartiallyEmittedExpressions
pub fn skip_partially_emitted_expressions(node: Node) -> Node {
    skip_outer_expressions(
        node,
        OuterExpressionKinds::OEK_PARTIALLY_EMITTED_EXPRESSIONS,
    )
}

// Walks up the parents of a parenthesized expression to find the containing node
// Go: ast/utilities.go:845 WalkUpParenthesizedExpressions
pub fn walk_up_parenthesized_expressions(node: Node) -> Node {
    let mut node = node;
    while node.is_some() && node.kind() == SyntaxKind::ParenthesizedExpression {
        node = node.parent();
    }
    node
}

// Walks up the parents of a parenthesized type to find the containing node
// Go: ast/utilities.go:853 WalkUpParenthesizedTypes
pub fn walk_up_parenthesized_types(node: Node) -> Node {
    let mut node = node;
    while node.is_some() && node.kind() == SyntaxKind::ParenthesizedType {
        node = node.parent();
    }
    node
}

// Walks up the parents of a node to find the containing SourceFile
// Go: ast/utilities.go:861 GetSourceFileOfNode
// PORT: a frozen store node whose parent walk ends at its store root gets
// that root in O(1) (`ast::store::frozen_source_file_of_node`). Other nodes
// (synthetic nodes, nodes before the freeze) walk the parents like Go.
pub fn get_source_file_of_node(node: Node) -> Node {
    if let Some(root) = frozen_source_file_of_node(node) {
        debug_assert_eq!(root, walk_to_source_file(node));
        return root;
    }
    walk_to_source_file(node)
}

/// The Go `GetSourceFileOfNode` parent walk.
fn walk_to_source_file(node: Node) -> Node {
    let mut node = node;
    while node.is_some() {
        if node.kind() == SyntaxKind::SourceFile {
            return node;
        }
        node = node.parent();
    }
    Node::NIL
}

// Go: ast/utilities.go:877 newParentInChildrenSetter
// PORT: Go keeps `parent` in a closure state and recurses through
// `node.ForEachChild(state.visit)`. Rust carries the same state in a
// struct. Go pools the closure with `sync.Pool`; that is only an
// allocation cache, so it is not ported.
struct ParentInChildrenSetter {
    parent: Node,
}

impl ParentInChildrenSetter {
    fn visit(&mut self, node: Node) -> bool {
        if self.parent.is_some() {
            crate::ast::synthetic::set_node_parent(node, self.parent);
        }
        let save_parent = self.parent;
        self.parent = node;
        node.for_each_child(|child| self.visit(child));
        self.parent = save_parent;
        false
    }
}

fn new_parent_in_children_setter() -> ParentInChildrenSetter {
    ParentInChildrenSetter { parent: Node::NIL }
}

// Go: ast/utilities.go:899 SetParentInChildren
pub fn set_parent_in_children(node: Node) {
    let mut f = new_parent_in_children_setter();
    f.visit(node);
}

#[cfg(test)]
mod symbol_id_tests {
    use super::*;

    /// Two checker arenas made from one binder arena share the ids of the
    /// binder symbols. Their own symbols (`is_own_index`) get ids of their
    /// own, also at one index in both arenas, and no later bind uses their
    /// index, so a checker that catches up reads the later binder symbol
    /// with its shared id.
    #[test]
    fn checker_symbols_have_ids_of_their_own() {
        let mut binder = SymbolArena::new();
        let shared = binder.new_symbol(SymbolFlags::NONE, "shared");
        let mut first = binder.for_checker();
        // A later program binds more files into the same binder arena.
        let later = binder.new_symbol(SymbolFlags::NONE, "later");
        let mut second = binder.for_checker();
        let own_first = first.new_symbol(SymbolFlags::NONE, "own");
        let own_second = second.new_symbol(SymbolFlags::NONE, "own");
        assert_eq!(
            own_first, own_second,
            "the test needs one index for two symbols"
        );
        assert!(is_own_index(own_first.0) && !is_own_index(later.0));

        let id_later = get_symbol_id(&second, later);
        let id_own_first = get_symbol_id(&first, own_first);
        let id_own_second = get_symbol_id(&second, own_second);
        assert_ne!(id_own_first, id_later);
        assert_ne!(id_own_first, id_own_second);
        assert_ne!(id_own_second, id_later);
        assert_eq!(get_symbol_id(&binder, later), id_later);
        assert_eq!(
            get_symbol_id(&first, shared),
            get_symbol_id(&second, shared)
        );
        // An id stays when the thread switches between arenas.
        assert_eq!(get_symbol_id(&first, own_first), id_own_first);
        assert_eq!(get_symbol_id(&second, own_second), id_own_second);
        // `first` catches up to the later bind.
        assert_eq!(first.symbol_at_slot(second.id_slot(later)), None);
        first.catch_up(&binder, &[]);
        assert_eq!(first.symbol_at_slot(second.id_slot(later)), Some(later));
        assert_eq!(first.sym(later).name.as_str(), "later");
        assert_eq!(get_symbol_id(&first, later), id_later);
        assert_eq!(get_symbol_id(&first, own_first), id_own_first);

        // A dropped checker arena frees its ids on this thread.
        let key = first.id_slot(own_first).key;
        assert_ne!(key, 0, "a checker symbol has an id of its own");
        drop(first);
        OWN_SYMBOL_IDS.with(|own| {
            let own = own.borrow();
            assert!(own.key != key && !own.others.contains_key(&key));
        });
    }

    /// A shadow (`SymbolArena::push_shadow`) is the same Go symbol as its
    /// origin: one id, given on first use through either, and kept after
    /// the arena of the origin drops.
    #[test]
    fn shadows_have_the_id_of_their_origin() {
        let mut binder = SymbolArena::new();
        let mut first = binder.for_checker();
        // A later program binds a symbol that `first` does not have.
        let later = binder.new_symbol(SymbolFlags::NONE, "later");
        let mut second = binder.for_checker();
        let own = first.new_symbol(SymbolFlags::TRANSIENT, "own");

        let origin = first.id_slot(own);
        let shadow = second.push_shadow(first.sym(own).clone(), origin);
        assert_eq!(second.symbol_at_slot(origin), Some(shadow));
        assert_eq!(first.symbol_at_slot(second.id_slot(shadow)), Some(own));
        let id = get_symbol_id(&second, shadow);
        assert_eq!(get_symbol_id(&first, own), id);

        let lineage = second.id_slot(later);
        assert_eq!(lineage.key, 0, "a binder symbol has the shared slot");
        assert_eq!(first.symbol_at_slot(lineage), None);
        let shadow_later = first.push_shadow(second.sym(later).clone(), lineage);
        assert_eq!(
            get_symbol_id(&first, shadow_later),
            get_symbol_id(&binder, later)
        );

        drop(first);
        assert_eq!(get_symbol_id(&second, shadow), id);
    }

    /// An id chunk of `LineageIdTable` holds the symbols of one
    /// `SymbolArena` chunk, so a freed lineage range frees whole id chunks.
    #[test]
    fn lineage_id_chunks_are_symbol_chunks() {
        let mut arena = SymbolArena::new_file();
        arena.new_symbol(SymbolFlags::NONE, "a");
        arena.end_chunk();
        assert_eq!(arena.symbol_count(), LINEAGE_ID_CHUNK);
    }

    /// When the lineage frees the symbols of a dead file version, their id
    /// chunks go to the free list and come back with no ids. The other ids
    /// stay, and an id read of a freed symbol panics.
    #[test]
    #[should_panic(expected = "file version 4194297 is released")]
    fn freed_lineage_symbols_free_their_ids() {
        const FILE: usize = (1 << 22) - 7;
        const START: usize = 1 << 26;
        let ids = LineageIdTable::new();
        let kept = ids.id(START - 1);
        let dying = ids.id(START + 5);
        assert_eq!(ids.id(START + 5), dying);

        ids.free(FILE, START..START + 2 * LINEAGE_ID_CHUNK);
        let later = ids.id(START + 2 * LINEAGE_ID_CHUNK + 5);
        assert!(later > dying);
        assert_eq!(
            ids.alloc.lock().unwrap().next,
            2,
            "the freed id chunk is used again"
        );
        assert_eq!(ids.id(START - 1), kept);
        ids.id(START + LINEAGE_ID_CHUNK + 1);
    }

    /// Go keeps the id on the `*ast.Symbol`, which every checker shares: two
    /// checker arenas on two threads give a binder symbol one id, whichever
    /// thread reads it first. The process counter gives ids that no other
    /// symbol has.
    #[test]
    fn checkers_on_two_threads_share_binder_symbol_ids() {
        let mut binder = SymbolArena::new();
        let shared: Vec<SymbolId> = (0..2000)
            .map(|i| binder.new_symbol(SymbolFlags::NONE, format!("s{i}").as_str()))
            .collect();
        let read = |mut symbols: SymbolArena, shared: Vec<SymbolId>| {
            std::thread::spawn(move || {
                let own = symbols.new_symbol(SymbolFlags::TRANSIENT, "own");
                let mut ids: Vec<u64> = shared
                    .iter()
                    .map(|&symbol| get_symbol_id(&symbols, symbol))
                    .collect();
                ids.push(get_symbol_id(&symbols, own));
                ids
            })
        };
        let first = read(binder.for_checker(), shared.clone());
        let second = read(binder.for_checker(), shared.iter().rev().copied().collect());
        let mut first = first.join().unwrap();
        let mut second = second.join().unwrap();
        let (own_first, own_second) = (first.pop().unwrap(), second.pop().unwrap());
        second.reverse();
        assert_eq!(first, second);
        let ids: FxHashSet<u64> = first.iter().copied().collect();
        assert_eq!(ids.len(), shared.len(), "one id per binder symbol");
        assert!(own_first != own_second && !ids.contains(&own_first) && !ids.contains(&own_second));
        assert_eq!(get_symbol_id(&binder, shared[7]), first[7]);
    }
}

#[cfg(test)]
mod binary_view_tests {
    use super::*;

    /// binderview1: a `BinaryView` holds what the field accessors read:
    /// before the publish (the accessor path), after it in an alias-free
    /// store (`FrozenIds::Direct`) and in a store with an alias slot (a
    /// synthetic right operand, `FrozenIds::Records`), and with no ids. It
    /// publishes, so no other test may build or publish stores while it runs
    /// (the runner uses one thread).
    #[test]
    fn binary_view_reads_what_the_accessors_read() {
        let plain = new_file_store("/binaryview/a.ts", "x = y;");
        let aliased = new_file_store("/binaryview/b.ts", "x + s;");
        let make = |file: usize, operator: SyntaxKind, right: Node| {
            let f = NodeFactory::for_file(file);
            let left = f.new_identifier("x");
            let operator_token = f.new_token(operator);
            f.new_binary_expression(ModifierList::NIL, left, Node::NIL, operator_token, right)
        };
        let a = make(
            plain,
            SyntaxKind::EqualsToken,
            NodeFactory::for_file(plain).new_identifier("y"),
        );
        let b = make(
            aliased,
            SyntaxKind::PlusToken,
            NodeFactory::new().new_identifier("s"),
        );
        let check = |when: &str| {
            for n in [a, b] {
                let operator_token = n.operator_token();
                let read = BinaryView {
                    left: n.left(),
                    operator_token,
                    operator: operator_token.kind(),
                    right: n.right(),
                    type_: n.type_(),
                };
                assert_eq!(BinaryView::new(n), read, "{when}");
                assert_eq!(BinaryView::with_ids(n, None), read, "{when}");
            }
        };
        check("built");
        freeze_file_store(plain);
        freeze_file_store(aliased);
        crate::program::publish_parsed_files("/");
        assert!(frozen_store_ast_node(a).is_some(), "a is published");
        assert!(matches!(
            frozen_store_ids(plain),
            Some(FrozenIds::Direct { .. })
        ));
        assert!(matches!(
            frozen_store_ids(aliased),
            Some(FrozenIds::Records { .. })
        ));
        check("published");
        assert_eq!(BinaryView::new(a).operator, SyntaxKind::EqualsToken);
        assert_eq!(BinaryView::new(b).operator, SyntaxKind::PlusToken);
    }
}
