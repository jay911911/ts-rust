//! infmemo1: a memo of top-level inference walks (Go `inferTypes`).
//!
//! PERF: not in Go. `inferTypes` (inference.go:53) runs one walk
//! `inferFromTypes(n, source, target)`. A walk whose path reads only final
//! values makes the same info writes each time it runs with the same
//! inputs, and a later run fills no cache and makes nothing. Such a walk is
//! stored by its inputs. A later walk with equal inputs writes the stored
//! outcome and does not walk. The rule and its proof are in
//! `target/continuation-r97-goport/studies/tsrsperf1/infmemo1/go-model.md`
//! (round 2, sections 7 and 13). In short:
//! - R0: lookups and stores run only outside an LSP rollback scope
//!   (services.go:367 `runWithoutResolvedSignatureCaching`).
//! - R1 key: source, target, `n.priority`, `contravariant`, and per info
//!   (at most 8) the type parameter, both candidate lists in order, priority,
//!   `topLevel`, `isFixed`, implied arity and whether the lists exist.
//! - R5 start: no instantiation (active mapper), variance computation,
//!   reverse mapped inference, early-set member resolution, type predicate
//!   inference or indexed access simplification is in progress; the
//!   instantiation count is under the TS2589 limit; the serialization level
//!   is under its limit.
//! - R2-R4, R6, R7 during the walk: it made no type, symbol, signature,
//!   mapper, index info, predicate, instantiation map or inference context,
//!   ran no nested walk, left both instantiation counts as they were, reset
//!   no count, added no diagnostic (also no deferred one), added no
//!   reliability bits, and read no transient state (taints).
//! - R8 tuning: the walk took at least `min_steps` steps, and an earlier walk
//!   of that many steps marked the target.
//! - A hit writes each changed info's candidate lists, priority and
//!   `topLevel`, then clears the cached inferences when the walk did.
//!
//! `GOPORT_INFERMEMO`: unset or `1` on, `0` off, `verify` walks every hit
//! again from the same state and panics when the walk differs from the entry
//! or has any effect. `GOPORT_INFERMEMO_STATS=<file>` writes the process
//! totals when the compile ends. `GOPORT_INFERMEMO_MAXHITS=<n>` replays only
//! the first n hits of the process (to find a hit that changes output).
//! `GOPORT_INFERMEMO_MIN_STEPS` sets the 16 (0 keys every walk).
//! `GOPORT_INFERMEMO_IGNORE=<rule>,...` (names as in the stats) stores walks
//! that break only those rules: a test of a rule, never for real runs.

use crate::checker::inference_p1::InferenceState;
use crate::prelude::*;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InferMemoMode {
    Off,
    On,
    Verify,
}

/// The mode when `GOPORT_INFERMEMO` is unset.
const DEFAULT_MODE: InferMemoMode = InferMemoMode::On;

/// At most this many infos in a key (port tuning, as tsrs).
const MAX_INFOS: usize = 8;

/// Go: checker/checker.go:22502 the TS2589 count limit.
const INSTANTIATION_COUNT_LIMIT: u32 = 5_000_000;

/// `GOPORT_INFERMEMO`, read once per process.
pub fn infer_memo_mode_from_env() -> InferMemoMode {
    static MODE: OnceLock<InferMemoMode> = OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("GOPORT_INFERMEMO").as_deref() {
        Ok("0") => InferMemoMode::Off,
        Ok("1") => InferMemoMode::On,
        Ok("verify") => InferMemoMode::Verify,
        _ => DEFAULT_MODE,
    })
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.parse().ok()
}

/// `GOPORT_INFERMEMO_IGNORE`, as rule bits.
fn ignored_rules_from_env() -> u32 {
    static RULES: OnceLock<u32> = OnceLock::new();
    *RULES.get_or_init(|| {
        let names = std::env::var("GOPORT_INFERMEMO_IGNORE").unwrap_or_default();
        names
            .split(',')
            .filter_map(|name| RULE_NAMES.iter().position(|r| *r == name))
            .fold(0, |bits, rule| bits | 1 << rule)
    })
}

fn stats_path() -> Option<&'static str> {
    static PATH: OnceLock<Option<String>> = OnceLock::new();
    PATH.get_or_init(|| std::env::var("GOPORT_INFERMEMO_STATS").ok())
        .as_deref()
}

fn max_hits() -> u64 {
    static MAX: OnceLock<u64> = OnceLock::new();
    *MAX.get_or_init(|| env_u64("GOPORT_INFERMEMO_MAXHITS").unwrap_or(u64::MAX))
}

static REPLAYED: AtomicU64 = AtomicU64::new(0);

