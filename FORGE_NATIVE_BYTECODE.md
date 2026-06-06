# Forge Language And Bytecode
src/kasm.rs

Forge is the content-addressed compute language of InGen. The current Rust
implementation still uses the old KASM name in many modules. Forge source is
the internal readable form produced by trusted compilers or expert tools; Forge
bytecode is the compact verified form Monster executes. External LLMs should
normally fill structured MathContracts through `/newcompute_`, then let InGen
compile them deterministically to Forge.

```text
InGen intent
-> Forge source
-> Forge bytecode
-> verifier
-> sealed capabilities
-> Monster interpreter, delta path or GPU backend
-> proof ledger
-> bounded projection
```

## Rules

- No guest OS, syscalls, raw filesystem/network/secrets, native pointers,
  unbounded loops or undeclared hostcalls.
- Bytecode programs receive sealed capabilities, not authority.
- Every accepted or denied run emits the same proof envelope shape.
- Raw data stays in InGen stores; callers receive hashes, refs, previews and
  proof summaries.

## Agent Quick Start

If you are a new agent working on `/newcompute_`, read this first.

The target circuit is:

```text
human request
-> LLM selects one math class
-> LLM fills a structured MathContract in classical math terms
-> InGen compiles MathContract to Forge deterministically
-> Forge verifies types, units, bounds, purity, samples and cost
-> Monster executes/reuses/proofs the generated Forge compute
```

Do not ask an external LLM to write raw Forge for professional math. Forge is
the internal verified language. Do not create a second compute path, a parallel
solver executor, or class-specific Monster engines. The math classes are
specialized contract views over the same `/newcompute_` path.

Math classes:

| Class | Use When The Human Asks For | Contract Must Contain | Forge/Monster Target |
| --- | --- | --- | --- |
| `/formula_symbolic` | identities, derivations, simplification, exact proof, symbolic solve | symbols, expressions, assumptions, domains, requested transforms, proof checks | symbolic Forge types/ops, symbolic math plan |
| `/numeric_model` | engineering formulas, deterministic calculations, dimensioned models | variables, units, bounds, equations, constraints, samples, outputs | scalar/vector Forge math, unit/bounds checks, scalar oracle |
| `/simulation_dynamics` | ODE/DAE/PDE, time stepping, physical dynamics | states, time domain, equations, initial/boundary conditions, events, integrator request | solver primitives when supported, otherwise `capability_missing` |
| `/optimization_design` | design search, calibration, constraints, Pareto studies | objective, design variables, constraints, algorithm family, stopping criteria | optimization/ranking Forge ops and validation |
| `/uncertainty_statistics` | Monte Carlo, QMC, confidence, robustness, sensitivity | distributions, samples, estimators, correlation assumptions, tolerances | sampling/statistics/uncertainty Forge ops |
| `/tensor_linalg_autodiff` | matrix/tensor compute, gradients, Jacobians, Hessians | shapes, batch axes, matrix ops, AD requests, precision/layout policy | tensor/linalg/autodiff Forge ops |
| `/signal_timeseries` | FFT, filters, vibration, rolling windows, market/sensor series | sample rate, channels, windows, transforms, filters, feature outputs | signal/window/time-series Forge ops |
| `/graph_sparse_discrete` | sparse systems, graph traversal, network topology | nodes, edges, sparse matrices, graph ops, topology checks | graph/sparse Forge ops |

Rejection is correct behavior when the contract is not exact. Return a compact
repair reason, never guess:

```text
missing_unit | missing_bounds | unknown_symbol | ambiguous_equation
unsupported_operation | capability_missing | invalid_shape | invalid_domain
```

Example: if the human says "build a rocket engine", the LLM should not see the
whole Forge language. It should choose a primary class such as
`/numeric_model` or `/optimization_design`, fill a propulsion MathContract with
pressure, temperature, throat area, expansion ratio, thrust/Isp equations,
constraints, samples and sensitivity slots, then let InGen compile that
contract into Forge. If combustion equilibrium or a PDE solver is not yet
native, the compiler must return `capability_missing` or require a reduced
model; it must not invent a hidden fallback.

Every implementation round that changes this path must update this document
and run a real store-backed `/newcompute_` app-runtime battery through
`forge_brain_run_actcode`, Monster preparation, typed buffers, proof/differential
status and compute-library reuse.

## Proof Envelope

```text
sourceHash
bytecodeHash
verifierHash
inputHash
outputHash
capabilityHash
hostcallHash
fuelUsed
memoryPeak
backend
deterministicReplayHash
proofHash
```

Verifier refusals are also runs: `fuelUsed=0`, `memoryPeak=0`, denied output
payload, stable denial `proofHash`.

## Current Surface

- `src/kasm.rs`: single source of truth for Forge source, Forge bytecode,
  tensor runtime, FBC v0 and the embedded dialect reference. The old KASM name
  remains in code during transition.
- `src/monster.rs`: Monster execution, caches, synthesis and proof paths.
- native service adapters: shared host runtime used by the InGen app and
  kernel projection routes.
- native Brain runtime adapters: pointers for `/newcompute_`, `/selectcompute_`
  and Banger-only `/newobject_`.

## Canonical Runtime Architecture

This file is the single source of truth for Forge language, Forge bytecode,
Monster execution and the InGen runtime route.

InGen should behave like one short circuit:

```text
user intent
-> LLM CLI inside InGen
-> BrainCommand or CodeAct program
-> Godel/policy gate
-> local route or Forge compute
-> Monster execution/reuse/proof when compute is needed
-> compact hashes/proofs/artifacts
-> native UI section bridge
-> optional memory commit
```

Do not add a second path for memory, browser actions, trading, Banger, file
analysis or compute if this route can carry it.

### Agent Action Surfaces

There are two action surfaces, and they must stay distinct:

| Surface | What the LLM writes | Executor | Role |
| --- | --- | --- | --- |
| BrainCommand | `/forge` programs such as `recall`, `plan`, `create`, `run`, `project`, `replay`, `commit`, `explain` | `forge_agent` / `forge_agent_runtime.rs` | general agent OS route |
| CodeAct compute/UI | `/newcompute_` class selectors and MathContracts, `/selectcompute_`, `/newobject_`, plus UI JSON blocks | `src/monster.rs` core contracts plus thin native Slint adapters | compute or UI actions |

Family A is compute. `/newcompute_`, `/selectcompute_` and dynamic
`/compute_<name>_` work in all sections and call Monster. `/newobject_` is
Banger-only and turns curated compute evidence into SDF/3D object structure.

Family B is UI-only. `/web_`, `FORGE_PLAN_JSON`,
`FORGE_QUESTIONNAIRE_JSON`, `FORGE_SESSION_TITLE_JSON` and Banger material JSON
render panels/events and must not enter the Monster compute executor.

Family C is gated Web CodeAct. `/navigateweb_` and future web action programs
are the final step of the WebExplorer RAM DOM atlas, not the first. They act on
the native WebExplorer atlas, not on raw visual selectors or prose. Before exposing
Family C to the LLM, WebExplorer must tag every captured page element with a
stable `webref` backed by DOM, accessibility, layout and visual evidence, and
`collection_os_webexplorer_atlas_report` must show acceptable coverage with
known blind spots:

```text
webref=<tree_hash>/<frame_path>/<backend_dom_node_id_or_node_hash>/<role>/<label_hash>
```

The LLM receives a pruned command map, for example buttons, links, search boxes,
forms, media controls, images, videos, canvas regions, menus, dialogs,
scrollable regions and ambiguous visual targets. It emits CodeAct with slots
that reference those tags:

```text
/navigateweb_
tree_hash=<current atlas tree hash>
goal=<short navigation goal>
action=click|type|select|scroll|focus|copy_text|capture_region|download_resource
target_ref=<webref from the command map>
target_kind=button|link|searchbox|textbox|image|video|canvas|menu|dialog|region
input_text=<optional text, only for text-entry actions>
expected_state=<url_change|tree_change|text_visible|download|no_navigation>
confirmation=required|not_required
```

The executor resolves `target_ref` against the latest atlas, verifies that the
tree hash or self-healing match is still valid, then performs the smallest
native action. Self-healing may use backend DOM id, stable node hash, AX
role/name, visible text, bounds and resource hash, but never lets the LLM run
arbitrary page JavaScript. Sensitive actions such as login, purchase, payment,
booking, upload, submit or destructive form changes require explicit human
confirmation before execution. Every executed action appends an action ledger
entry with the previous tree hash, target ref, result tree hash, URL delta and
proof hash.

### NewCompute MathContract Front Door

Target architecture: `/newcompute_` stays the single universal Monster compute
entrance, but external LLMs should not be required to author raw Forge source.
Forge is a private, full compute language that most outside models were not
trained on. The LLM should express the mathematics in a structured classical
math contract; InGen then compiles that contract deterministically into Forge.

```text
human asks for a technical result
-> LLM chooses a math class through /newcompute_
-> LLM fills the class-specific classical MathContract
-> InGen validates and compiles MathContract to Forge source
-> Forge parses/types/units/bounds/checks the generated source
-> Monster executes/reuses/proofs the Forge compute
-> LLM classifies the compact result for the caller section
```

This is not a second compute pipeline. Class commands are contract views over
the same `/newcompute_` path. Internally, `/numeric_model` means
`/newcompute_ math_class=numeric_model`; it does not bypass Forge, Monster,
proof hashes, typed buffers or the compute library.

Non-negotiable rule: no prose-to-Forge interpretation. If a MathContract cannot
compile exactly, the run is rejected before Monster execution with a mechanical
reason such as `missing_unit`, `unknown_symbol`, `ambiguous_equation`,
`unsupported_operation` or `capability_missing`.

The first `/newcompute_` response should be compact and cheap for the LLM to
read:

```text
/newcompute_
choose_math_class:
  /formula_symbolic
  /numeric_model
  /simulation_dynamics
  /optimization_design
  /uncertainty_statistics
  /tensor_linalg_autodiff
  /signal_timeseries
  /graph_sparse_discrete
```

After the LLM chooses a class, InGen returns only that class template, with its
full slots, allowed classical math vocabulary, Forge compile targets, Monster
support status and known limits.

Required implementation stages:

1. Done 2026-06-06: Monster core owns the compact `/newcompute_` math-class
   selector through `MonsterNode::math_capability_manifest`. It lists the
   eight class commands, required slots, accepted operators, compile status and
   a stable manifest hash. `ingen_native_services::math_compute_service`
   exposes this manifest through a thin native adapter instead of recreating a
   template registry.
2. Done 2026-06-06: Classical MathContract IR lives in `src/monster.rs` as
   typed Rust structs: `MonsterMathContract`, `MonsterMathVariable`,
   `MonsterMathConstant`, `MonsterMathOutputContract` and `MonsterMathSample`.
   A contract has a stable `contract_hash`; incomplete contracts return exact
   missing slots before Forge generation.
3. Done 2026-06-06: `MonsterNode::compile_math_contract`,
   `prepare_math_contract` and `execute_math_contract` compile class contracts
   deterministically into real Forge source, then reuse the normal
   `prepare_forge_source` and `execute_prepared_compute` path. This is not a
   second compute pipeline.
4. Done 2026-06-06: The first deterministic compiler slice supports scalar
   `numeric_model` and scalar executable contract slices for
   `formula_symbolic`, `optimization_design`, `uncertainty_statistics`,
   `tensor_linalg_autodiff`, `signal_timeseries` and `graph_sparse_discrete`.
   It emits `forge_inputs`, `forge_constants`, `forge_functions`,
   `forge_program`, `forge_outputs`, `forge_constraints`, `forge_samples`,
   `forge_validation`, `forge_cost` and `artifact_handoff`.
5. Done 2026-06-06: Monster rejects incomplete or unsupported class contracts
   mechanically through `missing_slots`, `unsupported_operator`,
   `ambiguous_expression`, `invalid_generated_forge` or `capability_missing`.
   Raw Rust internals stay hidden from the LLM.
6. Done 2026-06-06: The scalar oracle now covers the new scalar replay words
   used by compiled class contracts, including `optimize`,
   `constraint_solve`, `least_squares`, `uncertainty`, `p5`, `p50`, `p95`,
   `mean`, `variance`, `std`, `median`, `quantile`, `grad`, `hessian`,
   `jvp`, `vjp`, `adjoint`, `rank`, `pareto`, `fft`, `rfft`, `ifft`,
   `rolling`, `window_hann`, `window_blackman`, `convolution`,
   `csr_matvec`, `pagerank`, `shortest_path`, `connected_components`,
   `graph_degree` and `diff` when they appear in scalar smoke contracts.
7. In progress 2026-06-06: `/simulation_dynamics` contract exposes slots for
   state variables, time domain, ODE/DAE/PDE form, events,
   boundary/initial conditions, integrator request, stability policy and
   residual checks. Until native solver lowerings land, complete contracts
   normalize to IR and return `capability_missing`, not fake execution.
8. In progress 2026-06-06: Promote domain-native lowerings beyond scalar
   smoke contracts: symbolic expression types for true CAS `diff/solve`,
   vector/array signal contracts for `fft/rfft/rolling`, sparse graph typed
   outputs, tensor shape-specialized kernels, real optimization feasibility
   gates and robust Monte Carlo/QMC statistics.
9. Real app-runtime batteries per class. Each class still needs at least 10
   store-backed `/newcompute_` tests through the native app UI command layer,
   Monster preparation, typed buffers, scalar/artifact oracle status and
   compute library reuse. Current gates are `cargo test --lib math_contract`
   for Monster core and `cargo test --manifest-path
   examples\ingen_native_services\Cargo.toml math_compute` for the native
   service adapter.
10. Documentation and dictionary sync. After each class lands, this document
   must list the class purpose, contract slots, allowed math words, Forge
   compile target, Monster support level, examples, rejection reasons and test
   gate name.

### MathContract Class Sync

| Class | Current Slice | Required Contract Slots | Allowed Words | Forge Target | Monster Support | Example | Rejections | Test Gate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `/formula_symbolic` | scalar executable contract plus symbolic vocabulary manifest | `goal`, `symbols`, `expressions`, `assumptions`, `domains`, `requested_transforms`, `proof_checks`, `outputs` | `expand`, `canonicalize_expr`, `simplify`, `diff`, `solve`, `math_equiv`, `math_proof` | generated scalar Forge module today; true symbolic typed lowering next | partial | scalar replay for `(x*x)` under formula contract | `missing_slots`, `unsupported_operator`, `ambiguous_expression`, `invalid_generated_forge` | `monster_operator_math_contract_classes_compile_to_forge_and_execute` |
| `/numeric_model` | deterministic scalar formula lowering | `goal`, `variables`, `constants`, `units`, `bounds`, `equations`, `constraints`, `samples`, `validation`, `outputs` | scalar arithmetic, declared equations, generated `finite` constraints | `forge_inputs`, constants, program, outputs, constraints, samples, validation, cost | supported for scalar mass-math slice | rocket thrust `mass_flow * exhaust_velocity + pressure_delta` | `missing_slots`, `unsupported_operator`, `ambiguous_expression`, `invalid_generated_forge` | `monster_numeric_math_contract_compiles_prepares_and_executes` |
| `/optimization_design` | scalar operator contract lowering | `goal`, `objective`, `design_variables`, `constraints`, `algorithm_family`, `stopping_criteria`, `validation`, `outputs` | `optimize`, `constraint_solve`, `least_squares`, `rank`, `pareto`, `grad`, `hessian` | generated Forge scalar program plus validation/cost | partial | `optimize(x*x, x)` replay | `unsupported_operator`, `capability_missing` | `monster_operator_math_contract_classes_compile_to_forge_and_execute` |
| `/uncertainty_statistics` | scalar estimator contract lowering | `goal`, `distributions`, `samples`, `estimators`, `correlation_assumptions`, `tolerances`, `validation`, `outputs` | `sample`, `sobol`, `mean`, `variance`, `std`, `stddev`, `quantile`, `p5`, `p50`, `p95` | generated Forge scalar program plus validation/cost | partial | `p95(uncertainty(x,x,x))` replay | `unsupported_operator`, `capability_missing` | `monster_operator_math_contract_classes_compile_to_forge_and_execute` |
| `/tensor_linalg_autodiff` | scalar AD/tensor vocabulary contract lowering | `goal`, `shapes`, `batch_axes`, `matrix_ops`, `ad_requests`, `precision_policy`, `layout_policy`, `validation`, `outputs` | `matmul`, `dot`, `transpose`, `sum`, `top_k`, `grad`, `jacobian`, `hessian`, `jvp`, `vjp`, `adjoint` | generated Forge scalar program plus validation/cost today; typed tensor kernels next | partial | `grad(x*x)` replay | `unsupported_operator`, `capability_missing` | `monster_operator_math_contract_classes_compile_to_forge_and_execute` |
| `/signal_timeseries` | scalar-safe signal contract front door | `goal`, `sample_rate`, `channels`, `windows`, `transforms`, `filters`, `stationarity_assumptions`, `validation`, `outputs` | `fft`, `rfft`, `ifft`, `convolution`, `fir_filter`, `iir_filter`, `window_hann`, `window_blackman`, `rolling`, `asof_join` | generated scalar Forge program today; array/timeseries lowering next | partial | scalar signal contract replay | `unsupported_operator`, `capability_missing` | `monster_operator_math_contract_classes_compile_to_forge_and_execute` |
| `/graph_sparse_discrete` | scalar graph/sparse vocabulary contract lowering | `goal`, `nodes`, `edges`, `sparse_matrices`, `graph_ops`, `topology_checks`, `solver_requests`, `validation`, `outputs` | `csr_matvec`, `frontier`, `pagerank`, `shortest_path`, `connected_components`, `degree`, `graph_degree`, `top_k`, `constraint_solve` | generated scalar Forge program today; sparse graph typed output next | partial | `constraint_solve(x,x)` replay | `unsupported_operator`, `capability_missing` | `monster_operator_math_contract_classes_compile_to_forge_and_execute` |
| `/simulation_dynamics` | contract manifest only, solver lowering gated | `goal`, `states`, `time_domain`, `equations`, `initial_conditions`, `boundary_conditions`, `events`, `integrator_request`, `residual_checks` | none promoted yet | none until native solver slice lands | gated, compile returns capability missing | oscillator ODE contract | `capability_missing` | `monster_simulation_math_contract_returns_capability_missing_until_solver_lands` |

Compiled class contracts that reach Forge share the core gate
`cargo test --lib math_contract`. The native service adapter gate is
`cargo test --manifest-path examples\ingen_native_services\Cargo.toml
math_compute`; the Slint UI command layer still needs its full app-runtime
battery on top.

The LLM should never see hundreds of permanent tools. It sees stable entry
points, then InGen retrieves a few ranked compute/program/template candidates
from indexes when needed.

### State, Brain And Reuse

There is one state kernel and one Brain. Raw heavy data stays in stores; the LLM
gets compact refs, hashes, previews and proof summaries.

```text
State objects -> Brain meaning -> Godel verification -> persisted projection evidence
```

Monster compute reuse is SQLite-backed:

```text
brain/computes/compute_library.sqlite
-> computes
-> compute_runs
-> compute_fragments
```

Reuse is exact only: scale, fragment hash, contract hash, type hash, unit hash,
result hash and proof hash must agree. Probabilistic filters may accelerate
"not present" checks later, but never authorize reuse.

## Live Objectives

This is the list a new agent should follow. Do not keep a second private roadmap.

### Language Objectives

Done:

- Core Forge source parses/types/checks modules with functions, inputs,
  constants, outputs, constraints, samples, cost and artifact handoff.
- Core scalar/vector/matrix/array/tensor/table/graph/field types exist.
- Units, bounds, assertions, hashes, samples and bounded loops exist at source
  level.
- The broad universal primitive vocabulary is accepted by the language and
  classified into IR: scalar, bool, transcendental, bit/integer, vector/matrix,
  array/tensor, data-parallel, gather/scatter, sampling, statistics,
  optimization, autodiff, solvers, signal/FFT, sparse/graph, geometry/SDF,
  physics, contracts, memory/DOM/RAM and crypto/hash.
- Signal tranche 1: `fft`, `rfft` and `ifft` now have real shape semantics for
  static power-of-two signals. Forge represents complex spectra as
  `tensor<elem,Nx2>` rows `(re, im)`; `rfft(array<elem,N>)` returns
  `tensor<elem,(N/2+1)x2>`.
- Language enrichment tranche 17-21: Forge now parses/types/hashes
  `interval<T>`, `uncertainty<T>`, `p5`, `p50`, `p95`, canonical AD names
  `grad`, `jacobian`, `hessian`, `adjoint`, optimization names `optimize`,
  `constraint_solve`, `least_squares`, optional `forge_transforms` declarations
  for `jit`, `batch`, `vectorize`, `parallel` and AD/optimization transforms,
  plus optional `forge_schedule` declarations carrying `algorithm`, `tile`,
  `vectorize`, `gpu` and `layout`. These contracts lower into Forge IR metadata
  and Monster plans through `/newcompute_`.
- Runtime/layout tranche 22-27: Forge now parses/types/hashes
  `sparse_field<T,R,sparse_grid|page_table|hash_grid>`, optional
  `forge_runtime` declarations for `wgsl_rhi`, `cpu_simd`, optional/required
  CUDA, memory layout and sparse layout policy, plus optional
  `forge_hostcalls` declarations that accept only sealed non-raw hostcalls.
  These runtime and hostcall contracts lower into Forge IR metadata and Monster
  `/newcompute_` plans.
- Memory/DOM/RAM tranche 28-31: Forge now parses/types/hashes `snapshot`,
  `memory_map`, `heap_object`, `dom_node`, `dom_edge` and
  `taint<public|user_data|credential|secret>`, with typed graph/table queries
  `refs`, `retainers`, `leaks` and `mutation_diff`. Any module touching
  RAM/DOM capture/query contracts must declare a sealed InGen
  `read_capability` or `artifact_read_hash` hostcall; raw RAM/DOM reads remain
  rejected.
