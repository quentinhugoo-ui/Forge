// Forge — universal KASM interpreter on CUDA.
//
// One kernel that interprets ANY KASM bytecode. Compiled ONCE at Forge
// build time via nvcc → kasm.ptx → embedded in Forge.exe via include_str!.
// At runtime Forge only needs nvcuda.dll (the NVIDIA driver runtime),
// which ships with every NVIDIA driver since 2007. NO CUDA Toolkit
// required at runtime.
//
// Targets: compute_50 (sm_5.0+) — covers every NVIDIA GPU shipped since
// 2014. Forward-compatible: PTX is JIT'd to the user's specific GPU
// architecture by the driver at cuModuleLoadData time.
//
// Layout of a KASM node on the GPU (8 bytes, packed identical to the
// CPU `Node` struct's serialization):
//   byte 0    : op   (u8)
//   byte 1    : ty   (u8)
//   bytes 2-3 : a    (u16, little-endian) — index of left input node
//   bytes 4-5 : b    (u16, little-endian) — index of right input node
//   bytes 6-7 : imm  (i16, little-endian) — immediate (slot/const/etc.)
//
// The opcode constants must stay in lockstep with src/kasm/types.rs::Op.

#define OP_INPUT          0
#define OP_CONST_I64      1
#define OP_ADD_I64        2
#define OP_MUL_I64        3
#define OP_EQ_I64         4
#define OP_HASH64         5
#define OP_OUTPUT         6
#define OP_SUB_I64        7
#define OP_DIV_I64_CHK    8
#define OP_MIN_I64        9
#define OP_MAX_I64        10
#define OP_SELECT_I64     11
#define OP_AND_BOOL       12
#define OP_OR_BOOL        13
#define OP_NOT_BOOL       14
#define OP_LT_I64         15
#define OP_LE_I64         16
#define OP_BIT_AND_I64    17
#define OP_BIT_OR_I64     18
#define OP_BIT_XOR_I64    19
#define OP_SHL_I64        20
#define OP_SHR_I64        21
#define OP_SAT_ADD_I64    22
#define OP_SAT_SUB_I64    23
#define OP_MOD_I64_CHK    24
#define OP_BIT_FLIP_I64   28
#define OP_NEG_I64        29
#define OP_REV_BITS_I64   30
#define OP_BYTESWAP_I64   31

// KASM v1.0 mutation — features piquées à JAX/Mojo/Julia/OCaml.
// Le kernel handle les wrappers pass-through (Adaptive/Memoize/Comptime/
// Pipeline) et Cond. Les méta-ops (Grad/Vmap/Pmap/Fori/WhileLoop/Reduce/
// Scan) requièrent du runtime que le scalar kernel n'a pas — la
// pré-validation côté Forge brain doit refuser ces programmes avant
// de les envoyer au GPU. Si on en voit ici, fail-loud via le default.
#define OP_ADAPTIVE       34
#define OP_COMPTIME       35
#define OP_GRAD           36
#define OP_COND           37
#define OP_MEMOIZE        38
#define OP_PIPELINE       39
#define OP_VMAP           40
#define OP_PMAP           41
#define OP_FORI           42
#define OP_WHILE_LOOP     43
#define OP_REDUCE         44
#define OP_SCAN           45
#define OP_VLEN_I64       46
#define OP_VSUM_I64       47
#define OP_VADD_I64       48
#define OP_VMUL_I64       49
#define OP_VSUB_I64       50
#define OP_VMAX_I64       51
#define OP_VMIN_I64       52
#define OP_VRANGE_I64     53
#define OP_VCONCAT_I64    54
#define OP_VREVERSE_I64   55
#define OP_VBROADCAST_I64 56
#define OP_VEQ_I64        57
#define OP_VAND_I64       58
#define OP_VOR_I64        59
#define OP_VXOR_I64       60
#define OP_VABS_I64       61
#define OP_VNEG_I64       62
#define OP_VBITFLIP_I64   63