/// Why a long keyed walk was not stored. A walk can break several rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Rule {
    /// R2: made a type, symbol, signature, mapper, index info, predicate,
    /// instantiation map or inference context.
    Created,
    /// R2: ran another `inferTypes` (so every clear was of this walk's
    /// infos).
    Nested,
    /// R3: an instantiation count changed.
    Instantiated,
    /// R3: an instantiation count reset ran (checker.go:2287, :2550, :7735).
    Reset,
    /// R4: a diagnostic, suggestion or deferred diagnostic was added.
    Diagnostic,
    /// R5: started inside an instantiation (`activeMappers` not empty).
    InInstantiation,
    /// R5: started inside a variance computation (relater.go:1334).
    Variance,
    /// R5: started inside a reverse mapped inference (inference.go:1066).
    ReverseMapped,
    /// R5: started inside `resolveAnonymousTypeMembers` or
    /// `resolveMappedTypeMembers`, which set members early.
    EarlyMembers,
    /// R5 (C1): started inside a type predicate inference
    /// (relater.go:2068).
    Predicate,
    /// R5 (C2): started inside an indexed access simplification
    /// (checker.go:28385).
    Simplify,
    /// R5 (G4): started at the TS2589 count limit.
    CountLimit,
    /// R5: started at the maximum serialization level, where diagnostics
    /// are dropped (checker.go:14221).
    Serialization,
    /// R6: the walk added reliability bits.
    Reliability,
    /// R7: read a resolution older than the walk, `resolvingSignature`, a
    /// `flowTypeCache` entry, or a flow limit or loop label.
    Taint,
    /// R7 (C3): read the type of a parameter that Go does not store
    /// (checker.go:16875).
    ParamTaint,
}

const RULES: usize = 16;
const RULE_NAMES: [&str; RULES] = [
    "created",
    "nested",
    "instantiated",
    "reset",
    "diagnostic",
    "in_instantiation",
    "variance",
    "reverse_mapped",
    "early_members",
    "predicate",
    "simplify",
    "count_limit",
    "serialization",
    "reliability",
    "taint",
    "param_taint",
];

#[derive(Clone, Copy)]
#[repr(usize)]
enum Stat {
    Walks,
    Lookups,
    Hits,
    HitSteps,
    Stores,
    Short,
    Unkeyed,
    Scoped,
    OverMax,
    Verified,
    Lost,
}
const STATS: usize = 11;
const STAT_NAMES: [&str; STATS] = [
    "walks",
    "lookups",
    "hits",
    "hit_steps",
    "stores",
    "short",
    "unkeyed",
    "scoped",
    "over_max",
    "verified",
    "lost",
];
const SLOTS: usize = STATS + 2 * RULES;
static TOTALS: [AtomicU64; SLOTS] = [const { AtomicU64::new(0) }; SLOTS];

/// Writes the process totals to `GOPORT_INFERMEMO_STATS` (called once the
/// compile is done, `emit_and_report_statistics`).
pub fn write_infer_memo_stats() {
    let Some(path) = stats_path() else {
        return;
    };
    let total: Vec<u64> = TOTALS.iter().map(|a| a.load(Ordering::Relaxed)).collect();
    let mut text = format!("mode {:?}", infer_memo_mode_from_env());
    for (i, name) in STAT_NAMES.iter().enumerate() {
        text.push_str(&format!(" {name} {}", total[i]));
    }
    for (i, name) in RULE_NAMES.iter().enumerate() {
        text.push_str(&format!(" lost_{name} {}", total[STATS + i]));
    }
    for (i, name) in RULE_NAMES.iter().enumerate() {
        text.push_str(&format!(" only_{name} {}", total[STATS + RULES + i]));
    }
    text.push('\n');
    let _ = std::fs::write(path, text);
}

/// What a stored walk wrote to one info.
#[derive(Clone, Debug)]
struct InfoOutcome {
    index: u8,
    lists: Option<Box<InferenceCandidates>>,
    priority: InferencePriority,
    top_level: bool,
}

struct Entry {
    /// The infos that the walk changed.
    changed: Box<[InfoOutcome]>,
    /// The walk called `clearCachedInferences` (inference.go:200, :204,
    /// :209).
    cleared: bool,
    steps: u32,
    /// Verify mode: the hash of the walk's steps.
    trace: u64,
}

/// One keyed walk, for tests (`InferMemo::log`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoEvent {
    Hit,
    Stored,
    Short,
    /// Not stored: the bits are `1 << Rule as u32`.
    Lost(u32),
}

