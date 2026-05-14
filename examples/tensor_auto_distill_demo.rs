//! End-to-end demonstration of **automatic** tensor-program
//! distillation. (Φ.μ.7 : la démo manuelle pédagogique
//! `tensor_layer_distill_demo.rs` a été supprimée — celle-ci
//! couvre la même fonctionnalité avec en plus la découverte automatique.)
//! Le shortcut est **discovered**, not handwritten :
//!
//!   1. Build the full FFN program (matmul → ReLU → matmul).
//!   2. Run a workload of N realistic samples through it,
//!      capturing the inputs (the runtime would also capture
//!      activations, but `try_distill_ffn_block` re-derives them
//!      host-side from the observed inputs).
//!   3. Call `try_distill_ffn_block(program, samples, config)`.
//!   4. The function returns a shorter `TensorProgram` that
//!      reproduces every observed output within 1e-5 — without
//!      any human writing the shortcut.
//!   5. Bench: full vs shortcut, measure the gain.
//!
//! This is what the future "Always-On DreamForge-Tensor" daemon
//! would do automatically in the background of the GPUnode runtime,
//! the same way Vague 5's daemon distills i64 programs.

use std::time::Instant;

use scan::kasm::tensor::{
    execute_tensor, try_distill_ffn_block, DistillTensorConfig, TensorNode, TensorProgram,
    TensorShape, TensorTy,
};
use scan::Hash;

const IN_DIM: usize = 8;
const HIDDEN: usize = 16;
const OUT_DIM: usize = 4;
const N_SAMPLES_OBSERVATION: usize = 64;
const N_SAMPLES_BENCH: usize = 4_000;