// Wave 8 self-hosting — Forge-écrite-en-Forge. Op::Fractal/Op::Eval
// requièrent SelfHostingRuntime côté brain (callee_table + execute_with_
// fractal). Le kernel scalar n'a pas accès au Store, ces opcodes tombent
// dans le fail-loud bucket default au runtime.
#define OP_FRACTAL        64
#define OP_EVAL           65
#define OP_LAZY           72
#define OP_FORCE          73

#define KASM_GPU_MAX_NODES 256

extern "C" __global__ void kasm_interpret(
    const unsigned char* program_bytes,
    unsigned int n_nodes,
    const long long* inputs,
    long long* outputs,
    unsigned int n_threads
) {
    unsigned int tid = blockIdx.x * blockDim.x + threadIdx.x;
    if (tid >= n_threads) return;

    // Per-thread value file. All threads in a warp execute the same
    // opcode at the same time (same program, different inputs) → zero
    // branch divergence in the SIMD-typical use case.
    long long values[KASM_GPU_MAX_NODES];
    long long my_input = inputs[tid];
    long long result = 0;

    for (unsigned int i = 0; i < n_nodes; i++) {
        const unsigned char* p = program_bytes + i * 8;
        unsigned char op = p[0];
        unsigned short a = (unsigned short)p[2] | ((unsigned short)p[3] << 8);
        unsigned short b = (unsigned short)p[4] | ((unsigned short)p[5] << 8);
        short imm        = (short)((unsigned short)p[6] | ((unsigned short)p[7] << 8));

        long long v = 0;
        switch (op) {
            case OP_INPUT: {
                v = my_input;
                break;
            }
            case OP_CONST_I64: {
                v = (long long) imm;
                break;
            }
            case OP_ADD_I64: { v = values[a] + values[b]; break; }
            case OP_SUB_I64: { v = values[a] - values[b]; break; }
            case OP_MUL_I64: { v = values[a] * values[b]; break; }
            case OP_EQ_I64:  { v = (values[a] == values[b]) ? 1 : 0; break; }
            case OP_LT_I64:  { v = (values[a] <  values[b]) ? 1 : 0; break; }
            case OP_LE_I64:  { v = (values[a] <= values[b]) ? 1 : 0; break; }
            case OP_AND_BOOL: { v = ((values[a] != 0) && (values[b] != 0)) ? 1 : 0; break; }
            case OP_OR_BOOL:  { v = ((values[a] != 0) || (values[b] != 0)) ? 1 : 0; break; }
            case OP_NOT_BOOL: { v = (values[a] == 0) ? 1 : 0; break; }
            case OP_MIN_I64: { v = values[a] < values[b] ? values[a] : values[b]; break; }
            case OP_MAX_I64: { v = values[a] > values[b] ? values[a] : values[b]; break; }
            case OP_SELECT_I64: {
                v = values[(unsigned short)imm] != 0 ? values[a] : values[b];
                break;
            }
            case OP_BIT_AND_I64: { v = values[a] & values[b]; break; }
            case OP_BIT_OR_I64:  { v = values[a] | values[b]; break; }
            case OP_BIT_XOR_I64: { v = values[a] ^ values[b]; break; }
            case OP_SHL_I64: {
                unsigned long long s = ((unsigned long long) values[b]) & 63ULL;
                v = (long long)(((unsigned long long) values[a]) << s);
                break;
            }
            case OP_SHR_I64: {
                unsigned long long s = ((unsigned long long) values[b]) & 63ULL;
                v = (long long)(((unsigned long long) values[a]) >> s);
                break;
            }
            case OP_SAT_ADD_I64: {
                long long x = values[a], y = values[b];
                long long r = x + y;
                if (((x ^ r) & (y ^ r)) < 0) {
                    r = (x < 0) ? 0x8000000000000000LL : 0x7FFFFFFFFFFFFFFFLL;
                }
                v = r;
                break;
            }
            case OP_SAT_SUB_I64: {
                long long x = values[a], y = values[b];
                long long r = x - y;
                if (((x ^ y) & (x ^ r)) < 0) {
                    r = (x < 0) ? 0x8000000000000000LL : 0x7FFFFFFFFFFFFFFFLL;
                }
                v = r;
                break;
            }
            case OP_DIV_I64_CHK: {
                long long y = values[b];
                if (y == 0) v = 0;
                else if (values[a] == 0x8000000000000000LL && y == -1) v = 0x8000000000000000LL;
                else v = values[a] / y;
                break;
            }
            case OP_MOD_I64_CHK: {
                long long y = values[b];
                if (y == 0) v = 0;
                else if (values[a] == 0x8000000000000000LL && y == -1) v = 0;
                else v = values[a] % y;
                break;
            }
            case OP_BIT_FLIP_I64: { v = ~values[a]; break; }
            case OP_NEG_I64:      { v = -values[a]; break; }
            case OP_REV_BITS_I64: {
                unsigned long long x = (unsigned long long) values[a];
                x = ((x & 0xFFFFFFFF00000000ULL) >> 32) | ((x & 0x00000000FFFFFFFFULL) << 32);
                x = ((x & 0xFFFF0000FFFF0000ULL) >> 16) | ((x & 0x0000FFFF0000FFFFULL) << 16);
                x = ((x & 0xFF00FF00FF00FF00ULL) >> 8)  | ((x & 0x00FF00FF00FF00FFULL) << 8);
                x = ((x & 0xF0F0F0F0F0F0F0F0ULL) >> 4)  | ((x & 0x0F0F0F0F0F0F0F0FULL) << 4);
                x = ((x & 0xCCCCCCCCCCCCCCCCULL) >> 2)  | ((x & 0x3333333333333333ULL) << 2);
                x = ((x & 0xAAAAAAAAAAAAAAAAULL) >> 1)  | ((x & 0x5555555555555555ULL) << 1);
                v = (long long) x;
                break;
            }
            case OP_BYTESWAP_I64: {
                unsigned long long x = (unsigned long long) values[a];
                x = ((x & 0xFFFFFFFF00000000ULL) >> 32) | ((x & 0x00000000FFFFFFFFULL) << 32);
                x = ((x & 0xFFFF0000FFFF0000ULL) >> 16) | ((x & 0x0000FFFF0000FFFFULL) << 16);
                x = ((x & 0xFF00FF00FF00FF00ULL) >> 8)  | ((x & 0x00FF00FF00FF00FFULL) << 8);
                v = (long long) x;
                break;
            }
            case OP_HASH64: {
                // SplitMix64 / Stafford Mix13 — bit-identical to kasm::hash_i64.
                unsigned long long u = (unsigned long long) values[a];
                u += 0x9e3779b97f4a7c15ULL;
                u = (u ^ (u >> 30)) * 0xbf58476d1ce4e5b9ULL;
                u = (u ^ (u >> 27)) * 0x94d049bb133111ebULL;
                v = (long long)(u ^ (u >> 31));
                break;
            }
            case OP_OUTPUT: {
                result = values[a];
                v = values[a];
                break;
            }
            // KASM v1.0 — wrappers pass-through. À l'échelle du scalar
            // kernel, Adaptive/Memoize/Comptime ne font qu'avaler le
            // résultat de leur slot référencé. Le real auto-tuning
            // (Adaptive) et le real load-time eval (Comptime) se font à
            // la frontière du brain ou du build, pas dans le kernel.
            case OP_ADAPTIVE:
            case OP_MEMOIZE:
            case OP_COMPTIME: {
                v = values[a];
                break;
            }
            case OP_COND: {
                // pred=values[a] (Bool 0/1), then=values[b], else=values[imm]
                long long chosen = (values[a] != 0) ? values[b] : values[(unsigned short)imm];
                v = chosen;
                break;
            }
            // Méta-ops : doivent être interceptées avant d'arriver au
            // kernel. Si on les voit ici, c'est un bug de dispatch —
            // fail-loud (output 0 et return).
            //
            // Audit §1.5 (2026-05-01) : OP_PIPELINE déplacé du bucket
            // pass-through vers le bucket fail-loud pour s'aligner sur
            // l'interpreter Rust Wave 6 (commit `79e2647`). La voie
            // canonique est MonsterNode::call_pipeline brain-level,
            // jamais un OP_PIPELINE embedded dans un programme
            // dispatché au kernel.
            case OP_PIPELINE:
            case OP_GRAD:
            case OP_VMAP:
            case OP_PMAP:
            case OP_FORI:
            case OP_WHILE_LOOP:
            case OP_REDUCE:
            case OP_SCAN:
            // Wave 7d — VLenI64 nécessite un vec_pool runtime que le
            // kernel scalar n'a pas (cf. fail-loud bucket avec les
            // autres meta-ops). Brain dispatch handle Vec programs.
            case OP_VLEN_I64:
            // Wave 7d-bis — VSumI64/VAddI64/VMulI64 mêmes raisons.
            case OP_VSUM_I64:
            case OP_VADD_I64:
            case OP_VMUL_I64:
            // Wave 7e — VSubI64/VMaxI64/VMinI64/VRangeI64 idem.
            case OP_VSUB_I64:
            case OP_VMAX_I64:
            case OP_VMIN_I64:
            case OP_VRANGE_I64:
            // Wave 7f — VConcat/VReverse/VBroadcast idem.
            case OP_VCONCAT_I64:
            case OP_VREVERSE_I64:
            case OP_VBROADCAST_I64:
            // Wave 7g — VEq/VAnd/VOr/VXor idem.
            case OP_VEQ_I64:
            case OP_VAND_I64:
            case OP_VOR_I64:
            case OP_VXOR_I64:
            // Wave 7h — VAbs/VNeg/VBitFlip idem.
            case OP_VABS_I64:
            case OP_VNEG_I64:
            case OP_VBITFLIP_I64:
            // Wave 8 — Fractal/Eval requièrent SelfHostingRuntime (callee
            // table + Store) que le scalar kernel n'a pas. Brain dispatch
            // intercepte avant atteindre GPU.
            case OP_FRACTAL:
            case OP_EVAL:
            // Lazy/Force need the CPU-side atlas/future table. A Force that
            // hits atlas should be resolved before CUDA launch; unresolved
            // embedded futures stay fail-loud in this scalar kernel.
            case OP_LAZY:
            case OP_FORCE:
            default: {
                // Op inconnu ou méta-op qui aurait dû être interceptée
                // par try_eval_cuda_min. Fail-loud avec sentinel 0.
                outputs[tid] = 0;
                return;
            }
        }
        values[i] = v;
    }
    outputs[tid] = result;
}