/// The inference walk memo of one checker (`Checker::infer_memo`). The
/// hook counters are bumped at their Go sites in other files.
pub struct InferMemo {
    pub mode: InferMemoMode,
    pub min_steps: u32,
    /// Tests of a rule: walks that break only these rules are stored.
    pub ignored_rules: u32,
    /// `inferFromTypes` calls.
    pub steps: u32,
    /// `inferTypes` calls.
    pub walks: u32,
    /// Instantiation count resets (checker.go:2287, :2550, :7735).
    pub resets: u32,
    /// `clearCachedInferences` calls of `inferFromTypes`.
    pub clears: u32,
    /// Transient reads (R7).
    pub taints: u32,
    /// Parameter types that Go does not store (C3).
    pub param_taints: u32,
    /// The lowest index of a resolution cycle start (R7).
    pub taint_min_resolution: u32,
    /// The lowest index of an in-process loop label (R7).
    pub taint_min_loop: u32,
    /// Open `runWithoutResolvedSignatureCaching` scopes (R0).
    pub rollback_depth: u32,
    /// Running `resolveAnonymousTypeMembers` and `resolveMappedTypeMembers`.
    pub early_members_depth: u32,
    /// Running type predicate inferences from bodies (C1).
    pub predicate_depth: u32,
    /// Running indexed access simplifications (C2).
    pub simplify_depth: u32,
    /// Verify mode: `trace` takes each step.
    pub tracing: bool,
    pub trace: u64,
    /// Verify walks of hits that run now (no lookups or stores in them).
    bypass: u32,
    entries: FxHashMap<Box<[u32]>, Entry>,
    long_targets: FxHashSet<TypeId>,
    key: Vec<u32>,
    stats: bool,
    pub counts: [u64; SLOTS],
    /// Tests: each keyed walk.
    pub log: Option<Vec<(TypeId, TypeId, MemoEvent)>>,
}

impl Default for InferMemo {
    fn default() -> Self {
        let mode = infer_memo_mode_from_env();
        InferMemo {
            mode,
            min_steps: env_u64("GOPORT_INFERMEMO_MIN_STEPS").map_or(16, |v| v as u32),
            ignored_rules: ignored_rules_from_env(),
            steps: 0,
            walks: 0,
            resets: 0,
            clears: 0,
            taints: 0,
            param_taints: 0,
            taint_min_resolution: u32::MAX,
            taint_min_loop: u32::MAX,
            rollback_depth: 0,
            early_members_depth: 0,
            predicate_depth: 0,
            simplify_depth: 0,
            tracing: mode == InferMemoMode::Verify,
            trace: 0,
            bypass: 0,
            entries: FxHashMap::default(),
            long_targets: FxHashSet::default(),
            key: Vec::new(),
            stats: stats_path().is_some(),
            counts: [0; SLOTS],
            log: None,
        }
    }
}

impl std::fmt::Debug for InferMemo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferMemo")
            .field("mode", &self.mode)
            .field("entries", &self.entries.len())
            .finish_non_exhaustive()
    }
}

/// The per-info parts of a key (`infer_memo_key`): 4 words, then each
/// candidate list as its length and its ids.
fn info_segments(key: &[u32]) -> impl Iterator<Item = &[u32]> {
    let mut rest = &key[5..];
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let first = 4 + 1 + rest[4] as usize;
        let len = first + 1 + rest[first] as usize;
        let (segment, tail) = rest.split_at(len);
        rest = tail;
        Some(segment)
    })
}

const TRACE_PRIME: u64 = 0x0000_0100_0000_01b3;

impl InferMemo {
    /// Sets the mode (tests). Verify mode traces each step.
    pub fn set_mode(&mut self, mode: InferMemoMode) {
        self.mode = mode;
        self.tracing = mode == InferMemoMode::Verify;
    }

    fn count(&mut self, slot: usize, n: u64) {
        self.counts[slot] += n;
        if self.stats {
            TOTALS[slot].fetch_add(n, Ordering::Relaxed);
        }
    }

    fn event(&mut self, source: TypeId, target: TypeId, event: MemoEvent) {
        if let Some(log) = self.log.as_mut() {
            log.push((source, target, event));
        }
    }

    /// Verify mode: adds one `inferFromTypes` step to the trace hash.
    pub fn trace_step(&mut self, n: &InferenceState, source: TypeId, target: TypeId) {
        use std::hash::{Hash, Hasher};
        let mut h = rustc_hash::FxHasher::default();
        (
            source.0,
            target.0,
            n.priority.bits(),
            n.contravariant,
            n.bivariant,
            n.propagation_type.0,
        )
            .hash(&mut h);
        self.trace = self
            .trace
            .wrapping_mul(TRACE_PRIME)
            .wrapping_add(h.finish());
    }

    /// The hash of the last `steps` steps, from the hash before them.
    fn trace_since(&self, before: u64, steps: u32) -> u64 {
        let (mut power, mut base, mut e) = (1u64, TRACE_PRIME, steps);
        while e > 0 {
            if e & 1 == 1 {
                power = power.wrapping_mul(base);
            }
            base = base.wrapping_mul(base);
            e >>= 1;
        }
        self.trace.wrapping_sub(before.wrapping_mul(power))
    }
}

/// The counts that a stored walk must leave as they were (R2-R4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Effects {
    types: u32,
    symbols: u32,
    signatures: u32,
    mappers: usize,
    index_infos: usize,
    type_predicates: usize,
    instantiation_maps: usize,
    inference_contexts: usize,
    walks: u32,
    total_instantiations: u32,
    instantiations: u32,
    resets: u32,
    diagnostics: i32,
    suggestions: i32,
    deferred_diagnostics: usize,
}

