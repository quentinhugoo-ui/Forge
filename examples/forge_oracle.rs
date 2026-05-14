//! Φ.3 — Forge Oracle: external delegation interface.
//!
//! This is the missing link between Forge's internal capabilities and
//! external users (traders, mathematicians, biologists, AI agents
//! delegating computation, etc). Without it, every glyph and op added
//! in Φ.0 → Φ.2.1 is invisible to the world.
//!
//! ## Usage
//!
//! ```text
//! echo '{"examples":[[1,2],[2,4],[3,6]],"test_inputs":[10,20,100]}' \
//!     | cargo run --release --example forge_oracle
//! ```
//!
//! Stdin: a single JSON line per request (one-shot mode).
//! Stdout: a single JSON line with the synthesised program's hash, the
//! synthesis source, and predictions for the test inputs.
//!
//! ## Why this matters
//!
//! 1. The persistent `forge.cas` at `.codex-tmp/forge-oracle/` means a
//!    repeat call with identical examples returns the cached program
//!    in a few microseconds — Forge's "known computation = no
//!    recomputation" promise extended to external clients.
//!
//! 2. The returned `program_hash` IS a cryptographic identity. An AI
//!    delegating computation can store the hash, hand it to a teammate
//!    (or a future self), and re-derive the exact same program later.
//!
//! 3. The interface is dependency-free (pure Rust + std + sha2). A
//!    Python script, a Node service, a shell pipe, or another LLM can
//!    spawn this binary and pipe JSON in/out. No need for serde, no
//!    HTTP framework, no protobuf — just one stdin line, one stdout
//!    line.
//!
//! ## Schema
//!
//! Request:
//! ```json
//! {
//!   "examples":     [[x1, y1], [x2, y2], ...],   // required, ≥1 pair
//!   "test_inputs":  [x_test1, x_test2, ...],     // optional, default []
//!   "max_nodes":    8,                            // optional
//!   "beam_width":   256,                          // optional
//!   "generations":  3,                            // optional
//!   "holdout_stride": 4                           // optional
//! }
//! ```
//!
//! Response (success):
//! ```json
//! {
//!   "program_hash": "<40-hex>",
//!   "program_nodes": 6,
//!   "source": "memo|retrieval|glyph|ultra_glyph|structured|beam",
//!   "exact_train": true,
//!   "exact_holdout": true,
//!   "candidates_evaluated": 0,
//!   "predictions": [y_test1, y_test2, ...]
//! }
//! ```
//!
//! Response (error):
//! ```json
//! {"error": "<message>"}
//! ```

use std::io::{self, Read, Write};
use std::sync::Arc;

use scan::kasm::{execute, Program};
use scan::{
    MemoryGovernor, MonsterEvolutionConfig, MonsterEvolutionOutcome, MonsterNode, Store,
};

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let response = match handle_request() {
        Ok(s) => s,
        Err(msg) => format!(r#"{{"error":"{}"}}"#, escape(&msg)),
    };
    let _ = writeln!(out, "{response}");
}

fn handle_request() -> Result<String, String> {
    // Φ.3 — read one request from stdin. We don't bother with framing
    // (newline-delimited or otherwise) — the caller pipes in a single
    // JSON object and we read until EOF. Multi-shot mode would parse a
    // line at a time; left for a follow-up phase if real users want it.
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    let req = parse_request(&input)?;

    if req.examples.is_empty() {
        return Err("examples must contain at least one (x, y) pair".to_string());
    }

    // Φ.3 — persistent shared store, separate from the lab. Cross-call
    // memoisation kicks in automatically via the Φ.2.1 cache layer.
    let store_path = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(".codex-tmp")
        .join("forge-oracle");
    let store = Store::open(&store_path).map_err(|e: io::Error| e.to_string())?;
    let monster = MonsterNode::shared(Arc::new(store), MemoryGovernor::new(64 * 1024 * 1024));

    let outcome = monster
        .evolve_i64_program(&req.examples, req.config())
        .map_err(|e| e.to_string())?;

    // Φ.3 — execute the synthesised program on each test input. The
    // program is verified by construction (Program::from_bytes ran in
    // evolve_i64_program), so execute() can only fail on a true type
    // mismatch — which would itself be a synthesis bug.
    let predictions = req
        .test_inputs
        .iter()
        .map(|x| run_one(&outcome.program, *x))
        .collect::<Result<Vec<i64>, String>>()?;

    Ok(serialise_response(
        &outcome.program,
        &outcome,
        &predictions,
        req.f64_mode,
    ))
}