- Domain dialect tranche 32-43: Forge now parses/types/hashes Trading
  records `tick`, `bar`, `quote`, `trade`, `orderbook`, `position`, `pnl`;
  Bio/DNA records `dna`, `rna`, `protein`, `gene`, `variant`, `feature`,
  `alignment`; and Chemistry records `atom`, `bond`, `molecule_graph`,
  `reaction`, `conformer`. The source dialect typechecks point-in-time
  trading metrics/backtests, anti-lookahead, walk-forward/stress/cost
  contracts, k-mer/transcription/translation/alignment/annotation contracts
  and SMILES/SMARTS/fingerprint/substructure/valence/charge/aromaticity
  contracts.
- Crypto/code-agent tranche 44-50: Forge now parses/types/hashes
  `bitvec<N>`, `field<p>`, `curve`, `hash`, `merkle`, `signature`, plus
  `ast`, `symbol`, `cfg`, `callgraph`, `diff`, `patch` and `testcase`. The
  source dialect typechecks constant-time and secret-branch proof gates,
  ZK/SMT/Lean hooks, typed parse/typecheck/transform/patch/test/trace ops and
  a proof envelope for code modifications. These are sealed proof/artifact
  contracts, not arbitrary code execution.
- Symbolic math tranche 70: Forge now parses/types/hashes Wolfram-like
  source-level math contracts without creating a second compute pipeline:
  `expr<T>`, `polynomial<T>`, `piecewise<T>`, `assumption_set`,
  `math_domain` and `solution_set`, plus `symbol`, `domain`, `assume`,
  `to_expr`, `simplify`, `full_simplify`, `canonicalize_expr`, `expand`,
  `factor`, `collect`, `cancel`, `together`, `apart`, `function_expand`,
  `trig_reduce`, `series`, `refine`, `diff`, `integrate`, `limit`,
  `residue`, `solve`, `reduce_equations`, `find_instance`, `math_equiv`,
  `math_proof` and `expression_hash`. These lower into Forge IR as
  `symbolic_math` kernel hints and Monster `/newcompute_` plans. Monster now
  executes the first exact local CAS slice inside that same plan:
  `cpu_symbolic_exact_linear_polynomial_canonical_v1` canonicalizes exact
  polynomial expressions, expands bounded integer powers such as `(x+2)^2`,
  preserves neutral rewrites such as `x + 0 -> x`, computes polynomial
  derivatives such as `diff((x+2)^2,x) -> 2*x+4`, solves identity/constant/
  variable equations and emits typed canonical/proof buffers.
- Banger render tranche 51-60: Forge now parses/types/hashes `sdf`,
  `neural_sdf`, `voxel_page`, `surfel`, `micromesh`, `material_graph`,
  `meshlet_cluster`, `geometry_page`, `lod_node`, `radiance_probe`,
  `radiance_cache`, `shadow_page`, `pcg_graph`, `spatial_cell` and
  `light_budget`. The source dialect now typechecks SDF blending/gradient/
  normal/curvature, Banger virtual geometry pages, radiance caches, virtual
  shadow pages, physical material payloads, hashed PCG graphs, spatial
  streaming cells, light budgets and mesh/SDF/shader/proof/preview exports.
- Complex-primitive bounds tranche 69: the same six primitive families now
  carry fine amplitude-amplification bounds in the bounds-inference pipeline,
  not the [-1e15, 1e15] catch-all that used to blow `mean(...)` through any
  reasonable sample tolerance. The new arms in `infer_builtin_bounds`:
  - `convolution(signal, kernel)` / `fir_filter(signal, taps)` use the same
    finite-support Young's-inequality bound as `signal_transform_bounds`:
    `K · max|signal| · max|kernel|` with `K = 4096` (matches the FFT cap).
  - `iir_filter(signal, b, a)` adds an 8× pole budget on top of the same
    formula (`K · 8 · max|signal| · max|b|`).
  - `window_hann(signal)` / `window_blackman(signal)` pass the signal carrier
    through (window samples live in [-1, 1] so Young gives identity).
  - `spectrogram(signal, nperseg)` uses the DFT triangular bound
    `nperseg · max|signal|` when the `nperseg` literal is known (else the
    signal cap), so `spectrogram([-1,1], 16) ∈ [-16, 16]` exactly.
  - `wavelet_step(signal)` uses a 2× cover of the Haar `√2` bound.
  - `mean`, `median`, `quantile`, `minmax`, AD passthroughs (`grad`, `jvp`,
    `vjp`, `jacobian`, `hessian`, `hessian_diag`, `adjoint`,
    `sensitivity_forward/adjoint`) now propagate the carrier's bound exactly
    instead of widening to 1e15. `sum` and `sparse_reduce` use `bounded_sum`
    (the loop-cap × carrier range), `variance` / `std` use the
    Bhatia-Davis-style range bound `(b-a)²/4` and `(b-a)/2`.
  - Graph traversal carries domain-specific bounds:
    `bfs_step / connected_components_step ∈ [0, 1e12]` (node-id range),
    `shortest_path_step ∈ [0, 1e15]`, `pagerank_step ∈ [0, 1]` (normalised
    rank vector).
  - Unit propagation: `convolution`, `fir_filter`, `iir_filter`, `spectrogram`
    now propagate the signal's unit dim through their 2- or 3-arg shapes
    (kernel/taps/nperseg treated as dimensionless coefficients/indices). The
    pre-existing arm only matched the 1-arg variant, which prevented these
    primitives from ever being used in a real `forge_program` module before
    tranche 69. `csr_matvec` / `sparse_solve` carry the RHS unit through.

  Together with tranche 63-68 (typed shape), a Forge module can now write
  `emit out: f64 = mean(spectrogram(signal, 16u32))` with `signal ∈ [-1, 1]`
  and the bounds resolve to a tight `[-16, 16]` instead of the old
  `[-1e15, 1e15]`. The Monster `/newcompute_` plan still routes ops by name
  to the specialised signal/AD/graph/sparse shader profiles; the tighter
  bounds just unblock the constraint/sample-tolerance check.
- Complex-primitive shape tranche 63-68: six primitive families now carry real
  typed shape semantics in `ForgeExpr::infer_ty` instead of only-acceptance
  passthrough.
  - Signal `convolution(signal, kernel)` returns the signal carrier (SciPy /
    NumPy "same"-mode), kernel must be a float collection no longer than the
    signal; non-float signal/kernel rejected. `correlation` remains bound to
    the statistical Pearson primitive.
  - Signal `fir_filter(signal, taps)` and `iir_filter(signal, b_taps, a_taps)`
    preserve the signal's shape (SciPy `lfilter` convention); empty or
    oversized taps rejected. `window_hann(signal)` / `window_blackman(signal)`
    return the signal carrier element-wise.
  - Signal `spectrogram(signal, nperseg)` returns
    `tensor<elem, (nperseg/2+1) x frames x 2>` where `frames = signal_len /
    nperseg`. Signal must be a power-of-two float array/vec; `nperseg` must be
    a power-of-two literal that divides the signal length. `wavelet_step` does
    one Mallat DWT level and returns `tensor<elem, (N/2) x 2>` for an
    even-length signal (row 0 = approx, row 1 = detail).
  - AD `jacobian(point)` / `hessian(point)` return `mat<elem, N, N>` when the
    point is a `vec<elem, N>` (Jacobian / Hessian in JAX/Autograd convention)
    and a scalar when the point is a scalar. `grad`, `hessian_diag`, `adjoint`,
    `jvp`, `vjp`, `sensitivity_forward`, `sensitivity_adjoint` preserve the
    carrier shape and reject non-float.
  - Sparse `csr_matvec(A, x)` / `sparse_solve(A, b)` return the RHS carrier
    and require a float collection. `sparse_reduce(field)` returns a scalar in
    the field's element type (also accepts dense float collections).
  - Graph `bfs_step(graph)` → `column<node>`, `shortest_path_step(graph)` /
    `pagerank_step(graph)` → `column<f64>`, `connected_components_step(graph)`
    → `column<u64>`. Non-graph carriers rejected.

  Monster's `/newcompute_` path already routes these op names to the
  signal-tiled-convolution / autodiff-dual-jvp-vjp / graph-frontier-csr /
  sparse-csr-graphblas-spmv specialized shader profiles, so the tightened
  shapes flow into the typed buffer ABI without any plan-side changes.
- Property/metamorphic tranche 61-62: Forge now parses/types/hashes an optional
  `forge_properties` section. A module can promise declarative metamorphic /
  property relations about an emitted output across its whole bounded input
  domain — `finite`, `range`, `monotone_increasing`, `monotone_decreasing`,
  `even`, `odd`, `idempotent`, `involutive`, `scale_invariant`, `homogeneous`,
  `translation`, `permutation_invariant` and `conserves`. Targets must be
  emitted outputs and named params must be real inputs; `finite` and `range`
  are discharged statically against the inferred bounds, the structural
  relations are sealed into the language/IR/Monster contract. The relations
  follow the classical property-based / metamorphic-testing taxonomy (symmetry,
  idempotence, involution, monotonicity, homogeneity, translation, permutation
  invariance, range and conservation). Forge never invents relations: the LLM
  chooses them, Forge types, hashes and forwards them. `forge_imports` now also
  accepts an optional `dialect <major>.<minor>` clause: a content-addressed
  import is compatible when it omits the clause or shares the host major and was
  authored against a minor the host already implements (`minor <= host_minor`),
  matching MLIR/Unison/semver content-addressed compatibility. The host dialect
  is exposed through `forge_dialect_version`, `forge_dialect_tag` and the
  `FORGE_DIALECT_VERSION_MAJOR`/`FORGE_DIALECT_VERSION_MINOR` constants.

Still to do in the language:

1. Give complex primitives real type/shape semantics, not only acceptance:
   SVD/QR/Cholesky/eigen must return real triplet/factorization shapes, ODE/PDE
   primitives must distinguish step vs. solve shapes and time axes, and
   crypto/hash block chaining must model CV state vs. digest output instead of
   only fixed `array<u32,8>` results.
2. Add typed result forms for the remaining domain dialects that are not yet
   covered by Monster pages.
3. Keep the manual dictionary in this file aligned with `src/kasm.rs` after
   every language change.
4. Every Monster/Forge math round must finish with a real app-runtime
   `/newcompute_` stress test, not only parser/unit tests: the ActCode must
   enter the universal template, execute through Monster, return typed result
   buffers plus proof/differential status, record to the compute library and
   prove fragment reuse on the second run.
5. Broad math changes must run the 20-case real-domain battery
   `newcompute_multi_domain_stress_runs_twenty_real_monster_math_variants`.
   It launches `/newcompute_` twenty times through `forge_brain_run_actcode`
   and the compute library, covering symbolic CAS, statistics, vibration
   signal processing, filtering/spectrograms, dense linalg, ODE/PDE solvers,
   sparse FEM/GraphBLAS, graph traversal/PageRank, autodiff sensitivities,
   nonlinear optimization, SDF geometry, physics integration, crypto hashes,
   constant-time proof guards, proof-envelope hashes, trading math, bio
   sequence alignment, chemistry graph reactions, epidemiological uncertainty
   and data-parallel reductions. Native tandem DOM/RAM and mesh handoffs are
   tested separately; this gate is for Monster mass-math execution.
6. Before adding new Forge math vocabulary, run the existing-vocabulary edge
   battery
   `newcompute_existing_vocab_edgecase_battery_runs_real_math_variants`.
   It launches `/newcompute_` fourteen more times through the same app-runtime
   path and compute library, covering symbolic proof/canonicalization, checked
   physical units, transcendental domain bounds, interval/uncertainty
   quantiles, Sobol/Latin-hypercube sampling, rolling/as-of time-series
   queries, graph topology counts, vector-frame geometry, matrix
   flatten/sort/top-k, linalg solve/least-squares, nonlinear solver
   cross-checks, trading walk-forward/stress-test, bio variant annotation and
   chemistry SMARTS/valence guards.
   2026-06-06 round finding: this battery exposed two alignment bugs, now
   fixed without adding language words:
   - Monster scalar-family differential production no longer seeds typed
     result buffers from candidate GPU readback; it uses deterministic
     manifest/artifact production so scalar CPU/GPU promotion can compare.
   - Forge `Mat` is now treated as a numeric collection by the shared
     collection helpers, so existing shape vocabulary like
     `flatten(matrix) -> top_k(...) -> mean(...)` works in the real template.
7. Universal-template changes must also run the 20-profession full-template
   battery
   `newcompute_professional_persona_battery_runs_twenty_full_templates`.
   It fills twenty complete `/newcompute_` ActCodes as different professional
   roles (mathematician, metrology, signal, biomedical, structural, control,
   FEM, network, inverse problems, optimization, robotics geometry, CFD,
   crypto, formal methods, compiler, quant trading, bio, medicinal chemistry,
   epidemiology and data-parallel programming). Each case must fill the
   universal template, declare `forge_validation`, execute through
   `forge_brain_run_actcode`, reach Monster, return typed/proof/differential
   evidence and reuse the compute library. 2026-06-06 round finding: this gate
   exposed that the parser did not mark `forge_validation` as a section
   boundary; fixed by adding it to the Forge section dictionary and hashing it
   into the Monster graph proof. The repeated round also exposed that the
   app-runtime JSON did not surface the contract back to the LLM; the
   `monsterPreparedCompute.plan.validationContract` field is now emitted and
   all `/newcompute_` batteries assert that role, method, oracle, uncertainty,
   replay, promotion and rollback are visible in the returned compact result.
8. Scalar numeric correctness changes must run the real value battery
   `newcompute_real_scalar_math_battery_runs_twenty_value_checked_professions`.
   It launches twenty fully filled `/newcompute_` templates as aerospace,
   orbital, structural, fluid, thermal, pharmacokinetic, epidemiology, finance,
   signal, robotics, optics, chemistry, population genetics, statistics,
   machine-learning, battery, acoustics, hydrology, materials and astronomy
   practitioners. Each case must reach `forge_brain_run_actcode`, Monster plan
   preparation, typed result buffers and a `scalarOracleOutputs` record whose
   `status` is `sample_value_matched`. 2026-06-06 round finding: the old route
   proved manifests and typed pages but did not expose the actual scalar sample
   value; fixed by adding f64 scalar sample-oracle evaluation and `scalarF64`
   decoding in app-runtime results.
9. Existing numeric Forge word changes must run
   `newcompute_existing_numeric_words_battery_runs_twenty_value_checked_cases`.
   It launches twenty `/newcompute_` templates through the same app-runtime
   path and value-checks source vocabulary that already exists in Forge:
   `log2`, `log10`, `cbrt`, `rsqrt`, `fma`, `lerp`, `mix`, `saturate`,
   `clamp`, `floor`, `ceil`, `round`, `trunc`, `fract`, `sign`, `copysign`,
   `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `exp2`, `pow`
   and safe chained domains. 2026-06-06 round finding: builtin `pow(...)`
   bounds were still integer-only while operator `^` accepted safe fractional
   powers; fixed so both public spellings share the same positive-base
   fractional interval behavior.
10. Existing Forge function-word changes must run
   `newcompute_existing_function_words_battery_runs_twenty_value_checked_cases`.
   It launches twenty complete `/newcompute_` templates through
   `forge_brain_run_actcode` and Monster, value-checking the existing public
   function spellings `add`, `sub`, `mul`, `div`, `mod`, `rem`, `neg`,
   `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `and`, `or`, `xor`, `not`, `any`,
   `all`, `where`, `select`, `finite`, `finite_check`, `nan_guard`,
   `approx_equal`, `erf`, `erfc`, `gamma`, `lgamma` and `beta`.
   2026-06-06 round finding: Monster could evaluate several of these words,
   but Forge typing rejected real templates that used `any/all` as variadic
   scalar condition reducers, `and/or` as multi-condition gates, and
   `finite_check`/`nan_guard` as guard aliases. Fixed the type layer and kept
   bool-to-number conversion explicit through `select(condition, 1.0, 0.0)`;
   no implicit boolean arithmetic is allowed. `mod/rem` now accept numeric
   scalar expressions under non-zero divisor bounds, and integer
   `gamma(n)` uses an exact factorial fast path for small positive integers.

### Monster Objectives

Done:

- `/newcompute_` enters Monster's universal compute template.
- `MonsterPreparedCompute::execute_mass_compute` is the primary mass-math call.
- Monster extracts `primitiveOps` from Forge IR.
- Monster builds GPU batch plans and typed ABI buffers.
- Monster generates WGSL/RHI kernels from primitive ops.
- Monster can execute a real GPU smoke path and produce readback,
  `output_hash` and `proof_hash`.
- Monster can shard lanes across compatible non-CPU adapters.
- Signal tranche 1: `fft/rfft/ifft` lower to a real GPU DFT reference kernel
  that reads the signal buffer, computes complex bins with `cos/sin`, dispatches
  through Rust RHI/wgpu and produces readback/proof hashes.
- Monster returns typed result buffers and real artifact pages from prepared
  executions, with page bytes, page hashes and buffer hashes for scalar,
  array/tensor, field/SDF, table, graph and native handoff layouts.