// ─── Synth scoring kernel ────────────────────────────────────────────────────
//
// Each thread scores ONE explicit job across ALL examples.
// jobs[job_idx] = packed u32: (pair_idx << 8) | op_idx
//
// Input buffers:
//   candidates[n_candidates * n_examples] — flat i64 outputs per candidate
//   targets[n_examples] — target values
//   pairs[n_pairs] — packed u32: (left_idx:u16 | right_idx:u16 << 16)
//
// Output buffer:
//   jobs[total_jobs] — packed u32: (pair_idx << 8) | op_idx
//   results[total_jobs * 3] — per job: (loss_lo:u64, loss_hi:u64, fingerprint:u64)

extern "C" __global__ void synth_score(
    const long long* __restrict__ candidates,
    const long long* __restrict__ targets,
    const unsigned int* __restrict__ pairs,
    const unsigned int* __restrict__ jobs,
    unsigned long long* __restrict__ results,
    unsigned int n_examples,
    unsigned int n_pairs,
    unsigned int n_jobs
) {
    unsigned int job_idx = blockIdx.x * blockDim.x + threadIdx.x;
    unsigned int total_jobs = n_jobs;
    if (job_idx >= total_jobs) return;

    unsigned int job = jobs[job_idx];
    unsigned int pair_idx = job >> 8u;
    unsigned int op = job & 0xFFu;
    if (pair_idx >= n_pairs) return;

    unsigned int packed = pairs[pair_idx];
    unsigned int left_idx = packed & 0xFFFFu;
    unsigned int right_idx = (packed >> 16u) & 0xFFFFu;

    unsigned int left_base = left_idx * n_examples;
    unsigned int right_base = right_idx * n_examples;

    unsigned long long loss_lo = 0;
    unsigned long long loss_hi = 0;
    unsigned long long fp = 0xcbf29ce484222325ULL;

    for (unsigned int i = 0; i < n_examples; i++) {
        long long a = candidates[left_base + i];
        long long b = candidates[right_base + i];

        long long out;
        switch (op) {
            case 0: out = a + b; break;
            case 1: out = a - b; break;
            case 2: out = a * b; break;
            case 3: out = a ^ b; break;
            case 4: out = a & b; break;
            case 5: out = a | b; break;
            case 6: out = (a > b) ? 1LL : 0LL; break;
            case 7: out = (a < b) ? 1LL : 0LL; break;
            default: out = (a != 0) ? b : 0LL; break;
        }

        // FNV-1a
        fp ^= (unsigned long long)out;
        fp *= 0x100000001b3ULL;

        // |out - target| → 128-bit accumulation
        long long t = targets[i];
        long long diff = out - t;
        unsigned long long abs_diff = (diff < 0) ? (unsigned long long)(-diff) : (unsigned long long)diff;
        unsigned long long new_lo = loss_lo + abs_diff;
        if (new_lo < loss_lo) loss_hi++;
        loss_lo = new_lo;
    }

    unsigned int out_base = job_idx * 3u;
    results[out_base]     = loss_lo;
    results[out_base + 1] = loss_hi;
    results[out_base + 2] = fp;
}