/// Verify mode: sizes of the caches and stacks that a walk can fill.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CacheSizes {
    relations: [i32; 5],
    enum_relation: usize,
    reverse_mapped: usize,
    reverse_homomorphic_mapped: usize,
    cached_types: usize,
    cached_signatures: usize,
    subtype_reduction: usize,
    iteration_types: usize,
    flow_loop: usize,
    flow_node_reachable: usize,
    context_free_types: usize,
    instantiation_stack: usize,
    active_mappers: usize,
    type_resolutions: usize,
    flow_analysis_disabled: bool,
}

/// What a walk read and did besides its info writes.
struct WalkReads {
    effects_before: Effects,
    taints: u32,
    param_taints: u32,
    clears: u32,
    saved_min_resolution: u32,
    saved_min_loop: u32,
    resolutions: usize,
    loops: usize,
    saved_reliability: RelationComparisonResult,
    steps: u32,
    trace: u64,
}

/// The state of one info that a walk can write.
#[derive(Clone, Debug, PartialEq)]
struct InfoState {
    candidates: Vec<TypeId>,
    contra_candidates: Vec<TypeId>,
    lists: bool,
    priority: InferencePriority,
    top_level: bool,
}

impl Checker {
    fn infer_memo_effects(&self) -> Effects {
        Effects {
            types: self.type_count,
            symbols: self.symbol_count,
            signatures: self.signature_count,
            mappers: self.mappers.len(),
            index_infos: self.index_infos.len(),
            type_predicates: self.type_predicates.len(),
            instantiation_maps: self.object_type_instantiations.len(),
            inference_contexts: self.inference_contexts.len(),
            walks: self.infer_memo.walks,
            total_instantiations: self.total_instantiation_count,
            instantiations: self.instantiation_count,
            resets: self.infer_memo.resets,
            diagnostics: self.diagnostics.count,
            suggestions: self.suggestion_diagnostics.count,
            deferred_diagnostics: self.deferred_diagnostic_callbacks.len(),
        }
    }

    fn infer_memo_cache_sizes(&self) -> CacheSizes {
        CacheSizes {
            relations: [
                self.subtype_relation.borrow().size(),
                self.strict_subtype_relation.borrow().size(),
                self.assignable_relation.borrow().size(),
                self.comparable_relation.borrow().size(),
                self.identity_relation.borrow().size(),
            ],
            enum_relation: self.enum_relation.len(),
            reverse_mapped: self.reverse_mapped_cache.len(),
            reverse_homomorphic_mapped: self.reverse_homomorphic_mapped_cache.len(),
            cached_types: self.cached_types.len(),
            cached_signatures: self.cached_signatures.len(),
            subtype_reduction: self.subtype_reduction_cache.len(),
            iteration_types: self.iteration_types_cache.len(),
            flow_loop: self.flow_loop_cache.len(),
            flow_node_reachable: self.flow_node_reachable.len(),
            context_free_types: self.context_free_types.len(),
            instantiation_stack: self.instantiation_stack.len(),
            active_mappers: self.active_mappers.len(),
            type_resolutions: self.type_resolutions.len(),
            flow_analysis_disabled: self.flow_analysis_disabled,
        }
    }

    fn infer_memo_info_states(&self, ctx: InferenceContextId) -> Vec<InfoState> {
        self.inference_context(ctx)
            .inferences
            .iter()
            .map(|i| InfoState {
                candidates: i.candidates().to_vec(),
                contra_candidates: i.contra_candidates().to_vec(),
                lists: i.candidate_lists.is_some(),
                priority: i.priority,
                top_level: i.top_level,
            })
            .collect()
    }

    /// R5: the rules that the state at the start of a walk breaks.
    fn infer_memo_start_rules(&self) -> u32 {
        let m = &self.infer_memo;
        let mut rules = 0;
        let mut add = |broken: bool, rule: Rule| {
            if broken {
                rules |= 1 << rule as u32;
            }
        };
        add(!self.active_mappers.is_empty(), Rule::InInstantiation);
        add(!self.variance_stack.is_empty(), Rule::Variance);
        add(
            !self.reverse_mapped_source_stack.is_empty()
                || !self.reverse_mapped_target_stack.is_empty(),
            Rule::ReverseMapped,
        );
        add(m.early_members_depth != 0, Rule::EarlyMembers);
        add(m.predicate_depth != 0, Rule::Predicate);
        add(m.simplify_depth != 0, Rule::Simplify);
        add(
            self.instantiation_count >= INSTANTIATION_COUNT_LIMIT,
            Rule::CountLimit,
        );
        add(
            self.serialization_level >= MAX_SERIALIZATION_LEVEL,
            Rule::Serialization,
        );
        rules
    }