- Monster scalar sample outputs are value-checked on the real `/newcompute_`
  path. `MonsterComputeGraphPlan.scalar_oracle_outputs` records per-sample f64
  value, expected value, tolerance, absolute error, status and proof hash.
  `typed_result_buffers_for_execution` uses the matched scalar oracle as the
  first f64 bytes for scalar outputs, and Brain exposes both
  `monsterPreparedCompute.plan.scalarOracleOutputs` and per-buffer `scalarF64`.
  The scalar oracle currently evaluates real f64 arithmetic, comparisons,
  explicit boolean gates, user function calls and these public numeric/function
  words: `add`, `sub`, `mul`, `div`, `mod`, `rem`, `neg`, `finite`,
  `finite_check`, `nan_guard`, `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `and`,
  `or`, `xor`, `not`, `any`, `all`, `where`/`select`, `approx_equal`,
  `sqrt`, `rsqrt`, `cbrt`, `abs`, `exp`, `exp2`, `ln`/`log`, `log2`, `log10`,
  `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`,
  `tanh`, `erf`, `erfc`, `gamma`, `lgamma`, `beta`, `floor`, `ceil`,
  `round`, `trunc`, `fract`, `sign`, `saturate`, `min`, `max`, `clamp`,
  `copysign`, `fma`, `lerp`/`mix` and `pow`.
- Monster attaches deterministic CPU reference oracles per primitive family:
  statistics, optimization, autodiff, solvers, signal, sparse/graph,
  geometry/SDF, physics, DOM/RAM, trading, bio sequence, chemistry graph and
  crypto/hash. These are verification oracles and typed page producers, not
  final high-performance libraries.
- Monster emits a numeric policy per manifest: f32/f64 mode, deterministic
  reduction policy, NaN/Inf traps, bounds traps, overflow policy and
  reproducible RNG stream hash.
- Monster emits a differential test plan per manifest with reference backends,
  analytic/metamorphic cases and tolerances per precision tier.
- Monster executes differential promotion gates: each mass execution now carries
  `MonsterDifferentialExecution` entries with primitive family, CPU buffer hash,
  candidate backend/hash, readback hashes, tolerance, max absolute/relative
  error in ppm, promotion status and proof hash. CPU-only runs are promoted
  through named production CPU algorithms; real GPU runs must either promote
  against those production algorithms or block with a hashed mismatch.
- `/newcompute_` exposes optional `forge_transforms` and `forge_schedule`
  sections. Monster plans preserve their canonical contracts and
  `language_contract_hash`, include them in compute graph proof hashes, and
  buffer `interval<T>` / `uncertainty<T>` outputs as typed result layouts.
- `/newcompute_` also exposes optional `forge_runtime` and `forge_hostcalls`
  sections. Monster plans preserve runtime/hostcall contracts in proof hashes,
  advertise WGSL/RHI, CPU SIMD, optional CUDA and sealed-hostcall policies, and
  return sparse field result pages with page-table/hash-grid layouts.
- `/newcompute_` exposes memory/DOM/RAM and taint enrichments:
  `snapshot/memory_map/heap_object/dom_node/dom_edge`,
  `refs/retainers/leaks/mutation_diff`,
  `taint<public|user_data|credential|secret>` and the
  `ram_dom_reads_require_ingen_capabilities` policy. Monster plans route these
  ops through `wgsl.dom_ram_capability_graph_taint.v1`, preserve sealed
  capability contracts in proof hashes and return typed DOMSnapshot,
  memory-map, heap/DOM record, taint and columnar leak-table artifacts.
- `/newcompute_` exposes Banger render enrichments:
  SDF/neural SDF, voxel pages, surfels, micromeshes, material graphs,
  meshlet/geometry/LOD pages, radiance caches, shadow pages, hashed PCG,
  spatial streaming, light budgets and export payloads. Monster routes these
  ops through `wgsl.native_tandem_banger_render_pages.v1`, preserves the Forge
  proof chain and returns typed Banger SDF, voxel, meshlet, radiance, shadow,
  material, PCG, spatial-cell and light-budget artifacts.
- `/newcompute_` exposes Trading, Bio/DNA and Chemistry dialect enrichments.
  Monster routes their primitive ops through
  `wgsl.trading_point_in_time_backtest.v1`,
  `wgsl.bio_sequence_alignment_annotation.v1` and
  `wgsl.chem_graph_smarts_fingerprint.v1`, then returns typed market,
  portfolio/PnL, packed bio-sequence, bio-annotation/alignment, chemistry graph,
  reaction and conformer artifact pages with proof hashes. The mass-math
  differential path now has deterministic CPU production bytes for these
  families so GPU/RHI runs can promote against typed CPU buffers instead of
  falling back to generic manifest-seeded bytes.
- `/newcompute_` exposes Crypto/ZK and Code/Agent dialect enrichments. Monster
  routes object crypto/proof ops through
  `wgsl.crypto_object_constant_time_proof.v1` and code patch/test/proof ops
  through `wgsl.code_agent_patch_proof_envelope.v1`, then returns typed
  crypto hash/merkle/signature pages, code graph pages and patch/testcase
  pages with proof hashes.
- `/newcompute_` exposes symbolic math contract enrichments. Monster routes
  `expr/polynomial/piecewise/assumption/domain/solve/proof` ops through
  `wgsl.symbolic_math_contract_plan.v1`, preserves them in `primitiveOps`,
  includes the symbolic IR class in graph proof hashes and returns typed
  symbolic math DAG, assumption/domain and solution-set artifact pages.
  `MonsterSymbolicMathPlan` (`forge.monster.symbolic_math_plan.v1`) seals the
  primitive op set, rewrite/calculus/solve/proof families, assumption policy,
  e-graph contract policy, exact arithmetic promotion policy and optional
  dev-oracle policy into the graph proof. It also carries
  `MonsterSymbolicMathOutput` records for every emitted symbolic/proof output:
  `source_expr`, `canonical_form`, `result_kind` and per-output proof hash.
  Typed Monster result pages preserve those canonical forms before deterministic
  padding. Symbolic result layouts use byte-exact differential comparison, not
  numeric ppm comparison, because their bytes are canonical DAG/proof payloads
  even when the Forge element type is `f64`. The store-backed app-runtime stress
  test `newcompute_store_backed_symbolic_stress_runs_real_monster_math` runs a
  real `/newcompute_` ActCode through Monster, Vulkan/CPU execution,
  `MonsterTypedResultBuffer`, compute-library recording and fragment reuse.
  This keeps Wolfram-like symbolic capability on the same Forge source ->
  Monster plan -> proof path rather than creating a side CAS pipeline.
- Monster emits a multi-adapter schedule contract: adapter scoring, memory
  budget policy, chunk sizing, retry, blacklist and deterministic merge policy.
- Monster persists generated WGSL kernels into the Forge store under
  `brain/computes/kernel_shader_cache`, keyed by Forge IR hash,
  `primitiveOps`, ABI hash, adapter class and shader hash.
- Crypto tranche 1: `sha256_block` lowers to a real WGSL SHA-256 compression
  block kernel with message schedule, K256 constants, Ch/Maj/Sigma rounds,
  typed input binding, readback digest lanes, shader profile
  `wgsl.crypto_sha256_block.v1` and a regression test that forbids fallback to
  the generic `mix32` hash probe.
- Crypto tranche 2: `blake3_chunk` lowers to a real WGSL BLAKE3 chunk
  compression kernel with IV state, 64-byte block words, CHUNK_START/CHUNK_END
  flags, 7 BLAKE3 rounds, message permutation, G mixing function, typed input
  binding, readback chaining-value lanes, shader profile
  `wgsl.crypto_blake3_chunk.v1` and a regression test that forbids fallback to
  the generic `mix32` hash probe.
- Crypto tranche 3: `merkle_pair` lowers to a real WGSL BLAKE3 parent-node
  compression kernel for two 32-byte child chaining values, with PARENT domain
  flag, 64-byte parent message, 7 BLAKE3 rounds, message permutation, typed
  input binding, readback parent CV lanes, shader profile
  `wgsl.crypto_blake3_merkle_pair.v1` and a regression test that forbids
  fallback to the generic `mix32` hash probe.
- Crypto tranche 4: `hmac_block` lowers to a real WGSL HMAC-SHA256 block
  kernel with inner/outer pads, SHA-256 compression rounds, fixed 64-byte
  message-block padding, inner digest padding, typed input binding, readback
  MAC lanes, shader profile `wgsl.crypto_hmac_sha256_block.v1` and a
  regression test that forbids fallback to the generic `mix32` hash probe.
- Monster objective 1 is closed: first-stratum reference/probe kernels are
  replaced by specialized WGSL kernels for Stockham FFT/IFFT/RFFT passes,
  tiled signal convolution, CSR/GraphBLAS sparse matrix ops, CSR frontier graph
  traversal, 2x2 small linalg factorization/eigen/SVD probes, RK4/stencil
  solver steps, dual-number JVP/VJP/gradient kernels, ChaCha20 XOR stream and
  SHA-256 random-oracle probes. `monster_lowers_objective1_families_to_specialized_kernels`
  verifies that these profiles no longer use `probe` or `reference` names.
- Monster objective 2 is closed: CPU reference oracles are promoted into
  named production CPU algorithms per primitive family. Differential plans are
  now `forge.monster.differential_test_plan.v2` with production backends and a
  promotion policy. Differential executions bind `production_backend`,
  `production_algorithm`, `production_algorithm_hash` and `promotion_policy` in
  their proof hash. CPU-only executions report
  `cpu_production_algorithm_promoted`; GPU executions can only promote when
  typed buffers match the production CPU algorithm within family tolerance.
  `monster_promotes_cpu_oracles_to_production_algorithm_matrix` covers the
  family matrix.
- Monster prepares native tandem render artifacts for render handoffs without
  entering the mass-math executor: SDF brick pages, voxel pages, meshlet pages,
  surfel/radiance cache pages and PBR material payloads. These pages carry real
  bytes, page hashes and artifact hashes and are included in the prepared
  manifest hash.
- Monster objective 3 is closed: native tandem render artifacts are promoted
  into production renderer caches. Render artifacts now carry
  `renderer_cache_class`, residency policy, culling policy,
  `renderer_cache_hash`, material/radiance `renderer_variant_hash` and
  `renderer_promotion_hash` in the prepared manifest proof. Payloads include a
  compact `MRC3` cache header, NanoVDB/OpenVDB-like sparse SDF/VDB hierarchy
  metadata, meshlet sphere/cone/frustum/LOD culling metadata, multi-bounce
  surfel/radiance records and material variant payloads.
  `monster_promotes_native_render_pages_to_production_renderer_caches` verifies
  the promotion matrix.
- Monster prepares DOM/RAM cartography artifacts for graph/table handoffs
  without blocking the browser event loop: DOM graph pages, RAM region tables
  and a browser event-loop slice manifest with hashes and byte lengths.
- Monster objective 4 is closed: DOM/RAM cartography artifacts are promoted
  into live browser integration manifests. DOM/RAM artifacts now carry
  `live_capture_policy`, `live_resume_cursor`, `live_backpressure_policy`,
  `live_section_owner` and `live_slice_hash` in the prepared manifest proof.
  Payloads include a compact `MDR4` live header, CDP DOMSnapshot-style
  incremental CSR graph slices, resumable RAM region table slices, scheduler /
  idle-deadline event-loop budgets, long-task backpressure and section-owned
  WebExplorer rendering ownership. `monster_promotes_dom_ram_pages_to_live_browser_cartography`
  verifies the live cartography contract.
- `/newcompute_` exposes the Forge property/metamorphic enrichments. Each
  compute graph plan now carries a `property_contract` (verbatim relations) and
  a `MonsterPropertyCheckPlan` (`forge.monster.property_check_plan.v1`) that
  classifies every declared relation into statically discharged `static_bounds`
  entries and runtime `runtime_metamorphic` entries, each with a per-relation
  proof hash, and folds both into the compute graph proof hash. Monster never
  evaluates or invents a relation; it records, classifies and seals what the
  module promised. `monster_carries_forge_property_relations_into_the_compute_graph_plan`
  verifies the property plan and that declaring properties changes the proof.
- `/newcompute_` exposes the Forge validation contract. `forge_validation`
  lines declare who authored the mathematical validation (`role`), what style
  of check is required (`method`), which oracle family backs it (`oracle`), how
  uncertainty is treated, which compact reference/replay id was used, and the
  promotion/rollback rules. Monster carries this as `validation_contract`,
  includes it in compute graph proof hashes and advertises the slot in the
  universal template. Brain exposes the same contract in
  `monsterPreparedCompute.plan.validationContract` so the LLM can audit the
  returned result before deciding `usable_math`, `suspect_math` or
  `rejected_math`. This is metadata for verified execution and reuse, not a
  side validator or prose-quality score.

Still to do in Monster:

1. Delete or extract every remaining Monster helper that does not feed
    `Forge source -> MonsterPreparedCompute -> execute/reuse/proof`.
2. Extend symbolic math beyond the first exact local CAS slice on the same
   path: e-graph saturation, richer assumptions/refinement, exact rational
   multivariate polynomial normal forms, solve/reduce fragments and optional
   sealed Wolfram/SymPy dev oracles.

## Forge Language Reference

This section describes the current state of Forge. It is the implementation
reference for agents and developers. Treat it as the truth of what exists today,
not a promise of the future roadmap.

### Identity

Forge is the content-addressed compute language of InGen. It is not the app and
it is not Monster. InGen receives intent, Forge expresses verified compute,
Monster executes compact bytecode/proven programs. The Rust implementation
still uses the historical `kasm` name in many files; when reading code,
`src/kasm.rs` is the current Forge language implementation.

Forge has two layers:

- Forge source: readable module text generated by deterministic MathContract
  compilers or authored directly by expert Forge developers/tools.
- Forge bytecode/KASM runtime: compact typed programs, hashes, proofs and
  Monster execution paths.

The source layer is ahead of the full lowering layer. Today Forge can parse,
canonicalize and verify rich source modules. It cannot yet lower every source
feature to high-performance bytecode/GPU kernels.

### Design Doctrine

Forge follows current best practice from verified languages, array languages
and compiler IRs:

- like Dafny-style verification, Forge separates authored code from contracts:
  types, units, bounds, assertions, termination shape and sample tests must be
  explicit;
- like Futhark-style array compute, Forge aims for pure data-parallel
  operators, explicit shapes and predictable work before backend scheduling;
- like MLIR-style dialects, Forge should grow by typed dialect surfaces rather
  than by becoming a loose general-purpose scripting language;
- like WGSL/WebGPU-style compute, Forge should keep memory, capabilities,
  layouts and host authority explicit, never ambient.

The practical rule is simple: a Forge module must explain what it computes,
what units it uses, which bounds make the math safe, which assertions must hold,
how expensive it may be, and what proof/artifact leaves the system.

### Module Shape

A complete Forge source module is section-based:

```text
forge_module:
  module <name> version <u32>
forge_imports:
  none
  # or: import hash <name> = sha256:<64 hex chars>
  # optional dialect pin: import hash <name> = sha256:<64 hex> dialect <major>.<minor>
forge_constants:
  const <name>: <type> unit <unit> = <scalar>
forge_functions:
  fn <name>(arg: type, ...) -> <type> { return <expr> }
forge_program:
  let <name> = <expr>
  emit <output_name>: <type> = <expr>
forge_inputs:
  param <name>: <type> unit <unit> bounds [min,max] nominal value
forge_outputs:
  output <name>: <type> unit <unit> handoff <kind>
forge_constraints:
  assert finite(expr)
  assert bounds(expr,[min,max])
  assert approx(actual, expected, tolerance)
  assert <boolean expr>
forge_samples:
  case <name> seed <u64> { given a=1.0, b=2.0; expect y approx 3.0 tolerance 0.01 }
forge_properties:                 # optional
  property <name>: <kind>(<output>[, <param>...][, <number>...]) tolerance <f64>
forge_cost:
  max_steps=<u64>
  max_memory_mb=<u64>
  precision=f32|f64
  parallelism=<u32>       # optional
artifact_handoff:
  proof_hash,output_hash,compact_result
```

`forge_imports` and `forge_constants` may contain `none`. Other sections are
required by current `ForgeModuleSpec::parse`.

### Forge Language Dictionary

<!-- forge-language-dictionary:generated:start -->

This dictionary is maintained manually. The generator can miss language
enrichment helpers; when Forge gains words, update this block directly from
the audited parser/runtime changes.

User identifiers may still introduce module, function, parameter, constant,
let and output names, but they must not collide with reserved control names
or forbidden effect names.

Sections:

- `artifact_handoff`, `forge_constants`, `forge_constraints`, `forge_cost`,
  `forge_functions`, `forge_hostcalls`, `forge_imports`, `forge_inputs`, `forge_module`,
  `forge_outputs`, `forge_program`, `forge_properties`, `forge_runtime`, `forge_samples`,
  `forge_schedule`, `forge_transforms`, `mission`

Module and declaration words:

- `module`, `version`, `none`, `import`, `hash`, `sha256`, `dialect`, `const`, `fn`, `return`, `let`,
  `emit`, `param`, `output`, `unit`, `bounds`, `nominal`, `handoff`, `case`, `seed`,
  `given`, `expect`, `tolerance`, `max_steps`, `max_memory_mb`, `precision`,
  `parallelism`, `transform`, `schedule`, `property`, `target`, `algorithm`, `tile`, `vectorize`,
  `gpu`, `layout`, `runtime`, `lowering`, `cpu_simd`, `cuda`, `memory_layout`,
  `sparse_layout`, `hostcall`, `sealed`, `capability`, `proof_hash`, `output_hash`,
  `compact_result`

Types parsed by `ForgeType::parse` and `ForgeScalarTy::parse`:

- `alignment`, `array<T,N>`, `ast`, `atom`, `bar`, `bitvec<N>`, `bond`, `bool`,
  `callgraph`, `cfg`, `column<T>`, `complex`, `complex<T>`, `conformer`, `curve`,
  `diff`, `dna`, `dom_edge`, `dom_node`, `expr`, `expr<T>`, `f32`, `f64`, `feature`, `field`,
  `field<T,R>`, `field<p>`, `gene`, `geometry_page`, `graph<N,E>`, `hash`,
  `heap_object`, `i1`, `i32`, `i64`, `interval`, `interval<T>`, `assumption_set`,
  `light_budget`, `lod_node`, `mat<T,C,R>`, `mat2`, `mat3`, `mat4`, `material_graph`,
  `memory_map`, `merkle`, `meshlet_cluster`, `micromesh`, `molecule_graph`,
  `math_domain`, `neural_sdf`, `orderbook`, `patch`, `pcg_graph`, `piecewise`,
  `piecewise<T>`, `pnl`, `polynomial`, `polynomial<T>`, `position`, `protein`,
  `quote`, `radiance_cache`, `radiance_probe`, `reaction`, `rna`, `sdf`,
  `shadow_page`, `signature`, `snapshot`, `solution_set`, `spatial_cell`, `sparse_field`,
  `sparse_field<T,R,hash_grid>`, `sparse_field<T,R,page_table>`,
  `sparse_field<T,R,sparse_grid>`, `surfel`, `symbol`, `table<name:type;...>`,
  `testcase`, `tick`, `trade`,
  `taint<credential>`, `taint<public>`, `taint<secret>`, `taint<user_data>`,
  `tensor<T,shape>`, `u32`, `u64`, `uncertainty`, `uncertainty<T>`, `variant`,
  `vec<T,N>`, `vec2`, `vec3`, `vec4`, `voxel_page`
- taint labels: `public`, `user_data`, `credential`, `secret`. `secret` is a
  taint label only inside `taint<secret>` or `taint_secret`; free effect calls
  named `secret` remain rejected.

Optional language planning sections:

- `forge_transforms` accepts `transform name: kind(target)` where `kind` is
  `jit`, `batch`, `vectorize`, `parallel`, `grad`, `jacobian`, `hessian`,
  `adjoint`, `optimize`, `constraint_solve` or `least_squares`.
- `forge_schedule` accepts `schedule name: target=id algorithm=id tile=N
  vectorize=N gpu=true|false layout=soa|aos|tile|shared|page|stream`.
- `forge_properties` accepts `property name: kind(output[, param...][, number...])
  tolerance T` where `kind` is `finite`, `range`, `monotone_increasing`,
  `monotone_decreasing`, `even`, `odd`, `idempotent`, `involutive`,
  `scale_invariant`, `homogeneous`, `translation`, `permutation_invariant` or
  `conserves`. The target must be an emitted output and named params must be
  inputs. Per-kind arity: `finite|idempotent|involutive` take only the target;
  `range` takes two numeric bounds; `monotone_increasing|monotone_decreasing|
  even|odd|scale_invariant|permutation_invariant|conserves` take one param;
  `homogeneous|translation` take one param and one number. `finite` and `range`
  are verified statically against the inferred bounds; the rest are sealed for
  the Monster property plan.
- `forge_validation` accepts `validation name: role=<token> method=<token>
  oracle=<token> uncertainty=<token> reference=<token> replay=<token>
  promotion=<token> rollback=<token>`. Tokens are compact ASCII ids
  (`A-Z`, `a-z`, `0-9`, `_`, `-`, `.`, `:`, `/`, `@`) with no free prose. The
  section is optional but, when present, is parsed by Forge, canonicalized,
  hashed into the language contract and carried into Monster graph proofs.
- `forge_imports` accepts an optional `dialect <major>.<minor>` (or `dialect
  <major>`) suffix on `import hash name = sha256:<64 hex>`. An import is
  compatible when it omits the clause, or shares the host major
  (`FORGE_DIALECT_VERSION_MAJOR`) and was authored against a minor `<=` the host
  minor (`FORGE_DIALECT_VERSION_MINOR`). A newer minor or different major is
  rejected. Host dialect: `forge_dialect_version` / `forge_dialect_tag`.
- `forge_runtime` accepts `lowering=interpreter|wgsl_rhi|cpu_simd|cuda`,
  `cpu_simd=off|auto|optional|required`, `cuda=off|auto|optional|required`,
  `memory_layout=soa|aos|tile|shared|page|stream` and
  `sparse_layout=sparse_grid|page_table|hash_grid`.
- Symbolic math contracts are normal `/newcompute_` Forge expressions, not a
  side pipeline. `expr<T>`, `polynomial<T>`, `piecewise<T>`,
  `assumption_set`, `math_domain` and `solution_set` lower into the same
  Forge IR and Monster graph proof as numeric calls. Current tranche seals the
  typed DAG/proof contract and route (`symbolic_math` /
  `wgsl.symbolic_math_contract_plan.v1`) and executes a first local exact CAS
  slice (`cpu_symbolic_exact_linear_polynomial_canonical_v1`) for canonical
  polynomial forms, bounded power expansion, neutral rewrites, polynomial
  derivatives, identity/constant/variable solves, equivalence/proof hashes and
  byte-exact typed symbolic result pages. Later CAS depth must extend this
  path rather than bypass it.
- `forge_hostcalls` accepts `hostcall name: sealed hostcall_name
  capability=sha256:<64 hex>`. Source-level Forge accepts only sealed
  non-raw hostcalls such as `read_capability`, `artifact_read_hash`,
  `kernel_project`, `ui_emit_projection`, `ui_project_event`,
  `job_read_projection`, `toolcell_run`, `hash_bytes` and `csv_profile_tiny`;
  raw filesystem, raw network and secret hostcalls are rejected.
- RAM/DOM read/query modules using `snapshot`, `memory_map`, `refs`,
  `retainers`, `leaks` or `mutation_diff` must declare a sealed
  `read_capability` or `artifact_read_hash` hostcall.

Units parsed by `ForgeUnitDim::named` plus composed unit expressions:

- `A`, `C`, `cd`, `g`, `Hz`, `J`, `K`, `kg`, `m`, `mol`, `N`, `none`, `Ohm`, `Pa`, `s`,
  `V`, `W`
- composed forms may use `*`, `/` and `^`, for example `kg*m/s^2` or `m^2`.

Output handoff kinds parsed by `ForgeOutputKind::parse`:

- `artifact`, `field`, `graph`, `mesh_params`, `scalar`, `score`, `sdf`, `table`,
  `timeseries`, `vector`

Precision values parsed by `ForgePrecision::parse`:

- `f32`, `f64`

Module hash/proof fields emitted by `ForgeModuleHashes`:

- `source_hash`, `imports_hash`, `constants_hash`, `functions_hash`, `fragment_hash`,
  `input_hash`, `output_hash`, `type_hash`, `unit_hash`, `bounds_hash`, `contract_hash`,
  `proof_hash`

Expression syntax:

- `true`, `false`, `-x`, `!flag`, `+`, `-`, `*`, `/`, `^`, `==`, `!=`, `<`, `<=`, `>`,
  `>=`, `&&`, `||`, `(...)`, `<...>`, `[...]`, `{...}`, `,`, `;`, `:`, `=`, `->`
- identifiers start with an ASCII letter or `_`, then continue with ASCII
  letters, digits or `_`.
- numeric literal suffixes: `f32`, `f64`, `i32`, `i64`, `u32`, `u64`; underscores are ignored;
  unsuffixed integers parse as `i64`, unsuffixed decimal/exponent numbers parse as `f64`.

Constraint and proof words:

- `assert`, `finite`, `bounds`, `approx`, `proof_hash`, `output_hash`,
  `compact_result`.

Current builtin calls:

- numeric: `add`, `sub`, `mul`, `div`, `mod`, `rem`, `neg`, `finite`, `abs`, `min`, `max`, `clamp`, `saturate`, `floor`, `ceil`, `round`, `trunc`, `fract`, `sign`, `copysign`, `fma`, `lerp`, `mix`, `sqrt`, `rsqrt`, `cbrt`, `pow`/`^` including integer powers and safe positive-base fractional powers, `exp`, `exp2`, `ln`, `log`, `log2`, `log10`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `erf`, `erfc`, `gamma`, `lgamma`, `beta`; `mod`/`rem` accept numeric scalar operands when divisor bounds exclude zero;
- boolean/comparison: `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `and`, `or`, `xor`, `not`, `any`, `all`, `where`, `select`; `and`/`or` and scalar `any`/`all` accept one or more boolean conditions, while bool-to-f64 conversion stays explicit through `select`;
- bit/integer/hash: `shl`, `shr`, `rotl`, `rotr`, `bit_and`, `bit_or`, `bit_xor`, `bit_not`, `popcount`, `clz`, `ctz`, `byte_swap`, `bit_reverse`, `hash32`, `hash64`;
- vector/matrix/complex: `vec2`, `vec3`, `vec4`, `mat2`, `mat3`, `mat4`, `complex`, `dot`, `length`, `distance`, `normalize`, `cross`, `outer`, `matmul`, `transpose`, `determinant`, `inverse`, `trace`, `eigen_small`, `svd_small`, `qr_small`, `cholesky_small`;
- collection/shape: `len`, `shape`, `rank`, `size`, `reshape`, `flatten`, `squeeze`, `unsqueeze`, `slice`, `concat`, `split`, `tile`, `repeat`, `broadcast`, `transpose_axes`, `permute`, `index`, `sum`, `rows`, `cols`, `node_count`, `edge_count`, `sample`;
- data-parallel: `map`, `zip`, `zip_with`, `reduce`, `fold`, `scan`, `prefix_sum`, `filter`, `compact`, `partition`, `sort`, `argsort`, `unique`, `histogram`, `bin_count`, `gather`, `take`, `scatter`, `scatter_add`, `scatter_min`, `scatter_max`, `masked_load`, `masked_store`, `atomic_add`, `atomic_min`, `atomic_max`;
- query/time: `window`, `rolling`, `groupby`, `join`, `asof_join`;
- sampling: `rng_seed`, `uniform`, `normal`, `lognormal`, `poisson`, `bernoulli`, `sobol`, `halton`, `latin_hypercube`, `stratified_sample`, `monte_carlo`, `importance_sample`, `resample`;
- statistics: `mean`, `variance`, `std`, `covariance`, `correlation`, `quantile`, `p5`, `p50`, `p95`, `interval`, `uncertainty`, `median`, `minmax`, `zscore`, `normalize_stats`, `linear_regression`, `robust_loss`;
- selection: `argmin`, `argmax`, `pareto`, `pareto_front`, `rank`, `top_k`, `diversity_select`;
- optimization: `gradient_descent_step`, `adam_step`, `newton_step`, `bfgs_step`, `line_search`, `project_bounds`, `constraint_penalty`, `optimize`, `constraint_solve`;
- autodiff: `grad`, `jacobian`, `hessian`, `hessian_diag`, `adjoint`, `jvp`, `vjp`, `finite_diff_check`, `sensitivity_forward`, `sensitivity_adjoint`;
- solvers: `root_find`, `bisection`, `newton_root`, `fixed_point`, `linear_solve`, `sparse_solve`, `least_squares`, `ode_step_euler`, `ode_step_rk4`, `ode_solve`, `pde_stencil_step`, `relaxation_step`;
- signal/FFT: `fft`, `ifft`, `rfft`, `convolution`, `fir_filter`, `iir_filter`, `window_hann`, `window_blackman`, `spectrogram`, `wavelet_step`;
- sparse/graph: `csr_matvec`, `coo_to_csr`, `sparse_reduce`, `graph_neighbors`, `graph_degree`, `bfs_step`, `shortest_path_step`, `pagerank_step`, `connected_components_step`;
- geometry/SDF/3D/Banger render: `transform_point`, `transform_normal`, `sdf_sphere`, `sdf_box`, `sdf_capsule`, `sdf_torus`, `sdf_union`, `sdf_intersection`, `sdf_subtract`, `sdf_smooth_union`, `gradient_field`, `normal_from_sdf`, `raymarch_step`, `marching_cubes_cell`, `voxel_sample`, `surfel_accumulate`, `sdf_from_field`, `neural_sdf_decode`, `sdf_curvature`, `sdf_gradient`, `sdf_normal`, `sdf_to_voxel_page`, `micromesh_build`, `meshlet_cluster`, `geometry_page_pack`, `lod_select`, `cluster_cull`, `radiance_probe`, `radiance_cache_update`, `screen_trace`, `world_trace`, `shadow_page_alloc`, `shadow_cache_invalidate`, `material_eval`, `material_layer`, `substrate_mix`, `material_payload`, `pcg_hash`, `pcg_spawn`, `pcg_execute`, `world_partition_cell`, `streaming_plan`, `residency_update`, `light_cluster`, `light_budget_select`, `light_proof`, `export_mesh`, `export_sdf`, `export_shader`, `export_preview`, `export_proof`;
- physics/engineering: `integrate_force`, `integrate_velocity`, `inertia_tensor`, `stress_tensor_basic`, `strain_basic`, `thermal_flux_step`, `fluid_advect_step`, `pressure_projection_step`, `collision_distance`, `constraint_project`;
- units/contracts/proof: `unit_cast`, `dimensional_check`, `bounds_check`, `finite_check`, `nan_guard`, `invariant`, `assert`, `approx_equal`, `hash_value`, `hash_buffer`, `proof_emit`;
- memory/DOM/RAM: `byte_load`, `byte_store`, `u32_load`, `f32_load`, `span`, `slice_view`, `page_id`, `pointer_tag`, `dom_node_record`, `graph_edge_record`, `memory_region_hash`, `capture_snapshot`, `read_memory_map`, `snapshot_id`, `memory_region_count`, `heap_object_id`, `dom_node_id`, `dom_edge_endpoints`, `refs`, `retainers`, `leaks`, `mutation_diff`, `taint_public`, `taint_user_data`, `taint_credential`, `taint_secret`, `taint_join`, `taint_check`;
- trading: `vwap`, `ema`, `volatility`, `slippage`, `latency`, `backtest`, `anti_lookahead`, `walk_forward`, `stress_test`, `transaction_costs`;
- bio/DNA: `kmer_hash`, `transcribe`, `translate`, `reverse_complement`, `align`, `alignment_score`, `motif_scan`, `mutate`, `annotate`;
- chemistry: `smiles_parse`, `smarts_match`, `fingerprint`, `molecular_similarity`, `substructure_search`, `valence_check`, `charge_check`, `aromaticity_check`, `conformer_generate`, `reaction_apply`;
- crypto/hash/proof: `sha256_block`, `blake3_chunk`, `merkle_pair`,
  `hmac_block`, `xor_stream`, `random_oracle_probe`, `bitvec_and`,
  `bitvec_xor`, `field_add`, `field_mul`, `curve_mul`, `hash_commit`,
  `merkle_root`, `merkle_verify`, `signature_verify`, `constant_time`,
  `secret_branch_check`, `zk_prove`, `zk_verify`, `smt_check`, `lean_check`;