/// Φ.7b — Total f64 → i64 truncation. Mirrors KASM's `F64ToI64`
/// op semantics: NaN/±Inf collapse to 0, magnitudes outside i64
/// range saturate. Keeps every f64 input mapped to a finite i64.
fn f64_to_i64_trunc(v: f64) -> i64 {
    if !v.is_finite() {
        return 0;
    }
    if v >= i64::MAX as f64 {
        return i64::MAX;
    }
    if v <= i64::MIN as f64 {
        return i64::MIN;
    }
    v as i64
}

fn run_one(program: &Program, x: i64) -> Result<i64, String> {
    let bytes = x.to_le_bytes();
    let out = execute(program, &bytes).map_err(|e| e.to_string())?;
    if out.len() != 8 {
        return Err(format!("expected 8-byte i64 output, got {}", out.len()));
    }
    Ok(i64::from_le_bytes(out.try_into().unwrap()))
}

fn serialise_response(
    program: &Program,
    outcome: &MonsterEvolutionOutcome,
    predictions: &[i64],
    f64_mode: bool,
) -> String {
    let preds_i64 = predictions
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");
    // Φ.7b — In f64 mode we emit BOTH the raw bit-pattern array
    // (for callers wanting deterministic round-trip) and the
    // human-readable f64 array (cast back via from_bits).
    let extra = if f64_mode {
        // Φ.7b — f64 mode uses truncation encoding (not bit-cast)
        // so predictions are already small integers; converting to
        // f64 is just `as f64` — the reverse of `f64_to_i64_trunc`.
        let preds_f64 = predictions
            .iter()
            .map(|p| format!("{}", *p as f64))
            .collect::<Vec<_>>()
            .join(",");
        format!(r#","predictions_f64":[{preds_f64}]"#)
    } else {
        String::new()
    };
    format!(
        r#"{{"program_hash":"{hash}","program_nodes":{nodes},"source":"{src}","exact_train":{et},"exact_holdout":{eh},"candidates_evaluated":{cand},"predictions":[{preds_i64}]{extra}}}"#,
        hash = outcome.program_hash.as_hex(),
        nodes = program.nodes().len(),
        src = outcome.source,
        et = outcome.exact_train,
        eh = outcome.exact_holdout,
        cand = outcome.candidates_evaluated,
    )
}

// ---------------------------------------------------------------------------
// Φ.3 — Minimal JSON parser
//
// We accept the narrow schema documented at the top of the file. This
// is **not** a general JSON parser — it scans for known top-level keys
// and parses their value via a tiny recursive descent. Keeps Forge
// dependency-free (no serde, no `json` crate) while still letting any
// external client speak standard JSON.
// ---------------------------------------------------------------------------

struct Request {
    /// Bit patterns ready for the synthesizer. In f64 mode these
    /// carry IEEE 754 bit patterns of the user's f64 examples.
    examples: Vec<(i64, i64)>,
    test_inputs: Vec<i64>,
    /// Φ.7b — When true, the JSON came in via `examples_f64` /
    /// `test_inputs_f64`. The response will emit `predictions_f64`
    /// alongside the standard `predictions` (raw bit pattern).
    /// Synthesis behaviour is identical — the F64 ultra-glyphs from
    /// Φ.0..Φ.6 already operate on these bit patterns. When data
    /// fits an F64 ultra-glyph (sqrt-affine, fdiv-affine, etc.) the
    /// recognizer hits; integer-valued floats may also fit the
    /// regular i64 recognizers by coincidence.
    f64_mode: bool,
    max_nodes: Option<usize>,
    beam_width: Option<usize>,
    generations: Option<usize>,
    holdout_stride: Option<usize>,
}

impl Request {
    fn config(&self) -> MonsterEvolutionConfig {
        let mut cfg = MonsterEvolutionConfig::default();
        if let Some(v) = self.max_nodes {
            cfg.max_nodes = v.max(3);
        }
        if let Some(v) = self.beam_width {
            cfg.beam_width = v.max(32);
        }
        if let Some(v) = self.generations {
            cfg.generations = v.max(1);
        }
        if let Some(v) = self.holdout_stride {
            cfg.holdout_stride = v.max(2);
        }
        cfg
    }
}

fn parse_request(input: &str) -> Result<Request, String> {
    let max_nodes = parse_u64_at(input, "\"max_nodes\"").map(|v| v as usize);
    let beam_width = parse_u64_at(input, "\"beam_width\"").map(|v| v as usize);
    let generations = parse_u64_at(input, "\"generations\"").map(|v| v as usize);
    let holdout_stride = parse_u64_at(input, "\"holdout_stride\"").map(|v| v as usize);

    // Φ.7b — try f64 mode first (presence of "examples_f64" key).
    //
    // **Encoding choice — truncation, not bit-cast.** The
    // synthesizer's recognizers (Φ.0..Φ.6) interpret values as
    // integers when scoring formulas. Bit-casting f64 → i64 would
    // feed the recognizers nonsensical integers (sign+exponent+
    // mantissa packed in a u64). Truncating (`as i64`) matches
    // KASM's `F64ToI64` op semantics — the same lossy round-trip
    // the F64 ultra-glyphs use internally — so a user's f64 data
    // hits the same recognizer landscape that the lab benchmarks
    // probe.
    //
    // The cost: f64 inputs with non-integer parts lose precision
    // (1.5 → 1, 100.50 → 100). For finance / cents-precision use
    // cases the caller scales by 100 first. For physics / dose-
    // response with unit magnitudes the integer-valued floats
    // (1.0, 2.0, 3.0...) round-trip cleanly.
    if find_value_start(input, "\"examples_f64\"").is_some() {
        let examples_f64 = parse_examples_f64(input)?;
        let test_inputs_f64 =
            parse_array_f64_at(input, "\"test_inputs_f64\"").unwrap_or_default();
        return Ok(Request {
            examples: examples_f64
                .iter()
                .map(|(x, y)| (f64_to_i64_trunc(*x), f64_to_i64_trunc(*y)))
                .collect(),
            test_inputs: test_inputs_f64
                .iter()
                .map(|x| f64_to_i64_trunc(*x))
                .collect(),
            f64_mode: true,
            max_nodes,
            beam_width,
            generations,
            holdout_stride,
        });
    }

    // Fall through: classic i64 mode.
    Ok(Request {
        examples: parse_examples(input)?,
        test_inputs: parse_array_i64_at(input, "\"test_inputs\"").unwrap_or_default(),
        f64_mode: false,
        max_nodes,
        beam_width,
        generations,
        holdout_stride,
    })
}

fn parse_examples(input: &str) -> Result<Vec<(i64, i64)>, String> {
    let pos = find_value_start(input, "\"examples\"")
        .ok_or_else(|| "missing required key \"examples\" (or \"examples_f64\")".to_string())?;
    let mut reader = JsonReader::new(input, pos);
    let mut out = Vec::new();
    reader.expect(b'[')?;
    reader.skip_ws();
    if reader.peek() == Some(b']') {
        reader.bump();
        return Ok(out);
    }
    loop {
        reader.expect(b'[')?;
        let x = reader.parse_i64()?;
        reader.skip_ws();
        reader.expect(b',')?;
        let y = reader.parse_i64()?;
        reader.skip_ws();
        reader.expect(b']')?;
        out.push((x, y));
        reader.skip_ws();
        match reader.peek() {
            Some(b',') => {
                reader.bump();
                reader.skip_ws();
            }
            Some(b']') => {
                reader.bump();
                break;
            }
            _ => return Err(format!("expected ',' or ']' at pos {}", reader.pos)),
        }
    }
    Ok(out)
}

/// Φ.7b — Parse `[[1.5, 2.25], [2.0, 4.0], ...]`. Mirrors
/// `parse_examples` for the f64 surface introduced in Φ.0.
fn parse_examples_f64(input: &str) -> Result<Vec<(f64, f64)>, String> {
    let pos = find_value_start(input, "\"examples_f64\"")
        .ok_or_else(|| "missing key \"examples_f64\"".to_string())?;
    let mut reader = JsonReader::new(input, pos);
    let mut out = Vec::new();
    reader.expect(b'[')?;
    reader.skip_ws();
    if reader.peek() == Some(b']') {
        reader.bump();
        return Ok(out);
    }
    loop {
        reader.expect(b'[')?;
        let x = reader.parse_f64()?;
        reader.skip_ws();
        reader.expect(b',')?;
        let y = reader.parse_f64()?;
        reader.skip_ws();
        reader.expect(b']')?;
        out.push((x, y));
        reader.skip_ws();
        match reader.peek() {
            Some(b',') => {
                reader.bump();
                reader.skip_ws();
            }
            Some(b']') => {
                reader.bump();
                break;
            }
            _ => return Err(format!("expected ',' or ']' at pos {}", reader.pos)),
        }
    }
    Ok(out)
}

fn parse_array_f64_at(input: &str, key: &str) -> Option<Vec<f64>> {
    let pos = find_value_start(input, key)?;
    let mut reader = JsonReader::new(input, pos);
    reader.expect(b'[').ok()?;
    let mut out = Vec::new();
    reader.skip_ws();
    if reader.peek() == Some(b']') {
        reader.bump();
        return Some(out);
    }
    loop {
        let v = reader.parse_f64().ok()?;
        out.push(v);
        reader.skip_ws();
        match reader.peek()? {
            b',' => {
                reader.bump();
                reader.skip_ws();
            }
            b']' => {
                reader.bump();
                return Some(out);
            }
            _ => return None,
        }
    }
}

fn parse_array_i64_at(input: &str, key: &str) -> Option<Vec<i64>> {
    let pos = find_value_start(input, key)?;
    let mut reader = JsonReader::new(input, pos);
    reader.expect(b'[').ok()?;
    let mut out = Vec::new();
    reader.skip_ws();
    if reader.peek() == Some(b']') {
        reader.bump();
        return Some(out);
    }
    loop {
        let v = reader.parse_i64().ok()?;
        out.push(v);
        reader.skip_ws();
        match reader.peek()? {
            b',' => {
                reader.bump();
                reader.skip_ws();
            }
            b']' => {
                reader.bump();
                return Some(out);
            }
            _ => return None,
        }
    }
}

fn parse_u64_at(input: &str, key: &str) -> Option<u64> {
    let pos = find_value_start(input, key)?;
    let mut reader = JsonReader::new(input, pos);
    reader.skip_ws();
    let mut n = 0u64;
    let mut any = false;
    while let Some(c) = reader.peek() {
        if c.is_ascii_digit() {
            n = n * 10 + u64::from(c - b'0');
            reader.bump();
            any = true;
        } else {
            break;
        }
    }
    if any {
        Some(n)
    } else {
        None
    }
}

/// Locate the byte offset of the value following `"key" :`. Returns
/// `None` if the key isn't present in `input`.
fn find_value_start(input: &str, key: &str) -> Option<usize> {
    let idx = input.find(key)? + key.len();
    let bytes = input.as_bytes();
    let mut pos = idx;
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    if pos >= bytes.len() || bytes[pos] != b':' {
        return None;
    }
    pos += 1;
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    Some(pos)
}

struct JsonReader<'a> {
    s: &'a [u8],
    pos: usize,
}

