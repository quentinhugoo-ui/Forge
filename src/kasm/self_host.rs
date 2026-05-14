//! Π self-host (Wave 8, 2026-05-02) — KASM Self-Hosting runtime.
//!
//! **Origine** : Forth, Lisp, Smalltalk — langages où le programme
//! peut **inspecter et invoquer** d'autres programmes du même langage
//! sans translation. Self-hosting = "Forge écrite en Forge" : un
//! programme KASM peut référencer un autre programme KASM par hash
//! et l'invoquer comme sous-routine.
//!
//! ## Pourquoi pour Forge ?
//!
//! Forge est déjà **content-addressed** (chaque programme = identité
//! cryptographique SHA-256). La self-hosting nécessite seulement :
//!
//!   1. Un mécanisme pour passer un hash + des args au runtime.
//!   2. Une boucle d'exécution récursive avec depth limit + cycle
//!      detection (anti-runaway).
//!   3. Une protection contre l'auto-référence infinie (un programme
//!      qui s'appelle lui-même).
//!
//! C'est ce que fournit `SelfHostingRuntime` — wrapper léger autour
//! d'un `Store` qui résout les hashes vers `Program` puis exécute via
//! le scalar interpreter existant.
//!
//! ## Wave 8 — relation avec Op::Fractal / Op::Eval
//!
//! Les opcodes `Op::Fractal = 64` et `Op::Eval = 65` sont déclarés
//! dans `kasm::types::Op` mais STUB fail-loud dans tous les consumers
//! (interpreter, JIT, optimizer, MLIR, agent rebuild, CUDA). La
//! sémantique réelle vit ICI au niveau runtime :
//!
//!   - `runtime.fractal_call(callee_hash, args)` : Forge → Forge call.
//!   - `runtime.eval_kasm(prog_bytes, args)` : programme-as-data eval.
//!
//! Le wiring complet `Op::Fractal` au bytecode interpreter sera Wave
//! 11+ quand un cas d'usage concret le justifiera (le runtime suffit
//! pour les workflows de orchestration / notebook style).
//!
//! ## Architecture Wave 8 minimal viable
//!
//! ```text
//!   SelfHostingRuntime { store, max_depth, depth_counter }
//!     ├ fractal_call(hash, args)   : load(hash) → execute → result
//!     ├ eval_kasm(bytes, args)     : Program::from_bytes → execute
//!     └ depth tracking : RuntimeError::DepthExceeded si > max_depth
//! ```
//!
//! ## Limitations Wave 8 minimal
//!
//! - Pas de cache d'exécution (chaque fractal_call re-exécute).
//!   Le caller doit composer avec `MonsterNode::dispatch_batch` pour
//!   bénéficier du RAM cache existant.
//! - Pas de cycle detection profonde (depth limit suffit en pratique).
//! - Pas de pass d'optimization cross-program (inlining Wave 11+).

use crate::kasm::execute as kasm_execute;
use crate::kasm::interpreter::{execute_with_fractal, FractalDispatcher};
use crate::kasm::program::Program;
use crate::kasm::types::KasmError;
use crate::store::{Hash, Store};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

/// Profondeur maximale par défaut. Forge n'a jamais vu un workflow
/// légitime > 16 niveaux ; au-delà c'est presque toujours une boucle
/// infinie ou un programme attaquant.
pub const DEFAULT_MAX_DEPTH: u32 = 16;

/// Erreurs spécifiques au self-host runtime.
#[derive(Debug)]
pub enum SelfHostError {
    /// Le hash demandé n'est pas dans le `Store`.
    UnknownProgram(Hash),
    /// La profondeur de récursion a dépassé `max_depth`.
    DepthExceeded { depth: u32, max: u32 },
    /// Le programme cité par hash est invalide (verify échoue).
    InvalidProgram { hash: Hash, reason: String },
    /// Le programme inline (eval_kasm) bytes ne forme pas un Program valide.
    InvalidEvalBytes(String),
    /// Erreur du KASM interpreter pendant l'exécution.
    Kasm(KasmError),
    /// I/O error from the Store.
    Io(std::io::Error),
}