    /// Builds the key of a walk (R1) in `key`. False when the walk has no
    /// context or more than `MAX_INFOS` infos.
    fn infer_memo_key(
        &self,
        key: &mut Vec<u32>,
        n: &InferenceState,
        source: TypeId,
        target: TypeId,
    ) -> bool {
        if n.inferences.is_nil() {
            return false;
        }
        let infos = &self.inference_context(n.inferences).inferences;
        if infos.len() > MAX_INFOS {
            return false;
        }
        key.clear();
        key.extend_from_slice(&[
            source.0,
            target.0,
            n.priority.bits() as u32,
            u32::from(n.contravariant),
            infos.len() as u32,
        ]);
        for info in infos.iter() {
            key.extend_from_slice(&[
                info.type_parameter.0,
                info.priority.bits() as u32,
                u32::from(info.top_level)
                    | u32::from(info.is_fixed) << 1
                    | u32::from(info.candidate_lists.is_some()) << 2,
                info.implied_arity as u32,
            ]);
            for list in [info.candidates(), info.contra_candidates()] {
                key.push(list.len() as u32);
                key.extend(list.iter().map(|t| t.0));
            }
        }
        true
    }

    /// Starts recording what a walk reads and does.
    fn infer_memo_begin(&mut self) -> WalkReads {
        let reads = WalkReads {
            effects_before: self.infer_memo_effects(),
            taints: self.infer_memo.taints,
            param_taints: self.infer_memo.param_taints,
            clears: self.infer_memo.clears,
            saved_min_resolution: self.infer_memo.taint_min_resolution,
            saved_min_loop: self.infer_memo.taint_min_loop,
            resolutions: self.type_resolutions.len(),
            loops: self.flow_loop_stack.len(),
            saved_reliability: self.reliability_flags,
            steps: self.infer_memo.steps,
            trace: self.infer_memo.trace,
        };
        self.infer_memo.taint_min_resolution = u32::MAX;
        self.infer_memo.taint_min_loop = u32::MAX;
        // R6: each reader of the flags saves and zeroes them first
        // (relater.go:1371, :3155), so no reader inside the walk sees this
        // zero. `infer_memo_end` ORs the saved value back, as Go would have.
        self.reliability_flags = RelationComparisonResult::NONE;
        reads
    }

    /// Ends the recording of `infer_memo_begin`. Returns the rules that the
    /// walk broke (R2-R4, R6, R7), its steps, its trace hash and whether it
    /// cleared the cached inferences.
    fn infer_memo_end(&mut self, reads: &WalkReads) -> (u32, u32, u64, bool) {
        let after = self.infer_memo_effects();
        let before = &reads.effects_before;
        let added_reliability = self.reliability_flags;
        self.reliability_flags = reads.saved_reliability | added_reliability;
        let m = &mut self.infer_memo;
        let steps = m.steps.wrapping_sub(reads.steps);
        let trace = if m.tracing {
            m.trace_since(reads.trace, steps)
        } else {
            0
        };
        let tainted = m.taints != reads.taints
            || (m.taint_min_resolution as usize) < reads.resolutions
            || (m.taint_min_loop as usize) < reads.loops;
        m.taint_min_resolution = reads.saved_min_resolution.min(m.taint_min_resolution);
        m.taint_min_loop = reads.saved_min_loop.min(m.taint_min_loop);
        let mut rules = 0;
        let mut add = |broken: bool, rule: Rule| {
            if broken {
                rules |= 1 << rule as u32;
            }
        };
        add(
            after.types != before.types
                || after.symbols != before.symbols
                || after.signatures != before.signatures
                || after.mappers != before.mappers
                || after.index_infos != before.index_infos
                || after.type_predicates != before.type_predicates
                || after.instantiation_maps != before.instantiation_maps
                || after.inference_contexts != before.inference_contexts,
            Rule::Created,
        );
        add(after.walks != before.walks, Rule::Nested);
        add(
            after.total_instantiations != before.total_instantiations
                || after.instantiations != before.instantiations,
            Rule::Instantiated,
        );
        add(after.resets != before.resets, Rule::Reset);
        add(
            after.diagnostics != before.diagnostics
                || after.suggestions != before.suggestions
                || after.deferred_diagnostics != before.deferred_diagnostics,
            Rule::Diagnostic,
        );
        add(!added_reliability.is_empty(), Rule::Reliability);
        add(tainted, Rule::Taint);
        add(m.param_taints != reads.param_taints, Rule::ParamTaint);
        (rules, steps, trace, m.clears != reads.clears)
    }