- code/agents: `parse`, `parse_code`, `typecheck`, `typecheck_code`,
  `symbol_table`, `cfg_build`, `callgraph_build`, `transform`,
  `transform_code`, `patch`, `patch_apply`, `run_test`, `compare_trace`,
  `proof_envelope`;
- symbolic math / CAS contracts: `symbol`, `domain`, `assume`, `to_expr`,
  `polynomial`, `piecewise`, `simplify`, `full_simplify`,
  `canonicalize_expr`, `expand`, `factor`, `collect`, `cancel`, `together`,
  `apart`, `function_expand`, `trig_reduce`, `series`, `refine`, `diff`,
  `integrate`, `limit`, `residue`, `solve`, `reduce_equations`,
  `find_instance`, `math_equiv`, `math_proof`, `expression_hash`;
- bounded control: `fori`, `while_fuel`.

Reserved or rejected names:

- unbounded control calls are rejected: `for`, `loop`, `while`;
- reserved control identifiers cannot be used as function names: `for`, `fori`, `loop`, `while`, `while_fuel`;
- forbidden effect calls are rejected:
  `append_file`, `clock`, `delete_file`, `env`, `eval`, `exec`, `fetch`, `fractal`,
  `hostcall`, `http`, `io`, `load`, `network`, `now`, `open`, `print`, `rand`, `random`,
  `read`, `read_dom`, `read_file`, `read_ram`, `secret`, `shell`, `sleep`, `spawn`,
  `store`, `time`, `timestamp_now`, `write`, `write_file`
- forbidden effect prefixes are rejected: `host_`, `io_`, `sys_`.

Bytecode runtime dictionary:

- bytecode wire constants:
  `HEADER_LEN=32`, `NODE_LEN=8`, `FOOTER_LEN=32`, `MAX_NODES=4096`, `MAX_SLOTS=16`,
  `MAGIC=b"KASM"`, `VERSION=0`
- bytecode targets from `Target`:
  `Auto=0`, `Cpu=1`, `Kernel=2`, `Gpu=3`, `Qpu=4`
- bytecode types from `Ty`:
  `I64=1`, `Bool=2`, `F64=3`, `VecI64=4`
- bytecode opcodes from `Op`:
  `Input=0`, `ConstI64=1`, `AddI64=2`, `MulI64=3`, `EqI64=4`, `Hash64=5`, `Output=6`,
  `SubI64=7`, `DivI64Checked=8`, `MinI64=9`, `MaxI64=10`, `SelectI64=11`, `AndBool=12`,
  `OrBool=13`, `NotBool=14`, `LtI64=15`, `LeI64=16`, `BitAndI64=17`, `BitOrI64=18`,
  `BitXorI64=19`, `ShlI64=20`, `ShrI64=21`, `SatAddI64=22`, `SatSubI64=23`,
  `ModI64Checked=24`, `ClampI64=25`, `ReduceAddI64=26`, `ReduceMulI64=27`,
  `BitFlipI64=28`, `NegI64=29`, `ReverseBitsI64=30`, `ByteswapI64=31`, `ConstF64=32`,
  `F64Op=33`, `Adaptive=34`, `Comptime=35`, `Grad=36`, `Cond=37`, `Memoize=38`,
  `Pipeline=39`, `Vmap=40`, `Pmap=41`, `Fori=42`, `WhileLoop=43`, `Reduce=44`,
  `Scan=45`, `VLenI64=46`, `VSumI64=47`, `VAddI64=48`, `VMulI64=49`, `VSubI64=50`,
  `VMaxI64=51`, `VMinI64=52`, `VRangeI64=53`, `VConcatI64=54`, `VReverseI64=55`,
  `VBroadcastI64=56`, `VEqI64=57`, `VAndI64=58`, `VOrI64=59`, `VXorI64=60`,
  `VAbsI64=61`, `VNegI64=62`, `VBitFlipI64=63`, `Fractal=64`, `Eval=65`, `VGetI64=66`,
  `PopcntI64=67`, `LzcntI64=68`, `TzcntI64=69`, `PextI64=70`, `PdepI64=71`, `Lazy=72`,
  `Force=73`
- bytecode node fields from `Node`:
  `op`, `ty`, `a`, `b`, `imm`
- bytecode runtime errors from `KasmError`:
  `BadMagic`, `BadVersion`, `BadTarget`, `BadType`, `BadOp`, `BadLength`, `BadFooter`,
  `BadNodeCount`, `TooManySlots`, `FuelTooSmall`, `Truncated`, `BadInputLength`,
  `BadInputSlot`, `BadRef`, `TypeMismatch`, `OutputCount`, `ValueTypeMismatch`,
  `ComposeArity`, `ComposeType`, `ExternalTarget`, `BadReduceCount`, `BadF64SubOp`,
  `UnsupportedV1OpInScalarInterpreter`, `BadMultiMethod`
- `F64Op` immediate sub-ops from `F64SubOp`:
  `Add`, `Sub`, `Mul`, `DivChecked`, `Min`, `Max`, `Sqrt`, `Abs`, `Neg`, `FromI64`,
  `ToI64`, `Exp`, `Ln`
- `F64Op` immediate constants:
  `F64_ADD=0`, `F64_SUB=1`, `F64_MUL=2`, `F64_DIV=3`, `F64_MIN=4`, `F64_MAX=5`,
  `F64_SQRT=6`, `F64_ABS=7`, `F64_NEG=8`, `F64_FROM_I64=9`, `F64_TO_I64=10`,
  `F64_EXP=11`, `F64_LN=12`, `F64_OP_MAX=12`
- symbolic math bytecode/IR note:
  symbolic math is deliberately not added as dozens of scalar KASM opcodes.
  `expr/polynomial/piecewise/assumption/domain/solution` values are
  source-level typed artifacts lowered into Forge Compute IR class
  `symbolic_math`; Monster receives them through `/newcompute_` as
  `primitiveOps` and routes them to `wgsl.symbolic_math_contract_plan.v1`
  with proof hashes. Current execution canonicalizes exact polynomial DAG
  pages and compares symbolic layouts byte-for-byte during differential
  promotion. Future CAS execution must extend this typed IR/artifact path, not
  bypass it.
- validation contract note:
  `forge_validation` is source-level proof metadata, not a scalar opcode. It is
  lowered into Forge Compute IR as `validation_contract` strings and folded
  into Monster graph proof hashes so two mathematically identical programs with
  different validation/replay/promotion contracts do not share the same proof.
- physics source-level note:
  `integrate_force`, `integrate_velocity`, `fluid_advect_step`,
  `pressure_projection_step` and `constraint_project` are multi-argument Forge
  primitives whose result keeps the first argument's type, unit and bounded
  domain for mechanical preflight. This keeps real physics `/newcompute_`
  modules aligned across type inference, unit inference, bounds, cost and
  Monster primitive-family routing.

Tensor runtime dictionary:

- tensor wire constants:
  `TENSOR_MAGIC=b"SCANT01\0"`, `TENSOR_VERSION=1`, `TENSOR_HEADER_LEN=32`,
  `TENSOR_FOOTER_LEN=20`, `TENSOR_NODE_LEN=32`, `TENSOR_MAX_NODES=4096`,
  `TENSOR_MAX_DIMS=2`, `TENSOR_MAX_SLOTS=16`, `TENSOR_MAX_DIM_EXTENT=4096`
- tensor dtypes from `TensorTy`:
  `F32=1`, `Rational=2`, `Posit16=3`, `Posit32=4`
- tensor opcodes from `TensorOp`:
  `Const=1`, `Input=2`, `Output=3`, `AddF32=10`, `MulF32=11`, `AddRational=12`,
  `MulRational=13`, `AddPosit16=14`, `MulPosit16=15`, `AddPosit32=16`, `MulPosit32=17`,
  `MatmulTile=20`, `MatmulTileRational=23`, `MatmulTilePosit16=24`,
  `MatmulTilePosit32=25`, `ReduceSumAxis=21`, `Softmax=22`, `ReluF32=30`, `TanhF32=31`,
  `SigmoidF32=32`, `GeluTanhF32=33`
- tensor node fields from `TensorNode`:
  `op`, `dtype`, `a`, `b`, `imm`, `shape`
- tensor runtime errors from `TensorError`:
  `Truncated`, `TruncatedNode`, `BadMagic`, `BadVersion`, `BadFooter`, `BadNodeCount`,
  `FuelTooSmall`, `UnknownOp`, `UnknownDtype`, `DimOutOfRange`, `ShapeRankInvalid`,
  `ShapeNonZeroPastRank`, `BackrefOutOfBounds`, `ShapeMismatch`, `DtypeMismatch`,
  `BadAxis`, `BadSlot`, `NoOutput`, `TooManyInputs`, `TooManyOutputs`,
  `ConstPoolOverflow`, `ReservedNonZero`
- numeric contract reduction trees from `ReductionTree`:
  `Ltr=1`, `Pairwise=2`, `Avx2Tile=3`, `CudaBlock=4`, `GpuWarp=5`
- numeric contract kernel families from `KernelFamily`:
  `Scalar=1`, `Avx2=2`, `Avx512=3`, `CudaCublas=4`, `Metal=5`, `Rocm=6`
- numeric contract round modes from `RoundMode`:
  `NearestEven=1`, `TowardZero=2`, `Down=3`, `Up=4`
- numeric contract fields from `NumericContract`:
  `dtype`, `reduction_tree`, `kernel_family`, `tile_shape`, `quant_grid`, `error_budget`

FBC v0 dictionary:

- FBC wire constants:
  `FBC_VERSION=0`, `FBC_VERIFIER_VERSION="forge-fbc-verifier-v0"`
- FBC text directives parsed by `parse_program_v0`:
  `name=`, `schema=`, `deterministic=`, `hostcall=`, `cap=`, `op=`
- FBC program fields from `ForgeBytecodeProgram`:
  `name`, `version`, `capabilities`, `hostcalls`, `ops`, `expected_output_schema`,
  `deterministic`
- FBC op enum variants from `ForgeOpcode`:
  `PushBytes`, `PushText`, `PushCapability`, `ReadCapability`, `HashTop`,
  `CsvProfileTiny`, `ToolCellProjectTiny`, `KernelProject`, `JobReadProjection`,
  `UiIntentTransition`, `EmitProjection`, `RawFilesystemProbe`, `RawNetworkProbe`, `End`
- FBC text opcode names from `opcode_name`:
  `csv_profile_tiny`, `emit_projection`, `end`, `hash_top`, `job_read_projection`,
  `kernel_project`, `push_bytes`, `push_capability`, `push_text`,
  `raw_filesystem_probe`, `raw_network_probe`, `read_capability`,
  `toolcell_project_tiny`, `ui_intent_transition`
- FBC capability kinds from `ForgeCapabilityKind`:
  `FileHash`, `ArtifactHash`, `MemoryScope`, `NetworkSource`, `EventSchema`,
  `UiProjection`, `GpuBudget`, `ModelProviderScope`, `RawFilesystem`, `RawNetwork`,
  `Secret`
- FBC capability text names from `cap_kind_name`:
  `artifact_hash`, `event_schema`, `file_hash`, `gpu_budget`, `memory_scope`,
  `model_provider_scope`, `network_source`, `raw_filesystem`, `raw_network`, `secret`,
  `ui_projection`
- FBC capability fields from `ForgeCapability`:
  `kind`, `scope`, `sealed_hash`, `content_hash`, `limit_bytes`
- FBC hostcalls from `ForgeHostCall`:
  `HashBytes`, `CsvProfileTiny`, `UiProjectEvent`, `KernelProject`, `JobReadProjection`,
  `ToolCellRun`, `MemoryRecall`, `ArtifactReadHash`, `UiEmitProjection`,
  `NetworkFetchSourceId`, `ReadCapability`, `RawFilesystem`, `RawNetwork`, `ReadSecret`
- FBC hostcall text names from `hostcall_name`:
  `artifact_read_hash`, `csv_profile_tiny`, `hash_bytes`, `job_read_projection`,
  `kernel_project`, `memory_recall`, `network_fetch_source_id`, `raw_filesystem`,
  `raw_network`, `read_capability`, `read_secret`, `toolcell_run`, `ui_emit_projection`,
  `ui_project_event`
- FBC VM statuses from `ForgeVmStatus`:
  `Ok`, `VerifierDenied`, `FuelExhausted`, `MemoryLimitExceeded`, `RuntimeError`
- FBC VM errors from `ForgeVmError`:
  `VerifierDenied`, `FuelExhausted`, `MemoryLimitExceeded`, `OutputLimitExceeded`,
  `StackUnderflow`, `CapabilityDenied`, `MissingEnd`, `Parse`
- FBC verifier report fields from `ForgeVerifierReport`:
  `ok`, `verifier_hash`, `errors`, `warnings`, `declared_hostcalls`,
  `capability_summary`, `max_fuel`, `max_memory_bytes`, `program_hash`
- FBC run proof fields from `ForgeRunProof`:
  `program_hash`, `bytecode_hash`, `verifier_hash`, `input_hash`, `output_hash`,
  `capability_hash`, `hostcall_hash`, `fuel_used`, `memory_peak`, `backend`,
  `deterministic_replay_hash`, `proof_hash`
- FBC VM config fields from `ForgeVmConfig`:
  `max_fuel`, `max_memory_bytes`, `max_input_bytes`, `max_output_bytes`, `backend`,
  `forbidden_opcodes`

Embedded KASM/Forge dialect reference:

- embedded dialect constants:
  `FORGE_EMBEDDED_KASM_TABLEGEN_DIALECT`
- embedded dialect target spellings:
  `auto`, `cpu`, `gpu`, `kernel`, `qpu`
- embedded dialect operation mnemonics:
  `add`, `andb`, `band`, `bit_flip`, `bor`, `bswap`, `bxor`, `clamp`, `const`, `divc`,
  `eq`, `hash`, `input`, `le`, `lt`, `max`, `min`, `modc`, `mul`, `neg`, `notb`, `orb`,
  `output`, `program`, `reduce_add`, `reduce_mul`, `rev_bits`, `satadd`, `satsub`,
  `select`, `shl`, `shr`, `sub`

Complete public inventory from `src/kasm.rs`:

- public enums:
  `Action`, `ColumnarError`, `DistillError`, `ExecutionError`, `F64SubOp`,
  `ForgeBinaryOp`, `ForgeCapabilityKind`, `ForgeConstraintSpec`, `ForgeExpr`,
  `ForgeHostCall`, `ForgeIrKernelClass`, `ForgeIrOp`, `ForgeLoweringTarget`,
  `ForgeOpcode`, `ForgeOutputKind`, `ForgePrecision`, `ForgePropertyKind`, `ForgeRuntimeMode`,
  `ForgeScalarTy`, `ForgeScalarValue`, `ForgeScheduleLayout`, `ForgeSparseLayout`,
  `ForgeTaintKind`, `ForgeTransformKind`, `ForgeType`, `ForgeUnaryOp`, `ForgeVmError`,
  `ForgeVmStatus`, `Indicator`, `InteropError`, `JitError`, `KasmAbiType`,
  `KasmDeltaError`, `KasmError`, `KernelFamily`, `LoweringError`, `MlirError`,
  `OhlcvError`, `Op`, `OrderBookError`, `OrderBookEvent`, `Pattern`, `ProofError`,
  `RankError`, `ReductionTree`, `Replace`, `RoundMode`, `SelfHostError`, `Side`,
  `SsaOp`, `SsaVerifyError`, `Target`, `TensorError`, `TensorOp`, `TensorTy`,
  `TensorValue`, `Ty`
- public structs:
  `ApplyOutcome`, `BacktestSummary`, `BarResampler`, `Block`, `BlockId`, `ColumnStore`,
  `Decoded16`, `Decoded32`, `Deterministic`, `DistilledShortcut`, `DistillTensorConfig`,
  `Duration`, `ExecutionResult`, `Fill`, `ForgeAppRegistry`, `ForgeArtifactHandoff`,
  `ForgeBackendSelection`, `ForgeBounds`, `ForgeBytecodeProgram`, `ForgeCapability`,
  `ForgeCapabilityBinding`, `ForgeCompiledToolCell`, `ForgeComputeIrModule`,
  `ForgeConstSpec`, `ForgeCostSpec`, `ForgeFunctionArg`, `ForgeFunctionSpec`,
  `ForgeHostContext`, `ForgeImportSpec`, `ForgeIrBufferLayout`, `ForgeIrFunction`,
  `ForgeIrNode`, `ForgeIrValueId`, `ForgeModuleHashes`, `ForgeModuleSpec`,
  `ForgeOptimizerReport`, `ForgeOutputSpec`, `ForgeParamSpec`, `ForgePipelineOutput`,
  `ForgeProgramEmit`, `ForgeProgramLet`, `ForgeProgramSpec`, `ForgeProofLedgerEntry`,
  `ForgePropertySpec`, `ForgeRunProof`, `ForgeRuntimeSpec`, `ForgeSampleCase`, `ForgeScheduleSpec`,
  `ForgeSealedHostcallSpec`, `ForgeToolCellBatchOutput`, `ForgeToolCellBatchRecord`,
  `ForgeToolCellRegistry`, `ForgeToolCellSpec`, `ForgeTransformSpec`, `ForgeUnitDim`,
  `ForgeVerifierReport`, `ForgeVmConfig`, `ForgeVmOutput`, `Inst`, `JitKernel`,
  `KasmDeltaPatch`, `KasmDeltaProof`, `KasmErrno`, `KasmInteropProofEnvelope`,
  `MarketImpactModel`, `MlirLoweringReport`, `MultiMethod`, `NanBoxValue`, `Node`,
  `NoUB`, `NumericContract`, `OhlcvBar`, `OhlcvStore`, `OrderBook`, `PartialEvalReport`,
  `PeepholeStats`, `Posit16`, `Posit32`, `Program`, `ProgramSig`, `Proven`, `Pure`,
  `Q3132`, `QuantGrid`, `RankedTensor`, `Rational`, `ReservoirSampler`, `Rewrite`,
  `RewriteReport`, `SelfHostingRuntime`, `SelfHostStats`, `SsaBuilder`, `SsaFunction`,
  `Strategy`, `TensorErrorBudget`, `TensorNode`, `TensorProgram`, `TensorShape`,
  `Terminating`, `ThreadedCtx`, `Timestamp`, `ValueId`, `WasmComponentContract`,
  `WasmComponentContractProjection`, `WasmFunction`, `WasmFunctionProjection`,
  `WasmInterface`, `WasmInterfaceProjection`, `WasmParamProjection`, `WasmWorld`,
  `WasmWorldProjection`