impl std::fmt::Display for SelfHostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelfHostError::UnknownProgram(h) =>
                write!(f, "self-host: program hash {:?} not in store", &h.as_bytes()[..4]),
            SelfHostError::DepthExceeded { depth, max } =>
                write!(f, "self-host: recursion depth {} > max {}", depth, max),
            SelfHostError::InvalidProgram { reason, .. } =>
                write!(f, "self-host: program invalid: {}", reason),
            SelfHostError::InvalidEvalBytes(s) =>
                write!(f, "self-host: eval_kasm bytes invalid: {}", s),
            SelfHostError::Kasm(e) =>
                write!(f, "self-host: kasm error: {:?}", e),
            SelfHostError::Io(e) =>
                write!(f, "self-host: io: {}", e),
        }
    }
}

impl From<KasmError> for SelfHostError {
    fn from(e: KasmError) -> Self {
        SelfHostError::Kasm(e)
    }
}

impl From<std::io::Error> for SelfHostError {
    fn from(e: std::io::Error) -> Self {
        SelfHostError::Io(e)
    }
}

/// Snapshot des stats observabilité.
#[derive(Debug, Clone, Copy)]
pub struct SelfHostStats {
    pub fractal_calls: u32,
    pub eval_calls: u32,
    pub max_depth_seen: u32,
    pub depth_violations: u32,
}

/// Runtime self-host : wrapper autour d'un `Arc<Store>` avec depth
/// tracking + cycle protection + callee table pour Op::Fractal.
pub struct SelfHostingRuntime {
    store: Arc<Store>,
    max_depth: u32,
    depth: AtomicU32,
    fractal_calls: AtomicU32,
    eval_calls: AtomicU32,
    max_depth_seen: AtomicU32,
    depth_violations: AtomicU32,
    /// Wave 8 FULL : table callee_id i64 → Hash. Permet à
    /// `Op::Fractal(callee_id, arg)` de résoudre vers un programme
    /// concret. Single-thread populated, lock-free reads via RwLock.
    callee_table: RwLock<HashMap<i64, Hash>>,
    /// Wave 8 FULL : table eval_id i64 → Vec<u8> (program bytes).
    /// Op::Eval(eval_id, arg) interprète les bytes inline.
    eval_table: RwLock<HashMap<i64, Vec<u8>>>,
}

impl SelfHostingRuntime {
    /// Construit un runtime sur le store donné, max_depth par défaut.
    pub fn new(store: Arc<Store>) -> Self {
        Self::with_max_depth(store, DEFAULT_MAX_DEPTH)
    }

    /// Construit avec un `max_depth` custom.
    pub fn with_max_depth(store: Arc<Store>, max_depth: u32) -> Self {
        Self {
            store,
            max_depth,
            depth: AtomicU32::new(0),
            fractal_calls: AtomicU32::new(0),
            eval_calls: AtomicU32::new(0),
            max_depth_seen: AtomicU32::new(0),
            depth_violations: AtomicU32::new(0),
            callee_table: RwLock::new(HashMap::new()),
            eval_table: RwLock::new(HashMap::new()),
        }
    }

    /// Wave 8 FULL : enregistre une association `callee_id → hash`
    /// dans la table de Fractal. Op::Fractal(callee_id, arg) appellera
    /// désormais le programme à `hash`.
    pub fn register_callee(&self, callee_id: i64, hash: Hash) {
        self.callee_table.write().unwrap().insert(callee_id, hash);
    }

    /// Wave 8 FULL : enregistre `eval_id → bytes` pour Op::Eval.
    /// Op::Eval(eval_id, arg) interprétera ces bytes inline.
    pub fn register_eval(&self, eval_id: i64, prog_bytes: Vec<u8>) {
        self.eval_table.write().unwrap().insert(eval_id, prog_bytes);
    }

    /// Profondeur courante (statistique).
    pub fn current_depth(&self) -> u32 {
        self.depth.load(Ordering::Relaxed)
    }