    /// `infer_from_types` at the top of `infer_types`, through the memo.
    pub fn infer_from_types_memo(
        &mut self,
        n: &mut InferenceState,
        source: TypeId,
        target: TypeId,
    ) {
        self.infer_memo.walks = self.infer_memo.walks.wrapping_add(1);
        if self.infer_memo.bypass != 0 {
            self.infer_from_types(n, source, target);
            return;
        }
        self.infer_memo.count(Stat::Walks as usize, 1);
        if self.infer_memo.rollback_depth != 0 {
            // R0: the rollback scope empties caches and puts them back.
            self.infer_memo.count(Stat::Scoped as usize, 1);
            self.infer_from_types(n, source, target);
            return;
        }
        if self.infer_memo.min_steps != 0 && !self.infer_memo.long_targets.contains(&target) {
            let before = self.infer_memo.steps;
            self.infer_from_types(n, source, target);
            if self.infer_memo.steps.wrapping_sub(before) >= self.infer_memo.min_steps {
                self.infer_memo.long_targets.insert(target);
            }
            return;
        }
        let mut key = std::mem::take(&mut self.infer_memo.key);
        if !self.infer_memo_key(&mut key, n, source, target) {
            self.infer_memo.key = key;
            self.infer_memo.count(Stat::Unkeyed as usize, 1);
            self.infer_from_types(n, source, target);
            return;
        }
        self.infer_memo.count(Stat::Lookups as usize, 1);
        if self.infer_memo.entries.contains_key(key.as_slice()) {
            if max_hits() != u64::MAX && REPLAYED.fetch_add(1, Ordering::Relaxed) >= max_hits() {
                self.infer_memo.key = key;
                self.infer_memo.count(Stat::OverMax as usize, 1);
                self.infer_from_types(n, source, target);
                return;
            }
            self.infer_memo.count(Stat::Hits as usize, 1);
            self.infer_memo.event(source, target, MemoEvent::Hit);
            if self.infer_memo.mode == InferMemoMode::Verify {
                self.infer_memo_verify(n, source, target, &key);
            } else {
                self.infer_memo_replay(n.inferences, &key);
            }
            self.infer_memo.key = key;
            return;
        }
        // A miss: walk, then store the walk when the rules hold.
        let start_rules = self.infer_memo_start_rules();
        let ctx = n.inferences;
        let reads = self.infer_memo_begin();
        self.infer_from_types(n, source, target);
        let (walk_rules, steps, trace, cleared) = self.infer_memo_end(&reads);
        if steps < self.infer_memo.min_steps {
            self.infer_memo.key = key;
            self.infer_memo.count(Stat::Short as usize, 1);
            self.infer_memo.event(source, target, MemoEvent::Short);
            return;
        }
        let rules = start_rules | walk_rules;
        if rules & !self.infer_memo.ignored_rules != 0 {
            self.infer_memo.key = key;
            self.infer_memo.count(Stat::Lost as usize, 1);
            for rule in 0..RULES {
                if rules & (1 << rule) != 0 {
                    self.infer_memo.count(STATS + rule, 1);
                    if rules.count_ones() == 1 {
                        self.infer_memo.count(STATS + RULES + rule, 1);
                    }
                }
            }
            self.infer_memo
                .event(source, target, MemoEvent::Lost(rules));
            return;
        }
        // The key holds the info states before the walk (R1).
        let mut after_key = Vec::with_capacity(key.len());
        self.infer_memo_key(&mut after_key, n, source, target);
        let unchanged: Vec<bool> = info_segments(&key)
            .zip(info_segments(&after_key))
            .map(|(a, b)| a == b)
            .collect();
        let infos = &self.inference_context(ctx).inferences;
        let changed = (0..infos.len())
            .filter(|&i| !unchanged[i])
            .map(|i| InfoOutcome {
                index: i as u8,
                lists: infos[i].candidate_lists.clone(),
                priority: infos[i].priority,
                top_level: infos[i].top_level,
            })
            .collect();
        self.infer_memo.entries.insert(
            key.into_boxed_slice(),
            Entry {
                changed,
                cleared,
                steps,
                trace,
            },
        );
        self.infer_memo.count(Stat::Stores as usize, 1);
        self.infer_memo.event(source, target, MemoEvent::Stored);
    }

    /// A hit: writes the stored outcome (Go's walk would write the same).
    fn infer_memo_replay(&mut self, ctx: InferenceContextId, key: &[u32]) {
        let entry = &self.infer_memo.entries[key];
        let infos = &mut self.inference_contexts[ctx.index()].inferences;
        for o in entry.changed.iter() {
            let info = &mut infos[o.index as usize];
            info.candidate_lists.clone_from(&o.lists);
            info.priority = o.priority;
            info.top_level = o.top_level;
        }
        let (cleared, steps) = (entry.cleared, entry.steps);
        // The steps that Go's walk takes, for the long-walk marks.
        self.infer_memo.steps = self.infer_memo.steps.wrapping_add(steps);
        self.infer_memo
            .count(Stat::HitSteps as usize, u64::from(steps));
        if cleared {
            self.clear_cached_inferences(ctx);
        }
    }