fn f32_pool(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn positive_weights(rows: usize, cols: usize, seed: u64) -> Vec<f32> {
    let mut s = seed.max(1);
    let mut out = Vec::with_capacity(rows * cols);
    for _ in 0..rows * cols {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let bits = (s & 0xFFFF) as f32 / 65535.0;
        out.push(0.05 + bits * 0.25);
    }
    out
}

fn random_positive_input(seed: u64) -> Vec<f32> {
    let mut s = seed.max(1);
    let mut out = Vec::with_capacity(IN_DIM);
    for _ in 0..IN_DIM {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let bits = (s & 0xFFFF) as f32 / 65535.0;
        out.push(bits);
    }
    out
}

fn build_full_ffn(w1: &[f32], w2: &[f32]) -> TensorProgram {
    let x_shape = TensorShape::matrix(1, IN_DIM).unwrap();
    let w1_shape = TensorShape::matrix(IN_DIM, HIDDEN).unwrap();
    let h_shape = TensorShape::matrix(1, HIDDEN).unwrap();
    let w2_shape = TensorShape::matrix(HIDDEN, OUT_DIM).unwrap();
    let y_shape = TensorShape::matrix(1, OUT_DIM).unwrap();

    let w1_pool = f32_pool(w1);
    let w2_pool = f32_pool(w2);
    let mut pool = w1_pool.clone();
    let w2_off = pool.len() as u32;
    pool.extend_from_slice(&w2_pool);

    let nodes = vec![
        TensorNode::input(0, TensorTy::F32, x_shape),
        TensorNode::const_at(0, w1_pool.len() as u32, TensorTy::F32, w1_shape),
        TensorNode::const_at(w2_off, w2_pool.len() as u32, TensorTy::F32, w2_shape),
        TensorNode::matmul(0, 1, TensorTy::F32, h_shape),
        TensorNode::relu(3, TensorTy::F32, h_shape),
        TensorNode::matmul(4, 2, TensorTy::F32, y_shape),
        TensorNode::output(5, TensorTy::F32, y_shape),
    ];
    TensorProgram::new(1, 1, nodes.len() as u32, nodes, pool).unwrap()
}

fn main() -> std::io::Result<()> {
    println!("=== tensor_auto_distill_demo ===");
    println!("Automatic shortcut discovery on an FFN block —");
    println!("no human writes the shortcut, the runtime synthesises it.");
    println!();
    println!("  IN_DIM = {IN_DIM}   HIDDEN = {HIDDEN}   OUT_DIM = {OUT_DIM}");
    println!("  observation samples : {N_SAMPLES_OBSERVATION}");
    println!();

    // Step 1: build the full program.
    let w1 = positive_weights(IN_DIM, HIDDEN, 0xDEAD_BEEF);
    let w2 = positive_weights(HIDDEN, OUT_DIM, 0xCAFE_BABE);
    let full = build_full_ffn(&w1, &w2);
    let full_hash = Hash::for_blob(full.bytes());
    println!("  full program  : {} nodes  hash {}", full.nodes().len(), full_hash.as_hex());

    // Step 2: workload — observe samples that would be coming from
    // a real user. Here we synthesise them; in production the
    // colony's call_one_i64-equivalent for tensors would capture
    // these on the fly.
    let observations: Vec<Vec<f32>> = (0..N_SAMPLES_OBSERVATION as u64)
        .map(|s| random_positive_input(s.wrapping_mul(0x9E37_79B9_7F4A_7C15)))
        .collect();

    // Step 3: call try_distill_ffn_block. THIS IS THE WHOLE POINT.
    // No reference to W₁·W₂ in the caller; the function discovers
    // the pattern, computes the combined weights, builds the
    // shortcut, and validates it.
    let t0 = Instant::now();
    let outcome = try_distill_ffn_block(&full, &observations, DistillTensorConfig::default());
    let t_distill = t0.elapsed();
    let shortcut = match outcome {
        Ok(Some(s)) => s,
        Ok(None) => {
            println!("  pattern not matched — nothing to distill.");
            return Ok(());
        }
        Err(e) => {
            println!("  distill rejected: {e:?}");
            return Ok(());
        }
    };

    let shortcut_hash = Hash::for_blob(shortcut.shortcut.bytes());
    println!();
    println!("--- DISCOVERY ---");
    println!("  shortcut nodes       : {} (vs {} for full)", shortcut.shortcut_node_count, shortcut.original_node_count);
    println!("  shortcut hash        : {}", shortcut_hash.as_hex());
    println!("  samples validated    : {}", shortcut.samples_validated);
    println!("  max abs Δ on samples : {:.2e}", shortcut.max_abs_diff_observed);
    println!("  distill time         : {:.2?} (one-shot, paid once at admission)", t_distill);

    // Step 4: bench full vs shortcut.
    let inputs: Vec<Vec<f32>> = (0..N_SAMPLES_BENCH as u64)
        .map(|s| random_positive_input(s.wrapping_mul(0x95DA_CAFD_9876_4321)))
        .collect();

    let t0 = Instant::now();
    for x in &inputs {
        let _ = execute_tensor(&full, &[x.clone()]).unwrap();
    }
    let dt_full = t0.elapsed();

    let t0 = Instant::now();
    for x in &inputs {
        let _ = execute_tensor(&shortcut.shortcut, &[x.clone()]).unwrap();
    }
    let dt_shortcut = t0.elapsed();

    println!();
    println!("--- FORWARD-PASS TIMING ({N_SAMPLES_BENCH} samples) ---");
    println!(
        "  full     : {:>9.2?}  ({:.0} ns/call)",
        dt_full,
        dt_full.as_nanos() as f64 / N_SAMPLES_BENCH as f64
    );
    println!(
        "  shortcut : {:>9.2?}  ({:.0} ns/call)",
        dt_shortcut,
        dt_shortcut.as_nanos() as f64 / N_SAMPLES_BENCH as f64
    );
    let speedup = dt_full.as_secs_f64() / dt_shortcut.as_secs_f64();
    println!("  speedup  : ×{speedup:.2}");
    println!();
    println!("=== conclusion ===");
    println!("Without any human writing the shortcut, the runtime");
    println!("observed activations, recognised that ReLU was acting");
    println!("as identity on the input domain, and synthesised a");
    println!("shorter program that reproduces every output within ε.");
    println!("This is `try_distill_ffn_block` — the seed of an");
    println!("Always-On DreamForge-Tensor daemon that simplifies its");
    println!("own ML graphs by observation.");
    Ok(())
}
