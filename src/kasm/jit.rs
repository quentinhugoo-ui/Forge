//! Tiny x86-64 native JIT for verified KASM programs.
//!
//! This is intentionally narrow: it emits one Windows x64 function with
//! no calls, stores every DAG value in a stack slot, and falls back to
//! the interpreter if executable memory cannot be allocated.

use std::ffi::c_void;
use std::fmt;
use std::ptr;

use super::types::{Op, Target, Ty};
use super::Program;

pub struct JitKernel {
    pub func_ptr: extern "C" fn(*const i64, *mut i64),
    batch_i64_ptr: Option<extern "C" fn(*const i64, *mut i64, usize)>,
    pub arg_count: u8,
    pub output_count: u8,
    output_types: Vec<Ty>,
    _memory: ExecutableMemory,
}

unsafe impl Send for JitKernel {}
unsafe impl Sync for JitKernel {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JitError {
    ExternalTarget(Target),
    UnsupportedPlatform,
    Compile(String),
    BadInputLength { expected: usize, got: usize },
    BadOutputCount { expected: usize, got: usize },
}

impl fmt::Display for JitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JitError::ExternalTarget(target) => write!(f, "{target:?} target cannot be JIT compiled locally"),
            JitError::UnsupportedPlatform => write!(f, "native JIT currently supports Windows x86-64 only"),
            JitError::Compile(err) => write!(f, "JIT compile error: {err}"),
            JitError::BadInputLength { expected, got } => {
                write!(f, "bad JIT input length: expected {expected} bytes, got {got}")
            }
            JitError::BadOutputCount { expected, got } => {
                write!(f, "bad JIT output count: expected {expected}, got {got}")
            }
        }
    }
}

impl std::error::Error for JitError {}