    /// Snapshot des stats d'observabilité.
    pub fn stats(&self) -> SelfHostStats {
        SelfHostStats {
            fractal_calls: self.fractal_calls.load(Ordering::Relaxed),
            eval_calls: self.eval_calls.load(Ordering::Relaxed),
            max_depth_seen: self.max_depth_seen.load(Ordering::Relaxed),
            depth_violations: self.depth_violations.load(Ordering::Relaxed),
        }
    }

    /// **Forge → Forge call** : invoque un programme KASM par hash,
    /// passant `args` (raw bytes selon la convention KASM I/O).
    /// Retourne les bytes de sortie du programme.
    pub fn fractal_call(
        &self,
        callee_hash: &Hash,
        args: &[u8],
    ) -> Result<Vec<u8>, SelfHostError> {
        // 1. Depth check + bump.
        let new_depth = self.depth.fetch_add(1, Ordering::AcqRel) + 1;
        let _guard = DepthGuard { runtime: self };
        let prev_max = self.max_depth_seen.load(Ordering::Relaxed);
        if new_depth > prev_max {
            self.max_depth_seen.store(new_depth, Ordering::Relaxed);
        }
        if new_depth > self.max_depth {
            self.depth_violations.fetch_add(1, Ordering::Relaxed);
            return Err(SelfHostError::DepthExceeded {
                depth: new_depth,
                max: self.max_depth,
            });
        }
        self.fractal_calls.fetch_add(1, Ordering::Relaxed);

        // 2. Charger le programme depuis le Store.
        let bytes = self.store.load(callee_hash)
            .ok_or_else(|| SelfHostError::UnknownProgram(*callee_hash))?;
        let program = Program::from_bytes(&bytes)
            .map_err(|e| SelfHostError::InvalidProgram {
                hash: *callee_hash,
                reason: format!("{:?}", e),
            })?;

        // 3. Exécuter via l'interpreter scalar.
        let out = kasm_execute(&program, args)?;
        Ok(out)
    }

    /// **Programme-as-data eval** : prend les bytes d'un programme
    /// KASM construit à l'exécution, le verify, l'exécute, retourne
    /// la sortie.
    pub fn eval_kasm(
        &self,
        prog_bytes: &[u8],
        args: &[u8],
    ) -> Result<Vec<u8>, SelfHostError> {
        // Depth check (eval_kasm doit aussi être protégé contre la
        // récursion infinie si le programme construit appelle lui-même
        // un autre eval_kasm via fractal_call).
        let new_depth = self.depth.fetch_add(1, Ordering::AcqRel) + 1;
        let _guard = DepthGuard { runtime: self };
        if new_depth > self.max_depth {
            self.depth_violations.fetch_add(1, Ordering::Relaxed);
            return Err(SelfHostError::DepthExceeded {
                depth: new_depth,
                max: self.max_depth,
            });
        }
        self.eval_calls.fetch_add(1, Ordering::Relaxed);

        let program = Program::from_bytes(prog_bytes)
            .map_err(|e| SelfHostError::InvalidEvalBytes(format!("{:?}", e)))?;

        let out = kasm_execute(&program, args)?;
        Ok(out)
    }
}

// ─── Wave 8 FULL : trait impl pour bytecode dispatch ─────────────────