    /// Verify mode, a hit: runs Go's walk from this state and panics when
    /// its outcome, steps or trace differ from the entry, or when it has any
    /// effect besides the info writes.
    fn infer_memo_verify(
        &mut self,
        n: &mut InferenceState,
        source: TypeId,
        target: TypeId,
        key: &[u32],
    ) {
        let ctx = n.inferences;
        let mut expected = self.infer_memo_info_states(ctx);
        let entry = &self.infer_memo.entries[key];
        for o in entry.changed.iter() {
            let state = &mut expected[o.index as usize];
            state.lists = o.lists.is_some();
            state.candidates = o
                .lists
                .as_ref()
                .map_or(Vec::new(), |l| l.candidates.clone());
            state.contra_candidates = o
                .lists
                .as_ref()
                .map_or(Vec::new(), |l| l.contra_candidates.clone());
            state.priority = o.priority;
            state.top_level = o.top_level;
        }
        let (entry_cleared, entry_steps, entry_trace) = (entry.cleared, entry.steps, entry.trace);
        let start_rules = self.infer_memo_start_rules();
        let caches_before = self.infer_memo_cache_sizes();
        let reads = self.infer_memo_begin();
        self.infer_memo.bypass += 1;
        self.infer_from_types(n, source, target);
        self.infer_memo.bypass -= 1;
        let (rules, steps, trace, cleared) = self.infer_memo_end(&reads);
        let caches_after = self.infer_memo_cache_sizes();
        let outcome = self.infer_memo_info_states(ctx);
        self.infer_memo
            .count(Stat::HitSteps as usize, u64::from(steps));
        self.infer_memo.count(Stat::Verified as usize, 1);
        let problem = if outcome != expected {
            Some(format!("outcome {outcome:?}, entry {expected:?}"))
        } else if cleared != entry_cleared {
            Some(format!("cleared {cleared}, entry {entry_cleared}"))
        } else if steps != entry_steps || trace != entry_trace {
            Some(format!(
                "steps {steps} trace {trace:x}, entry steps {entry_steps} trace {entry_trace:x}"
            ))
        } else if rules != 0 {
            let names: Vec<&str> = (0..RULES)
                .filter(|r| rules & (1 << r) != 0)
                .map(|r| RULE_NAMES[r])
                .collect();
            Some(format!(
                "the walk broke {names:?} (start rules {start_rules:b})"
            ))
        } else if caches_after != caches_before {
            Some(format!("caches {caches_before:?} -> {caches_after:?}"))
        } else {
            None
        };
        if let Some(problem) = problem {
            let node = self.current_node;
            let file = if node.is_some() {
                source_file_file_name(get_source_file_of_node(node)).to_string()
            } else {
                String::new()
            };
            panic!(
                "infermemo verify: {file} pos {} ({:?}) source {} target {}: {problem}",
                if node.is_some() { node.pos() } else { -1 },
                if node.is_some() {
                    node.kind()
                } else {
                    SyntaxKind::Unknown
                },
                self.type_to_string(source),
                self.type_to_string(target),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Diagnostics as (code, start), and each keyed walk as (source,
    /// target, event).
    type Checked = (Vec<(i32, i32)>, Vec<(String, String, MemoEvent)>);

    /// Checks `a.ts` (`source`, strict) in a new project with the memo in
    /// `mode`, every walk keyed (`min_steps` 0) and the `ignored` rules not
    /// applied. Verify mode panics on a hit that differs from Go's walk.
    fn check_with_memo(label: &str, source: &str, mode: InferMemoMode, ignored: u32) -> Checked {
        let dir = std::env::temp_dir().join(format!(
            "ts_goport_infermemo_{label}_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.ts"), source).unwrap();
        std::fs::write(
            dir.join("tsconfig.json"),
            r#"{ "compilerOptions": { "strict": true, "target": "es2020", "types": [] }, "files": ["a.ts"] }"#,
        )
        .unwrap();
        let config = dir.join("tsconfig.json");
        let program = crate::program::try_load_version(&config.to_string_lossy(), |_| {})
            .unwrap_or_else(|e| panic!("cannot load {}: {e}", config.display()));
        let _ = std::fs::remove_dir_all(&dir);
        let scope = crate::core::enter_program(Some(program));
        let file = program
            .source_files()
            .find(|file| file.info.file_name.ends_with("/a.ts"))
            .expect("a.ts is not in the program")
            .root;
        let result = crate::program::with_type_checker_for_file(file, move |checker| {
            checker.infer_memo.set_mode(mode);
            checker.infer_memo.min_steps = 0;
            checker.infer_memo.ignored_rules = ignored;
            checker.infer_memo.log = Some(Vec::new());
            let ctx = crate::gostd::context::background();
            let diagnostics = checker
                .get_diagnostics(&ctx, file, false)
                .iter()
                .map(|d| (d.code(), d.pos()))
                .collect();
            let log = checker.infer_memo.log.take().unwrap();
            let log = log
                .into_iter()
                .map(|(s, t, e)| (checker.type_to_string(s), checker.type_to_string(t), e))
                .collect();
            (diagnostics, log)
        });
        drop(scope);
        crate::program::release_program(program);
        result
    }

    /// Checks `source` off, on and in verify mode, and once more on with
    /// `rule` not applied. Asserts that on and verify give the diagnostics of
    /// off, which has the `codes` of Go (tsgo at pin 673a5f17d713), that the
    /// walk `walk` broke `rule`, that no walk with its key hit before a later
    /// one was stored, and, when `unguarded_differs`, that a hit without the
    /// rule changes the output.
    fn assert_rule_guards(
        label: &str,
        source: &str,
        codes: &[i32],
        rule: Rule,
        walk: (&str, &str),
        unguarded_differs: bool,
    ) {
        let bit = 1 << rule as u32;
        let (off, _) = check_with_memo(&format!("{label}off"), source, InferMemoMode::Off, 0);
        assert_eq!(off.iter().map(|d| d.0).collect::<Vec<_>>(), codes);
        let (on, log) = check_with_memo(&format!("{label}on"), source, InferMemoMode::On, 0);
        assert_eq!(on, off);
        let (verified, _) =
            check_with_memo(&format!("{label}vfy"), source, InferMemoMode::Verify, 0);
        assert_eq!(verified, off);
        let events: Vec<MemoEvent> = log
            .iter()
            .filter(|(s, t, _)| (s.as_str(), t.as_str()) == walk)
            .map(|e| e.2)
            .collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, MemoEvent::Lost(r) if r & bit != 0)),
            "{walk:?} not lost to {rule:?}: {log:?}"
        );
        let first_store = events.iter().position(|e| *e == MemoEvent::Stored);
        let first_hit = events.iter().position(|e| *e == MemoEvent::Hit);
        assert!(
            first_hit.is_none_or(|hit| first_store.is_some_and(|store| store < hit)),
            "{walk:?} hit an entry of a walk that broke {rule:?}: {events:?}"
        );
        if unguarded_differs {
            let (unguarded, _) =
                check_with_memo(&format!("{label}ign"), source, InferMemoMode::On, bit);
            assert_ne!(unguarded, off, "{rule:?} changes nothing here");
        }
    }

    /// C1 (go-model.md 13): a walk inside the body check of a predicate
    /// inference reads `isS`'s predicate as none (relater.go:2068). Go's
    /// later walk reads `x is string` and infers `string` for `T`.
    #[test]
    fn predicate_in_progress_is_not_stored() {
        assert_rule_guards(
            "c1",
            "declare function isStr2(x: unknown, d: number): x is string;
declare function g<T>(o: { f: (x: unknown) => x is T }): T;
declare function h<U>(o: { f: (x: unknown) => x is U }): U;
g({ f: (x: unknown): x is number => true });
const obj = { f: isS };
h(obj);
function isS(x: unknown) { return isStr2(x, g(obj)); }
isStr2(0, g(obj));
",
            &[7023, 2345, 2345],
            Rule::Predicate,
            (
                "{ f: (x: unknown) => x is string; }",
                "{ f: (x: unknown) => x is T; }",
            ),
            true,
        );
    }

    /// C2: the first simplification of `X` reads `typeof obj2.x`, whose
    /// initializer runs a walk to `X` that reads the simplification cache
    /// as `X` itself (checker.go:28385). Go's later walk reads the
    /// simplified `number | T` and infers `string` for `T`.
    #[test]
    fn simplification_in_progress_is_not_stored() {
        assert_rule_guards(
            "c2",
            r#"const r1 = k("hello");
const obj2 = { x: (k("hello"), 1) };
const r = k("hello");
const s: string = r;
declare function k<T, K extends string>(v: { [P in K]: T | typeof obj2.x }[K]): T;
"#,
            &[7022],
            Rule::Simplify,
            (r#""hello""#, "{ [P in K]: number | T; }[K]"),
            true,
        );
    }

    /// C3: the walk of a failed call reads parameters of context-sensitive
    /// arrows, whose types Go does not store (checker.go:16875). Here the
    /// read parameter is annotated, so a hit would still be right: the test
    /// only shows that the taint keeps such walks out.
    #[test]
    fn parameter_of_context_sensitive_signature_is_not_stored() {
        let source = r#"interface P<T> { v: T }
declare function codec<A, B>(a: A, b: B, p: { decode: (value: A, payload: P<A>) => B; encode: (value: B, payload: P<B>) => A }): [A, B];
codec("s", 1, {
  decode: (value: never, _payload) => 1,
  encode: (value: number, _payload) => "x",
});
codec("s", 1, {
  decode: (value: never, _payload) => 1,
  encode: (value: number, _payload) => "x",
});
"#;
        assert_rule_guards(
            "c3",
            source,
            &[2322, 2322],
            Rule::ParamTaint,
            (
                "{ decode: (value: never, _payload: P<string>) => number; encode: (value: number, _payload: P<number>) => string; }",
                "{ decode: (value: A, payload: P<A>) => B; encode: (value: B, payload: P<B>) => A; }",
            ),
            false,
        );
    }
}
