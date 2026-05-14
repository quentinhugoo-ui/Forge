//! Public value types observed by callers of `MonsterNode`.

use crate::Hash;

/// V8 Solution C — read CPU cycles via RDTSC. Returns 0 on non-x86_64.
/// Used by the slow-lane dispatch (to populate `PhysicalEnvelope`) AND
/// by the Gödel benches (to filter samples interrupted mid-measurement).
/// Inlined so no function-call overhead pollutes the measurement.
#[inline(always)]
pub fn read_cycles() -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        std::arch::x86_64::_rdtsc()
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonsterStats {
    pub program_cache_hits: u64,
    pub program_cache_misses: u64,
    pub arg_cache_hits: u64,
    pub arg_cache_stores: u64,
    pub ram_memo_hits: u64,
    pub ram_value_hits: u64,
    pub git_memo_hits: u64,
    pub executions: u64,
    pub cache_denials: u64,
    pub batch_dedupe_hits: u64,
    pub rule_hits: u64,
    pub oracle_hits: u64,
    /// Phase 12.1 — Hash64 op-level memoization hits / misses, used
    /// by the slow-lane `execute_with_op_memo` interpreter when the
    /// program is `is_decomposable()`. For atomic / monomorphic
    /// programs (e.g. SplitMix64) these stay at 0 — silent.
    pub op_memo_hits: u64,
    pub op_memo_misses: u64,
    /// Number of times shadow execution caught a lying oracle and
    /// invalidated it. Should remain 0 in healthy steady-state.
    pub shadow_invalidations: u64,
    /// Number of synthetic probes fired by the always-on distillation
    /// daemon. Counts every probe attempted whether or not it
    /// ultimately produced a learned rule.
    pub distillations_attempted: u64,
    /// Number of programs the daemon successfully distilled into a
    /// learned oracle (one increment per program, not per probe).
    pub distillations_succeeded: u64,
}

impl MonsterStats {
    pub fn avoided(&self) -> u64 {
        self.ram_memo_hits
            + self.ram_value_hits
            + self.git_memo_hits
            + self.batch_dedupe_hits
            + self.rule_hits
            + self.oracle_hits
    }

    pub fn total_calls(&self) -> u64 {
        self.avoided() + self.executions
    }

    pub fn avoidance_ratio(&self) -> f64 {
        let total = self.total_calls();
        if total == 0 {
            0.0
        } else {
            self.avoided() as f64 / total as f64
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonsterSource {
    RamMemo,
    GitMemo,
    StructuralRule,
    LearnedOracle,
    ExecutedHot,
}

/// V8 Solution C — première brique : envelope physique observé pour
/// l'appel. Le `result_hash` reste canonique (computational, byte-exact
/// cross-machine) ; cet envelope porte les métriques *physiques* —
/// distinctes par machine, par run, par état thermique.
///
/// La Gödel-machine en V8 ζ utilisera la distribution de cet envelope
/// sur N machines du swarm pour préférer les rewrites qui réduisent
/// le coût physique réel — pas seulement la latence wall-time locale.
///
/// V7 γ.1 : seul `cycles` est rempli (RDTSC sur x86_64 ; 0 sinon, et 0
/// sur les fast lanes pour ne pas perturber le hot path). Les champs
/// `energy_uj` et `l3_misses` arrivent en V8 ζ via RAPL et
/// `perf_event_open`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhysicalEnvelope {
    /// Cycles CPU mesurés via RDTSC autour de l'exécution. 0 si non-x86,
    /// 0 sur les fast lanes (rule/oracle/static), 0 sur cache hits.
    pub cycles: u64,
    /// V8 ζ : énergie consommée en microjoules (RAPL). 0 en V7.
    pub energy_uj: u32,
    /// V8 ζ : cache misses L3 (perf_event_open). 0 en V7.
    pub l3_misses: u32,
}

#[derive(Debug, Clone)]
pub struct MonsterCall {
    pub result: Hash,
    pub source: MonsterSource,
    pub envelope: PhysicalEnvelope,
}

#[derive(Debug, Clone)]
pub struct MonsterValue {
    pub bytes: Vec<u8>,
    pub source: MonsterSource,
    pub envelope: PhysicalEnvelope,
}