impl FractalDispatcher for SelfHostingRuntime {
    /// Op::Fractal(callee_id, arg) → résout via callee_table puis
    /// exécute le programme avec arg comme i64 input.
    fn fractal(&self, callee_id: i64, arg: i64) -> Result<i64, KasmError> {
        let hash = {
            let table = self.callee_table.read().unwrap();
            *table.get(&callee_id).ok_or(KasmError::BadInputSlot {
                node: 0,
                slot: callee_id as i16,
            })?
        };
        let bytes = self.store.load(&hash).ok_or(KasmError::BadInputSlot {
            node: 0,
            slot: callee_id as i16,
        })?;
        let program = Program::from_bytes(&bytes)?;
        let mut args = Vec::with_capacity(8);
        args.extend_from_slice(&arg.to_le_bytes());
        // Récursivité : on peut imbriquer Op::Fractal à profondeur N.
        let depth = self.depth.fetch_add(1, Ordering::AcqRel) + 1;
        let _guard = DepthGuard { runtime: self };
        if depth > self.max_depth {
            self.depth_violations.fetch_add(1, Ordering::Relaxed);
            return Err(KasmError::FuelTooSmall);
        }
        let prev_max = self.max_depth_seen.load(Ordering::Relaxed);
        if depth > prev_max {
            self.max_depth_seen.store(depth, Ordering::Relaxed);
        }
        self.fractal_calls.fetch_add(1, Ordering::Relaxed);
        // Exécution récursive avec le même dispatcher (self).
        let out = execute_with_fractal(&program, &args, self)?;
        if out.len() < 8 {
            return Err(KasmError::BadInputLength {
                expected: 8,
                got: out.len(),
            });
        }
        Ok(i64::from_le_bytes(out[..8].try_into().unwrap()))
    }

    /// Op::Eval(eval_id, arg) → résout via eval_table, parse les bytes
    /// inline, exécute.
    fn eval(&self, eval_id: i64, arg: i64) -> Result<i64, KasmError> {
        let bytes = {
            let table = self.eval_table.read().unwrap();
            table.get(&eval_id).cloned().ok_or(KasmError::BadInputSlot {
                node: 0,
                slot: eval_id as i16,
            })?
        };
        let program = Program::from_bytes(&bytes)?;
        let mut args = Vec::with_capacity(8);
        args.extend_from_slice(&arg.to_le_bytes());
        let depth = self.depth.fetch_add(1, Ordering::AcqRel) + 1;
        let _guard = DepthGuard { runtime: self };
        if depth > self.max_depth {
            self.depth_violations.fetch_add(1, Ordering::Relaxed);
            return Err(KasmError::FuelTooSmall);
        }
        self.eval_calls.fetch_add(1, Ordering::Relaxed);
        let out = execute_with_fractal(&program, &args, self)?;
        if out.len() < 8 {
            return Err(KasmError::BadInputLength {
                expected: 8,
                got: out.len(),
            });
        }
        Ok(i64::from_le_bytes(out[..8].try_into().unwrap()))
    }
}

/// RAII guard qui décrémente la depth au drop.
struct DepthGuard<'a> {
    runtime: &'a SelfHostingRuntime,
}