pub fn compile(program: &Program) -> Result<JitKernel, JitError> {
    if program.target().needs_external_backend() {
        return Err(JitError::ExternalTarget(program.target()));
    }
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    {
        let _ = program;
        Err(JitError::UnsupportedPlatform)
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        compile_x64_windows(program)
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn compile_x64_windows(program: &Program) -> Result<JitKernel, JitError> {
    // Ω-6.1 unaires bijectifs : implémentation native dans emit_program_body.
    // Pour ReverseBitsI64 le pattern x86 nécessite une séquence de masques
    // peu rentable face à l'interpréteur ; on bail proprement et le caller
    // (hotplan) retombe sur l'interpréteur sans casser le contrat.
    if program
        .nodes()
        .iter()
        .any(|n| n.op == Op::ReverseBitsI64)
    {
        return Err(JitError::Compile(
            "Op::ReverseBitsI64 not yet supported in JIT (interpreter fallback used)".to_string(),
        ));
    }
    if program.nodes().iter().any(|n| matches!(
        n.op,
        Op::PopcntI64 | Op::LzcntI64 | Op::TzcntI64 | Op::PextI64 | Op::PdepI64
    )) {
        return Err(JitError::Compile(
            "hardware bit intrinsics use the runtime-dispatched CPU path (interpreter fallback used)"
                .to_string(),
        ));
    }
    // Φ.0 — F64 IEEE 754 ops are not yet emitted natively. The
    // interpreter handles them via bit-cast; the JIT bails so callers
    // (hotplan) fall back transparently. ConstF64 alone is fine
    // (literal `i16` cast to f64 bits at runtime), but any program
    // that **uses** the F64 surface needs F64Op which is not lowered
    // here.
    if program
        .nodes()
        .iter()
        .any(|n| n.op == Op::F64Op || n.op == Op::ConstF64)
    {
        return Err(JitError::Compile(
            "F64 ops not yet supported in JIT (interpreter fallback used)".to_string(),
        ));
    }
    // Wave 7b — Vec inputs/outputs use a length-prefixed wire format
    // that the JIT's flat 8-bytes-per-slot calling convention can't
    // accommodate. Bail so the program runs through the interpreter
    // (which handles the Vec wire format correctly).
    if program.nodes().iter().any(|n| n.ty == Ty::VecI64) {
        return Err(JitError::Compile(
            "Ty::VecI64 ops require interpreter (length-prefixed wire format \
             incompatible with JIT calling convention)"
                .to_string(),
        ));
    }
    // KASM v1.0 — meta-ops that need atlas / runtime support not
    // available in raw x86 codegen. Brain dispatch handles them; JIT
    // bails so hotplan falls back transparently.
    //
    // Op::Cond was rejected here historically but now lowers to the
    // same codegen as Op::SelectI64 (CMOVNE branchless arithmetic).
    // Both ops have identical encoding (a=pred, b=then, imm=else)
    // and identical runtime semantics ("pred != 0 ? then : else").
    if program.nodes().iter().any(|n| matches!(
        n.op,
        Op::Adaptive | Op::Comptime | Op::Grad | Op::Memoize
        | Op::Lazy | Op::Force
        | Op::Pipeline | Op::Vmap | Op::Pmap | Op::Fori
        | Op::WhileLoop | Op::Reduce | Op::Scan
        | Op::Fractal | Op::Eval  // Wave 8 self-hosting — runtime only
    )) {
        return Err(JitError::Compile(
            "KASM v1.0+ meta-ops require Forge brain dispatch (interpreter / GPU fallback used)"
                .to_string(),
        ));
    }
    let scalar = emit_scalar_code(program)?;
    let batch = emit_batch_i64_code(program)?;
    let batch_offset = batch.as_ref().map(|_| align_to(scalar.len(), 16));
    let mut bytes = scalar;
    if let Some(batch_code) = batch {
        while bytes.len() < batch_offset.unwrap() {
            bytes.push(0x90);
        }
        bytes.extend_from_slice(&batch_code);
    }

    let memory = ExecutableMemory::new(&bytes)?;
    let func_ptr = unsafe {
        std::mem::transmute::<*const u8, extern "C" fn(*const i64, *mut i64)>(memory.ptr as *const u8)
    };
    let batch_i64_ptr = batch_offset.map(|offset| unsafe {
        std::mem::transmute::<*const u8, extern "C" fn(*const i64, *mut i64, usize)>(memory.ptr.add(offset) as *const u8)
    });

    Ok(JitKernel {
        func_ptr,
        batch_i64_ptr,
        arg_count: program.inputs(),
        output_count: program.outputs(),
        output_types: program.output_types(),
        _memory: memory,
    })
}

impl JitKernel {
    pub fn execute(&self, args: &[u8]) -> Result<Vec<u8>, JitError> {
        let expected = self.arg_count as usize * 8;
        if args.len() != expected {
            return Err(JitError::BadInputLength { expected, got: args.len() });
        }

        let inputs = args
            .chunks_exact(8)
            .map(|chunk| i64::from_le_bytes(chunk.try_into().unwrap()))
            .collect::<Vec<_>>();
        let mut output_slots = vec![0i64; self.output_count as usize];
        (self.func_ptr)(inputs.as_ptr(), output_slots.as_mut_ptr());
        Ok(self.encode_outputs(&output_slots))
    }

    pub fn execute_i64_slots(&self, inputs: &[i64], output_slots: &mut [i64]) -> Result<(), JitError> {
        if inputs.len() != self.arg_count as usize {
            return Err(JitError::BadInputLength {
                expected: self.arg_count as usize * 8,
                got: inputs.len() * 8,
            });
        }
        if output_slots.len() != self.output_count as usize {
            return Err(JitError::BadOutputCount {
                expected: self.output_count as usize,
                got: output_slots.len(),
            });
        }
        (self.func_ptr)(inputs.as_ptr(), output_slots.as_mut_ptr());
        Ok(())
    }

    pub fn execute_batch_i64(&self, inputs: &[i64], outputs: &mut [i64]) -> Result<(), JitError> {
        if self.arg_count != 1 {
            return Err(JitError::BadInputLength {
                expected: self.arg_count as usize * 8,
                got: 8,
            });
        }
        if self.output_count != 1 || self.output_types.first().copied() != Some(Ty::I64) {
            return Err(JitError::BadOutputCount {
                expected: 1,
                got: self.output_count as usize,
            });
        }
        if inputs.len() != outputs.len() {
            return Err(JitError::BadOutputCount {
                expected: inputs.len(),
                got: outputs.len(),
            });
        }
        let Some(batch) = self.batch_i64_ptr else {
            return Err(JitError::Compile("batch entry unavailable".to_string()));
        };
        batch(inputs.as_ptr(), outputs.as_mut_ptr(), inputs.len());
        Ok(())
    }

    fn encode_outputs(&self, slots: &[i64]) -> Vec<u8> {
        let mut out = Vec::with_capacity(slots.len() * 8);
        for (slot, ty) in slots.iter().copied().zip(self.output_types.iter().copied()) {
            match ty {
                // Φ.0 — F64 wire format equals the I64 8-byte LE bit
                // pattern; reachable only if a future JIT lowers F64
                // ops (currently this branch bails compile_x64_windows).
                Ty::I64 | Ty::F64 => out.extend_from_slice(&slot.to_le_bytes()),
                Ty::Bool => out.push(u8::from(slot != 0)),
                Ty::VecI64 => panic!("Ty::VecI64 not supported yet in KASM JIT output encoding"),
            }
        }
        out
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn emit_scalar_code(program: &Program) -> Result<Vec<u8>, JitError> {
    let stack_size = align_to(program.nodes().len() * 8, 16);
    let mut code = Code::default();

    // r11 = args, r10 = outputs.
    code.bytes.extend_from_slice(&[0x49, 0x89, 0xcb]);
    code.bytes.extend_from_slice(&[0x49, 0x89, 0xd2]);
    code.bytes.extend_from_slice(&[0x48, 0x81, 0xec]);
    code.i32(stack_size as i32);
    emit_program_body(program, &mut code)?;
    code.bytes.extend_from_slice(&[0x48, 0x81, 0xc4]);
    code.i32(stack_size as i32);
    code.bytes.push(0xc3);
    Ok(code.bytes)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn emit_batch_i64_code(program: &Program) -> Result<Option<Vec<u8>>, JitError> {
    if program.inputs() != 1 || program.outputs() != 1 || program.output_types() != vec![Ty::I64] {
        return Ok(None);
    }

    let stack_size = align_to(program.nodes().len() * 8, 16);
    let mut code = Code::default();

    // Preserve the loop count in r12. The body may use every volatile
    // register, but it never touches r12.
    code.bytes.extend_from_slice(&[0x41, 0x54]);
    code.bytes.extend_from_slice(&[0x4d, 0x89, 0xc4]);
    code.bytes.extend_from_slice(&[0x4d, 0x85, 0xe4]);
    let empty = code.jcc_rel32(0x84);

    // r11 = current input row, r10 = current output row.
    code.bytes.extend_from_slice(&[0x49, 0x89, 0xcb]);
    code.bytes.extend_from_slice(&[0x49, 0x89, 0xd2]);
    code.bytes.extend_from_slice(&[0x48, 0x81, 0xec]);
    code.i32(stack_size as i32);
    let loop_start = code.bytes.len();
    emit_program_body(program, &mut code)?;
    code.bytes.extend_from_slice(&[0x49, 0x83, 0xc3, 0x08]);
    code.bytes.extend_from_slice(&[0x49, 0x83, 0xc2, 0x08]);
    code.bytes.extend_from_slice(&[0x49, 0xff, 0xcc]);
    code.bytes.extend_from_slice(&[0x0f, 0x85]);
    let backpatch = code.bytes.len();
    code.i32(0);
    code.patch_rel32(backpatch, loop_start);
    code.bytes.extend_from_slice(&[0x48, 0x81, 0xc4]);
    code.i32(stack_size as i32);
    let done = code.bytes.len();
    code.patch_rel32(empty, done);
    code.bytes.extend_from_slice(&[0x41, 0x5c]);
    code.bytes.push(0xc3);

    Ok(Some(code.bytes))
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn emit_program_body(program: &Program, code: &mut Code) -> Result<(), JitError> {
    let use_counts = node_use_counts(program);
    let mut emitted_outputs = 0usize;
    for (index, node) in program.nodes().iter().copied().enumerate() {
        let a_in_rax = index > 0 && node.a as usize == index - 1;
        match node.op {
            Op::Input => code.load_arg_rax(node.imm as usize),
            Op::ConstI64 => code.mov_rax_imm(node.imm as i64),
            Op::AddI64 => code.bin_mem_rax(node.a, node.b, &[0x48, 0x03], a_in_rax),
            Op::MulI64 => code.bin_mem_rax(node.a, node.b, &[0x48, 0x0f, 0xaf], a_in_rax),
            Op::SubI64 => code.bin_mem_rax(node.a, node.b, &[0x48, 0x2b], a_in_rax),
            Op::DivI64Checked => code.div_or_rem(node.a, node.b, false, a_in_rax),
            Op::MinI64 => code.min_or_max(node.a, node.b, true, a_in_rax),
            Op::MaxI64 => code.min_or_max(node.a, node.b, false, a_in_rax),
            Op::EqI64 => code.cmp_bool(node.a, node.b, 0x94),
            Op::Hash64 => code.hash64(node.a),
            Op::SelectI64 => code.select_i64(node.a, node.b, node.imm as u16),
            // KASM v1.0 Op::Cond — branchless lowering via CMOVNE,
            // identique à SelectI64. Encoding partagé : a=pred, b=then,
            // imm=else. La sémantique "pred != 0 ? then : else" matche
            // directement le `cmp [pred], 0 ; cmovne rax, [then]` de
            // `select_i64` (qui charge else dans rax d'abord, puis
            // remplace par then si pred != 0).
            Op::Cond => code.select_i64(node.a, node.b, node.imm as u16),
            Op::AndBool => code.bin_mem_rax(node.a, node.b, &[0x48, 0x23], a_in_rax),
            Op::OrBool => code.bin_mem_rax(node.a, node.b, &[0x48, 0x0b], a_in_rax),
            Op::NotBool => {
                if !a_in_rax {
                    code.load_slot_rax(node.a);
                }
                code.bytes.extend_from_slice(&[0x48, 0x83, 0xf0, 0x01]);
            }
            Op::LtI64 => code.cmp_bool(node.a, node.b, 0x9c),
            Op::LeI64 => code.cmp_bool(node.a, node.b, 0x9e),
            Op::BitAndI64 => code.bin_mem_rax(node.a, node.b, &[0x48, 0x23], a_in_rax),
            Op::BitOrI64 => code.bin_mem_rax(node.a, node.b, &[0x48, 0x0b], a_in_rax),
            Op::BitXorI64 => code.bin_mem_rax(node.a, node.b, &[0x48, 0x33], a_in_rax),
            Op::ShlI64 => code.shift(node.a, node.b, true, a_in_rax),
            Op::ShrI64 => code.shift(node.a, node.b, false, a_in_rax),
            Op::SatAddI64 => code.sat_add_or_sub(node.a, node.b, true, a_in_rax),
            Op::SatSubI64 => code.sat_add_or_sub(node.a, node.b, false, a_in_rax),
            Op::ModI64Checked => code.div_or_rem(node.a, node.b, true, a_in_rax),
            Op::ClampI64 => {
                code.min_or_max(node.a, node.b, false, a_in_rax);
                code.store_rax_slot(index as u16);
                code.min_or_max(index as u16, node.imm as u16, true, true);
            }
            Op::ReduceAddI64 => code.reduce(node.a, node.imm as usize, true),
            Op::ReduceMulI64 => code.reduce(node.a, node.imm as usize, false),
            Op::BitFlipI64 => {
                if !a_in_rax {
                    code.load_slot_rax(node.a);
                }
                // NOT rax — 0x48 0xf7 0xd0
                code.bytes.extend_from_slice(&[0x48, 0xf7, 0xd0]);
            }
            Op::NegI64 => {
                if !a_in_rax {
                    code.load_slot_rax(node.a);
                }
                // NEG rax — 0x48 0xf7 0xd8
                code.bytes.extend_from_slice(&[0x48, 0xf7, 0xd8]);
            }
            Op::ByteswapI64 => {
                if !a_in_rax {
                    code.load_slot_rax(node.a);
                }
                // BSWAP rax — 0x48 0x0f 0xc8
                code.bytes.extend_from_slice(&[0x48, 0x0f, 0xc8]);
            }
            Op::ReverseBitsI64 => {
                // Ecartée par la garde précoce dans compile_x64_windows.
                unreachable!(
                    "ReverseBitsI64 must bail out in compile_x64_windows before reaching JIT codegen"
                );
            }
            Op::ConstF64 | Op::F64Op => {
                // Φ.0 — bailed out at the top of compile_x64_windows.
                unreachable!(
                    "F64 ops must bail out in compile_x64_windows before reaching JIT codegen"
                );
            }
            Op::Output => {
                if !a_in_rax {
                    code.load_slot_rax(node.a);
                }
                code.store_rax_output(emitted_outputs);
                emitted_outputs += 1;
            }
            // KASM v1.0 — JIT bails out before reaching here. The
            // compile_x64_windows guard rejects programs containing
            // these ops; if we still land here it's a bug.
            // Op::Cond was historically here ; now it lowers to the
            // same CMOVNE branchless codegen as Op::SelectI64.
            Op::Adaptive | Op::Comptime | Op::Grad | Op::Memoize
            | Op::Pipeline | Op::Vmap | Op::Pmap | Op::Fori
            | Op::WhileLoop | Op::Reduce | Op::Scan | Op::VLenI64
            | Op::VSumI64 | Op::VAddI64 | Op::VMulI64
            | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64 | Op::VRangeI64
            | Op::VConcatI64 | Op::VReverseI64 | Op::VBroadcastI64
            | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
            | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 | Op::VGetI64
            | Op::PopcntI64 | Op::LzcntI64 | Op::TzcntI64 | Op::PextI64 | Op::PdepI64
            | Op::Lazy | Op::Force
            | Op::Fractal | Op::Eval => {
                unreachable!(
                    "KASM v1.0+ ops must bail out in compile_x64_windows \
                     before reaching JIT codegen"
                );
            }
        }
        if should_store_value(program, &use_counts, index) {
            code.store_rax_slot(index as u16);
        }
    }

    if emitted_outputs != program.outputs() as usize {
        return Err(JitError::BadOutputCount {
            expected: program.outputs() as usize,
            got: emitted_outputs,
        });
    }
    Ok(())
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[derive(Default)]
struct Code {
    bytes: Vec<u8>,
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
impl Code {
    fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn patch_rel32(&mut self, at: usize, target: usize) {
        let rel = target as isize - (at as isize + 4);
        self.bytes[at..at + 4].copy_from_slice(&(rel as i32).to_le_bytes());
    }

    fn jcc_rel32(&mut self, cc: u8) -> usize {
        self.bytes.extend_from_slice(&[0x0f, cc]);
        let at = self.bytes.len();
        self.i32(0);
        at
    }

    fn jmp_rel32(&mut self) -> usize {
        self.bytes.push(0xe9);
        let at = self.bytes.len();
        self.i32(0);
        at
    }

    fn load_arg_rax(&mut self, slot: usize) {
        self.bytes.extend_from_slice(&[0x49, 0x8b, 0x83]);
        self.i32((slot * 8) as i32);
    }

    fn load_slot_rax(&mut self, idx: u16) {
        self.bytes.extend_from_slice(&[0x48, 0x8b, 0x84, 0x24]);
        self.i32(idx as i32 * 8);
    }

    fn load_slot_rcx(&mut self, idx: u16) {
        self.bytes.extend_from_slice(&[0x48, 0x8b, 0x8c, 0x24]);
        self.i32(idx as i32 * 8);
    }

    fn load_slot_r8(&mut self, idx: u16) {
        self.bytes.extend_from_slice(&[0x4c, 0x8b, 0x84, 0x24]);
        self.i32(idx as i32 * 8);
    }

    fn store_rax_slot(&mut self, idx: u16) {
        self.bytes.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]);
        self.i32(idx as i32 * 8);
    }

    fn store_rax_output(&mut self, output: usize) {
        self.bytes.extend_from_slice(&[0x49, 0x89, 0x82]);
        self.i32((output * 8) as i32);
    }

    fn mov_rax_imm(&mut self, value: i64) {
        self.bytes.extend_from_slice(&[0x48, 0xb8]);
        self.i64(value);
    }

    fn mov_r8_imm(&mut self, value: i64) {
        self.bytes.extend_from_slice(&[0x49, 0xb8]);
        self.i64(value);
    }

    fn bin_mem_rax(&mut self, a: u16, b: u16, opcode: &[u8], a_in_rax: bool) {
        if !a_in_rax {
            self.load_slot_rax(a);
        }
        self.bytes.extend_from_slice(opcode);
        self.bytes.extend_from_slice(&[0x84, 0x24]);
        self.i32(b as i32 * 8);
    }

    fn cmp_bool(&mut self, a: u16, b: u16, setcc: u8) {
        self.load_slot_rax(a);
        self.bytes.extend_from_slice(&[0x48, 0x3b, 0x84, 0x24]);
        self.i32(b as i32 * 8);
        self.bytes.extend_from_slice(&[0x0f, setcc, 0xc0]);
        self.bytes.extend_from_slice(&[0x48, 0x0f, 0xb6, 0xc0]);
    }

    fn min_or_max(&mut self, a: u16, b: u16, min: bool, a_in_rax: bool) {
        if !a_in_rax {
            self.load_slot_rax(a);
        }
        self.bytes.extend_from_slice(&[0x48, 0x3b, 0x84, 0x24]);
        self.i32(b as i32 * 8);
        self.bytes.extend_from_slice(&[0x48, 0x0f, if min { 0x4f } else { 0x4c }, 0x84, 0x24]);
        self.i32(b as i32 * 8);
    }

    fn select_i64(&mut self, cond: u16, if_true: u16, if_false: u16) {
        self.load_slot_rax(if_false);
        self.bytes.extend_from_slice(&[0x48, 0x83, 0xbc, 0x24]);
        self.i32(cond as i32 * 8);
        self.bytes.push(0x00);
        self.bytes.extend_from_slice(&[0x48, 0x0f, 0x45, 0x84, 0x24]);
        self.i32(if_true as i32 * 8);
    }

    fn shift(&mut self, a: u16, b: u16, left: bool, a_in_rax: bool) {
        if !a_in_rax {
            self.load_slot_rax(a);
        }
        self.load_slot_rcx(b);
        self.bytes.extend_from_slice(&[0x48, 0x83, 0xe1, 0x3f]);
        self.bytes.extend_from_slice(&[0x48, 0xd3, if left { 0xe0 } else { 0xe8 }]);
    }

    fn reduce(&mut self, base: u16, count: usize, add: bool) {
        self.mov_rax_imm(if add { 0 } else { 1 });
        for idx in base as usize..base as usize + count {
            self.bytes.extend_from_slice(if add { &[0x48, 0x03] } else { &[0x48, 0x0f, 0xaf] });
            self.bytes.extend_from_slice(&[0x84, 0x24]);
            self.i32(idx as i32 * 8);
        }
    }

    fn sat_add_or_sub(&mut self, a: u16, b: u16, add: bool, a_in_rax: bool) {
        if !a_in_rax {
            self.load_slot_rax(a);
        }
        self.bytes.extend_from_slice(&[0x49, 0x89, 0xc0]);
        self.bytes.extend_from_slice(if add { &[0x48, 0x03] } else { &[0x48, 0x2b] });
        self.bytes.extend_from_slice(&[0x84, 0x24]);
        self.i32(b as i32 * 8);
        let overflow = self.jcc_rel32(0x80);
        let done_fast = self.jmp_rel32();
        let overflow_target = self.bytes.len();
        self.patch_rel32(overflow, overflow_target);
        self.bytes.extend_from_slice(&[0x4d, 0x85, 0xc0]);
        let min = self.jcc_rel32(0x88);
        self.mov_rax_imm(i64::MAX);
        let done_sat = self.jmp_rel32();
        let min_target = self.bytes.len();
        self.patch_rel32(min, min_target);
        self.mov_rax_imm(i64::MIN);
        let done = self.bytes.len();
        self.patch_rel32(done_fast, done);
        self.patch_rel32(done_sat, done);
    }

    fn div_or_rem(&mut self, a: u16, b: u16, rem: bool, a_in_rax: bool) {
        self.load_slot_r8(b);
        self.bytes.extend_from_slice(&[0x4d, 0x85, 0xc0]);
        let invalid_zero = self.jcc_rel32(0x84);
        if !a_in_rax {
            self.load_slot_rax(a);
        }
        self.bytes.extend_from_slice(&[0x49, 0xb9]);
        self.i64(i64::MIN);
        self.bytes.extend_from_slice(&[0x4c, 0x39, 0xc8]);
        let do_div_after_min_check = self.jcc_rel32(0x85);
        self.bytes.extend_from_slice(&[0x49, 0x83, 0xf8, 0xff]);
        let invalid_overflow = self.jcc_rel32(0x84);
        let do_div = self.bytes.len();
        self.patch_rel32(do_div_after_min_check, do_div);
        self.bytes.extend_from_slice(&[0x48, 0x99]);
        self.bytes.extend_from_slice(&[0x49, 0xf7, 0xf8]);
        if rem {
            self.bytes.extend_from_slice(&[0x48, 0x89, 0xd0]);
        }
        let done_after_div = self.jmp_rel32();
        let invalid = self.bytes.len();
        self.patch_rel32(invalid_zero, invalid);
        self.patch_rel32(invalid_overflow, invalid);
        self.bytes.extend_from_slice(&[0x31, 0xc0]);
        let done = self.bytes.len();
        self.patch_rel32(done_after_div, done);
    }

    fn hash64(&mut self, a: u16) {
        self.load_slot_rax(a);
        self.mov_r8_imm(0x9e3779b97f4a7c15u64 as i64);
        self.bytes.extend_from_slice(&[0x4c, 0x01, 0xc0]);
        self.mix_xor_shr(30);
        self.mov_r8_imm(0xbf58476d1ce4e5b9u64 as i64);
        self.bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc0]);
        self.mix_xor_shr(27);
        self.mov_r8_imm(0x94d049bb133111ebu64 as i64);
        self.bytes.extend_from_slice(&[0x49, 0x0f, 0xaf, 0xc0]);
        self.mix_xor_shr(31);
    }

    fn mix_xor_shr(&mut self, shift: u8) {
        self.bytes.extend_from_slice(&[0x49, 0x89, 0xc0]);
        self.bytes.extend_from_slice(&[0x49, 0xc1, 0xe8, shift]);
        self.bytes.extend_from_slice(&[0x4c, 0x31, 0xc0]);
    }
}

fn align_to(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn node_use_counts(program: &Program) -> Vec<u16> {
    let mut counts = vec![0u16; program.nodes().len()];
    for node in program.nodes().iter().copied() {
        let mut bump = |idx: u16| {
            if let Some(count) = counts.get_mut(idx as usize) {
                *count = count.saturating_add(1);
            }
        };
        match node.op {
            Op::Input | Op::ConstI64 => {}
            Op::Hash64
            | Op::NotBool
            | Op::Output
            | Op::BitFlipI64
            | Op::NegI64
            | Op::ReverseBitsI64
            | Op::ByteswapI64 => bump(node.a),
            Op::AddI64
            | Op::MulI64
            | Op::SubI64
            | Op::DivI64Checked
            | Op::MinI64
            | Op::MaxI64
            | Op::EqI64
            | Op::AndBool
            | Op::OrBool
            | Op::LtI64
            | Op::LeI64
            | Op::BitAndI64
            | Op::BitOrI64
            | Op::BitXorI64
            | Op::ShlI64
            | Op::ShrI64
            | Op::SatAddI64
            | Op::SatSubI64
            | Op::ModI64Checked => {
                bump(node.a);
                bump(node.b);
            }
            Op::SelectI64 | Op::ClampI64 | Op::Cond => {
                bump(node.a);
                bump(node.b);
                bump(node.imm as u16);
            }
            Op::ReduceAddI64 | Op::ReduceMulI64 => {
                for idx in node.a as usize..node.a as usize + node.imm as usize {
                    bump(idx as u16);
                }
            }
            // Φ.0 — guarded out at top of compile_x64_windows. Counters
            // unused for F64 since the JIT bails before this is invoked.
            Op::ConstF64 | Op::F64Op => unreachable!(
                "F64 ops must bail out in compile_x64_windows before reaching node_use_counts"
            ),
            // KASM v1.0 — guarded out at top of compile_x64_windows.
            // Op::Cond moved to the SelectI64 branch above (same arity,
            // same CMOVNE lowering).
            Op::Adaptive | Op::Comptime | Op::Grad | Op::Memoize
            | Op::Pipeline | Op::Vmap | Op::Pmap | Op::Fori
            | Op::WhileLoop | Op::Reduce | Op::Scan | Op::VLenI64
            | Op::VSumI64 | Op::VAddI64 | Op::VMulI64
            | Op::VSubI64 | Op::VMaxI64 | Op::VMinI64 | Op::VRangeI64
            | Op::VConcatI64 | Op::VReverseI64 | Op::VBroadcastI64
            | Op::VEqI64 | Op::VAndI64 | Op::VOrI64 | Op::VXorI64
            | Op::VAbsI64 | Op::VNegI64 | Op::VBitFlipI64 | Op::VGetI64
            | Op::PopcntI64 | Op::LzcntI64 | Op::TzcntI64 | Op::PextI64 | Op::PdepI64
            | Op::Lazy | Op::Force
            | Op::Fractal | Op::Eval  // Wave 8 self-hosting
            => unreachable!(
                "KASM v1.0+ ops must bail out in compile_x64_windows before reaching node_use_counts"
            ),
        }
    }
    counts
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn should_store_value(program: &Program, use_counts: &[u16], index: usize) -> bool {
    let uses = use_counts[index];
    if uses == 0 {
        return false;
    }
    !(uses == 1 && next_uses_as_primary_input(program, index))
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
fn next_uses_as_primary_input(program: &Program, index: usize) -> bool {
    let Some(next) = program.nodes().get(index + 1).copied() else {
        return false;
    };
    let idx = index as u16;
    match next.op {
        Op::AddI64
        | Op::MulI64
        | Op::SubI64
        | Op::DivI64Checked
        | Op::MinI64
        | Op::MaxI64
        | Op::AndBool
        | Op::OrBool
        | Op::NotBool
        | Op::BitAndI64
        | Op::BitOrI64
        | Op::BitXorI64
        | Op::ShlI64
        | Op::ShrI64
        | Op::SatAddI64
        | Op::SatSubI64
        | Op::ModI64Checked
        | Op::ClampI64
        | Op::Output
        | Op::BitFlipI64
        | Op::NegI64
        | Op::ReverseBitsI64
        | Op::ByteswapI64
        | Op::PopcntI64
        | Op::LzcntI64
        | Op::TzcntI64 => next.a == idx,
        _ => false,
    }
}

struct ExecutableMemory {
    ptr: *mut u8,
    len: usize,
}

unsafe impl Send for ExecutableMemory {}
unsafe impl Sync for ExecutableMemory {}

impl ExecutableMemory {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    fn new(code: &[u8]) -> Result<Self, JitError> {
        const MEM_COMMIT: u32 = 0x1000;
        const MEM_RESERVE: u32 = 0x2000;
        const PAGE_READWRITE: u32 = 0x04;
        const PAGE_EXECUTE_READ: u32 = 0x20;

        let ptr = unsafe {
            VirtualAlloc(
                ptr::null_mut(),
                code.len(),
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as *mut u8;
        if ptr.is_null() {
            return Err(JitError::Compile("VirtualAlloc failed".to_string()));
        }
        unsafe {
            ptr::copy_nonoverlapping(code.as_ptr(), ptr, code.len());
        }
        let mut old = 0u32;
        let ok = unsafe { VirtualProtect(ptr as *mut c_void, code.len(), PAGE_EXECUTE_READ, &mut old) };
        if ok == 0 {
            unsafe {
                VirtualFree(ptr as *mut c_void, 0, 0x8000);
            }
            return Err(JitError::Compile("VirtualProtect failed".to_string()));
        }
        Ok(Self { ptr, len: code.len() })
    }
}

impl Drop for ExecutableMemory {
    fn drop(&mut self) {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        unsafe {
            let _ = self.len;
            VirtualFree(self.ptr as *mut c_void, 0, 0x8000);
        }
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(address: *mut c_void, size: usize, allocation_type: u32, protect: u32) -> *mut c_void;
    fn VirtualProtect(address: *mut c_void, size: usize, new_protect: u32, old_protect: *mut u32) -> i32;
    fn VirtualFree(address: *mut c_void, size: usize, free_type: u32) -> i32;
}