- public constants:
  `DEFAULT_MAX_DEPTH`, `F64_ABS`, `F64_ADD`, `F64_DIV`, `F64_EXP`, `F64_FROM_I64`,
  `F64_LN`, `F64_MAX`, `F64_MIN`, `F64_MUL`, `F64_NEG`, `F64_OP_MAX`, `F64_SQRT`,
  `F64_SUB`, `F64_TO_I64`, `FBC_VERIFIER_VERSION`, `FBC_VERSION`, `FOOTER_LEN`,
  `FORGE_DIALECT_VERSION_MAJOR`, `FORGE_DIALECT_VERSION_MINOR`,
  `FORGE_EMBEDDED_KASM_TABLEGEN_DIALECT`, `GENERALIZED_SCORE_K`, `HEADER_LEN`, `MAGIC`,
  `MAX_NODES`, `MAX_SLOTS`, `NANOS_PER_DAY`, `NANOS_PER_HOUR`, `NANOS_PER_MICRO`,
  `NANOS_PER_MILLI`, `NANOS_PER_MIN`, `NANOS_PER_SEC`, `NODE_LEN`, `TENSOR_FOOTER_LEN`,
  `TENSOR_HEADER_LEN`, `TENSOR_MAGIC`, `TENSOR_MAX_DIM_EXTENT`, `TENSOR_MAX_DIMS`,
  `TENSOR_MAX_NODES`, `TENSOR_MAX_SLOTS`, `TENSOR_NODE_LEN`, `TENSOR_VERSION`, `VERSION`
- public `KasmErrno` constants:
  `ABSTRACT_DISPATCH`, `BAD_F64_SUB_OP`, `BAD_FOOTER`, `BAD_INPUT_LENGTH`,
  `BAD_INPUT_SLOT`, `BAD_LENGTH`, `BAD_MAGIC`, `BAD_MULTI_METHOD`, `BAD_NODE_COUNT`,
  `BAD_OP`, `BAD_REDUCE_COUNT`, `BAD_REF`, `BAD_TARGET`, `BAD_TYPE`, `BAD_VERSION`,
  `COMPOSE_ARITY`, `COMPOSE_TYPE`, `EXTERNAL_TARGET`, `FUEL_TOO_SMALL`,
  `NO_METHOD_FOUND`, `OK`, `OUTPUT_COUNT`, `TOO_MANY_SLOTS`, `TRUNCATED`,
  `TYPE_MISMATCH`, `UNKNOWN`, `UNSUPPORTED_V1_OP`, `VALUE_TYPE_MISMATCH`
- public functions:
  `a_ty`, `abs`, `adaptive`, `add`, `add_many`, `add_posit16`, `add_posit32`,
  `add_rational`, `add_row`, `add_rule`, `add_tick`, `affine_score_program`,
  `affine_self_host_program`, `and`, `apply`, `apply_rank_0`, `apply_rank_1`,
  `artifact_handle`, `as_bool`, `as_f64`, `as_i48`, `as_inner`, `as_vec_handle`,
  `as_wit_string`, `asks_levels`, `atr`, `b_ty`, `band`, `bar`, `bars_emitted`,
  `best_ask`, `best_bid`, `bids_levels`, `bit_and`, `bit_flip`, `bit_or`, `bit_xor`,
  `bor`, `broadcast_add`, `bucket`, `build_denial_proof`, `build_proof`,
  `build_runtime_error_proof`, `bxor`, `byte_size`, `bytecode_ty`, `bytes`,
  `bytes_used`, `byteswap`, `calls`, `canonical`, `canonical_bytes`,
  `canonical_hash_hex`, `canonical_mlir_text`, `canonical_source`, `canonicalize`,
  `capacity`, `checked_add`, `checked_div`, `checked_mul`, `checked_sub`, `clamp`,
  `close_column`, `code`, `column_max`, `column_min`, `column_sum`, `columns`,
  `compile`, `compile_tool_cell_bundle`, `compile_tool_cell_bundle_with_graph`,
  `compile_tool_cell_program`, `compile_wit_export_stub`, `compose`, `comptime`, `cond`,
  `const_at`, `const_f64`, `const_hash_hex`, `const_i64`, `const_pool`, `contains_zero`,
  `cse`, `csv_profile_tiny_program`, `current_depth`, `decode`, `decode_posit16`,
  `decode_posit32`, `default_action`, `denom`, `description`, `diff`, `dispatch_table`,
  `div`, `div_checked`, `div_dim`, `dtype`, `elem_ty`, `elements`, `emit_mlir`, `empty`,
  `encode`, `encode_program`, `encode_vec_sum_delta_frame`, `entry_block`,
  `entry_block_mut`, `eq`, `errno_result`, `eval_kasm`, `evaluate`, `evaluate_all`,
  `evaluate_at`, `event_count`, `execute`, `execute_batch_i64`, `execute_i64_slots`,
  `execute_program_interpreter`, `execute_program_interpreter_with_context`,
  `execute_program_pipeline`, `execute_program_pipeline_with_context`, `execute_tensor`,
  `execute_tensor_polymorphic`, `execute_tensor_posit16`, `execute_tensor_posit32`,
  `execute_tensor_rational`, `execute_tool_cell_batch`,
  `execute_tool_cell_batch_groups`, `execute_vec_sum_delta`,
  `execute_vec_sum_delta_checked`, `execute_with_fractal`, `exp`, `f64_abs`, `f64_add`,
  `f64_div`, `f64_exp`, `f64_from_i64`, `f64_ln`, `f64_max`, `f64_min`, `f64_mul`,
  `f64_neg`, `f64_sqrt`, `f64_sub`, `f64_to_i64`, `filter_sum`, `finish`, `flush`,
  `force`, `forge_dialect_tag`, `forge_dialect_version`, `fractal_call`,
  `fractional_part`, `fragment_hash_hex`, `from_bits`,
  `from_bool`, `from_bytes`, `from_days`, `from_error`, `from_f64`, `from_hours`,
  `from_i48`, `from_imm`, `from_int`, `from_micros`, `from_millis`, `from_minutes`,
  `from_mlir`, `from_nanos`, `from_rational`, `from_raw`, `from_scalar`, `from_seconds`,
  `from_strategy`, `from_u8`, `from_vec_handle`, `from_wit`, `fuel`,
  `function_hash_hex`, `gelu_tanh`, `general_6node_self_host_program`,
  `generalized_score_program`, `get`, `grad`, `has_only_bounded_control_source`,
  `has_pending`, `hash_bytes_program`, `hash_mlir_canonical`, `hash_mlir_canonical_hex`,
  `hash_program`, `hash64`, `high_column`, `hostcall_abi_v0`, `iadd`, `iconst`,
  `identity`, `imm`, `import_hash_hex`, `imul`, `infer_scalar_ty`, `infer_ty`, `input`,
  `input_f64`, `input_hash_hex`, `input_types`, `input_vec`, `inputs`, `integer_part`,
  `into_inner`, `into_samples`, `is_binary`, `is_dimensionless`, `is_empty`, `is_err`,
  `is_float`, `is_integer`, `is_numeric`, `is_numeric_composite`, `is_ok`,
  `is_pure_source`, `is_valid`, `ishl`, `isub`, `iter`, `job_read_projection_program`,
  `kasm_input_types`, `kasm_interop_proof_projection_json`, `kasm_output_types`,
  `kernel_project_program`, `label`, `language_contract_hash_hex`,
  `language_contract_source`, `lazy`, `le`, `len`, `ln`, `low_column`,
  `lower_kasm_to_ssa`, `lower_mlir_func_to_kasm`, `lower_to_compute_ir`, `lt`, `lzcnt`,
  `matmul`, `matmul_posit16`, `matmul_posit32`, `matmul_rational`, `matrix`, `max`,
  `max_bounds`, `max_drawdown`, `memoize`, `memory_scope_handle`, `mid_price`, `millis`,
  `min`, `min_bounds`, `mlir_func_proof_envelope`, `mod_checked`,
  `model_provider_scope_handle`, `module_hashes`, `mul`, `mul_dim`, `mul_posit16`,
  `mul_posit32`, `mul_rational`, `name`, `nanos`, `needs_external_backend`,
  `needs_source_dialect`, `neg`, `new`, `nodes`, `not`, `num`, `open_column`,
  `operands`, `optimize_program_v0`, `optimize_program_v0_with_context`, `or`,
  `outer_product_mul`, `output`, `output_hash_hex`, `output_types`, `outputs`,
  `pack_node_to_i64`, `pack_program_to_vec_i64`, `param`, `parse`,
  `parse_app_section_registry_v0`, `parse_mlir`, `parse_program_v0`,
  `parse_tool_cell_registry_v0`, `parse_wit_component_contract`, `partial_eval_report`,
  `partial_evaluate`, `pdep`, `peephole`, `period_ns`, `pext`, `pipeline`, `point`,
  `popcnt`, `powf`, `powi`, `pretty_print`, `produces_value`, `projection`, `promote_numeric`,
  `proof_hash_hex`, `proof_ledger_entry`, `proof_ledger_projection_json`,
  `proof_projection_json`, `prove_deterministic`, `prove_no_ub`, `prove_pure`,
  `prove_terminating`, `push_bar`, `rank`, `raw`, `reduce_add`, `reduce_mul`,
  `reduce_sum`, `register_callee`, `register_eval`, `relu`,
  `require_deterministic_for_swarm`, `require_pure_for_caching`,
  `require_terminating_for_realtime`, `reshape`, `resolve`, `result_ty`, `ret`,
  `reverse_bits`, `rewrite_fixpoint`, `rewrite_program`, `rewrite_report`, `rows`,
  `rule_count`, `run_threaded`, `sample_hash_hex`, `samples`, `sat_add`, `sat_sub`,
  `saturating_abs`, `saturating_add`, `saturating_mul`, `saturating_neg`,
  `saturating_sub`, `scalar`, `scalar_ty`, `scan_column`, `sealed`, `seconds`,
  `seed_rewrites`, `seen`, `select_backend`, `select_i64`, `select_where`,
  `semantic_fingerprint`, `semantic_fingerprint_hex`, `sha1`, `shl`, `shr`, `sig`,
  `sigmoid`, `simplified`, `simplify`, `slice_by_time`, `slippage`, `sma_close`,
  `softmax`, `source_hash_hex`, `spread`, `sqrt`, `square`, `static_output`, `stats`,
  `strict_f32_scalar_ltr`, `structural_hash_hex`, `sub`, `sum_along_last_axis`,
  `summary`, `tanh`, `target`, `ticks_seen`, `to_bits`, `to_byte`, `to_f64`,
  `to_f64_lossy`, `to_kasm_ty`, `tool_cell_output_artifact_json`, `top_asks`,
  `top_bids`, `total_ask_size`, `total_bid_size`, `try_distill_ffn_block`,
  `try_execute_i64_inline`, `timestamp_column`, `twap_slice`, `ty`, `tzcnt`,
  `ui_intent_transition_program`, `ui_projection_handle`, `unit_dim`, `ushr`, `v_abs`,
  `v_add`, `v_and`, `v_bit_flip`, `v_broadcast`, `v_concat`, `v_eq`, `v_get`, `v_len`,
  `v_max`, `v_min`, `v_mul`, `v_neg`, `v_or`, `v_range`, `v_reverse`, `v_sub`, `v_sum`,
  `v_xor`, `v0`, `variables`, `vec`, `vec_i64_state_hash`, `vector`, `verify`,
  `verify_program`, `verify_tensor`, `verify_uncrossed`, `volume_column`, `vwap_slice`,
  `walk_buy`, `walk_sell`, `wit_export_proof_envelope`, `with_binding`,
  `with_capability`, `with_capacity`, `with_default`, `with_hostcall`, `with_max_depth`,
  `with_method`