impl<'a> Drop for DepthGuard<'a> {
    fn drop(&mut self) {
        self.runtime.depth.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kasm::types::{Node, Target, Ty};
    use crate::{fresh_tmp_path, TmpDir};

    /// Helper : programme KASM `f(x) = x + N`. Encodé en bytes pour
    /// stockage dans le Store.
    fn build_add_n_bytes(n: i16) -> Vec<u8> {
        let prog = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(n),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        prog.bytes().to_vec()
    }

    fn open_store(tag: &str) -> (TmpDir, Store) {
        let path = fresh_tmp_path("self-host", tag);
        std::fs::create_dir_all(&path).unwrap();
        let guard = TmpDir::new(path.clone());
        let store = Store::open(&path).unwrap();
        (guard, store)
    }

    fn write_bytes(store: &Store, bytes: &[u8]) -> Hash {
        store.store(bytes).unwrap()
    }

    #[test]
    fn fractal_call_executes_program_by_hash() {
        // Programme : f(x) = x + 7
        let (_guard, store) = open_store("fractal-basic");
        let bytes = build_add_n_bytes(7);
        let hash = write_bytes(&store, &bytes);

        let runtime = SelfHostingRuntime::new(Arc::new(store));
        let mut args = Vec::new();
        args.extend_from_slice(&5i64.to_le_bytes());
        let out = runtime.fractal_call(&hash, &args).unwrap();
        let result = i64::from_le_bytes(out[..8].try_into().unwrap());
        assert_eq!(result, 12, "f(5) = 5 + 7 = 12");
    }

    #[test]
    fn fractal_call_unknown_hash_errors() {
        let (_guard, store) = open_store("fractal-unknown");
        let runtime = SelfHostingRuntime::new(Arc::new(store));
        let bogus = Hash::from_bytes([0u8; 20]);
        let args = 0i64.to_le_bytes().to_vec();
        let err = runtime.fractal_call(&bogus, &args).unwrap_err();
        assert!(matches!(err, SelfHostError::UnknownProgram(_)));
    }

    #[test]
    fn fractal_call_increments_stats() {
        let (_guard, store) = open_store("fractal-stats");
        let bytes = build_add_n_bytes(1);
        let hash = write_bytes(&store, &bytes);

        let runtime = SelfHostingRuntime::new(Arc::new(store));
        let args = 10i64.to_le_bytes().to_vec();
        runtime.fractal_call(&hash, &args).unwrap();
        runtime.fractal_call(&hash, &args).unwrap();
        runtime.fractal_call(&hash, &args).unwrap();
        let s = runtime.stats();
        assert_eq!(s.fractal_calls, 3);
        assert_eq!(s.eval_calls, 0);
        assert!(s.max_depth_seen >= 1);
        assert_eq!(s.depth_violations, 0);
    }

    #[test]
    fn eval_kasm_executes_inline_program() {
        let (_guard, store) = open_store("eval-inline");
        let runtime = SelfHostingRuntime::new(Arc::new(store));
        let bytes = build_add_n_bytes(100);
        let args = 42i64.to_le_bytes().to_vec();
        let out = runtime.eval_kasm(&bytes, &args).unwrap();
        let result = i64::from_le_bytes(out[..8].try_into().unwrap());
        assert_eq!(result, 142, "f(42) = 42 + 100 = 142");
        assert_eq!(runtime.stats().eval_calls, 1);
    }

    #[test]
    fn eval_kasm_invalid_bytes_errors() {
        let (_guard, store) = open_store("eval-invalid");
        let runtime = SelfHostingRuntime::new(Arc::new(store));
        let bogus_bytes = [0u8; 32]; // pas un programme KASM valide
        let args = 0i64.to_le_bytes().to_vec();
        let err = runtime.eval_kasm(&bogus_bytes, &args).unwrap_err();
        assert!(matches!(err, SelfHostError::InvalidEvalBytes(_)));
    }

    #[test]
    fn fractal_depth_tracking_returns_to_zero() {
        let (_guard, store) = open_store("fractal-depth");
        let bytes = build_add_n_bytes(1);
        let hash = write_bytes(&store, &bytes);

        let runtime = SelfHostingRuntime::new(Arc::new(store));
        assert_eq!(runtime.current_depth(), 0);
        let args = 0i64.to_le_bytes().to_vec();
        runtime.fractal_call(&hash, &args).unwrap();
        // Depth doit revenir à 0 après le call (DepthGuard).
        assert_eq!(runtime.current_depth(), 0);
    }

    #[test]
    fn fractal_max_depth_can_be_customized() {
        let (_guard, store) = open_store("fractal-max-depth");
        let bytes = build_add_n_bytes(1);
        let hash = write_bytes(&store, &bytes);

        let runtime = SelfHostingRuntime::with_max_depth(Arc::new(store), 5);
        // Un seul call dépasse pas max_depth=5.
        let args = 0i64.to_le_bytes().to_vec();
        runtime.fractal_call(&hash, &args).unwrap();
        assert_eq!(runtime.stats().depth_violations, 0);
    }

    #[test]
    fn forge_calls_forge_round_trip() {
        // Test self-hosting concret : programme A = f(x) = x*2,
        // on fait 3 fractal_calls sur A et on vérifie composition.
        let (_guard, store) = open_store("forge-on-forge");
        let prog_a = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(2),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let bytes_a = prog_a.bytes().to_vec();
        let hash_a = write_bytes(&store, &bytes_a);

        let runtime = SelfHostingRuntime::new(Arc::new(store));
        // Chain : 5 → 10 → 20 → 40 (3 doublings).
        let mut acc = 5i64;
        for _ in 0..3 {
            let args = acc.to_le_bytes().to_vec();
            let out = runtime.fractal_call(&hash_a, &args).unwrap();
            acc = i64::from_le_bytes(out[..8].try_into().unwrap());
        }
        assert_eq!(acc, 40, "5 → 10 → 20 → 40 chain via fractal_call");
        assert_eq!(runtime.stats().fractal_calls, 3);
    }

    #[test]
    fn fractal_full_program_with_op_fractal_executes() {
        // ═══ TEST E2E WAVE 8 FULL ═══
        // 1. Programme A = f(x) = x*2 (callee).
        // 2. Programme B = g(x) = Fractal(callee_id=42, arg=x) + 100.
        // 3. Register A sous callee_id=42.
        // 4. Execute B avec x=5 via execute_with_fractal.
        //    Expected : 5*2 + 100 = 110.
        use crate::kasm::execute_with_fractal;

        let (_guard, store) = open_store("e2e-fractal");
        // Programme A : f(x) = x * 2
        let prog_a = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(2),
                Node::mul(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let bytes_a = prog_a.bytes().to_vec();
        let hash_a = write_bytes(&store, &bytes_a);

        // Programme B : g(x) = Fractal(42, x) + 100
        // Layout :
        //   node 0 : Input(0) → x
        //   node 1 : ConstI64(42) → callee_id
        //   node 2 : Op::Fractal(a=1, b=0) → calls callee 42 with x
        //   node 3 : ConstI64(100) → 100
        //   node 4 : AddI64(2, 3) → result + 100
        //   node 5 : Output(4)
        use crate::kasm::Op;
        let fractal_node = Node {
            op: Op::Fractal,
            ty: Ty::I64,
            a: 1, // callee_id slot
            b: 0, // arg slot (the input x)
            imm: 0,
        };
        let prog_b = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),         // 0 : x
                Node::const_i64(42),    // 1 : callee_id = 42
                fractal_node,           // 2 : Fractal(42, x)
                Node::const_i64(100),   // 3 : 100
                Node::add(2, 3),        // 4 : Fractal(42, x) + 100
                Node::output(4, Ty::I64),
            ],
        ).unwrap();