static __device__ __forceinline__ long long synth_apply_op(long long a, long long b, unsigned int op) {
    switch (op) {
        case 0: return a + b;
        case 1: return a - b;
        case 2: return a * b;
        case 3: return a ^ b;
        case 4: return a & b;
        case 5: return a | b;
        case 6: return (a > b) ? 1LL : 0LL;
        case 7: return (a < b) ? 1LL : 0LL;
        default: return (a != 0) ? b : 0LL;
    }
}

static __device__ __forceinline__ void synth_accumulate(
    long long out,
    long long target,
    unsigned long long* loss_lo,
    unsigned long long* loss_hi,
    unsigned long long* fp
) {
    *fp ^= (unsigned long long)out;
    *fp *= 0x100000001b3ULL;

    long long diff = out - target;
    unsigned long long abs_diff = (diff < 0) ? (unsigned long long)(-diff) : (unsigned long long)diff;
    unsigned long long new_lo = *loss_lo + abs_diff;
    if (new_lo < *loss_lo) (*loss_hi)++;
    *loss_lo = new_lo;
}

// Dense pair-fused variant.
//
// One thread scores all 9 opcodes for one pair in a single pass over examples.
// Compared with synth_score, this avoids loading the same left/right/target
// rows 9 times for dense pair x opcode batches, while preserving the exact
// output layout: pair0 op0..8, pair1 op0..8, ...
extern "C" __global__ void synth_score_pairs(
    const long long* __restrict__ candidates,
    const long long* __restrict__ targets,
    const unsigned int* __restrict__ pairs,
    unsigned long long* __restrict__ results,
    unsigned int n_examples,
    unsigned int n_pairs,
    unsigned int n_ops
) {
    unsigned int pair_idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (pair_idx >= n_pairs) return;

    unsigned int packed = pairs[pair_idx];
    unsigned int left_idx = packed & 0xFFFFu;
    unsigned int right_idx = (packed >> 16u) & 0xFFFFu;

    unsigned int left_base = left_idx * n_examples;
    unsigned int right_base = right_idx * n_examples;

    unsigned long long loss_lo[9];
    unsigned long long loss_hi[9];
    unsigned long long fp[9];
    for (unsigned int op = 0; op < 9; op++) {
        loss_lo[op] = 0ULL;
        loss_hi[op] = 0ULL;
        fp[op] = 0xcbf29ce484222325ULL;
    }

    for (unsigned int i = 0; i < n_examples; i++) {
        long long a = candidates[left_base + i];
        long long b = candidates[right_base + i];
        long long t = targets[i];

        for (unsigned int op = 0; op < 9; op++) {
            long long out = synth_apply_op(a, b, op);
            synth_accumulate(out, t, &loss_lo[op], &loss_hi[op], &fp[op]);
        }
    }

    unsigned int out_base = pair_idx * n_ops * 3u;
    for (unsigned int op = 0; op < 9; op++) {
        unsigned int dst = out_base + op * 3u;
        results[dst]     = loss_lo[op];
        results[dst + 1] = loss_hi[op];
        results[dst + 2] = fp[op];
    }
}