- public enum variants:
  `Action::Buy`, `Action::ClosePosition`, `Action::Hold`, `Action::Sell`,
  `ColumnarError::BadColumnIdx`, `ColumnarError::BadRowArity`,
  `ColumnarError::BadRowIdx`, `DistillError::ActivationNotIdentityOnSamples`,
  `DistillError::InsufficientSamples`, `DistillError::PatternNotMatched`,
  `DistillError::ShortcutDiverges`, `DistillError::Tensor`, `ExecutionError::BadRange`,
  `ExecutionError::EmptyRange`, `ExecutionError::InsufficientVolume`,
  `ExecutionError::Ohlcv`, `F64SubOp::Abs`, `F64SubOp::Add`, `F64SubOp::DivChecked`,
  `F64SubOp::Exp`, `F64SubOp::FromI64`, `F64SubOp::Ln`, `F64SubOp::Max`,
  `F64SubOp::Min`, `F64SubOp::Mul`, `F64SubOp::Neg`, `F64SubOp::Sqrt`, `F64SubOp::Sub`,
  `F64SubOp::ToI64`, `ForgeBinaryOp::Add`, `ForgeBinaryOp::And`, `ForgeBinaryOp::Div`,
  `ForgeBinaryOp::Eq`, `ForgeBinaryOp::Ge`, `ForgeBinaryOp::Gt`, `ForgeBinaryOp::Le`,
  `ForgeBinaryOp::Lt`, `ForgeBinaryOp::Mul`, `ForgeBinaryOp::Ne`, `ForgeBinaryOp::Or`,
  `ForgeBinaryOp::Pow`, `ForgeBinaryOp::Sub`, `ForgeCapabilityKind::ArtifactHash`,
  `ForgeCapabilityKind::EventSchema`, `ForgeCapabilityKind::FileHash`,
  `ForgeCapabilityKind::GpuBudget`, `ForgeCapabilityKind::MemoryScope`,
  `ForgeCapabilityKind::ModelProviderScope`, `ForgeCapabilityKind::NetworkSource`,
  `ForgeCapabilityKind::RawFilesystem`, `ForgeCapabilityKind::RawNetwork`,
  `ForgeCapabilityKind::Secret`, `ForgeCapabilityKind::UiProjection`,
  `ForgeConstraintSpec::Approx`, `ForgeConstraintSpec::Assert`,
  `ForgeConstraintSpec::Bounds`, `ForgeConstraintSpec::Finite`, `ForgeExpr::Binary`,
  `ForgeExpr::Call`, `ForgeExpr::Scalar`, `ForgeExpr::Unary`, `ForgeExpr::Var`,
  `ForgeHostCall::ArtifactReadHash`, `ForgeHostCall::CsvProfileTiny`,
  `ForgeHostCall::HashBytes`, `ForgeHostCall::JobReadProjection`,
  `ForgeHostCall::KernelProject`, `ForgeHostCall::MemoryRecall`,
  `ForgeHostCall::NetworkFetchSourceId`, `ForgeHostCall::RawFilesystem`,
  `ForgeHostCall::RawNetwork`, `ForgeHostCall::ReadCapability`,
  `ForgeHostCall::ReadSecret`, `ForgeHostCall::ToolCellRun`,
  `ForgeHostCall::UiEmitProjection`, `ForgeHostCall::UiProjectEvent`,
  `ForgeIrKernelClass::Control`, `ForgeIrKernelClass::Elementwise`,
  `ForgeIrKernelClass::Field`, `ForgeIrKernelClass::FunctionCall`,
  `ForgeIrKernelClass::GatherScatter`, `ForgeIrKernelClass::Graph`,
  `ForgeIrKernelClass::Reduction`, `ForgeIrKernelClass::Sampling`,
  `ForgeIrKernelClass::Scalar`, `ForgeIrKernelClass::Scan`,
  `ForgeIrKernelClass::Selection`, `ForgeIrKernelClass::SymbolicMath`, `ForgeIrKernelClass::Table`,
  `ForgeIrKernelClass::Window`, `ForgeIrOp::Binary`, `ForgeIrOp::Call`,
  `ForgeIrOp::Constant`, `ForgeIrOp::Constraint`, `ForgeIrOp::Emit`, `ForgeIrOp::Input`,
  `ForgeIrOp::Let`, `ForgeIrOp::Literal`, `ForgeIrOp::Sample`, `ForgeIrOp::Unary`,
  `ForgeLoweringTarget::CpuSimd`, `ForgeLoweringTarget::Cuda`,
  `ForgeLoweringTarget::Interpreter`, `ForgeLoweringTarget::WgslRhi`,
  `ForgeOpcode::CsvProfileTiny`, `ForgeOpcode::EmitProjection`, `ForgeOpcode::End`,
  `ForgeOpcode::HashTop`, `ForgeOpcode::JobReadProjection`,
  `ForgeOpcode::KernelProject`, `ForgeOpcode::PushBytes`, `ForgeOpcode::PushCapability`,
  `ForgeOpcode::PushText`, `ForgeOpcode::RawFilesystemProbe`,
  `ForgeOpcode::RawNetworkProbe`, `ForgeOpcode::ReadCapability`,
  `ForgeOpcode::ToolCellProjectTiny`, `ForgeOpcode::UiIntentTransition`,
  `ForgeOutputKind::Artifact`, `ForgeOutputKind::Field`, `ForgeOutputKind::Graph`,
  `ForgeOutputKind::MeshParams`, `ForgeOutputKind::Scalar`, `ForgeOutputKind::Score`,
  `ForgeOutputKind::Sdf`, `ForgeOutputKind::Table`, `ForgeOutputKind::Timeseries`,
  `ForgeOutputKind::Vector`, `ForgePrecision::F32`, `ForgePrecision::F64`,
  `ForgePropertyKind::Conserves`, `ForgePropertyKind::Even`, `ForgePropertyKind::Finite`,
  `ForgePropertyKind::Homogeneous`, `ForgePropertyKind::Idempotent`,
  `ForgePropertyKind::Involutive`, `ForgePropertyKind::MonotoneDecreasing`,
  `ForgePropertyKind::MonotoneIncreasing`, `ForgePropertyKind::Odd`,
  `ForgePropertyKind::PermutationInvariant`, `ForgePropertyKind::Range`,
  `ForgePropertyKind::ScaleInvariant`, `ForgePropertyKind::Translation`,
  `ForgeRuntimeMode::Auto`, `ForgeRuntimeMode::Off`, `ForgeRuntimeMode::Optional`,
  `ForgeRuntimeMode::Required`, `ForgeScalarTy::Bool`, `ForgeScalarTy::F32`,
  `ForgeScalarTy::F64`, `ForgeScalarTy::I32`, `ForgeScalarTy::I64`,
  `ForgeScalarTy::U32`, `ForgeScalarTy::U64`, `ForgeScalarValue::Bool`,
  `ForgeScalarValue::F32`, `ForgeScalarValue::F64`, `ForgeScalarValue::I32`,
  `ForgeScalarValue::I64`, `ForgeScalarValue::U32`, `ForgeScalarValue::U64`,
  `ForgeScheduleLayout::Aos`, `ForgeScheduleLayout::Page`,
  `ForgeScheduleLayout::Shared`, `ForgeScheduleLayout::Soa`,
  `ForgeScheduleLayout::Stream`, `ForgeScheduleLayout::Tile`,
  `ForgeSparseLayout::HashGrid`, `ForgeSparseLayout::PageTable`,
  `ForgeSparseLayout::SparseGrid`, `ForgeTaintKind::Credential`,
  `ForgeTaintKind::Public`, `ForgeTaintKind::Secret`, `ForgeTaintKind::UserData`,
  `ForgeTransformKind::Adjoint`, `ForgeTransformKind::Batch`,
  `ForgeTransformKind::ConstraintSolve`, `ForgeTransformKind::Grad`,
  `ForgeTransformKind::Hessian`, `ForgeTransformKind::Jacobian`,
  `ForgeTransformKind::Jit`, `ForgeTransformKind::LeastSquares`,
  `ForgeTransformKind::Optimize`, `ForgeTransformKind::Parallel`,
  `ForgeTransformKind::Vectorize`, `ForgeType::Array`, `ForgeType::AssumptionSet`, `ForgeType::Column`,
  `ForgeType::Complex`, `ForgeType::DomEdge`, `ForgeType::DomNode`, `ForgeType::Field`,
  `ForgeType::Graph`, `ForgeType::HeapObject`, `ForgeType::Interval`, `ForgeType::Mat`, `ForgeType::MathDomain`,
  `ForgeType::MemoryMap`, `ForgeType::Piecewise`, `ForgeType::Polynomial`, `ForgeType::Scalar`, `ForgeType::Snapshot`,
  `ForgeType::SolutionSet`, `ForgeType::SparseField`, `ForgeType::SymbolicExpr`, `ForgeType::Table`, `ForgeType::Taint`, `ForgeType::Tensor`,
  `ForgeType::Uncertainty`, `ForgeType::Vec`, `ForgeUnaryOp::Neg`, `ForgeUnaryOp::Not`,
  `ForgeVmError::CapabilityDenied`, `ForgeVmError::FuelExhausted`,
  `ForgeVmError::MemoryLimitExceeded`, `ForgeVmError::MissingEnd`,
  `ForgeVmError::OutputLimitExceeded`, `ForgeVmError::Parse`,
  `ForgeVmError::StackUnderflow`, `ForgeVmError::VerifierDenied`,
  `ForgeVmStatus::FuelExhausted`, `ForgeVmStatus::MemoryLimitExceeded`,
  `ForgeVmStatus::Ok`, `ForgeVmStatus::RuntimeError`, `ForgeVmStatus::VerifierDenied`,
  `Indicator::AlwaysFalse`, `Indicator::AlwaysTrue`, `Indicator::And`,
  `Indicator::AtrAbove`, `Indicator::Not`, `Indicator::Or`, `Indicator::PriceAbove`,
  `Indicator::PriceBelow`, `Indicator::SmaBearishCross`, `Indicator::SmaBullishCross`,
  `InteropError::BadInteger`, `InteropError::Kasm`, `InteropError::MissingFunction`,
  `InteropError::MissingWorld`, `InteropError::Mlir`, `InteropError::TooManySlots`,
  `InteropError::UnsupportedOp`, `InteropError::UnsupportedType`, `InteropError::Wit`,
  `JitError::BadInputLength`, `JitError::BadOutputCount`, `JitError::Compile`,
  `JitError::ExternalTarget`, `JitError::UnsupportedPlatform`, `KasmAbiType::Bool`,
  `KasmAbiType::F64`, `KasmAbiType::S32`, `KasmAbiType::S64`, `KasmAbiType::String`,
  `KasmAbiType::U32`, `KasmAbiType::U64`, `KasmAbiType::Unit`,
  `KasmAbiType::Unsupported`, `KasmDeltaError::BadPatch`,
  `KasmDeltaError::BadPreviousOutput`, `KasmDeltaError::FullReplayMismatch`,
  `KasmDeltaError::Kasm`, `KasmDeltaError::UnsupportedProgram`,
  `KasmError::BadF64SubOp`, `KasmError::BadFooter`, `KasmError::BadInputLength`,
  `KasmError::BadInputSlot`, `KasmError::BadLength`, `KasmError::BadMagic`,
  `KasmError::BadMultiMethod`, `KasmError::BadNodeCount`, `KasmError::BadOp`,
  `KasmError::BadReduceCount`, `KasmError::BadRef`, `KasmError::BadTarget`,
  `KasmError::BadType`, `KasmError::BadVersion`, `KasmError::ComposeArity`,
  `KasmError::ComposeType`, `KasmError::ExternalTarget`, `KasmError::FuelTooSmall`,
  `KasmError::OutputCount`, `KasmError::TooManySlots`, `KasmError::Truncated`,
  `KasmError::TypeMismatch`, `KasmError::UnsupportedV1OpInScalarInterpreter`,
  `KasmError::ValueTypeMismatch`, `KernelFamily::Avx2`, `KernelFamily::Avx512`,
  `KernelFamily::CudaCublas`, `KernelFamily::Metal`, `KernelFamily::Rocm`,
  `KernelFamily::Scalar`, `LoweringError::BadProgram`, `LoweringError::UnsupportedOp`,
  `MlirError::BadFooter`, `MlirError::BadHeader`, `MlirError::BadIndex`,
  `MlirError::BadInteger`, `MlirError::BadTarget`, `MlirError::BadType`,
  `MlirError::Kasm`, `MlirError::NodeOverflow`, `MlirError::Syntax`,
  `MlirError::UnknownOp`, `OhlcvError::BadIndex`, `OhlcvError::BadPeriod`,
  `OhlcvError::EmptyStore`, `OhlcvError::InvalidBar`, `Op::Adaptive`, `Op::AddI64`,
  `Op::AndBool`, `Op::BitAndI64`, `Op::BitFlipI64`, `Op::BitOrI64`, `Op::BitXorI64`,
  `Op::ByteswapI64`, `Op::ClampI64`, `Op::Comptime`, `Op::Cond`, `Op::ConstF64`,
  `Op::ConstI64`, `Op::DivI64Checked`, `Op::EqI64`, `Op::Eval`, `Op::F64Op`,
  `Op::Force`, `Op::Fori`, `Op::Fractal`, `Op::Grad`, `Op::Hash64`, `Op::Input`,
  `Op::Lazy`, `Op::LeI64`, `Op::LtI64`, `Op::LzcntI64`, `Op::MaxI64`, `Op::Memoize`,
  `Op::MinI64`, `Op::ModI64Checked`, `Op::MulI64`, `Op::NegI64`, `Op::NotBool`,
  `Op::OrBool`, `Op::Output`, `Op::PdepI64`, `Op::PextI64`, `Op::Pipeline`, `Op::Pmap`,
  `Op::PopcntI64`, `Op::Reduce`, `Op::ReduceAddI64`, `Op::ReduceMulI64`,
  `Op::ReverseBitsI64`, `Op::SatAddI64`, `Op::SatSubI64`, `Op::Scan`, `Op::SelectI64`,
  `Op::ShlI64`, `Op::ShrI64`, `Op::SubI64`, `Op::TzcntI64`, `Op::VAbsI64`,
  `Op::VAddI64`, `Op::VAndI64`, `Op::VBitFlipI64`, `Op::VBroadcastI64`,
  `Op::VConcatI64`, `Op::VEqI64`, `Op::VGetI64`, `Op::VLenI64`, `Op::Vmap`,
  `Op::VMaxI64`, `Op::VMinI64`, `Op::VMulI64`, `Op::VNegI64`, `Op::VOrI64`,
  `Op::VRangeI64`, `Op::VReverseI64`, `Op::VSubI64`, `Op::VSumI64`, `Op::VXorI64`,
  `Op::WhileLoop`, `OrderBookError::CrossedBook`,
  `OrderBookError::InsufficientLiquidity`, `OrderBookError::NegativeSize`,
  `OrderBookEvent::AddAsk`, `OrderBookEvent::AddBid`, `OrderBookEvent::RemoveAsk`,
  `OrderBookEvent::RemoveBid`, `OrderBookEvent::SetAsk`, `OrderBookEvent::SetBid`,
  `Pattern::Any`, `Pattern::Capture`, `Pattern::LiteralI64`, `Pattern::Op`,
  `ProofError::BaseVerify`, `ProofError::DisallowedOp`,
  `ProofError::StructureViolation`, `RankError::BadAxis`, `RankError::BadReshape`,
  `RankError::IncompatibleBroadcast`, `RankError::ShapeMismatch`,
  `ReductionTree::Avx2Tile`, `ReductionTree::CudaBlock`, `ReductionTree::GpuWarp`,
  `ReductionTree::Ltr`, `ReductionTree::Pairwise`, `Replace::LiteralI64`, `Replace::Op`,
  `Replace::Slot`, `RoundMode::Down`, `RoundMode::NearestEven`, `RoundMode::TowardZero`,
  `RoundMode::Up`, `SelfHostError::DepthExceeded`, `SelfHostError::InvalidEvalBytes`,
  `SelfHostError::InvalidProgram`, `SelfHostError::Io`, `SelfHostError::Kasm`,
  `SelfHostError::UnknownProgram`, `Side::Buy`, `Side::Sell`, `SsaOp::Band`,
  `SsaOp::Bor`, `SsaOp::Bxor`, `SsaOp::Const`, `SsaOp::Hash64`, `SsaOp::Iadd`,
  `SsaOp::Imul`, `SsaOp::Ishl`, `SsaOp::Isub`, `SsaOp::Param`, `SsaOp::Return`,
  `SsaOp::Ushr`, `SsaVerifyError::InvalidParam`, `SsaVerifyError::MissingTerminator`,
  `SsaVerifyError::MultipleDef`, `SsaVerifyError::UseBeforeDef`, `Target::Auto`,
  `Target::Cpu`, `Target::Gpu`, `Target::Kernel`, `Target::Qpu`,
  `TensorError::BackrefOutOfBounds`, `TensorError::BadAxis`, `TensorError::BadFooter`,
  `TensorError::BadMagic`, `TensorError::BadNodeCount`, `TensorError::BadSlot`,
  `TensorError::BadVersion`, `TensorError::ConstPoolOverflow`,
  `TensorError::DimOutOfRange`, `TensorError::DtypeMismatch`,
  `TensorError::FuelTooSmall`, `TensorError::NoOutput`, `TensorError::ReservedNonZero`,
  `TensorError::ShapeMismatch`, `TensorError::ShapeNonZeroPastRank`,
  `TensorError::ShapeRankInvalid`, `TensorError::TooManyInputs`,
  `TensorError::TooManyOutputs`, `TensorError::Truncated`, `TensorError::TruncatedNode`,
  `TensorError::UnknownDtype`, `TensorError::UnknownOp`, `TensorOp::AddF32`,
  `TensorOp::AddPosit16`, `TensorOp::AddPosit32`, `TensorOp::AddRational`,
  `TensorOp::Const`, `TensorOp::GeluTanhF32`, `TensorOp::Input`, `TensorOp::MatmulTile`,
  `TensorOp::MatmulTilePosit16`, `TensorOp::MatmulTilePosit32`,
  `TensorOp::MatmulTileRational`, `TensorOp::MulF32`, `TensorOp::MulPosit16`,
  `TensorOp::MulPosit32`, `TensorOp::MulRational`, `TensorOp::Output`,
  `TensorOp::ReduceSumAxis`, `TensorOp::ReluF32`, `TensorOp::SigmoidF32`,
  `TensorOp::Softmax`, `TensorOp::TanhF32`, `TensorTy::F32`, `TensorTy::Posit16`,
  `TensorTy::Posit32`, `TensorTy::Rational`, `TensorValue::F32`, `TensorValue::Posit16`,
  `TensorValue::Posit32`, `TensorValue::Rational`, `Ty::Bool`, `Ty::F64`, `Ty::I64`,
  `Ty::VecI64`
- public struct fields:
  `ApplyOutcome.fired_rules`, `ApplyOutcome.rewrites_applied`, `BacktestSummary.buys`,
  `BacktestSummary.closes`, `BacktestSummary.final_pnl`,
  `BacktestSummary.final_position`, `BacktestSummary.holds`, `BacktestSummary.sells`,
  `BarResampler::<tuple_or_marker>`, `Block::<tuple_or_marker>`,
  `BlockId::<tuple_or_marker>`, `ColumnStore::<tuple_or_marker>`, `Decoded16.frac`,
  `Decoded16.frac_bits`, `Decoded16.is_nar`, `Decoded16.is_zero`, `Decoded16.scale`,
  `Decoded16.sign`, `Decoded32.frac`, `Decoded32.frac_bits`, `Decoded32.is_nar`,
  `Decoded32.is_zero`, `Decoded32.scale`, `Decoded32.sign`,
  `Deterministic::<tuple_or_marker>`, `DistilledShortcut.max_abs_diff_observed`,
  `DistilledShortcut.original_node_count`, `DistilledShortcut.samples_validated`,
  `DistilledShortcut.shortcut`, `DistilledShortcut.shortcut_node_count`,
  `DistillTensorConfig.min_samples`, `DistillTensorConfig.tolerance`,
  `Duration::<tuple_or_marker>`, `ExecutionResult.avg_fill_price`,
  `ExecutionResult.fills`, `ExecutionResult.slippage_vs_first_close`,
  `ExecutionResult.total_qty`, `Fill.price`, `Fill.size`, `ForgeAppRegistry.cells`,
  `ForgeAppRegistry.graph_jsonl`, `ForgeAppRegistry.registry_hash`,
  `ForgeAppRegistry.section_count`, `ForgeAppRegistry.sensitive_command_count`,
  `ForgeArtifactHandoff.items`, `ForgeBackendSelection.reason`,
  `ForgeBackendSelection.requested`, `ForgeBackendSelection.selected`,
  `ForgeBackendSelection.selector_hash`, `ForgeBounds.max`, `ForgeBounds.min`,
  `ForgeBytecodeProgram.capabilities`, `ForgeBytecodeProgram.deterministic`,
  `ForgeBytecodeProgram.expected_output_schema`, `ForgeBytecodeProgram.hostcalls`,
  `ForgeBytecodeProgram.name`, `ForgeBytecodeProgram.ops`,
  `ForgeBytecodeProgram.version`, `ForgeCapability.content_hash`,
  `ForgeCapability.kind`, `ForgeCapability.limit_bytes`, `ForgeCapability.scope`,
  `ForgeCapability.sealed_hash`, `ForgeCapabilityBinding.bytes`,
  `ForgeCapabilityBinding.sealed_hash`, `ForgeCompiledToolCell.graph_hash`,
  `ForgeCompiledToolCell.host_context`, `ForgeCompiledToolCell.manifest_hash`,
  `ForgeCompiledToolCell.program`, `ForgeComputeIrModule.buffers`,
  `ForgeComputeIrModule.constraints`, `ForgeComputeIrModule.contract_hash`,
  `ForgeComputeIrModule.estimated_memory_bytes`, `ForgeComputeIrModule.estimated_steps`,
  `ForgeComputeIrModule.functions`, `ForgeComputeIrModule.hostcall_contract`,
  `ForgeComputeIrModule.ir_hash`, `ForgeComputeIrModule.kernel_classes`,
  `ForgeComputeIrModule.language_contract_hash`, `ForgeComputeIrModule.module_name`,
  `ForgeComputeIrModule.module_version`, `ForgeComputeIrModule.nodes`,
  `ForgeComputeIrModule.outputs`, `ForgeComputeIrModule.runtime_contract`,
  `ForgeComputeIrModule.property_contract`, `ForgeComputeIrModule.samples`,
  `ForgeComputeIrModule.schedule_contract`,
  `ForgeComputeIrModule.schema`, `ForgeComputeIrModule.source_hash`,
  `ForgeComputeIrModule.transform_contract`, `ForgeConstSpec.name`, `ForgeConstSpec.ty`,
  `ForgeConstSpec.unit`, `ForgeConstSpec.value`, `ForgeCostSpec.max_memory_mb`,
  `ForgeCostSpec.max_steps`, `ForgeCostSpec.parallelism`, `ForgeCostSpec.precision`,
  `ForgeFunctionArg.name`, `ForgeFunctionArg.ty`, `ForgeFunctionSpec.args`,
  `ForgeFunctionSpec.body`, `ForgeFunctionSpec.name`, `ForgeFunctionSpec.result_ty`,
  `ForgeHostContext.bindings`, `ForgeImportSpec.dialect`, `ForgeImportSpec.hash`,
  `ForgeImportSpec.name`,
  `ForgeIrBufferLayout.byte_width`, `ForgeIrBufferLayout.name`,
  `ForgeIrBufferLayout.role`, `ForgeIrBufferLayout.ty`, `ForgeIrFunction.args`,
  `ForgeIrFunction.hash`, `ForgeIrFunction.name`, `ForgeIrFunction.node_range`,
  `ForgeIrFunction.result_ty`, `ForgeIrFunction.return_value`, `ForgeIrNode.hash`,
  `ForgeIrNode.id`, `ForgeIrNode.inputs`, `ForgeIrNode.kernel_class`, `ForgeIrNode.op`,
  `ForgeIrNode.ty`, `ForgeIrValueId::<tuple_or_marker>`,
  `ForgeModuleHashes.bounds_hash`, `ForgeModuleHashes.constants_hash`,
  `ForgeModuleHashes.contract_hash`, `ForgeModuleHashes.fragment_hash`,
  `ForgeModuleHashes.functions_hash`, `ForgeModuleHashes.imports_hash`,
  `ForgeModuleHashes.input_hash`, `ForgeModuleHashes.output_hash`,
  `ForgeModuleHashes.proof_hash`, `ForgeModuleHashes.source_hash`,
  `ForgeModuleHashes.type_hash`, `ForgeModuleHashes.unit_hash`,
  `ForgeModuleSpec.artifact_handoff`, `ForgeModuleSpec.constants`,
  `ForgeModuleSpec.constraints`, `ForgeModuleSpec.cost`, `ForgeModuleSpec.functions`,
  `ForgeModuleSpec.hostcalls`, `ForgeModuleSpec.imports`, `ForgeModuleSpec.inputs`,
  `ForgeModuleSpec.name`, `ForgeModuleSpec.outputs`, `ForgeModuleSpec.program`,
  `ForgeModuleSpec.properties`, `ForgeModuleSpec.runtime`, `ForgeModuleSpec.samples`,
  `ForgeModuleSpec.schedules`, `ForgeModuleSpec.transforms`, `ForgeModuleSpec.version`,
  `ForgePropertySpec.kind`, `ForgePropertySpec.name`, `ForgePropertySpec.numeric_args`,
  `ForgePropertySpec.params`, `ForgePropertySpec.target`, `ForgePropertySpec.tolerance`,
  `ForgeOptimizerReport.changed`, `ForgeOptimizerReport.fuel_after`,
  `ForgeOptimizerReport.fuel_before`, `ForgeOptimizerReport.fused_capability_hash_ops`,
  `ForgeOptimizerReport.fused_hash_ops`, `ForgeOptimizerReport.optimized_program_hash`,
  `ForgeOptimizerReport.optimizer_hash`, `ForgeOptimizerReport.original_program_hash`,
  `ForgeOutputSpec.handoff`, `ForgeOutputSpec.name`, `ForgeOutputSpec.ty`,
  `ForgeOutputSpec.unit`, `ForgeParamSpec.max`, `ForgeParamSpec.min`,
  `ForgeParamSpec.name`, `ForgeParamSpec.nominal`, `ForgeParamSpec.ty`,
  `ForgeParamSpec.unit`, `ForgePipelineOutput.backend`,
  `ForgePipelineOutput.optimized_program_hash`, `ForgePipelineOutput.optimizer`,
  `ForgePipelineOutput.original_program_hash`, `ForgePipelineOutput.proof_projection`,
  `ForgePipelineOutput.verifier`, `ForgePipelineOutput.vm_output`,
  `ForgeProgramEmit.expr`, `ForgeProgramEmit.name`, `ForgeProgramEmit.ty`,
  `ForgeProgramLet.expr`, `ForgeProgramLet.name`, `ForgeProgramSpec.emits`,
  `ForgeProgramSpec.lets`, `ForgeProofLedgerEntry.backend`,
  `ForgeProofLedgerEntry.capability_hash`, `ForgeProofLedgerEntry.fuel_used`,
  `ForgeProofLedgerEntry.ledger_hash`, `ForgeProofLedgerEntry.memory_peak`,
  `ForgeProofLedgerEntry.program_hash`, `ForgeProofLedgerEntry.proof_hash`,
  `ForgeProofLedgerEntry.sequence`, `ForgeProofLedgerEntry.status`,
  `ForgeProofLedgerEntry.verifier_hash`, `ForgeRunProof.backend`,
  `ForgeRunProof.bytecode_hash`, `ForgeRunProof.capability_hash`,
  `ForgeRunProof.deterministic_replay_hash`, `ForgeRunProof.fuel_used`,
  `ForgeRunProof.hostcall_hash`, `ForgeRunProof.input_hash`,
  `ForgeRunProof.memory_peak`, `ForgeRunProof.output_hash`,
  `ForgeRunProof.program_hash`, `ForgeRunProof.proof_hash`,
  `ForgeRunProof.verifier_hash`, `ForgeRuntimeSpec.cpu_simd`, `ForgeRuntimeSpec.cuda`,
  `ForgeRuntimeSpec.lowering`, `ForgeRuntimeSpec.memory_layout`,
  `ForgeRuntimeSpec.sparse_layout`, `ForgeSampleCase.expect_output`,
  `ForgeSampleCase.expect_value`, `ForgeSampleCase.givens`, `ForgeSampleCase.name`,
  `ForgeSampleCase.seed`, `ForgeSampleCase.tolerance`, `ForgeScheduleSpec.algorithm`,
  `ForgeScheduleSpec.gpu`, `ForgeScheduleSpec.layout`, `ForgeScheduleSpec.name`,
  `ForgeScheduleSpec.target`, `ForgeScheduleSpec.tile`, `ForgeScheduleSpec.vectorize`,
  `ForgeSealedHostcallSpec.capability_hash`, `ForgeSealedHostcallSpec.hostcall`,
  `ForgeSealedHostcallSpec.name`, `ForgeToolCellBatchOutput.denied_count`,
  `ForgeToolCellBatchOutput.graph_hash`, `ForgeToolCellBatchOutput.ledger_root_hash`,
  `ForgeToolCellBatchOutput.ok_count`, `ForgeToolCellBatchOutput.projection_json`,
  `ForgeToolCellBatchOutput.records`, `ForgeToolCellBatchOutput.tool_count`,
  `ForgeToolCellBatchRecord.command`, `ForgeToolCellBatchRecord.error`,
  `ForgeToolCellBatchRecord.ledger_hash`, `ForgeToolCellBatchRecord.output_hash`,
  `ForgeToolCellBatchRecord.program_hash`, `ForgeToolCellBatchRecord.projection_json`,
  `ForgeToolCellBatchRecord.proof_hash`, `ForgeToolCellBatchRecord.ranked_action_count`,
  `ForgeToolCellBatchRecord.selected_evidence_count`, `ForgeToolCellBatchRecord.status`,
  `ForgeToolCellBatchRecord.tool_id`, `ForgeToolCellRegistry.cells`,
  `ForgeToolCellRegistry.default_engine`, `ForgeToolCellRegistry.denied`,
  `ForgeToolCellRegistry.input_schema_hash`, `ForgeToolCellRegistry.output_schema_hash`,
  `ForgeToolCellRegistry.permissions`, `ForgeToolCellRegistry.registry_hash`,
  `ForgeToolCellRegistry.schema_version`, `ForgeToolCellSpec.command`,
  `ForgeToolCellSpec.denied`, `ForgeToolCellSpec.focus`, `ForgeToolCellSpec.id`,
  `ForgeToolCellSpec.input_schema_hash`, `ForgeToolCellSpec.output_schema_hash`,
  `ForgeToolCellSpec.permissions`, `ForgeToolCellSpec.query`, `ForgeTransformSpec.kind`,
  `ForgeTransformSpec.name`, `ForgeTransformSpec.target`, `ForgeUnitDim.a`,
  `ForgeUnitDim.cd`, `ForgeUnitDim.k`, `ForgeUnitDim.kg`, `ForgeUnitDim.m`,
  `ForgeUnitDim.mol`, `ForgeUnitDim.s`, `ForgeVerifierReport.capability_summary`,
  `ForgeVerifierReport.declared_hostcalls`, `ForgeVerifierReport.errors`,
  `ForgeVerifierReport.max_fuel`, `ForgeVerifierReport.max_memory_bytes`,
  `ForgeVerifierReport.ok`, `ForgeVerifierReport.program_hash`,
  `ForgeVerifierReport.verifier_hash`, `ForgeVerifierReport.warnings`,
  `ForgeVmConfig.backend`, `ForgeVmConfig.forbidden_opcodes`, `ForgeVmConfig.max_fuel`,
  `ForgeVmConfig.max_input_bytes`, `ForgeVmConfig.max_memory_bytes`,
  `ForgeVmConfig.max_output_bytes`, `ForgeVmOutput.bytes`, `ForgeVmOutput.fuel_used`,
  `ForgeVmOutput.memory_peak`, `ForgeVmOutput.preview`, `ForgeVmOutput.proof`,
  `ForgeVmOutput.status`, `Inst.op`, `Inst.result`, `JitKernel.arg_count`,
  `JitKernel.func_ptr`, `JitKernel.output_count`, `KasmDeltaPatch.index`,
  `KasmDeltaPatch.new`, `KasmDeltaPatch.old`, `KasmDeltaProof.delta_hash`,
  `KasmDeltaProof.full_replay_checked`, `KasmDeltaProof.full_replay_hash`,
  `KasmDeltaProof.new_state_hash`, `KasmDeltaProof.old_state_hash`,
  `KasmDeltaProof.output_hash`, `KasmDeltaProof.patches`,
  `KasmDeltaProof.previous_output_hash`, `KasmDeltaProof.program_hash`,
  `KasmDeltaProof.proof_hash`, `KasmDeltaProof.saved_ops_estimate`,
  `KasmErrno::<tuple_or_marker>`, `KasmInteropProofEnvelope.canonical_mlir_hash`,
  `KasmInteropProofEnvelope.contract_hash`, `KasmInteropProofEnvelope.interop_kind`,
  `KasmInteropProofEnvelope.kasm_program_hash`, `KasmInteropProofEnvelope.proof_hash`,
  `KasmInteropProofEnvelope.semantic_fingerprint`,
  `KasmInteropProofEnvelope.verifier_hash`, `MarketImpactModel.bps_per_pct_volume`,
  `MlirLoweringReport.function`, `MlirLoweringReport.inputs`,
  `MlirLoweringReport.lowered_loops`, `MlirLoweringReport.lowered_ops`,
  `MlirLoweringReport.outputs`, `MlirLoweringReport.program_hash`,
  `MultiMethod::<tuple_or_marker>`, `NanBoxValue::<tuple_or_marker>`, `Node.a`,
  `Node.b`, `Node.imm`, `Node.op`, `Node.ty`, `NoUB::<tuple_or_marker>`,
  `NumericContract.dtype`, `NumericContract.error_budget`,
  `NumericContract.kernel_family`, `NumericContract.quant_grid`,
  `NumericContract.reduction_tree`, `NumericContract.tile_shape`, `OhlcvBar.close`,
  `OhlcvBar.high`, `OhlcvBar.low`, `OhlcvBar.open`, `OhlcvBar.timestamp`, `OhlcvBar.volume`,
  `OhlcvStore::<tuple_or_marker>`, `OrderBook::<tuple_or_marker>`,
  `PartialEvalReport.eliminated_nodes`, `PartialEvalReport.is_static`,
  `PartialEvalReport.original_nodes`, `PartialEvalReport.residual_nodes`,
  `PartialEvalReport.residual_ratio`, `PeepholeStats.constant_folds`,
  `PeepholeStats.dead_code_removed`, `PeepholeStats.identity_eliminated`,
  `Posit16::<tuple_or_marker>`, `Posit32::<tuple_or_marker>`,
  `Program::<tuple_or_marker>`, `ProgramSig.inputs`, `ProgramSig.outputs`,
  `Proven::<tuple_or_marker>`, `Pure::<tuple_or_marker>`, `Q3132::<tuple_or_marker>`,
  `QuantGrid.bits`, `QuantGrid.round_mode`, `RankedTensor.data`, `RankedTensor.shape`,
  `Rational::<tuple_or_marker>`, `ReservoirSampler::<tuple_or_marker>`, `Rewrite.name`,
  `Rewrite.pattern`, `Rewrite.replace`, `RewriteReport.passes`,
  `RewriteReport.reduced_to_constant`, `RewriteReport.residual_nodes`,
  `SelfHostingRuntime::<tuple_or_marker>`, `SelfHostStats.depth_violations`,
  `SelfHostStats.eval_calls`, `SelfHostStats.fractal_calls`,
  `SelfHostStats.max_depth_seen`, `SsaBuilder::<tuple_or_marker>`, `SsaFunction.blocks`,
  `SsaFunction.entry`, `SsaFunction.param_count`, `SsaFunction.value_count`,
  `Strategy::<tuple_or_marker>`, `TensorErrorBudget.max_abs`,
  `TensorErrorBudget.max_rel`, `TensorErrorBudget.max_ulp`, `TensorNode.a`,
  `TensorNode.b`, `TensorNode.dtype`, `TensorNode.imm`, `TensorNode.op`,
  `TensorNode.shape`, `TensorProgram::<tuple_or_marker>`, `TensorShape.d`,
  `TensorShape.dims`, `Terminating::<tuple_or_marker>`, `ThreadedCtx.dispatch_count`,
  `ThreadedCtx.inputs`, `ThreadedCtx.output`, `Timestamp::<tuple_or_marker>`,
  `ValueId::<tuple_or_marker>`, `WasmComponentContract.contract_hash`,
  `WasmComponentContract.interfaces`, `WasmComponentContract.package`,
  `WasmComponentContract.worlds`, `WasmComponentContractProjection.contract_hash`,
  `WasmComponentContractProjection.interfaces`,
  `WasmComponentContractProjection.package`, `WasmComponentContractProjection.worlds`,
  `WasmFunction.name`, `WasmFunction.params`, `WasmFunction.result`,
  `WasmFunctionProjection.name`, `WasmFunctionProjection.params`,
  `WasmFunctionProjection.result`, `WasmInterface.functions`, `WasmInterface.name`,
  `WasmInterfaceProjection.functions`, `WasmInterfaceProjection.name`,
  `WasmParamProjection.name`, `WasmParamProjection.ty`, `WasmWorld.exports`,
  `WasmWorld.imports`, `WasmWorld.name`, `WasmWorldProjection.exports`,
  `WasmWorldProjection.imports`, `WasmWorldProjection.name`