        // Setup runtime + register callee.
        let runtime = SelfHostingRuntime::new(Arc::new(store));
        runtime.register_callee(42, hash_a);

        // Execute B avec x = 5.
        let mut args = Vec::new();
        args.extend_from_slice(&5i64.to_le_bytes());
        let out = execute_with_fractal(&prog_b, &args, &runtime).unwrap();
        let result = i64::from_le_bytes(out[..8].try_into().unwrap());
        assert_eq!(result, 110, "Fractal(42, 5) + 100 = 5*2 + 100 = 110");
        // Verify dispatcher was called.
        let stats = runtime.stats();
        assert_eq!(stats.fractal_calls, 1);
    }

    #[test]
    fn fractal_full_op_eval_executes_inline() {
        // Programme avec Op::Eval qui interprète des bytes registered.
        use crate::kasm::execute_with_fractal;
        use crate::kasm::Op;

        let (_guard, store) = open_store("e2e-eval");
        // Eval target : f(x) = x + 7
        let prog_inline = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(7),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let bytes_inline = prog_inline.bytes().to_vec();

        // Outer programme : g(x) = Eval(99, x) * 3
        let eval_node = Node {
            op: Op::Eval,
            ty: Ty::I64,
            a: 1, // eval_id slot
            b: 0, // arg slot
            imm: 0,
        };
        let prog_outer = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),       // 0 : x
                Node::const_i64(99),  // 1 : eval_id
                eval_node,            // 2 : Eval(99, x)
                Node::const_i64(3),   // 3 : 3
                Node::mul(2, 3),      // 4 : Eval(...) * 3
                Node::output(4, Ty::I64),
            ],
        ).unwrap();

        let runtime = SelfHostingRuntime::new(Arc::new(store));
        runtime.register_eval(99, bytes_inline);

        let mut args = Vec::new();
        args.extend_from_slice(&5i64.to_le_bytes());
        let out = execute_with_fractal(&prog_outer, &args, &runtime).unwrap();
        let result = i64::from_le_bytes(out[..8].try_into().unwrap());
        assert_eq!(result, 36, "Eval(99, 5)*3 = (5+7)*3 = 36");
        assert_eq!(runtime.stats().eval_calls, 1);
    }

    #[test]
    fn fractal_full_recursive_fractal_calls() {
        // Programme A appelle un autre A via Fractal — récursion bornée
        // par max_depth.
        use crate::kasm::execute_with_fractal;
        use crate::kasm::Op;

        let (_guard, store) = open_store("e2e-recursive");
        // Programme : f(x) = x + 1 (simple, pas de récursion en lui-même).
        let prog = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(1),
                Node::add(0, 1),
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let bytes = prog.bytes().to_vec();
        let hash = write_bytes(&store, &bytes);

        // Outer : Fractal(7, Fractal(7, Fractal(7, x))).
        // Avec x = 0 : ((0+1)+1)+1 = 3.
        let fractal = |a: u16, b: u16| Node {
            op: Op::Fractal,
            ty: Ty::I64,
            a, b, imm: 0,
        };
        let prog_outer = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),       // 0 : x
                Node::const_i64(7),   // 1 : callee_id
                fractal(1, 0),        // 2 : Fractal(7, x)
                fractal(1, 2),        // 3 : Fractal(7, prev)
                fractal(1, 3),        // 4 : Fractal(7, prev)
                Node::output(4, Ty::I64),
            ],
        ).unwrap();

        let runtime = SelfHostingRuntime::new(Arc::new(store));
        runtime.register_callee(7, hash);

        let args = 0i64.to_le_bytes().to_vec();
        let out = execute_with_fractal(&prog_outer, &args, &runtime).unwrap();
        let result = i64::from_le_bytes(out[..8].try_into().unwrap());
        assert_eq!(result, 3, "3 chained Fractal(+1) calls = 3");
        assert_eq!(runtime.stats().fractal_calls, 3);
    }

    #[test]
    fn fractal_full_unregistered_callee_errors() {
        // Op::Fractal avec callee_id non enregistré → erreur claire.
        use crate::kasm::execute_with_fractal;
        use crate::kasm::Op;

        let (_guard, store) = open_store("e2e-unregistered");
        let fractal_node = Node {
            op: Op::Fractal,
            ty: Ty::I64,
            a: 1,
            b: 0,
            imm: 0,
        };
        let prog = Program::new(
            Target::Cpu, 1, 1, 32,
            vec![
                Node::input(0),
                Node::const_i64(99),  // callee_id NON ENREGISTRÉ
                fractal_node,
                Node::output(2, Ty::I64),
            ],
        ).unwrap();
        let runtime = SelfHostingRuntime::new(Arc::new(store));
        let args = 5i64.to_le_bytes().to_vec();
        let err = execute_with_fractal(&prog, &args, &runtime).unwrap_err();
        // L'erreur surface comme BadInputSlot (pattern fail-loud propre).
        assert!(matches!(err, KasmError::BadInputSlot { .. }));
    }

    #[test]
    fn eval_kasm_doesnt_persist_to_store() {
        // eval_kasm exécute un programme inline sans le persister.
        // Vérifions que le store est bien vide après un eval.
        let (_guard, store) = open_store("eval-no-persist");
        let store_arc = Arc::new(store);
        let runtime = SelfHostingRuntime::new(Arc::clone(&store_arc));
        let bytes = build_add_n_bytes(99);
        let args = 1i64.to_le_bytes().to_vec();
        runtime.eval_kasm(&bytes, &args).unwrap();
        // L'eval ne doit pas avoir introduit le programme dans le store.
        // Sa hash n'est pas trouvable.
        let prog_hash = Hash::for_blob(&bytes);
        let lookup = store_arc.load(&prog_hash);
        assert!(lookup.is_none(), "eval_kasm ne doit pas persister");
    }
}
