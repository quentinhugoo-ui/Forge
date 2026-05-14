//! Ω-7.2 first-mile-honnête — Corpus SCAN-natif de programmes KASM.
//!
//! ## Doctrine
//!
//! La promesse Ω-7.2 historique ("corpus MLIR : Linux kernel + mathlib +
//! top-100k Rust") nécessite Polygeist + Lean4 + rustc-MLIR backend.
//! Doctrinairement impossible (3 toolchains externes).
//!
//! Réinterprétation honnête : générer un corpus DÉTERMINISTE de N programmes
//! KASM via fuzz, avec leurs émissions MLIR text et leurs embeddings Term.
//! Le corpus est SCAN-natif (pas un import externe), reproductible (seed
//! fixe), et utilisable par BanditAgent pour entraînement.
//!
//! Ce N'EST PAS le corpus Linux/mathlib. Document pour ce qu'il est : un
//! corpus first-mile auto-généré, pierre angulaire pour Ω-7.2.x étendu.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use crate::kasm::{Node, Program, Target, Ty, MAX_NODES};

/// Une entrée du corpus : un Program + son MLIR-text + son embedding Term.
#[derive(Debug, Clone)]
pub struct CorpusEntry {
    pub program: Program,
    pub mlir_text: String,
    pub term_hash: [u8; 32],
}

/// Corpus = vec d'entrées + meta. Reproductible via le seed.
#[derive(Debug, Clone)]
pub struct Corpus {
    pub seed: u64,
    pub entries: Vec<CorpusEntry>,
}

impl Corpus {
    /// Génère un corpus de `n` programmes à partir de `seed`. Pour chaque
    /// programme : random_program → emit_mlir → embed_program.hash.
    pub fn generate(seed: u64, n: usize) -> Self {
        let mut rng = XorshiftRng::new(seed);
        let mut entries = Vec::with_capacity(n);
        for i in 0..n {
            // Taille cible pseudo-uniforme dans [4, 64].
            let target_size = 4 + (rng.next_u64() as usize) % 60;
            let n_inputs = 1 + (rng.next_u64() as usize) % 4;
            let entry_seed = seed
                .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                .wrapping_add(i as u64);
            let p = generate_random_program(entry_seed, target_size, n_inputs as u8);
            let mlir = p.canonical_mlir_text().unwrap_or_default();
            let term = crate::meta::embed_program(&p);
            let term_hash = term.hash();
            entries.push(CorpusEntry { program: p, mlir_text: mlir, term_hash });
        }
        Self { seed, entries }
    }