extern "C" __global__ void synth_score_pairs_vec2(
    const long long* __restrict__ candidates,
    const long long* __restrict__ targets,
    const unsigned int* __restrict__ pairs,
    unsigned long long* __restrict__ results,
    unsigned int n_examples,
    unsigned int n_pairs,
    unsigned int n_ops
) {
    unsigned int pair_idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (pair_idx >= n_pairs) return;

    unsigned int packed = pairs[pair_idx];
    unsigned int left_idx = packed & 0xFFFFu;
    unsigned int right_idx = (packed >> 16u) & 0xFFFFu;

    unsigned int left_base = left_idx * n_examples;
    unsigned int right_base = right_idx * n_examples;
    unsigned int pairs_of_examples = n_examples >> 1u;

    const longlong2* __restrict__ left2 =
        reinterpret_cast<const longlong2*>(candidates + left_base);
    const longlong2* __restrict__ right2 =
        reinterpret_cast<const longlong2*>(candidates + right_base);
    const longlong2* __restrict__ target2 =
        reinterpret_cast<const longlong2*>(targets);

    unsigned long long loss_lo[9];
    unsigned long long loss_hi[9];
    unsigned long long fp[9];
    #pragma unroll
    for (unsigned int op = 0; op < 9; op++) {
        loss_lo[op] = 0ULL;
        loss_hi[op] = 0ULL;
        fp[op] = 0xcbf29ce484222325ULL;
    }

    for (unsigned int i = 0; i < pairs_of_examples; i++) {
        longlong2 a = left2[i];
        longlong2 b = right2[i];
        longlong2 t = target2[i];

        #pragma unroll
        for (unsigned int op = 0; op < 9; op++) {
            long long out0 = synth_apply_op(a.x, b.x, op);
            synth_accumulate(out0, t.x, &loss_lo[op], &loss_hi[op], &fp[op]);
            long long out1 = synth_apply_op(a.y, b.y, op);
            synth_accumulate(out1, t.y, &loss_lo[op], &loss_hi[op], &fp[op]);
        }
    }

    unsigned int out_base = pair_idx * n_ops * 3u;
    #pragma unroll
    for (unsigned int op = 0; op < 9; op++) {
        unsigned int dst = out_base + op * 3u;
        results[dst]     = loss_lo[op];
        results[dst + 1] = loss_hi[op];
        results[dst + 2] = fp[op];
    }
}