impl<'a> JsonReader<'a> {
    fn new(input: &'a str, pos: usize) -> Self {
        Self { s: input.as_bytes(), pos }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.skip_ws();
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected '{}' at pos {}",
                c as char, self.pos
            ))
        }
    }

    fn parse_i64(&mut self) -> Result<i64, String> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let mut digits = 0;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
                digits += 1;
            } else {
                break;
            }
        }
        if digits == 0 {
            return Err(format!("expected integer at pos {}", start));
        }
        let s = std::str::from_utf8(&self.s[start..self.pos]).map_err(|e| e.to_string())?;
        s.parse::<i64>()
            .map_err(|e| format!("bad integer at pos {start}: {e}"))
    }

    /// Φ.7b — Parse a JSON number as f64. Accepts the standard JSON
    /// number grammar: optional minus, integer part, optional fraction
    /// (`.123`), optional exponent (`e+12` or `E-3`). Delegates to
    /// `f64::from_str` once the slice is bounded — keeps Forge's
    /// dependency stack at `std + sha2`.
    fn parse_f64(&mut self) -> Result<f64, String> {
        self.skip_ws();
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        // Integer part.
        let int_digits = self.consume_digits();
        // Optional fractional part.
        if self.peek() == Some(b'.') {
            self.pos += 1;
            self.consume_digits();
        }
        // Optional exponent.
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            self.consume_digits();
        }
        if self.pos == start || (self.pos == start + 1 && self.s[start] == b'-') || int_digits == 0
        {
            return Err(format!("expected number at pos {}", start));
        }
        let s = std::str::from_utf8(&self.s[start..self.pos]).map_err(|e| e.to_string())?;
        s.parse::<f64>()
            .map_err(|e| format!("bad number at pos {start}: {e}"))
    }

    fn consume_digits(&mut self) -> usize {
        let mut count = 0;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
                count += 1;
            } else {
                break;
            }
        }
        count
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