    /// Nombre d'entrées.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// xorshift 64-bit (no external dep).
struct XorshiftRng(u64);
impl XorshiftRng {
    fn new(seed: u64) -> Self {
        Self(seed | 0xdead_beef)
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// Génère un Program KASM aléatoire valide. Forward refs only, types corrects.
fn generate_random_program(seed: u64, target_size: usize, n_inputs: u8) -> Program {
    let mut rng = XorshiftRng::new(seed);
    let target = target_size.clamp(4, MAX_NODES);
    let inputs = n_inputs.clamp(1, 16);

    let mut nodes: Vec<Node> = Vec::with_capacity(target);
    let mut i64_idx: Vec<u16> = Vec::new();

    for slot in 0..inputs {
        nodes.push(Node::input(slot));
        i64_idx.push(nodes.len() as u16 - 1);
    }
    nodes.push(Node::const_i64((rng.next_u64() as i16) % 100));
    i64_idx.push(nodes.len() as u16 - 1);

    while nodes.len() < target.saturating_sub(1) {
        let kind = rng.next_u64() % 9;
        let n = match kind {
            0 => Node::const_i64((rng.next_u64() as i16) % 200 - 100),
            1 => Node::add(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            2 => Node::sub(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            3 => Node::mul(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            4 => Node::min(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            5 => Node::max(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            6 => Node::bit_and(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            7 => Node::bit_or(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
            _ => Node::bit_xor(pick(&i64_idx, &mut rng), pick(&i64_idx, &mut rng)),
        };
        let idx = nodes.len() as u16;
        nodes.push(n);
        // Tous les ops sélectionnés produisent I64.
        i64_idx.push(idx);
    }
    let last = *i64_idx.last().expect("at least 1 i64");
    nodes.push(Node::output(last, Ty::I64));
    let total = nodes.len() as u32;
    Program::new(Target::Cpu, inputs, 1, total, nodes).expect("valid by construction")
}

fn pick(slice: &[u16], rng: &mut XorshiftRng) -> u16 {
    slice[(rng.next_u64() as usize) % slice.len()]
}

// =============================================================================
// η — Lifting de fonctions système (kraken-η.0)
// =============================================================================
//
// Pipe la sortie d'un disassembleur local (dumpbin Windows, objdump Linux,
// otool macOS — Intel syntax requis) et lifte les fonctions arithmétiques
// pures straight-line en programmes KASM. Lazy detection, no-op si l'outil
// est absent. Allow-list stricte d'opcodes : mov, add, sub, imul, and, or,
// xor, shl, shr, neg, push/pop (no-op stack frame), ret. Toute fonction
// avec branche, syscall, mémoire indirecte ou registre xmm/ymm est skippée
// entière. Le lift est SSA : chaque GPR mappe à un index Node KASM courant.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisasmTool {
    Dumpbin,
    Objdump,
    Otool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Reg {
    Rax, Rbx, Rcx, Rdx, Rsi, Rdi, Rbp, Rsp,
    R8, R9, R10, R11, R12, R13, R14, R15,
}

impl Reg {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('%').to_ascii_lowercase();
        Some(match s.as_str() {
            "rax" => Reg::Rax, "rbx" => Reg::Rbx, "rcx" => Reg::Rcx, "rdx" => Reg::Rdx,
            "rsi" => Reg::Rsi, "rdi" => Reg::Rdi, "rbp" => Reg::Rbp, "rsp" => Reg::Rsp,
            "r8" => Reg::R8, "r9" => Reg::R9, "r10" => Reg::R10, "r11" => Reg::R11,
            "r12" => Reg::R12, "r13" => Reg::R13, "r14" => Reg::R14, "r15" => Reg::R15,
            _ => return None,
        })
    }

    fn is_stack(self) -> bool {
        matches!(self, Reg::Rsp | Reg::Rbp)
    }
}

#[derive(Debug, Clone, Copy)]
enum Operand {
    Reg(Reg),
    Imm(i64),
}

impl Operand {
    fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if let Some(r) = Reg::parse(s) {
            return Some(Operand::Reg(r));
        }
        // Immédiat décimal ou hex (avec/sans préfixe 0x, suffixe h).
        let raw = s.trim_start_matches('$').trim_start_matches('#');
        let (body, radix) = if let Some(stripped) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
            (stripped, 16)
        } else if let Some(stripped) = raw.strip_suffix('h').or_else(|| raw.strip_suffix('H')) {
            (stripped, 16)
        } else {
            (raw, 10)
        };
        // Gestion du signe pour décimal.
        if radix == 10 {
            if let Ok(v) = body.parse::<i64>() {
                return Some(Operand::Imm(v));
            }
        } else if let Ok(v) = i64::from_str_radix(body.trim_start_matches('-'), radix) {
            let signed = if body.starts_with('-') { -(v as i64) } else { v };
            return Some(Operand::Imm(signed));
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mnem {
    Mov, Add, Sub, Imul, And, Or, Xor, Shl, Shr, Neg, Push, Pop, Ret,
}

impl Mnem {
    fn parse(s: &str) -> Option<Self> {
        // Intel syntax requis (objdump invoqué avec -M intel ; dumpbin Intel
        // par défaut). Les suffixes AT&T (movq, addl) ne sont pas tolérés
        // pour éviter les ambiguïtés type `imul`/`imul`+suffixe.
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "mov" => Mnem::Mov,
            "add" => Mnem::Add,
            "sub" => Mnem::Sub,
            "imul" => Mnem::Imul,
            "and" => Mnem::And,
            "or" => Mnem::Or,
            "xor" => Mnem::Xor,
            "shl" | "sal" => Mnem::Shl,
            "shr" => Mnem::Shr,
            "neg" => Mnem::Neg,
            "push" => Mnem::Push,
            "pop" => Mnem::Pop,
            "ret" => Mnem::Ret,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone)]
struct Instr {
    mnem: Mnem,
    dst: Option<Operand>,
    src: Option<Operand>,
}

#[derive(Debug, Clone)]
struct RawFunction {
    instrs: Vec<Instr>,
    /// `true` si une ligne non-parsable ou un opcode hors allow-list a été
    /// rencontré dans cette fonction → fonction entière à skip.
    poisoned: bool,
}

/// Detection lazy de l'outil de désassemblage local. Cache le résultat dans
/// un OnceLock pour éviter les `Command::new` répétés.
pub fn detect_disasm_tool() -> Option<DisasmTool> {
    static CACHED: OnceLock<Option<DisasmTool>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        // Ordre de préférence : outil natif de la plateforme courante d'abord.
        if cfg!(windows) {
            if probe_tool("dumpbin", &["/?"]) {
                return Some(DisasmTool::Dumpbin);
            }
            if probe_tool("objdump", &["--version"]) {
                return Some(DisasmTool::Objdump);
            }
        } else if cfg!(target_os = "macos") {
            if probe_tool("otool", &["--version"]) {
                return Some(DisasmTool::Otool);
            }
            if probe_tool("objdump", &["--version"]) {
                return Some(DisasmTool::Objdump);
            }
        } else if probe_tool("objdump", &["--version"]) {
            return Some(DisasmTool::Objdump);
        }
        None
    })
}

fn probe_tool(name: &str, args: &[&str]) -> bool {
    Command::new(name)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_disasm(tool: DisasmTool, path: &Path) -> Option<String> {
    let output = match tool {
        DisasmTool::Dumpbin => Command::new("dumpbin")
            .arg("/DISASM:NOBYTES")
            .arg(path)
            .output()
            .ok()?,
        DisasmTool::Objdump => Command::new("objdump")
            .args(["-d", "-M", "intel", "--no-show-raw-insn"])
            .arg(path)
            .output()
            .ok()?,
        DisasmTool::Otool => Command::new("otool")
            .args(["-tv"])
            .arg(path)
            .output()
            .ok()?,
    };
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Convention d'appel : ordre des registres argument selon la plateforme
/// déduite depuis l'outil. Windows MS x64 : RCX, RDX, R8, R9. SysV (Linux/
/// macOS) : RDI, RSI, RDX, RCX, R8, R9.
fn call_conv_args(tool: DisasmTool) -> &'static [Reg] {
    match tool {
        DisasmTool::Dumpbin => &[Reg::Rcx, Reg::Rdx, Reg::R8, Reg::R9],
        DisasmTool::Objdump | DisasmTool::Otool => {
            &[Reg::Rdi, Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8, Reg::R9]
        }
    }
}

/// Découpe la sortie en fonctions. Une fonction commence à un label de la
/// forme `<name>:` (objdump), `name:` seul sur sa ligne (dumpbin) ou
/// `_name:` (otool). Les lignes d'instructions sont toutes les lignes après
/// le header jusqu'au prochain label ou EOF.
fn parse_disasm_output(text: &str, tool: DisasmTool) -> Vec<RawFunction> {
    let mut out: Vec<RawFunction> = Vec::new();
    let mut current: Option<RawFunction> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_function_label(trimmed, tool) {
            if let Some(f) = current.take() {
                if !f.instrs.is_empty() && !f.poisoned {
                    out.push(f);
                }
            }
            current = Some(RawFunction { instrs: Vec::new(), poisoned: false });
            continue;
        }
        let Some(func) = current.as_mut() else { continue };
        if func.poisoned {
            continue;
        }
        if let Some(instr_line) = strip_address_prefix(trimmed) {
            match parse_instr_line(instr_line) {
                ParseLine::Instr(i) => func.instrs.push(i),
                ParseLine::Skip => {}
                ParseLine::Poison => func.poisoned = true,
            }
        }
    }
    if let Some(f) = current {
        if !f.instrs.is_empty() && !f.poisoned {
            out.push(f);
        }
    }
    out
}

fn is_function_label(line: &str, _tool: DisasmTool) -> bool {
    // objdump : "0000000000401000 <my_func>:"
    if let Some(open) = line.find('<') {
        if let Some(close) = line[open..].find('>') {
            let name = &line[open + 1..open + close];
            if line.trim_end().ends_with(':') && !name.is_empty() {
                return true;
            }
        }
    }
    // dumpbin : "my_func:" en début de ligne
    // otool : "_my_func:"
    if line.ends_with(':') && !line.contains(' ') {
        let name = line.trim_end_matches(':');
        if !name.is_empty() && name.chars().next().map(is_label_start).unwrap_or(false) {
            // Filtrer les labels d'adresses purs ("00401000:") qu'on ne veut pas.
            if !name.chars().all(|c| c.is_ascii_hexdigit()) {
                return true;
            }
        }
    }
    false
}

fn is_label_start(c: char) -> bool {
    c == '_' || c == '.' || c == '$' || c.is_ascii_alphabetic()
}

/// Strip un préfixe d'adresse type `00401000:` ou `0000000180001000:`.
fn strip_address_prefix(line: &str) -> Option<&str> {
    let mut chars = line.char_indices();
    // Skip hex digits.
    let mut last_hex = 0;
    let mut saw_hex = false;
    for (i, c) in chars.by_ref() {
        if c.is_ascii_hexdigit() {
            last_hex = i + c.len_utf8();
            saw_hex = true;
        } else {
            if saw_hex && c == ':' {
                let rest = &line[last_hex + 1..];
                return Some(rest.trim());
            }
            // Pas de préfixe d'adresse : retourner la ligne telle quelle.
            return Some(line);
        }
    }
    if saw_hex {
        Some(&line[last_hex..])
    } else {
        Some(line)
    }
}

enum ParseLine {
    Instr(Instr),
    Skip,
    Poison,
}

fn parse_instr_line(line: &str) -> ParseLine {
    // Strip commentaire éventuel après `;` ou `#`.
    let line = match line.find(|c| c == ';' || c == '#') {
        Some(i) => &line[..i],
        None => line,
    };
    let line = line.trim();
    if line.is_empty() {
        return ParseLine::Skip;
    }
    // Tokenizer : mnemonic = premier token, le reste = operandes séparées par `,`.
    let mut parts = line.splitn(2, char::is_whitespace);
    let mnem_str = parts.next().unwrap_or("");
    let ops_str = parts.next().unwrap_or("").trim();
    let Some(mnem) = Mnem::parse(mnem_str) else {
        return ParseLine::Poison;
    };
    if mnem == Mnem::Ret {
        return ParseLine::Instr(Instr { mnem, dst: None, src: None });
    }
    if matches!(mnem, Mnem::Push | Mnem::Pop | Mnem::Neg) {
        let Some(dst) = Operand::parse(ops_str) else {
            return ParseLine::Poison;
        };
        return ParseLine::Instr(Instr { mnem, dst: Some(dst), src: None });
    }
    // Deux opérandes attendus.
    let mut ops = ops_str.splitn(2, ',');
    let dst_s = ops.next().unwrap_or("").trim();
    let src_s = ops.next().unwrap_or("").trim();
    if dst_s.is_empty() || src_s.is_empty() {
        return ParseLine::Poison;
    }
    // Référence mémoire `[...]` non supportée → poison.
    if dst_s.contains('[') || src_s.contains('[') {
        return ParseLine::Poison;
    }
    let Some(dst) = Operand::parse(dst_s) else {
        return ParseLine::Poison;
    };
    let Some(src) = Operand::parse(src_s) else {
        return ParseLine::Poison;
    };
    ParseLine::Instr(Instr { mnem, dst: Some(dst), src: Some(src) })
}

/// Lifte une fonction straight-line en Program KASM. Renvoie None si elle
/// dépasse les contraintes (taille, opcodes, capacités KASM).
fn lift_function(func: &RawFunction, args_order: &[Reg]) -> Option<Program> {
    if func.instrs.is_empty() || func.instrs.len() > 50 {
        return None;
    }
    if !matches!(func.instrs.last()?.mnem, Mnem::Ret) {
        return None;
    }

    // Phase 1 : déterminer les registres argument lus avant écriture (= inputs).
    let mut written: [bool; 16] = [false; 16];
    let mut read_first: [bool; 16] = [false; 16];
    for instr in &func.instrs {
        let (reads, writes) = instr_reg_uses(instr);
        for r in reads {
            let idx = reg_index(r);
            if !written[idx] {
                read_first[idx] = true;
            }
        }
        for r in writes {
            written[reg_index(r)] = true;
        }
    }
    // Sélectionne les regs argument utilisés, dans l'ordre de la convention.
    let inputs: Vec<Reg> = args_order
        .iter()
        .copied()
        .filter(|r| read_first[reg_index(*r)])
        .collect();
    if inputs.is_empty() {
        return None;
    }

    // Phase 2 : émission SSA.
    let mut nodes: Vec<Node> = Vec::with_capacity(func.instrs.len() * 2 + 4);
    let mut reg_idx: [Option<u16>; 16] = [None; 16];
    for (slot, &reg) in inputs.iter().enumerate() {
        nodes.push(Node::input(slot as u8));
        reg_idx[reg_index(reg)] = Some((nodes.len() - 1) as u16);
    }

    for instr in &func.instrs {
        if !lift_instr(instr, &mut nodes, &mut reg_idx) {
            return None;
        }
        if nodes.len() > MAX_NODES.saturating_sub(2) {
            return None;
        }
    }

    let rax_final = reg_idx[reg_index(Reg::Rax)]?;
    nodes.push(Node::output(rax_final, Ty::I64));
    let total = nodes.len() as u32;
    Program::new(Target::Cpu, inputs.len() as u8, 1, total, nodes).ok()
}

fn reg_index(r: Reg) -> usize {
    r as usize
}

fn instr_reg_uses(instr: &Instr) -> (Vec<Reg>, Vec<Reg>) {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    match instr.mnem {
        Mnem::Ret => {}
        Mnem::Push => {
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
            }
        }
        Mnem::Pop => {
            if let Some(Operand::Reg(r)) = instr.dst {
                writes.push(r);
            }
        }
        Mnem::Neg => {
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
                writes.push(r);
            }
        }
        Mnem::Mov => {
            if let Some(Operand::Reg(r)) = instr.dst {
                writes.push(r);
            }
            if let Some(Operand::Reg(r)) = instr.src {
                reads.push(r);
            }
        }
        Mnem::Xor => {
            if let (Some(Operand::Reg(d)), Some(Operand::Reg(s))) = (instr.dst, instr.src) {
                if d == s {
                    writes.push(d);
                    return (reads, writes);
                }
            }
            // Fallthrough → comme Add.
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
                writes.push(r);
            }
            if let Some(Operand::Reg(r)) = instr.src {
                reads.push(r);
            }
        }
        Mnem::Add | Mnem::Sub | Mnem::Imul | Mnem::And | Mnem::Or | Mnem::Shl | Mnem::Shr => {
            if let Some(Operand::Reg(r)) = instr.dst {
                reads.push(r);
                writes.push(r);
            }
            if let Some(Operand::Reg(r)) = instr.src {
                reads.push(r);
            }
        }
    }
    (reads, writes)
}

/// Effectue le lift d'une instruction. Renvoie false si elle ne peut pas
/// être liftée (constante hors i16, opcode mémoire, etc.).
fn lift_instr(instr: &Instr, nodes: &mut Vec<Node>, reg_idx: &mut [Option<u16>; 16]) -> bool {
    match instr.mnem {
        Mnem::Ret => true,
        Mnem::Push => true, // no-op pour SSA arithmétique
        Mnem::Pop => {
            // Sur RBP/RSP, no-op. Sinon, le tracking devient incertain (la
            // valeur poppée vient de la pile, qu'on ne modélise pas) → fail.
            if let Some(Operand::Reg(r)) = instr.dst {
                if r.is_stack() {
                    return true;
                }
                // Désync : invalide le tracking de ce registre.
                reg_idx[reg_index(r)] = None;
                return true;
            }
            false
        }
        Mnem::Neg => {
            let Some(Operand::Reg(r)) = instr.dst else { return false };
            if r.is_stack() {
                return true;
            }
            let Some(src_idx) = reg_idx[reg_index(r)] else { return false };
            nodes.push(Node::neg(src_idx));
            reg_idx[reg_index(r)] = Some((nodes.len() - 1) as u16);
            true
        }
        Mnem::Mov => {
            let (Some(Operand::Reg(dst)), Some(src)) = (instr.dst, instr.src) else { return false };
            if dst.is_stack() {
                return true;
            }
            match src {
                Operand::Reg(s) => {
                    if s.is_stack() {
                        // mov r, rsp/rbp : on ne modélise pas la pile → invalide r.
                        reg_idx[reg_index(dst)] = None;
                        return true;
                    }
                    let Some(idx) = reg_idx[reg_index(s)] else { return false };
                    reg_idx[reg_index(dst)] = Some(idx);
                    true
                }
                Operand::Imm(v) => {
                    let Some(idx) = push_const(nodes, v) else { return false };
                    reg_idx[reg_index(dst)] = Some(idx);
                    true
                }
            }
        }
        Mnem::Xor => {
            let (Some(Operand::Reg(dst)), Some(src)) = (instr.dst, instr.src) else { return false };
            if dst.is_stack() {
                return true;
            }
            // xor r, r → const_i64(0)
            if let Operand::Reg(s) = src {
                if s == dst {
                    let Some(idx) = push_const(nodes, 0) else { return false };
                    reg_idx[reg_index(dst)] = Some(idx);
                    return true;
                }
            }
            lift_binop(dst, src, nodes, reg_idx, Node::bit_xor)
        }
        Mnem::Add => bin(instr, nodes, reg_idx, Node::add),
        Mnem::Sub => bin(instr, nodes, reg_idx, Node::sub),
        Mnem::Imul => bin(instr, nodes, reg_idx, Node::mul),
        Mnem::And => bin(instr, nodes, reg_idx, Node::bit_and),
        Mnem::Or => bin(instr, nodes, reg_idx, Node::bit_or),
        Mnem::Shl => bin(instr, nodes, reg_idx, Node::shl),
        Mnem::Shr => bin(instr, nodes, reg_idx, Node::shr),
    }
}

fn bin(
    instr: &Instr,
    nodes: &mut Vec<Node>,
    reg_idx: &mut [Option<u16>; 16],
    op: fn(u16, u16) -> Node,
) -> bool {
    let (Some(Operand::Reg(dst)), Some(src)) = (instr.dst, instr.src) else { return false };
    if dst.is_stack() {
        return true;
    }
    lift_binop(dst, src, nodes, reg_idx, op)
}

fn lift_binop(
    dst: Reg,
    src: Operand,
    nodes: &mut Vec<Node>,
    reg_idx: &mut [Option<u16>; 16],
    op: fn(u16, u16) -> Node,
) -> bool {
    let Some(a_idx) = reg_idx[reg_index(dst)] else { return false };
    let b_idx = match src {
        Operand::Reg(s) => {
            if s.is_stack() {
                return false;
            }
            let Some(i) = reg_idx[reg_index(s)] else { return false };
            i
        }
        Operand::Imm(v) => match push_const(nodes, v) {
            Some(i) => i,
            None => return false,
        },
    };
    nodes.push(op(a_idx, b_idx));
    reg_idx[reg_index(dst)] = Some((nodes.len() - 1) as u16);
    true
}

fn push_const(nodes: &mut Vec<Node>, v: i64) -> Option<u16> {
    if v < i16::MIN as i64 || v > i16::MAX as i64 {
        return None;
    }
    nodes.push(Node::const_i64(v as i16));
    Some((nodes.len() - 1) as u16)
}

impl Corpus {
    /// Lifte les fonctions arithmétiques pures d'un binaire système via
    /// l'outil de désassemblage local. Renvoie un Vec vide (sans erreur) si
    /// l'outil est absent ou si la commande échoue. Conservateur : seules
    /// les fonctions straight-line, sans branches/syscalls/mémoire/SSE,
    /// sont liftées. Cible Ω-7.2 corpus système sans Polygeist.
    pub fn lift_system_binary(path: &Path) -> Vec<CorpusEntry> {
        let Some(tool) = detect_disasm_tool() else { return Vec::new() };
        let Some(text) = run_disasm(tool, path) else { return Vec::new() };
        Self::lift_disasm_text(&text, tool)
    }

    /// Variante testable sans dépendance sur un outil installé : prend le
    /// texte de désassemblage en entrée et retourne les fonctions liftées.
    pub fn lift_disasm_text(text: &str, tool: DisasmTool) -> Vec<CorpusEntry> {
        let raws = parse_disasm_output(text, tool);
        let args = call_conv_args(tool);
        raws.iter()
            .filter_map(|raw| {
                let p = lift_function(raw, args)?;
                let mlir = p.canonical_mlir_text().unwrap_or_default();
                let term_hash = crate::meta::embed_program(&p).hash();
                Some(CorpusEntry { program: p, mlir_text: mlir, term_hash })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_generate_n_entries() {
        let c = Corpus::generate(42, 16);
        assert_eq!(c.len(), 16);
        assert_eq!(c.seed, 42);
    }

    #[test]
    fn corpus_is_deterministic_under_same_seed() {
        let c1 = Corpus::generate(42, 8);
        let c2 = Corpus::generate(42, 8);
        for (e1, e2) in c1.entries.iter().zip(c2.entries.iter()) {
            assert_eq!(e1.program.bytes(), e2.program.bytes());
            assert_eq!(e1.mlir_text, e2.mlir_text);
            assert_eq!(e1.term_hash, e2.term_hash);
        }
    }

    #[test]
    fn corpus_distinct_seeds_produce_distinct_entries() {
        let c1 = Corpus::generate(1, 4);
        let c2 = Corpus::generate(2, 4);
        // Au moins une entrée doit différer.
        let mut all_match = true;
        for (e1, e2) in c1.entries.iter().zip(c2.entries.iter()) {
            if e1.program.bytes() != e2.program.bytes() {
                all_match = false;
                break;
            }
        }
        assert!(!all_match, "seeds différents doivent diverger");
    }

    #[test]
    fn each_corpus_entry_has_valid_program() {
        let c = Corpus::generate(123, 8);
        for entry in &c.entries {
            // Programme doit passer round-trip.
            let p2 = Program::from_bytes(entry.program.bytes()).unwrap();
            assert_eq!(entry.program.bytes(), p2.bytes());
        }
    }

    #[test]
    fn each_corpus_entry_has_non_empty_mlir() {
        let c = Corpus::generate(123, 4);
        for entry in &c.entries {
            assert!(!entry.mlir_text.is_empty());
            assert!(entry.mlir_text.starts_with("kasm.program"));
        }
    }

    #[test]
    fn each_corpus_entry_has_nonzero_term_hash() {
        let c = Corpus::generate(123, 4);
        for entry in &c.entries {
            assert_ne!(entry.term_hash, [0u8; 32]);
        }
    }

    #[test]
    fn corpus_term_hashes_distinguish_distinct_programs() {
        let c = Corpus::generate(456, 8);
        // Au moins 2 entrées doivent avoir des hashes différents.
        let mut hashes: std::collections::BTreeSet<_> = std::collections::BTreeSet::new();
        for entry in &c.entries {
            hashes.insert(entry.term_hash);
        }
        assert!(hashes.len() >= 2, "corpus doit contenir programmes diversifiés");
    }

    // =========================================================================
    // η : tests du lifter binaire système
    // =========================================================================

    /// Évalue un programme KASM avec inputs concrets et retourne son output.
    /// Utilisé pour vérifier que le lift préserve la sémantique x86 → KASM.
    fn run_one_output(program: &Program, inputs: &[i64]) -> Option<i64> {
        use crate::kasm::execute;
        let mut buf = Vec::new();
        for v in inputs {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        let result = execute(program, &buf).ok()?;
        if result.len() != 8 {
            return None;
        }
        Some(i64::from_le_bytes(result[..8].try_into().ok()?))
    }

    #[test]
    fn lift_dumpbin_simple_add_preserves_semantics() {
        // Convention MS x64 : RCX = arg0, RDX = arg1, RAX = retour.
        let text = "\
add_two:
0000000180001000: mov         rax,rcx
0000000180001003: add         rax,rdx
0000000180001006: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 1, "doit lifter exactement add_two");
        let p = &entries[0].program;
        assert_eq!(p.inputs(), 2);
        assert_eq!(p.outputs(), 1);
        assert_eq!(run_one_output(p, &[5, 3]), Some(8));
        assert_eq!(run_one_output(p, &[100, -42]), Some(58));
    }

    #[test]
    fn lift_objdump_simple_mul_preserves_semantics() {
        // Convention SysV : RDI = arg0, RSI = arg1, RAX = retour.
        let text = "\
0000000000401000 <mul_two>:
  401000:       mov    rax, rdi
  401003:       imul   rax, rsi
  401006:       ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Objdump);
        assert_eq!(entries.len(), 1);
        let p = &entries[0].program;
        assert_eq!(p.inputs(), 2);
        assert_eq!(run_one_output(p, &[6, 7]), Some(42));
        assert_eq!(run_one_output(p, &[-3, 5]), Some(-15));
    }

    #[test]
    fn lift_xor_zero_idiom_then_add() {
        // xor rax,rax → 0 ; add rax, rcx → rcx ; ret. Output = arg0.
        let text = "\
identity:
0000000180001000: xor         rax,rax
0000000180001003: add         rax,rcx
0000000180001006: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 1);
        assert_eq!(run_one_output(&entries[0].program, &[42]), Some(42));
        assert_eq!(run_one_output(&entries[0].program, &[-7]), Some(-7));
    }

    #[test]
    fn lift_handles_push_pop_prologue() {
        // Prologue/épilogue MSVC standard, intercalé avec un add.
        let text = "\
framed_add:
0000000180001000: push        rbp
0000000180001001: mov         rbp,rsp
0000000180001004: mov         rax,rcx
0000000180001007: add         rax,rdx
000000018000100a: pop         rbp
000000018000100b: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 1, "le prologue/épilogue doit être ignoré");
        assert_eq!(run_one_output(&entries[0].program, &[10, 20]), Some(30));
    }

    #[test]
    fn lift_skips_branch_instructions() {
        let text = "\
branchy:
0000000180001000: mov         rax,rcx
0000000180001003: jmp         0000000180001100h
0000000180001008: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction avec jmp doit être skipée");
    }

    #[test]
    fn lift_skips_call_instructions() {
        let text = "\
callable:
0000000180001000: mov         rax,rcx
0000000180001003: call        0000000180002000h
0000000180001008: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty());
    }

    #[test]
    fn lift_skips_xmm_registers() {
        let text = "\
sse_func:
0000000180001000: movaps      xmm0,xmm1
0000000180001003: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction avec xmm doit être skipée");
    }

    #[test]
    fn lift_skips_memory_references() {
        let text = "\
mem_func:
0000000180001000: mov         rax,qword ptr [rcx+8]
0000000180001004: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction avec mémoire doit être skipée");
    }

    #[test]
    fn lift_skips_imm_too_large_for_i16() {
        // 1 000 000 ne fit pas en i16 : on ne peut pas représenter la const → skip.
        let text = "\
big_imm:
0000000180001000: mov         rax,1000000
0000000180001005: add         rax,rcx
0000000180001008: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "constante > i16 doit échouer le lift");
    }

    #[test]
    fn lift_returns_empty_when_no_function_label() {
        let text = "mov rax,rcx\nadd rax,rdx\nret\n";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty());
    }

    #[test]
    fn lift_handles_multiple_functions_in_one_input() {
        let text = "\
add_two:
0000000180001000: mov         rax,rcx
0000000180001003: add         rax,rdx
0000000180001006: ret
sub_two:
0000000180002000: mov         rax,rcx
0000000180002003: sub         rax,rdx
0000000180002006: ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert_eq!(entries.len(), 2);
        // Les deux entrées doivent avoir des term_hash différents (sémantiques distinctes).
        assert_ne!(entries[0].term_hash, entries[1].term_hash);
    }

    #[test]
    fn lift_function_caps_at_50_instructions() {
        // 51 movs + ret = 52 instructions → skip.
        let mut text = String::from("huge:\n");
        for i in 0..51 {
            text.push_str(&format!("0000000180001{i:03}: mov         rax,rcx\n"));
        }
        text.push_str("00000001800010ff: ret\n");
        let entries = Corpus::lift_disasm_text(&text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "> 50 instructions doit être rejeté");
    }

    #[test]
    fn lift_system_binary_returns_empty_on_bogus_path() {
        // Path bidon : soit l'outil est absent → vec vide ; soit il est
        // présent mais retourne une erreur sur le path → vec vide. Aucun panic.
        let entries = Corpus::lift_system_binary(Path::new("Z:/__nonexistent__/__forge_eta_test__"));
        assert!(entries.is_empty());
    }

    #[test]
    fn lift_skips_function_without_ret() {
        let text = "\
no_ret:
0000000180001000: mov         rax,rcx
0000000180001003: add         rax,rdx
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Dumpbin);
        assert!(entries.is_empty(), "fonction sans ret doit être skipée");
    }

    #[test]
    fn lift_handles_objdump_imm_hex_format() {
        // objdump utilise typiquement 0x... pour les immédiats.
        let text = "\
0000000000401000 <add_seven>:
  401000:       mov    rax, rdi
  401003:       add    rax, 0x7
  401006:       ret
";
        let entries = Corpus::lift_disasm_text(text, DisasmTool::Objdump);
        assert_eq!(entries.len(), 1);
        assert_eq!(run_one_output(&entries[0].program, &[10]), Some(17));
    }
}