- public associated constants:
  `Decoded16::NAR`, `Decoded16::ZERO`, `Decoded32::NAR`, `Decoded32::ZERO`,
  `Duration::ZERO`, `KasmErrno::ABSTRACT_DISPATCH`, `KasmErrno::BAD_F64_SUB_OP`,
  `KasmErrno::BAD_FOOTER`, `KasmErrno::BAD_INPUT_LENGTH`, `KasmErrno::BAD_INPUT_SLOT`,
  `KasmErrno::BAD_LENGTH`, `KasmErrno::BAD_MAGIC`, `KasmErrno::BAD_MULTI_METHOD`,
  `KasmErrno::BAD_NODE_COUNT`, `KasmErrno::BAD_OP`, `KasmErrno::BAD_REDUCE_COUNT`,
  `KasmErrno::BAD_REF`, `KasmErrno::BAD_TARGET`, `KasmErrno::BAD_TYPE`,
  `KasmErrno::BAD_VERSION`, `KasmErrno::COMPOSE_ARITY`, `KasmErrno::COMPOSE_TYPE`,
  `KasmErrno::EXTERNAL_TARGET`, `KasmErrno::FUEL_TOO_SMALL`,
  `KasmErrno::NO_METHOD_FOUND`, `KasmErrno::OK`, `KasmErrno::OUTPUT_COUNT`,
  `KasmErrno::TOO_MANY_SLOTS`, `KasmErrno::TRUNCATED`, `KasmErrno::TYPE_MISMATCH`,
  `KasmErrno::UNKNOWN`, `KasmErrno::UNSUPPORTED_V1_OP`,
  `KasmErrno::VALUE_TYPE_MISMATCH`, `MarketImpactModel::LARGE`,
  `MarketImpactModel::MODERATE`, `MarketImpactModel::NONE`, `MarketImpactModel::SMALL`,
  `Posit16::MAXPOS`, `Posit16::MINPOS`, `Posit16::NAR`, `Posit16::NEG_ONE`,
  `Posit16::ONE`, `Posit16::ZERO`, `Posit32::MAXPOS`, `Posit32::MINPOS`, `Posit32::NAR`,
  `Posit32::NEG_ONE`, `Posit32::ONE`, `Posit32::ZERO`, `Q3132::MAX`, `Q3132::MIN`,
  `Q3132::ONE`, `Q3132::ZERO`, `Timestamp::EPOCH`, `Timestamp::MAX`, `Timestamp::MIN`

<!-- forge-language-dictionary:generated:end -->

### Example

```text
forge_module:
  module hover_margin version 1
forge_imports:
  none
forge_constants:
  const g: f64 unit m/s^2 = 9.80665
forge_functions:
  fn weight(mass: f64) -> f64 { return mass * g }
  fn margin(thrust: f64, mass: f64) -> f64 { return thrust - weight(mass) }
forge_program:
  let margin_value = margin(thrust, mass)
  emit thrust_margin: f64 = margin_value
forge_inputs:
  param mass: f64 unit kg bounds [0.1,10.0] nominal 1.2
  param thrust: f64 unit N bounds [0.0,100.0] nominal 16.0
forge_outputs:
  output thrust_margin: f64 unit N handoff scalar
forge_constraints:
  assert finite(thrust_margin)
  assert bounds(thrust_margin,[-100.0,100.0])
forge_samples:
  case hover seed 42 { given mass=1.2, thrust=16.0; expect thrust_margin approx 4.227 tolerance 0.01 }
forge_cost:
  max_steps=100000
  max_memory_mb=64
  precision=f64
artifact_handoff:
  proof_hash,output_hash,compact_result
```

### Types

Forge currently parses and reasons about these source types:

- scalars: `bool`, `i32`, `i64`, `u32`, `u64`, `f32`, `f64`;
- vectors: `vec2`, `vec3`, `vec4`, `vec<T,N>`;
- matrices: `mat2`, `mat3`, `mat4`, `mat<T,C,R>`;
- complex values: `complex`, `complex<T>`;
- collections: `array<T,N>`, `tensor<T,shape>`, `column<T>`;
- records/tables: `table<name:type;name:type>`;
- graphs: `graph<N,E>`;
- fields: `field<T,R>` and display form `field`;
- sparse fields: `sparse_field<T,R,sparse_grid|page_table|hash_grid>`;
- memory/DOM/RAM records: `snapshot`, `memory_map`, `heap_object`,
  `dom_node`, `dom_edge` and `taint<public|user_data|credential|secret>`;
- domain records: Trading `tick`, `bar`, `quote`, `trade`, `orderbook`,
  `position`, `pnl`; Bio/DNA `dna`, `rna`, `protein`, `gene`, `variant`,
  `feature`, `alignment`; Chemistry `atom`, `bond`, `molecule_graph`,
  `reaction`, `conformer`;
- crypto/proof records: `bitvec<N>`, `field<p>`, `curve`, `hash`, `merkle`,
  `signature`;
- code/agent records: `ast`, `symbol`, `cfg`, `callgraph`, `diff`, `patch`,
  `testcase`;
- Banger render records: `sdf`, `neural_sdf`, `voxel_page`, `surfel`,
  `micromesh`, `material_graph`, `meshlet_cluster`, `geometry_page`,
  `lod_node`, `radiance_probe`, `radiance_cache`, `shadow_page`, `pcg_graph`,
  `spatial_cell` and `light_budget`.

Not every type has a complete bytecode lowering today. The source verifier can
still use them for shape/type/unit validation before later compiler work.

### Expressions

Forge expressions are pure expression trees:

- literals: booleans and typed numbers such as `1i32`, `2u64`, `3.5f32`,
  `9.80665`;
- variables: inputs, constants, lets, outputs in constraint scope, and function
  arguments;
- unary ops: `-x`, `!flag`;
- binary ops: `+`, `-`, `*`, `/`, `^`, `==`, `!=`, `<`, `<=`, `>`, `>=`,
  `&&`, `||`;
- calls: approved builtins, local pure functions, and bounded loop forms.

Expression parsing respects precedence, parentheses and function-call argument
splitting. It rejects unknown variables/calls during module validation unless
the name is a local function or approved builtin.

### Builtins

Current builtin families:

- numeric safety and math: `finite`, `abs`, `sqrt`, `pow`, `min`, `max`,
  `clamp`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`,
  `ln`, `log`, `log10`;
- vector/matrix/complex constructors and ops: `vec2`, `vec3`, `vec4`, `mat2`,
  `mat3`, `mat4`, `complex`, `dot`, `length`, `normalize`, `cross`,
  `transpose`, `determinant`;
- collection/shape ops: `len`, `shape`, `index`, `sum`, `rows`, `cols`,
  `node_count`, `edge_count`, `sample`;
- data-parallel source combinators: `map`, `reduce`, `scan`, `zip`, `filter`,
  `gather`, `scatter`;
- query/time source ops: `window`, `rolling`, `groupby`, `join`, `asof_join`;
- deterministic sampling source ops: `sobol`, `latin_hypercube`,
  `monte_carlo`;
- domain source ops: Trading `vwap`, `ema`, `volatility`, `slippage`,
  `latency`, `backtest`, `anti_lookahead`, `walk_forward`, `stress_test`,
  `transaction_costs`; Bio/DNA `kmer_hash`, `transcribe`, `translate`,
  `reverse_complement`, `align`, `alignment_score`, `motif_scan`, `mutate`,
  `annotate`; Chemistry `smiles_parse`, `smarts_match`, `fingerprint`,
  `molecular_similarity`, `substructure_search`, `valence_check`,
  `charge_check`, `aromaticity_check`, `conformer_generate`,
  `reaction_apply`;
- crypto/proof source ops: `bitvec_and`, `bitvec_xor`, `field_add`,
  `field_mul`, `curve_mul`, `hash_commit`, `merkle_root`, `merkle_verify`,
  `signature_verify`, `constant_time`, `secret_branch_check`, `zk_prove`,
  `zk_verify`, `smt_check`, `lean_check`;
- code/agent source ops: `parse`, `parse_code`, `typecheck`,
  `typecheck_code`, `symbol_table`, `cfg_build`, `callgraph_build`,
  `transform`, `transform_code`, `patch`, `patch_apply`, `run_test`,
  `compare_trace`, `proof_envelope`;
- Banger render source ops: `sdf_from_field`, `neural_sdf_decode`,
  `sdf_gradient`, `sdf_normal`, `sdf_curvature`, `sdf_to_voxel_page`,
  `micromesh_build`, `meshlet_cluster`, `geometry_page_pack`, `lod_select`,
  `cluster_cull`, `radiance_probe`, `radiance_cache_update`, `screen_trace`,
  `world_trace`, `shadow_page_alloc`, `shadow_cache_invalidate`,
  `material_eval`, `material_layer`, `substrate_mix`, `material_payload`,
  `pcg_hash`, `pcg_spawn`, `pcg_execute`, `world_partition_cell`,
  `streaming_plan`, `residency_update`, `light_cluster`,
  `light_budget_select`, `light_proof`, `export_mesh`, `export_sdf`,
  `export_shader`, `export_preview` and `export_proof`;
- bounded control forms: `fori(step,start,stop,init)` and
  `while_fuel(cond,body,state,fuel)`.

The builtin list is intentionally small. If a domain needs more, add a typed,
verified builtin/dialect instead of letting arbitrary names through.

### Units

Every input, output and constant has a unit. `unit none` means dimensionless.
Supported base/derived units include SI-style dimensions such as `kg`, `m`,
`s`, `A`, `K`, `mol`, `cd`, plus derived forms such as `N`, `Pa`, `W`, and
compositions like `kg*m/s^2` or `m^2`.

Unit inference currently enforces:

- addition/subtraction/comparison require compatible dimensions;
- multiplication/division compose dimensions;
- integer `pow` raises dimensions;
- `sqrt` requires even unit exponents;
- trigonometric/log/exponential functions require dimensionless inputs;
- `dot`, `cross`, `length`, `normalize`, `sum`, `index`, `sample` propagate
  or transform units according to conservative rules.

This catches errors like adding kilograms to meters before Monster execution.

### Bounds

Every input has `[min,max]` and `nominal`. Forge uses conservative interval
analysis over constants, inputs, function calls, `let` values, emits and
constraints.

The interval pass currently proves:

- division denominator cannot include zero;
- `sqrt` input is non-negative;
- `ln`/`log` input is strictly positive;
- `pow` with small integer exponent has a bounded interval;
- `pow` / `^` with finite fractional exponent has a bounded interval when the
  base is proven non-negative and the exponent domain is safe;
- structural squares such as `x * x` are proven non-negative even when `x`
  spans negative and positive values, which keeps norms/RMS formulas valid;
- `min`, `max`, `clamp`, comparisons, boolean ops and selected builtins have
  conservative intervals;
- `bounds(expr,[min,max])` holds if the inferred interval is inside the
  requested interval;
- `approx(actual, expected, tolerance)` holds if the absolute interval
  difference is within tolerance.

This is not a full nonlinear theorem prover. If the interval pass cannot prove
safety, the module is rejected.

### Functions

Functions use this form:

```text
fn name(arg: type, ...) -> type { return expr }
```

Current restrictions:

- body length is bounded;
- body is a single pure return expression;
- argument names cannot duplicate;
- reserved control names cannot be used as function names;
- local function call cycles are rejected;
- calls must typecheck, unit-check and bounds-check in their call context.

This keeps Forge closer to a verified compute DSL than to Rust/Python.

### Bounded Control

Forge rejects unbounded source control names: `for`, `while`, `loop`.

Allowed forms:

```text
fori(step, start, stop, init)
while_fuel(cond, body, state, fuel)
```

`fori` requires a local pure `step(i, acc) -> acc` function. `while_fuel`
requires `cond(state) -> bool`, `body(state) -> state`, and an integer fuel
bound. These are source-level forms aligned with existing Monster bounded loop
machinery. Forge also sees function references hidden inside these forms when
checking for cycles.

### Constraints

`forge_constraints` supports:

```text
assert finite(expr)
assert bounds(expr,[min,max])
assert approx(actual, expected, tolerance)
assert boolean_expr
```

All constraint expressions must be pure, bounded-control-safe, type-valid,
unit-valid and interval-safe. `finite` accepts expressions, not only output
names. Boolean assertions must infer `bool`.

### Built-In Samples

Samples are deterministic declarations:

```text
case name seed 42 { given x=1.0, y=2.0; expect out approx 3.0 tolerance 0.01 }
```

The `seed` is optional; unseeded cases canonicalize as `seed 0`.
Samples reject duplicate given names and modules reject duplicate case names.
Each sample has a `sample_hash_hex`, making examples content-addressed and
reproducible. Current samples are parsed and validated as declarations; a full
property/sample executor is still future work.

### Content Addressing

Forge source exposes hashes for:

- whole module source;
- imports;
- constants;
- functions;
- program fragments: each `let`/`emit` and the whole `forge_program`;
- declared input contract;
- declared output contract;
- type contract;
- unit contract;
- bounds contract;
- overall contract envelope;
- accepted source-validation proof;
- individual samples.

Imports use:

```text
import hash name = sha256:<64 hex chars>
```

Import existence and typed compatibility are not fully enforced yet; current
support is source identity and canonical hashing.

`input_hash` and `output_hash` at this layer identify the declared source
contracts, not runtime value payloads. Monster/backend runs must later attach
their own runtime input/output hashes to the source proof envelope.

### Artifact Handoff

`artifact_handoff` declares the compact output contract. It must include
`proof_hash` and `output_hash`; `compact_result` is the common third item.
The design goal is that raw data remains in InGen stores while callers receive
hashes, refs, previews and proof summaries.

### Cost Contract

`forge_cost` currently parses and validates:

```text
max_steps=<u64>
max_memory_mb=<u64>
precision=f32|f64
parallelism=<u32>   # optional
```

`max_steps` and `max_memory_mb` must be positive. `parallelism` must be between
1 and 4096 when present. Forge also performs a conservative static source-cost
check before accepting a module:

- expression work must fit under `max_steps`;
- declared inputs, outputs, constants and program/sample slots must fit under
  `max_memory_mb`;
- `precision=f32` rejects `f64` types and unsuffixed floating literals in the
  compute source.

This is a source-level preflight contract, not yet a full backend scheduler or
runtime peak-memory profiler.

### Verification Pipeline

Current source validation order is effectively:

```text
parse sections
-> parse imports/constants/functions/program/inputs/outputs/constraints/samples/cost
-> reject duplicate names and invalid cross references
-> typecheck function/program/constraint expressions
-> unit-check program and constraints
-> interval-check program and constraints
-> enforce purity and bounded control
-> enforce source cost, memory and precision contract
-> reject local function cycles
-> canonicalize source
-> compute source/module/contract hashes
```

Only after this should a future lowering path create richer bytecode kernels for
Monster.

### What Forge Can Do Today

Forge can currently verify generated or expert-authored compute specs for:

- scalar math with safe domains;
- vector/matrix/complex shape-aware formulas;
- basic collection, table, graph and field typing;
- source-level data-parallel combinator validation;
- source-level query/time operation validation;
- source-level deterministic sampling and Monte Carlo validation;
- source-level ranking, top-k, Pareto and diversity-selection validation;
- unit-aware engineering formulas;
- typed Trading, Bio/DNA, Chemistry, Crypto/ZK and Code/Agent dialect
  contracts;
- interval-proved assertions;
- deterministic example declarations;
- bounded loop skeletons;
- source-level cost, memory and precision preflight;
- compact output handoffs including `scalar`, `vector`, `field`, `sdf`,
  `mesh_params`, `score`, `table`, `graph`, `timeseries` and `artifact`;
- content-addressed module reuse.

### What Forge Cannot Yet Do

Do not claim these are implemented today:

- full lowering of every source feature to executable bytecode kernels;
- industrial GPU kernels for tensors, fields, SDFs, sparse grids or domain
  combinators; Monster has first-stratum primitive lowering, not complete
  specialized algorithms for every family;
- complete nonlinear/SMT/Lean proof of arbitrary formulas;
- large built-in domain libraries for trading, biology, chemistry,
  cryptography, game/3D engineering or memory cartography;
- automatic property-based sample generation from seeds;
- typed import registry compatibility checks;
- full optimizer/scheduler split like mature MLIR/Halide/Futhark stacks.

The roadmap below is the path to those capabilities.

## Detailed Language Backlog

The live priorities are listed in `Live Objectives` near the top of this file.
This detailed backlog keeps the older numbered language plan for continuity; do
not treat it as a second roadmap when it conflicts with the live list.

Remaining historical enrichment objectives: 10 active items, objectives 61-70.

Forge must become a compact verified compute language plus domain dialects, not
a giant Rust/Python clone. Add features in this order, promoting each only when
it has a verifier, proof hash and rollback path.

First landed step: `src/kasm.rs` now exposes source-level `ForgeType`,
`ForgeScalarTy`, `ForgeScalarValue`, `ForgeExpr`, `ForgeUnitDim`,
`ForgeParamSpec`, `ForgeOutputSpec`, `ForgeConstraintSpec`, `ForgeConstSpec`,
`ForgeFunctionSpec`,
`ForgeProgramSpec`, `ForgeSampleCase`, `ForgeArtifactHandoff`,
`ForgeCostSpec` and `ForgeModuleSpec`. They parse
scalar/vector/array/tensor type text, SI-style units, bounded
`param <name>: <type> unit <unit> bounds [min,max] nominal x` lines,
typed scalar literals such as `1i32`, `2u64`, `3.5f32`, `true` and `false`,
constants, function signatures with bounded bodies, `let` and `emit` program
lines,
`output <name>: <type> unit <unit> handoff <kind>` lines, `assert ...`
constraints, deterministic `case ... given ... expect ... tolerance ...`
samples, artifact handoff declarations, `forge_cost` blocks and complete core
`/newcompute_` module sections, then lower to the current bytecode storage types
when possible. `ForgeModuleSpec` also emits a stable canonical source string
and SHA-256 source hash for reuse and proof envelopes before bytecode lowering.
The scalar layer now performs minimal expression inference for arithmetic,
comparisons, boolean logic and builtin calls such as `finite`, `pow`, `sqrt`,
`min`, `max` and `clamp`, so invalid mixes like `true + mass` are rejected
before bytecode lowering.

Second landed step: vector, matrix and complex source types now parse and
participate in expression inference. Forge accepts `vec2`, `vec3`, `vec4`,
generic `vec<T,N>`, `mat2`, `mat3`, `mat4`, generic `mat<T,C,R>`, `complex`
and `complex<T>`. Constructors and core math builtins infer shapes for
`vec*`, `mat*`, `complex`, `dot`, `length`, `normalize`, `cross`,
`transpose` and `determinant`; basic composite arithmetic covers same-shape
vector/matrix/complex operations, scalar broadcast and square matrix-vector or
matrix-matrix products. This is still source-level validation; bytecode/GPU
lowering comes later.

Third landed step: collection source types now parse and participate in basic
shape/type inference. Forge accepts `array<T,N>`, `tensor<T,shape>`,
`column<T>`, `table<name:type;name:type>`, `graph<N,E>` and `field<T,R>` with
canonical display forms. Collection expressions infer types for `len`, `shape`,
`index`, `sum`, `rows`, `cols`, `node_count`, `edge_count` and field
`sample(field, coord)`. Generic function signatures now split arguments at
top-level commas, so `fn total(xs: array<f64,4>) -> f64` is valid. This step is
only the collection type layer; full data-parallel `map/reduce/scan` kernels
belong to the Massive Compute objective.

Fourth landed step: dimensioned units now propagate through Forge expressions
and `/newcompute_` modules. Inputs, constants and outputs contribute
`ForgeUnitDim` values; user functions are checked by binding argument units at
their call sites. Addition, subtraction and comparisons require matching
dimensions, with literal zero allowed as a neutral comparison/addition value.
Multiplication and division combine SI exponents, integer `pow` raises unit
dimensions, and `sqrt` requires even exponents. Builtins such as `finite`,
`abs`, `sqrt`, trigonometric/log functions, `dot`, `cross`, `length`,
`normalize`, `sum`, `index` and `sample` now carry unit rules. This rejects
invalid math like `kg + m` before bytecode lowering while accepting
`kg*m/s^2 == N`.

Fifth landed step: required parameter bounds now feed a conservative interval
analysis pass. `ForgeBounds` tracks `[min,max]` for parameters, constants,
function calls, `let` bindings and emitted outputs. Arithmetic propagates
intervals; division is rejected when the denominator may contain zero; `sqrt`
requires a non-negative interval; logarithms require a strictly positive
interval; integer `pow` propagates interval powers. The pass is intentionally
strict: if bounds cannot prove a scalar expression domain is safe, the
`/newcompute_` module is rejected before Monster execution. This is a first
abstract-interpretation layer, not a full nonlinear verifier.

Sixth landed step: audit found that Forge/KASM already had strong
content-addressing at bytecode/runtime level: structural and canonical program
hashes, DeltaKASM state/delta/output/proof hashes, MultiMethod hash dispatch
and self-host/fractal calls by program hash. The missing layer was the authored
Forge source module. `ForgeImportSpec` now parses `import hash name =
sha256:<64hex>` with canonical normalization. Constants and functions expose
canonical source plus `const_hash_hex` / `function_hash_hex`. `ForgeModuleSpec`
now carries optional `forge_imports`, emits them in canonical source, rejects
duplicate import names, and exposes `ForgeModuleHashes`: source, imports,
constants, functions, types, units, bounds and contract hashes. Import existence
and typed compatibility still require a future registry; this step completes
the source-level content-addressed identity layer.

Seventh landed step: audit found existing bytecode-level proof gates for purity
and determinism: `prove_pure`, `prove_deterministic`, and disallowed runtime
opcodes such as `Fractal`/`Eval` in proof contexts. The missing layer was
source Forge purity. `ForgeExpr::is_pure_source` now rejects forbidden
effectful calls before module validation: `random`, `now`, `time`, `http`,
`shell`, `read_file`, `write_file`, `hostcall`, `eval`, `fractal`, `secret`,
`read_ram`, `read_dom`, and `host_`/`io_`/`sys_` prefixed calls. Functions,
program `let`/`emit` expressions and assertions all pass this source purity
gate. Unknown non-effect calls are still rejected separately by type checking
unless they are local pure functions or approved pure builtins.

Eighth landed step: audit found that the runtime already has bounded loop
machinery: `Op::Fori`, `Op::WhileLoop`, `Reduce`/`Scan`, program `fuel`, and
Monster `call_fori` / `call_while` with explicit fuel exhaustion. The missing
layer was the authored Forge source gate. Source expressions now reserve
unbounded control names (`for`, `while`, `loop`) and accept only canonical
bounded loop calls: `fori(step, start, stop, init)` and
`while_fuel(cond, body, state, fuel)`. `fori` requires a pure local
`step(i, acc) -> acc`; `while_fuel` requires `cond(state) -> bool`,
`body(state) -> state`, and an integer fuel bound. Module validation now also
rejects local function call cycles, including cycles hidden behind bounded-loop
function references. This guarantees source-level termination structure; deeper
loop invariants remain for assertions, tests and cost contracts.

Ninth landed step: audit found that Forge already had generic `assert`,
`finite(output)`, parameter `bounds [min,max]`, sample `approx` expectations
and `tolerance`. The missing layer was assertion-level numeric contracts.
`forge_constraints` now accepts `assert finite(expr)`,
`assert bounds(expr,[min,max])` and `assert approx(actual, expected, tolerance)`
beside boolean `assert expr`. These contracts pass the same source gates as
normal expressions: purity, bounded control, type checking, unit checking and
interval bounds. `bounds(...)` is proven by the current interval analysis;
`approx(...)` requires unit-compatible numeric expressions and proves the
absolute interval difference is within tolerance. The top-level comma splitter
now respects `<...>`, `(...)` and `[...]`, so contracts can safely contain
nested calls and bracketed bounds.

Tenth landed step: audit found that built-in sample tests already existed as
`case ... { given ...; expect ... approx ... tolerance ... }`, but they had no
canonical deterministic seed. `ForgeSampleCase` now accepts optional
`case name seed <u64> { ... }`, preserves unseeded legacy cases, rejects
duplicate given names, rejects duplicate case names at module level, emits a
canonical source form with `seed 0` for unseeded cases, and exposes
`sample_hash_hex`. This makes authored examples reproducible and
content-addressed before any future full sample executor is added.

Eleventh landed step: audit found that `forge_cost` already parsed
`max_steps`, `max_memory_mb`, `precision` and optional `parallelism`, but the
module validator treated it mostly as syntax. `ForgeModuleSpec` now enforces a
source preflight contract: static expression work must fit under `max_steps`,
declared data plus program/sample slots must fit under `max_memory_mb`,
`parallelism` is bounded to `1..=4096`, and `precision=f32` rejects `f64`
types or unsuffixed floating literals in compute expressions. This gives Forge
a concrete cost/memory/precision refusal path before Monster execution while
leaving full backend scheduling and runtime peak profiling for later steps.

Twelfth landed step: audit found that source-level module, import, constant,
function and sample hashes already existed, while fragments, declared inputs,
declared outputs and accepted source-validation proof were not first-class in
the module envelope. `ForgeParamSpec`, `ForgeOutputSpec`, `ForgeProgramLet`,
`ForgeProgramEmit` and `ForgeProgramSpec` now expose canonical source strings
and stable hashes. `ForgeModuleHashes` now includes `fragment_hash`,
`input_hash`, `output_hash` and `proof_hash` beside the previous source,
imports, constants, functions, type, unit, bounds and contract hashes. This
seals the accepted Forge source graph before runtime lowering; runtime
Monster/Banger proofs still add actual value payload hashes later.

Thirteenth landed step: audit found that KASM/Monster already had runtime
building blocks such as `Reduce`, `Scan`, `Vmap/Pmap` and vector ops, but Forge
source could not author typed data-parallel combinators. Source expressions now
validate `map(fn, xs)`, `reduce(fn, init, xs)`, `scan(fn, init, xs)`,
`filter(pred, xs)`, `zip(xs, ys)`, `gather(xs, indices)` and
`scatter(base, indices, values)`. Function references hidden inside
`map/reduce/scan/filter` are included in call-cycle detection. Units and
interval bounds propagate conservatively through these forms, so complete
`/newcompute_` modules can use them before lowering to future optimized
parallel kernels.

Fourteenth landed step: audit found existing timestamp primitives, OHLCV
time-window slicing, resamplers and trading walk-forward windows, but no Forge
source operators for query/time expressions. Forge source now validates
`window(xs, width)`, `rolling(fn, width, xs)`, `groupby(table, key_index)`,
`join(left, right, left_key, right_key)` and
`asof_join(left, right, left_time, right_time)`. Fixed array windows produce
typed tensor views, rolling functions require an `array<T,width>` parameter,
joins merge typed table schemas, and `asof_join` requires integer time columns.
Units and bounds propagate conservatively. This is not a SQL engine yet; it is
the verified source surface that future columnar/query backends can lower.

Fifteenth landed step: audit found existing `sample(field, coord)` source
typing, deterministic sample-case seeds, and runtime random/reservoir helpers,
but no Forge source surface for deterministic design sampling or Monte Carlo
estimation. Forge source now validates `sobol(dim, count, seed)`,
`latin_hypercube(dim, count, seed)`, `sample(array_or_tensor, selector)` and
`monte_carlo(fn, samples, seed)`. The samplers return typed
`tensor<f64,countxdim>` values, enforce positive bounded dimensions/counts,
carry `[0,1]` intervals, and `monte_carlo` checks that the referenced pure
function accepts one sample vector of the sampler dimension. Units, bounds,
cycle detection and static source cost now propagate through this path. This is
still a source verifier, not yet a full runtime QMC engine or property-test
executor.

Sixteenth landed step: audit found no Forge source builtins for selection,
only unrelated runtime/table selection helpers and tensor rank accessors. Forge
source now validates `rank(xs)`, `top_k(xs, k)`, `pareto(points)` and
`diversity_select(points, k)`. `rank` returns `u64` ranks with the source
collection shape, `top_k` requires a positive literal `k` and returns a bounded
selected collection, `pareto` requires a numeric `tensor<countxdim>` and returns
one `u64` front/rank per point, and `diversity_select` returns `k` selected
point rows. Units, interval bounds and static source cost now propagate through
these calls. This is the verified source surface, not yet a massive NSGA/top-k
or diversity kernel backend.

Sixteenth-bis landed step: Forge source now accepts the broad universal
primitive vocabulary needed before Monster can specialize kernels: scalar
arithmetic, boolean/comparison, transcendental math, bit/integer ops,
vector/matrix decompositions, shape transforms, richer data-parallel and
gather/scatter ops, deterministic sampling, statistics, optimization,
autodiff, solvers, signal/FFT, sparse/graph, SDF/3D geometry,
physics/engineering, unit/proof helpers, future memory/DOM/RAM records and
crypto/hash blocks. This is source-level parse/type/unit/bounds/IR
classification, not yet expert execution kernels for every primitive.

Banger render landed step: Forge source now has a typed Banger render dialect
for SDF/neural SDF, voxel pages, surfels, micromeshes, material graphs,
meshlet/geometry/LOD pages, radiance caches, shadow pages, hashed PCG,
spatial streaming, light budgets and exports. Monster exposes the same words
through `/newcompute_`, routes them to `wgsl.native_tandem_banger_render_pages.v1`
and returns typed Banger artifact pages with proof hashes.

Domain dialect landed step: Forge source now has typed Trading, Bio/DNA and
Chemistry records plus point-in-time backtest, anti-lookahead, walk-forward,
k-mer/alignment/annotation and chemistry graph/fingerprint/substructure
contracts. Monster exposes the same words through `/newcompute_`, routes them
to domain shader profiles and returns typed market, portfolio, bio and
chemistry artifact pages.

Crypto/code-agent landed step: Forge source now has typed crypto/proof records
(`bitvec<N>`, `field<p>`, `curve`, `hash`, `merkle`, `signature`) and typed
code-agent records (`ast`, `symbol`, `cfg`, `callgraph`, `diff`, `patch`,
`testcase`). It validates constant-time/secret-branch gates, ZK/SMT/Lean hooks,
parse/typecheck/transform/patch/test/trace ops and proof envelopes. Monster
exposes the same words through `/newcompute_`, routes them to specialized
crypto proof and code patch shader profiles and returns typed proof/code
artifact pages.

## Delta Path

The delta path is the first native compute-saving bytecode route. It recognizes
a proven Forge/KASM shape and updates the previous output from a
content-addressed input patch:

```text
programHash + oldStateHash + previousOutputHash + deltaHash
-> outputHash + newStateHash + proofHash
```

Initial promoted shape:

```text
VecI64 input
-> VSumI64
-> I64 output
```

Standard path: normal `kasm::execute` auto-promotes matching raw `VecI64` calls
and also accepts `encode_vec_sum_delta_frame(...)`; Monster/InGen routes that
execute Forge bytecode inherit the shortcut without a parallel engine.

## Fragment Reuse

Monster `/newcompute_` is the universal compute entry for all InGen sections:
general, Google/OAuth, WebExplorer, Banger, trading, real estate and future
domains. Brain keeps a compact pointer/command surface; Monster owns the
universal compute template, preflight contract, execution path, proof hashes and
compute cache. Banger-specific `/newobject_` consumes curated compute evidence
only when the caller is doing 3D/SDF work.

`/newcompute_` memoizes fragments inside a compute, not only whole runs. The
runtime records generated Forge sections, MathContract fragments, atoms, lines
and sequence windows as content-addressed fragments in the InGen store SQLite
library:

```text
brain/computes/compute_library.sqlite
-> computes
-> compute_runs
-> compute_fragments
```

The hot index is exact over scale, fragment hash, contract hash, type hash,
unit hash, result hash and proof hash. Probabilistic filters may accelerate
absence checks later; they never authorize reuse.

## Monster Compute Graph

Monster is the execution engine for Forge. The current SOTA target is not a
giant hand-written library of domain programs; it is the same core idea seen in
modern compute systems: explicit op semantics like StableHLO, bulk array
combinators like Futhark, block/lane GPU kernels like Triton and portable native
GPU limits like wgpu/RHI. Forge provides typed, content-addressed math;
Monster lowers it to a compact compute graph and executes/reuses/proves it.

The target `/newcompute_` front door is:

```text
LLM selects a math class and fills a structured MathContract
-> deterministic MathContract-to-Forge compiler
-> generated Forge source
-> Forge parse/type/unit/bounds/cost checks
-> Forge IR
-> MonsterComputeGraphPlan
-> MonsterPreparedCompute
-> GPU batch plan with primitiveOps + typed ABI buffers
-> execute_prepared_compute for mass_math
-> proof_hash / output_hash / typed result pages / artifact refs
```

`MonsterComputeGraphPlan` records module/source/contract/proof hashes, scale
(`micro`..`large`), backend hint, per-fragment cache keys, native output
handoffs and the compute IR hash. `MonsterPreparedCompute` is the
scheduler-facing object: it contains only cache misses, native-ready output refs
and one stable `manifest_hash`.

Current execution state:

- Large `mass_math` jobs require the GPU path. Micro/mini jobs may use a bounded
  CPU oracle only for smoke/proof work.
- The Rust RHI path uses wgpu adapter enumeration and shards work across usable
  non-CPU adapters, so a Windows machine with NVIDIA + AMD can use more than one
  GPU when both adapters expose compatible limits.
- Monster generates WGSL kernels from `primitiveOps`, not only from broad
  classes like `elementwise` or `reduction`.
- `primitiveOps` are part of kernel/shader/batch hashes and are exposed in the
  `/newcompute_` JSON.
- The GPU ABI is typed from Forge IR buffers. Seeds/readback are bindings `0`
  and `1`; Forge constants, inputs and outputs start at binding `2` with
  init policies and content hashes.
- Executions now return `MonsterTypedResultBuffer` values with real page bytes,
  page hashes and buffer hashes. Compact JSON projections expose refs and
  lengths; the Rust path carries the bytes.
- Each plan carries a numeric policy, a differential test plan, a symbolic math
  plan and a multi-adapter schedule policy. The symbolic plan is inactive for
  pure numeric modules and active when Forge IR contains `symbolic_math`
  primitives or symbolic artifacts; its `plan_hash` is part of the graph proof.
  Active symbolic plans also carry an execution backend/hash and per-output
  canonical forms that feed `MonsterTypedResultBuffer` pages. Symbolic typed
  buffers are verified byte-exactly against the CPU production canonical form,
  because `expr<f64>` pages are symbolic payloads, not floating-point arrays.
  Each mass execution now also carries a hashed differential execution gate
  with CPU/candidate buffer hashes, readback hashes, tolerance, error ppm,
  promotion status and proof hash.
- Generated WGSL is persisted under
  `brain/computes/kernel_shader_cache/index.jsonl`; the cache key includes
  Forge IR hash, `primitiveOps`, ABI hash, adapter class and shader hash.
- Native tandem render routes now prepare hash-addressed pages before the frame
  loop sees them: SDF bricks, voxel pages, meshlet pages, surfel/radiance cache
  pages and material payloads. The JSON projection exposes lengths and hashes;
  Rust carries the page bytes.
- DOM/RAM native tandem routes now prepare hash-addressed graph/table pages and
  a nonblocking browser event-loop slice manifest before the browser loop sees
  them. The JSON projection exposes hashes and byte lengths; Rust carries the
  page bytes.
- The universal primitive vocabulary has GPU lowering coverage: scalar,
  boolean, transcendental, bit/integer, vector/matrix, array/tensor shape,
  data-parallel, gather/scatter, random, stats, optimization, AD, solvers,
  signal/FFT, sparse/graph, geometry/SDF, physics, units/contracts/proof,
  memory/DOM/RAM and crypto/hash.

This does **not** mean every primitive is already an industrial-grade algorithm.
Simple primitives execute directly. Complex families now have deterministic
reference kernels/oracles and typed result pages so the pipeline is real and
testable; FFT currently uses a DFT reference kernel, and SVD/eigen, graph
traversal, sparse solve, AD, PDE/ODE and crypto still need high-performance GPU
kernels before they are production-grade.

Historical hard-coded renderer/drone numeric executors have been removed.
Forge language features must express domain math directly; Monster then plans,
hashes, reuses and schedules those fragments through the universal manifest.

Legacy Monster features survive only if they serve this contract. Cache,
dispatch, hotplan, mmap, GPU and atlas are core. Lab/train/evolve/oracle are not
second brains; they may remain only as compute optimizer internals. Domain
helpers such as walk-forward, NNUE, seminaive Datalog or Lua-table indexing
should move out of Monster or be reattached explicitly to a verified compute
graph.

Remaining Monster work:

1. Promote reference/probe kernels to high-performance GPU kernels for
   FFT/IFFT/RFFT, sparse ops, graph traversal, SVD/QR/Cholesky/eigen, ODE/PDE,
   AD/JVP/VJP and crypto/hash blocks.
2. Broaden high-performance GPU kernels behind the differential gates, then
   promote only when real GPU result comparison passes per primitive family.
3. Later, production native render promotion: sparse VDB hierarchy, renderer
   residency management, meshlet culling metadata and multi-bounce radiance
   cache updates.
4. Later, live DOM/RAM cartography integration: incremental capture, resumable
   slices, browser backpressure and section-owned graph/table rendering.

The public route shape is `forge.monster.engine.route.v1`:

- `mass_math`: regular verified `/newcompute_` math;
- `native_tandem_render`: Banger/future Rust renderer artifacts such as SDF,
  fields and mesh parameters;
- `native_tandem_dom_ram`: Google-Web native browser memoryreading artifacts,
  DOM graphs, heap/RAM maps and high-level browser state labels.

All three lanes still start from the same Forge source contract. The difference
is only the native consumer and blocking rule: math may run inline only for
micro work, render never blocks the frame loop, DOM/RAM never blocks the browser
event loop.

## Promotion Rule

Forge bytecode wins only when it removes actors, scripts, duplicated routes or
repeated computation. Promote an experiment only when it beats the current path
on clarity, speed, capability or verifiability.
