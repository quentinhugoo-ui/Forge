# Forge

> Substrat de calcul content-addressed, déterministe, mémoïsable.
> Chaque programme valide est une identité cryptographique stable
> cross-machine et cross-décennie.

> 📚 **Règle docs** : ce projet maintient **4 documents au maximum**
> à la racine. Si tu veux ajouter un doc, fusionne-le dans un existant.
> Les 4 fichiers canoniques : `README.md`, `CLAUDE.md`, `ROADMAP.md`,
> `CARNET.md`. `AGENTS.md` fusionné dans `CLAUDE.md` (Φ.ν.9f),
> `PROTOCOLE.md` supprimé (theater métaphorique sans ROI).
> Pas de `STATE.md`, `TENSIONS.md`, `AMBITIONS.md`,
> `ARCHITECTURE.md`, `DOCUMENTATION.md`, `docs/principles/`
> — tout a été absorbé.
>
> 📁 **Règle dossiers** : ce projet maintient **5 dossiers de code au
> maximum** dans `src/` (Φ.μ.7.2). Les 5 canoniques actuels :
> `agent/` (Ω-7 + Ω-2 extract), `godel/` (Ω-5), `kasm/` (Ω-1 + Ω-3 numeric +
> tensor), `meta/` (Ω-4), `monster/` (runtime). Plus aucun `extract/` ni
> `numeric/` au top-level (absorbés dans `agent/` et `kasm/`).
>
> Si le projet évolue (V8, V9, V∞), les noms de dossiers **doivent
> suivre la doctrine architecturale** (les Ω-couches L0→L7 sont stables ;
> seules les nouvelles couches L8+ peuvent introduire un nouveau dossier
> — auquel cas une fusion préalable libère un slot pour respecter la
> règle des 5).
>
> 📂 **Top-level projet** : 3 dossiers visibles seulement (`src/`,
> `examples/`, `tests/` — conventions Rust verrouillées par Cargo).
> Les anciens `docs/`, `.mojo-env/`, `.pixi-home/` ont été supprimés
> en Φ.μ.7.2 (Mojo abandonné, .td specs déplacées dans `src/{kasm,meta}/`)

Les polices d'écritures de la charte graphique sont : Geist et Geist Mono, interdiction d'en utiliser d'autre (sauf pour les terminaux embarqués)
La barre de chat, les panels dépliables de gauche et de droite, les icônes en haut à gauche pour changer de section, ainsi que les icônes en haut à droite, sont des éléments immuables de la charte graphique Forge. Lorsqu'on construit une nouvelle interface ou un nouvel outil, il faut toujours les conserver.

**Branche unique V7** : `master` (héritière de `kraken/v7.0` post Φ.μ.7).
**Tests** : dernière suite complète documentée **1097 / 1097 PASS** lib core + Tauri PASS (cf. `CARNET.md` 2026-05-05). Session pivot 2026-05-06 vérifiée par `node --check`, `cargo check --bin forge-ui`, `cargo check --bin forge_mcp`.
**validate-features** : **70 / 70 PASS** (suite régression KASM ISA + features Wave 1-17).
**Doctrine** : pure Rust + `std` + `sha2`. **`git2` supprimé en γ.0.**
**Lab synthesis** : ~1 046 iter/sec (`lab_runner -- 10000`, atlas warm 166k entrées).
**Lab holdout-exact** : **95.4 %** (9537/10000 + 463 partiels, 0 erreur), 28/30 cibles à 0% miss.
**KASM ISA v1.3 + Wave 7i self-host + compute economy** : **74 opcodes** au total (v0.x scalar + v1.0 meta-ops + v1.1 Vec arithmetic + v1.2 Op::Fractal/Op::Eval + v1.2.1 Op::VGetI64 + v1.3 bit intrinsics + Op::Lazy/Op::Force). Les extensions 2026-05-08 sont exactes : elles ne changent pas la qualite des calculs, elles evitent de reconstruire/recalculer ce qui a deja une identite content-addressed.
**Λ trajectory (session 2026-05-05) — bloc B 100% complete** : `apply(func, input) → output` est maintenant l'opération singulière de Forge (commit `bb07b2d`) ; atlas runtime API à **1 keying scheme + 1 value-bearing kind** après M1+M2 collapse (commits `82230b1`→`d7848f0`) ; Λ.2 v2 self-host généralisé interprète n'importe quel programme 6-node sur subset {Input, Const, Add, Sub, Mul, Output} via dispatch dynamique (commit `7ee4bee`) ; Λ.3 v2 score K=4 examples en pure KASM (`1be35c5`) ; `apply_program` Tauri command exposée (`fa531eb`) ; `apply_subtree` sub-expression memoization opérationnelle (`aa53941`). 5 root nodes Hassabis-style inscrits en doctrine (cf. `ROADMAP.md`). 24 commits session. Foundation pour Bloc C (atlas distribué Root #1) une fois Forge local prêt-à-l'emploi.
**Φ.ν.8 atlas unifié (2026-05-03)** : bloc top-level [`src/atlas.rs`](src/atlas.rs) (~330 LoC, fichier format hérite de 9 kind tags pour rétro-compat lecture, runtime n'utilise plus que `kind::RESULT` post-M1.5+M2). Partagé `Arc<Atlas>` entre `ForgeBackend` (Tauri) et `MonsterNode` (lib core) via `attach_atlas`. Persiste tous les calculs cross-session sous une seule clé `(func_hash, input_hash)`.
**GPU stack autonome (Phase C, 2026-05-01)** : Forge possède sa propre couche CUDA via [`src/cuda_min.rs`](src/cuda_min.rs) — direct FFI vers `nvcuda.dll` (~700 LoC), kernel KASM universel pré-compilé en multi-cubin fatbin (sm_75/80/86/89/90, 67 KB embarqué). **Aucune dépendance à cudarc, aucun CUDA Toolkit requis chez les users** — juste le driver NVIDIA. Détection per-GPU automatique : NVIDIA → CUDA, AMD/Intel/Apple → WGPU.

**Pivot Forge MCP Compute (session 2026-05-06)** : Forge n'est plus pensé comme un logiciel piloté par boutons UI. Le Tauri desktop devient un **panneau d'observation** : historique de jobs, dropbox multi-CSV, graphique léger, transcript de calcul, panneau latéral de preuves. Le calcul est exposé à Claude/Codex/autres agents via [`examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs`](examples/forge_tauri_ui/src-tauri/src/bin/forge_mcp.rs). Un upload UI crée une session `pending` visible via MCP ; un agent peut ensuite réclamer le job, lancer Alpha reverse trading, écrire les résultats et rendre les artefacts disponibles sans injecter CSV/logs dans son contexte LLM. L'ancien objectif "bouton Start qui orchestre tout depuis l'UI" est **remplacé** par "agent MCP lance le compute, UI affiche ce qui se passe".

**Logs et preuves (session 2026-05-06)** : séparation stricte. Les logs internes d'audit (`job_id`, agent client, bytes, cache hits/misses, CPU/GPU dispatch, tailles d'artefacts, contexte non envoyé au LLM) vont au terminal sous préfixe `[forge-internal:<job>]`. L'application affiche uniquement les logs métier : parsing OHLC, VWAP/anchored VWAP, RSI/ATR/ADX/Stochastic, labels LONG/SHORT, entraînement de détecteurs, évaluation holdout. Les hashes, preuves et vérifications ne polluent plus le canva principal : un panneau latéral droit expose `Verify hashes`, `Download verification` et `Inject into MCP`.

> 📘 **Sessions Claude + Codex** : lire [`CLAUDE.md`](CLAUDE.md) en
> début de session. Le journal append-only des décisions vit dans
> [`CARNET.md`](CARNET.md). Plan tactique + science : [`ROADMAP.md`](ROADMAP.md).

**MCP safety 2026-05-07** : Codex local is installed through `C:\Users\quent\.codex\config.toml` with `[mcp_servers.Forge]` so `/mcp` displays the product name with a capital F. Every Forge MCP tool response carries `token_safety`; CSV content, full logs and heavy artifact content are not included by default. `read` returns a sanitized manifest summary, `logs` is cursor-based and capped, and lists are capped at 50 items.

**Agent-defined compute programs (2026-05-07)** : Forge MCP now lets agents create reusable compute programs from domain metric tags instead of relying only on fixed templates. `create` accepts JSON metrics or `<metric ...>` balises, normalizes the spec, hashes it as a content-addressed program, and stores it under `forge-store/programs`. `capabilities` exposes the builtin universal toolbox, `programs` lists reusable specs, `program` reads one by hash, and `execute` creates a content-addressed run from a program plus input references without returning source file content to the LLM. Builtin operators already cover universal/text/tabular/timeseries/biology basics (`bytes`, `line_count`, `byte_entropy`, `csv_profile`, `zscore`, `rolling_mean`, `rolling_std`, `correlation`, `correlation_delta`, `entropy`, `gc_content`, `kmer_count`, `kmer_collision_rate`). Unknown ops are accepted as custom hash-addressed extensions and recorded as `custom_unresolved` until an executor plugin/kernel/template is added. Legacy aliases `define` and `ops` remain accepted for compatibility.

**Unified metric execution path** : all metrics, whether they come from an existing Forge template or an agent-created program, follow the same contract: normalized metric + input content hashes -> `metric_hash` -> shared metric cache lookup -> executor -> `metrics.json` + `proof.json`. This avoids a two-speed architecture. Different operations can have different mathematical cost, but Forge does not create a special slow path for agent-authored programs; identical metric invocations reuse the same cached result across programs and sessions.

**Exact compute economy (session 2026-05-08)** : l'idee initiale de prefiltrage Lazy approximatif sur le synth Alpha a ete rejetee pour ne pas degrader la qualite des programmes. Le gain livre passe par des caches exacts et universels : `Program::new` memoise les graphes KASM valides par hash structurel, `Atlas::blob_result_key` donne une cle generique aux resultats blob, Alpha stocke la matrice brute H4 en un blob verifie au lieu de centaines de milliers de scalaires, et les programmes KASM de labels trade sont reutilises via `OnceLock`. Mesure NATGAS H4 : raw+labels ~30.8 s baseline -> ~4.7 s cold exact -> ~79 ms warm exact, sans changer les labels ni les scores.

**Canvas multi-LLM + Atlas + compiler universel (session 2026-05-08)** : Forge devient aussi un canvas de travail pour agents locaux. Codex, Gemini CLI et Claude Code CLI peuvent coexister dans la meme session, selectionner modele/effort depuis la barre de chat, recevoir le meme contexte compact, et utiliser les memes outils internes Forge. Les fichiers uploades restent dans la session courante, s'affichent comme cartes 2D/3D a droite du chat, et les agents doivent proposer/creer/lancer des programmes au lieu de lire les donnees brutes. Chaque programme et chaque balise metrique creee passe par `program_compile_validate_route`, est sauvegarde dans **My Atlas**, et tout run deja connu est reutilise instantanement par hash au lieu d'etre recalcule.

**Update 2026-05-09 - Provider workbench + Planet/GeoNode + store canonique** : Forge a maintenant un vrai workbench providers dans l'application, avec un seul terminal embarque pour Codex, Gemini et Claude, base sur PTY + xterm et pense pour Windows/macOS/Linux. Le chemin produit n'est plus "ouvrir PowerShell" mais "installer, connecter et lancer le CLI dans Forge". En parallele, le store canonique est fixe dans `./.forge-store` a la racine du repo pour que `cargo tauri dev` et la build release lisent les memes sessions, le meme Atlas et les memes artifacts. Cote spatial, Forge expose maintenant `planet_sphere` comme tool visuel, `GeoNode` et `MiniGeoNode` comme entites Atlas visibles par les LLM, et le globe Mars HD comme premier renderer concret, sans les anciens panneaux editorialises de Mars Magazine.

**Update 2026-05-09 - BOOM workspace 3D agentique** : Forge embarque maintenant un vrai workspace BOOM pour scene 3D dans `examples/forge_tauri_ui`. Le bouton etoile/BOOM ouvre une matrice 3D plein ecran transparente, avec grille infinie, gizmo, chat compact et panel gauche hybride Blender/Vectary. Le workflow est structure en deux grandes phases : **Design** puis **Slicer**. BOOM sait deja importer des meshes (`OBJ`, `STL`, `PLY`, `OFF`, `glTF`, `GLB`) cote frontend et peut normaliser des formats plus larges (`FBX`, `DAE`, `3DS`, `3MF`, `USD/USDZ`, etc.) vers `GLB` via Blender cote backend quand il est disponible. Le panel gauche expose un Outliner simplifie, un inspector contextuel, une stack de modifiers (`Mirror`, `Array`, `Inflate`, `Bevel`, `Subdivide`, `Solidify`), une barre de modes `Object / Vertex / Edge / Face`, un mode Slicer avec preview de couches, profils printer/material/quality et detection de machines. Au-dessus du runtime mesh, BOOM maintient maintenant une couche KASM explicite `cell / coordinate / vertex / edge / face / object / modifier`, des requetes de voisinage spatiales, des regions hash-addressed et un contrat UI universel pour que le LLM puisse manipuler la scene, les outils et les parametres comme des entites adressables.

## Forge actuel — canvas agentique local

Forge n'est plus seulement un backend MCP observe par une UI. Le produit courant combine :

- un **chat canvas** style Codex/Claude, propre et persistant, ou les messages restent lisibles pendant que les calculs continuent en arriere-plan ;
- une **barre multi-agent** : Codex, Gemini, Claude ou All, avec choix modele + effort, stop, microphone, fichiers, geonodes et programmes assignables ;
- un **provider workbench embarque** : un seul terminal noir incruste dans la matiere de la page, lanceur Codex / Gemini / Claude, installation/login/refresh dans Forge, et rendu PTY fidele plutot qu'un simple log texte ;
- une **integration locale prioritaire** : Codex passe par le runtime local deja authentifie, Claude par Claude Code CLI, et Gemini par Gemini CLI ou cle configuree. Le chemin produit privilegie les abonnements/logins locaux quand ils existent, pas une API pay-as-you-go par defaut ;
- des **reponses backend deterministes** pour salutations, aide Forge, erreurs connues et actions simples, afin de ne pas bruler des milliers de tokens CLI pour un dialogue basique ;
- des **sessions persistantes** : changer de session ne doit pas arreter un LLM, un programme ou un calcul en cours ; `cargo tauri dev` et la build desktop pointent maintenant vers le meme `./.forge-store` canonique ;
- des **cartes fichier 2D/3D** comme messages visuels : OHLCV -> price/time/volume + VWAP/EMA par defaut, 3D -> axes configurables, camera orbit lente, zoom au curseur, split view optionnelle ;
- une **surface spatiale** : `planet_sphere` comme tool visuel universel, `GeoNode` / `MiniGeoNode` dans l'Atlas, globe Mars HD comme premier renderer, geonodes injectables dans le chat et pills cliquables quand un lieu connu est cite ;
- un **workspace BOOM** : viewport 3D type Blender, panel hybride Blender/Vectary, Outliner + inspector, import mesh, selection composant, modifiers, slicer et regions KASM spatiales ;
- un **Atlas UI** (`My Atlas`) qui expose programmes, balises metriques, geonodes, mini-geonodes et runs reutilisables ;
- un **compiler de programmes universel** qui transforme les balises en graphe mathematique, valide les contrats, route les executors, verifie les dimensions/unites, produit un plan explicable et refuse les programmes vagues tant qu'ils ne sont pas reparables.

## BOOM - workspace 3D actuel

Le chantier BOOM n'est plus une simple experience visuelle. L'etat courant dans `examples/forge_tauri_ui` est :

- **Viewport** : matrice 3D transparente plein ecran, grille avec fondu distance, gizmo, orbite + pan au clic droit maintenu, origine recalee sur le canvas Forge.
- **Panel gauche** : Outliner simplifie type Blender en haut, inspector plus doux type Vectary en bas.
- **Workflow produit** : separation explicite `Design -> Slicer` pour modeler d'abord, preparer l'impression ensuite.
- **Import 3D** : upload via le `+` du chat ou drag-and-drop direct dans la matrice, avec rattachement au chat et affichage du mesh au centre.
- **Selection** : clic viewport synchronise avec l'Outliner, picking `object / vertex / edge / face`, overlay de surbrillance.
- **Modifiers** : `Mirror`, `Array`, `Inflate`, `Bevel`, `Subdivide`, `Solidify` avec preview directe.
- **Slicer** : profils printer/material/quality, preview de couches dans le viewport, detection initiale de machines cote Tauri.
- **KASM scene layer** : topologie symbolique `cell / coordinate / vertex / edge / face / object / modifier`, regions spatiales hash-addressed, preview slicer annote par cellules, seeds geonodes symboliques.
- **UI contract** : chaque bouton, champ et parametre visible BOOM recoit un hash KASM stable et un nom d'outil logique, exposes au runtime pour le pilotage LLM.
- **Console laterale** : le chat reste dans la barre native Forge ; le panneau droit repliable expose maintenant `Verification / Console`, avec une console de script `Rust / KASM`, injection de contexte BOOM (selection, region, graphe KASM, contrat UI) et preparation de payloads outilles.

Ce qui reste ouvert dans BOOM n'est plus la base du workspace, mais les grosses briques avancees : box/lasso selection de region, vrais geonodes proceduraux, outillage rigging type Auto-Rig Pro, execution reelle de la console Rust/KASM, et envoi slicer/export machine de bout en bout.

## Pitch

Le projet construit, étage par étage, ce que les compilateurs traditionnels externalisent :

| Étage | Promesse |
|---|---|
| **L0 — KASM** | Bytecode pur, **74 opcodes** (v0.x + v1.0 meta + v1.1 Vec + v1.2 self-hosting + v1.3 bit/lazy), 8 octets/nœud, vérifié, mémoïsable, JIT x86-64. Programme = 1 hash, 1 résultat, partout. |
| **L1 — Numérique** | f32/f64 sortis du contrat. Rational exact, Posit16/32. Mur d'associativité réel mais documenté. |
| **L2 — Tenseur** | Multi-dtype (F32 + Posit16 + Posit32 + Rational), même surface que KASM-Int. **NumericContract** content-addressé pour reproductibilité bit-exact (Φ.μ.7). |
| **L3 — Méta** | Calcul des Constructions content-addressed (`meta::Term` + `kasm_embed` + `mlir_bridge`). |
| **L4 — Réflexif** | Agent symbolique non-LLM (9 règles algébriques), pattern-matcher Term. Pas de tokenizer. |
| **L5 — Gödel-machine** | Boucle d'auto-amélioration formellement prouvée. Jour 0 : 2026-04-27. |
| **L6 — Réversible** | Coût Landauer first-class, modèles hardware (CMOS / Adiabatique), 5/32 opcodes strictement bijectifs. |
| **L7 — Ghost storage** | Snapshot content-addressed (`introspection/snapshot.rs`). |

## V7 — état actuel

**Codebase** : ~27 000 lignes (-26 % vs V6 baseline 36 925).
**Dépendances externes** : 1 (`sha2`).
**Hot path miss** : ~5 600 ns/miss (`monster_miss_bench`, -31 % vs V6).
**Hot Atlas L1** : 65 395 entrées persistées (`.codex-tmp/hot-atlas.bin`).
**Atom catalogue** : 183 atomes uniques minés, 148 universels (≥ 2 familles), 13 atomes en 10/30 familles.

**Sauts majeurs V8 (session 2026-05-02, Wave 1-17 livrées en mega-bundle)** :
- **Wave 1** Oracle×100 : Π.1 NNUE int8 + Π.3 Mathematica rewrite + Π.8 Datalog seminaive
- **Wave 2** Speed Bundle : Σ.3 bump alloc + Σ.4/Π.6 NaN-boxing + Π.4 LMAX Disruptor + Π.5 Forth threaded code + Π.7 TigerBeetle static pool + Π.13 Lua tables + audits Σ.8/9/10
- **Wave 3** Π.2 Cranelift-style SSA IR (~1024 LoC, builder + verifier + peephole + KASM lowering, zéro `cranelift-codegen` dep)
- **Wave 4** Data Layout : Π.9 Q/Kdb+ columnar storage + Π.10 APL/J rank semantics avec broadcasting NumPy
- **Wave 5** Concurrency : Π.11 Erlang hot code swap + Π.12 Go goroutines M:N green scheduler
- **Wave 6** Via Negativa Heavy : audit hot path certifié 0/0/0 (`&mut self` / `Box<dyn>` / `Arc::new`/call)
- **Wave 7** Via Negativa Light : 24 cuts + 2 deletions, **Δ -584 LoC** (AGENTS.md, colony.rs, 21 unused imports)
- **Wave 8 FULL** Self-Hosting : Op::Fractal=64 + Op::Eval=65 ISA-level + bytecode interpreter dispatch via `FractalDispatcher` trait + `SelfHostingRuntime` avec callee table
- **Wave 9** Π.14 CompCert-style proofs : witness types `Terminating/NoUB/Pure/Deterministic`, wrapper `Proven<T, W>`, type-level enforcement (compile-time refusal of unproven programs)
- **Wave 10** `.cas portable` : `Store::snapshot_to(target)` + `verify_portable_format()` + tests endian-explicit
- **Wave 11** Trading Foundation : Π.16 fixed-point Q31.32 + Π.17 timestamp arithmetic + Π.18 OHLCV columnar + Σ.14 errno-style errors
- **Wave 12** Strategy & Execution : Π.20 OrderBook L2/L3 + Π.21 tick→bar resampler + Π.22 strategy graph DSL + Π.24 VWAP/TWAP simulator
- **Wave 13** Statistical : Π.19 reservoir sampling + Π.23 walk-forward optimization + Σ.11 monomorphization audit + Σ.12 Swiss tables HashMap
- **Wave 14** Pure Speed Ablation : Σ.13 StackStr + Σ.15 ManuallyDrop arena + Σ.18 inline_always + audits Σ.16/17/19/20
- **Wave 15** RAM Hot Path Foundation : Σ.21 boot prefault + Σ.23 seqlock + Π.26 arena lifetimes + Π.29 slab allocator
- **Wave 16** RAM CAS + mmap : Π.25 MmapStore zero-copy + Π.27 IntrusiveBlobIndex 16B/entry + Σ.22 huge pages 2MB hint API
- **Wave 17** Cross-process Swarm + CoW : Π.28 SwarmRegistry shared Arc<Store> + Π.31 CowSnapshotter O(1) Arc::clone snapshots

**Sauts majeurs V7** :
- α : -12 244 lignes mortes (`numeric/posit_lut`, `bigint`, `interval`, `meta/{inductive,typecheck,universe,tactic,bootstrap,reduce}`, `agent/bandit_agent`)
- β.1-β.4 : pipeline hot path 7 → 1 layer + reverse-index opt-in
- γ.0 : `store.rs` réécrit (append-only `forge.cas`), **suppression `git2`** + `scan_db.rs`
- γ.1 : cache-line alignment + `PhysicalEnvelope` (énergie dans le content-address)
- Lab-A/B/C/D : retrieval polynomial + glyph segmentation inverse + ultra_glyph compositions + kill-switch holdout. **41.7 → 4322 iter/sec** (×103). Doctrine : *le meilleur calcul est celui qu'on ne fait pas*.
- η.0 : lifter binaire x86_64 → KASM dans `corpus.rs` (allow-list stricte)
- Φ.μ.1-3 : recognizers algébriques + nano_probe gen-2 (183 atomes universels minés)
- Φ.μ.5 : Tier 1 aging primitives (Michaelis-Menten, Hill, Beer-Lambert, Arrhenius, Logistic)
- Φ.μ.7 : 3 primitives extraites des branches abandonnées (`nanocube_pack_recipe_i64`, `NumericContract`, `InlineCache`+`StridePredictor`)
- **Φ.ν.8 (2026-05-03) — Atlas unifié + pipeline cross-session** : voir section "Pipeline" ci-dessous.

## Pipeline actuel (2026-05-09 — canvas multi-agent + MCP/dynamic tools)

```text
CSV(s), documents, programs ou intention depuis UI ou agent
        │
        ▼
Forge session manifest
  status | title | file_paths | programs | visual_maps | context_accounting
        │
        ├─ Canvas Tauri
        │    - chat persistant facon Codex/Claude
        │    - provider workbench integre : Codex / Gemini / Claude / All
        │    - cartes 2D/3D de fichiers dans la session courante
        │    - visual programs, Planet tool, My Atlas, logs mathematiques lisibles
        │    - proof panel droit : hashes, manifest, artifacts, mapping
        │
        └─ MCP + dynamic tools
             - détecte `clientInfo` depuis initialize
             - contexte compact par session, sans donnees brutes
             - Atlas lookup avant creation/run
             - compile/validate/route les programmes agent-created
             - lance compute/visual programs et ecrit artifacts/preuves
             - logs internes -> terminal `[forge-internal:<job>]`
             - logs metier/maths -> cartes live et fichiers job
```

Le Tauri backend conserve des commandes d'observation et d'intégration :

- `create_forge_pending_job` : upload UI → session pending visible par MCP.
- `list_forge_jobs` : alimente l'historique, pinned/recents et statuts.
- `read_forge_job_log` : transcript live métier.
- `read_forge_job_manifest` : source de vérité pour hashes/résultats/preuves.
- `read_forge_job_file` : recharger le CSV primaire pour graphique/vérification.
- `read_forge_job_artifact_text` : télécharger depuis l'UI des artifacts texte bornés (`visual_mapping`, `proof`, `metrics`, `3d index`, manifest/log), sans ouvrir arbitrairement les données lourdes.
- `publish_forge_job_to_mcp` : marque le résultat comme disponible à l'agent via `mcp_result`.
- `update_forge_job` : pin/unpin, rename, archive, delete.
- `list_forge_programs` : alimente l'overlay UI **Programs** depuis `forge-store/programs` avec des resumes compacts : titre, domaine, hash, metriques, readiness builtin/custom, sans contenu source ni depense token inutile.
- `list_forge_capability_templates` : hydrate le picker **Create program** depuis le vrai catalogue `capabilities` de `forge_mcp`, avec fallback local si le MCP n'est pas encore pret.
- `create_forge_program` : fait de l'UI un client MCP local pour `create` ; le formulaire **Create program** envoie le Metric DSL a `forge_mcp`, qui normalise, valide, hash et stocke le programme.
- `run_forge_program` : fait de l'UI un client MCP local ; le bouton `Programs > Run` appelle `forge_mcp` en stdio et laisse le serveur MCP creer le job, les logs, `metrics.json` et `proof.json`.
- `export_forge_3d_artifacts` : persiste les mappings 3D visibles dans l'UI en artifacts `.ply` + index `.json`, ecrit un contrat `visual_mapping` lie au resultat, puis les inscrit dans le manifest sans jamais inliner le nuage de points dans le contexte LLM.
- `get_forge_atlas_overview` : hydrate l'onglet **Atlas > My Atlas** avec programmes, balises metriques et runs deja materialises, filtrables par query/kind.
- `program_compile_validate_route` / `forge_compile_validate_route` : compile un programme en graphe de metriques, valide contrats/dimensions/objectifs, route chaque op vers un executor et produit le plan explicable avant stockage/run.

Surface MCP `forge_mcp` visible aux agents : **9 tools maximum**.

- `about` : point d'entree doctrine quand l'agent hesite, quand l'utilisateur demande "a quoi sert Forge", ou quand une tache sent gros fichier/calcul/preuve/artifact.
- `capabilities` : GPS compact avant `create` ou `run`; mappe domaine/intention/dataset vers templates, metriques et prochain appel.
- `create` : cree un programme reusable depuis une intention naturelle, des balises metriques (`<metric name="..." op="..." input="..." />`) ou un tableau JSON.
- `program_compile_validate_route` : compile, valide, route et explique un programme ou une intention avant stockage/run ; outil reflexe pour toute creation complexe ou tout programme `needs_repair`.
- `run` : dispatcher unique pour pending upload, programme de bibliotheque (`program`, `program_title`, `program_query` ou `program_hash`), capability/template, CSV direct, ou intention naturelle avec `plan_only`.
- `jobs` : trouve les sessions pending/running/completed sans enumerer les fichiers store/logs/CSV.
- `read` : lit les resumes compacts, programmes, previews, preuves, hashes et artifacts telechargeables, y compris les mappings 3D `.ply`, sans renvoyer les donnees lourdes au LLM.
- `logs` : stream les logs metier par curseur byte offset, sans charger le fichier complet.
- `cancel` : annule proprement un job pending/running sans tuer arbitrairement des processus.

Descriptions MCP : les tools sont decrits en mode action-first pour maximiser l'usage naturel par Claude/Codex/Gemini/autres agents. Audit 2026-05-08 : chaque description contient des marqueurs explicites `FIRST CALL`, `TRIGGER WHEN`, `CALL BEFORE`, `DO NOT`, des domaines larges, les anti-patterns Read/shell/log full/artifact inline, et le workflow reflexe `capabilities -> run plan_only -> program_compile_validate_route -> create/run -> logs/read`. Objectif : faire declencher Forge avant qu'un agent ne lise un gros fichier ou ne se lance dans des calculs longs en contexte LLM.

Les anciens noms (`execute`, `alpha`, `programs`, `program`, `pending`, `artifacts`, `inject`, `rename`, `docs`, `doc`, `preview`, `sessions`, et les anciens `forge_*`) restent acceptes comme alias caches pour compatibilite, mais ils ne sont plus retournes par `tools/list` hors surface officielle. La surface visible reste stable meme si Forge ajoute 100+ capabilities internes.

Doctrine agent : utiliser Forge avant de lire/calculer dans le contexte LLM quand l'input est gros, repetitif, couteux, scientifique, numerique, document-heavy ou quand le resultat doit etre verifiable. L'agent doit d'abord verifier l'Atlas, puis planifier, compiler/valider/router, creer ou reparer, et seulement ensuite lancer. `capabilities` et `run { intent, inputs, plan_only:true }` produisent un planner automatique ; `program_compile_validate_route` transforme ce plan en contrat executable ; `create` stocke le programme reusable ; `run` lance les programmes existants ou agent-created depuis la bibliotheque.

My Atlas : chaque balise metrique, programme cree, geonode cree et run termine est sauve sous `.forge-store/atlas/my_atlas.json` avec hashes, domaine, objectif, dependances, status et references de resultats. Si un agent pioche dans My Atlas un programme ou une balise deja run avec les memes inputs/params, Forge renvoie un hit immediat : le resultat est disponible sans relancer le calcul. Les agents recoivent ce resume dans `forge_session_context` et peuvent interroger `forge_atlas_overview` / `get_forge_atlas_overview` par `query` et `kind` (`program`, `metric_tag`, `run`, `geonode`, `mini_geonode`).

Construction de programme : un programme agent-created ne doit plus "fail and cancel". S'il manque une formule, une unite, un executor ou une dependance, Forge le marque `needs_repair`, expose les erreurs du compiler/linter, et le LLM doit le corriger avant run. Les micro-evenements de construction sont interleaves dans le chat : creation de balise, appel tool, modification, compilation, validation, routage executor, sauvegarde Atlas. Pendant la construction, l'agent explique brievement ce qu'il fait, comme Codex quand il code.

Authentification LLM / couts : Forge distingue explicitement les wrappers par abonnement des APIs pay-as-you-go. **Codex/OpenAI** vise le runtime local Codex deja connecte au compte ChatGPT/Codex de l'utilisateur ; Forge ne demande pas de cle `OPENAI_API_KEY` pour ce mode et les compteurs affiches sont des compteurs d'usage/contexte, pas une facture OpenAI API. **Claude** vise Claude Code CLI avec login navigateur Claude.ai Pro/Max quand disponible ; une cle API Anthropic peut exister comme mode optionnel, mais doit etre etiquetee pay-as-you-go. **Gemini** vise Gemini CLI ou configuration locale ; une cle Gemini API saisie dans les settings est un mode API separe. Le chemin UX cible est le workbench providers embarque : un seul terminal PTY/xterm dans Forge, auto-install du CLI si possible, et aucun recours obligatoire a une fenetre PowerShell externe. Le contrat produit n'est pas "gratuit illimite" : c'est usage via abonnement/login local quand le provider le permet, dans ses limites, avec Forge qui evite surtout les appels et tokens inutiles.

Planet / GeoNode : Forge distingue maintenant clairement les couches spatiales. `planet_sphere` est un tool visuel universel utilisable dans un visual program ; ce n'est pas un programme autonome. Les coordonnees nommees deviennent des `GeoNode`; les sous-lieux precis deviennent des `MiniGeoNode` avec `parent_geonode`; une node metrique classique peut se lier a une geonode via `geo_ref` / `geo_refs`. Le globe Mars HD est le premier renderer concret : il tourne par defaut, zoome doucement vers une geonode ciblee, injecte des pills cliquables dans le chat pour les lieux cites, et sert de preuve visuelle sans embarquer les anciens panneaux "articles" de Mars Magazine.

UI Programs : le panneau `Programs` lit maintenant la vraie bibliotheque de programmes content-addressed creee par MCP. Chaque ligne affiche le nom, le hash court, le domaine, le nombre de metriques et l'etat d'execution (`builtin`, `custom`, `needs ops`). `Create program` sauvegarde un nouveau programme depuis des balises Metric DSL via `forge_mcp create`; son picker de templates interroge `capabilities` pour rester synchronise avec les domaines Forge, puis retombe sur des scaffolds locaux si le MCP n'est pas pret. `Plan` copie un appel `run ... plan_only:true`; `Run` execute vraiment via `forge_mcp` local en reutilisant le fichier du job selectionne quand il existe. L'UI reste un observateur/client : elle ne renvoie pas le corps complet des specs ni les donnees source au LLM.

Artifacts 3D : la vue 3D produit des mappings exportables (`phase`, `heightmap`, `manifold`, `lattice`) en `.ply` content-addressed. Ces fichiers sont references dans `visualization_3d`, `artifacts_3d`, `visualization_3d_index_path`, `visual_mapping` et `visual_mapping_path`. `visual_mapping` est le contrat qui relie une vue a ses metriques/resultats/proofs/hash : axes, encodage couleur/taille, selection contract et artifacts sources. Les agents les recuperent via `read { job_id, kind:"artifacts" }` puis peuvent les injecter/attacher dans leur logiciel par reference locale, sans copier les points 3D, metrics ou proofs dans le chat.

Visual mapping contract : tout job de programme cree par agent produit maintenant `metrics.json`, `proof.json` et `<job>.visual_mapping.json`. Le mapping transforme les resultats de metriques en noeuds selectionnables (`metric_tag`, `op`, `status`, `metric_hash`, `elapsed_ms`, `cache_hit`) lies aux artifacts de preuve. Pour les jobs Alpha/3D, le meme contrat pointe vers les `.ply` exportes. Objectif produit : chaque visualisation doit etre une vue d'un resultat verifiable, pas une decoration UI isolee.

UI proof panel : la section `Visual mapping` affiche si le contrat existe, combien de vues/artifacts 3D sont liés au résultat, les hashes/paths compacts et l'index 3D. Les actions `Download visual mapping` et `Copy artifact refs` fournissent le contrat ou les références compactes, pas le nuage de points ni les fichiers lourds.

UI 3D result overlay : la vue 3D affiche maintenant, directement sur le canvas transparent, le resultat lie au job selectionne : mode de projection, statut, bars, holdout/trades/PnL/hash quand disponibles, nombre de vues et references compactes `visual_mapping`/`.ply`. Ce n'est pas une card de resultats separee : les resultats numeriques complets restent dans les logs metier et les preuves/hash dans le panneau droit.

3D interactive selection : un clic sur la vue 3D selectionne le point le plus proche avec un picking CPU leger sur les positions deja calculees. L'UI affiche un anneau discret sur le point, ajoute `Pick` dans l'overlay, et le panneau droit recoit une section `3D selection` : mode, vertex/point, bar ou cellule, coordonnees normalisees, artifact hash et mapping hash/path. La selection est une reference compacte ; elle ne lit ni n'injecte le CSV, le nuage de points ou les proofs complets.

Catalogue interne : Forge garde une surface MCP compacte, mais `capabilities` expose un registre de templates internes (`template_registry`) pour router les domaines sans polluer l'inventaire MCP : alpha strategy, anomalies/regimes timeseries, large CSV profiling, k-mer sequence/hash, source code metrics, hash quality lab, document comparison, chemistry molecular metrics, medical signals, engineering sensors, aerospace telemetry, simulation sweeps, math optimization et energy/grid timeseries. Un template peut etre `runnable` via executor existant ou `create_then_run` via Metric DSL scaffold.

Politique input-aware universelle : chaque plan indique si l'agent doit demander un fichier/dataset/artifact ou partir en mode libre. S'il n'y a pas encore d'input, Forge renvoie toujours une question prete a poser : "Do you have a file/dataset/artifact for Forge to analyze, or should Forge start in free mode with a synthetic/no-input program?" Le mode fichier produit des resultats ancres dans les donnees utilisateur ; le mode libre laisse l'agent creer/lancer un programme synthetique : simulations, samples generes, benchmarks, espaces de recherche mathematiques, toy datasets, etc. La crypto/security n'est qu'un exemple de domaine avec `security_crypto` et ses metriques synthetiques (`synthetic_hash_avalanche`, `synthetic_hash_collision_rate`, `synthetic_hash_bit_bias`), pas une architecture speciale.

Forge Metric DSL v1 : les balises de metriques sont la grammaire universelle des programmes agent-created. Un programme est un DAG content-addressed de noeuds `<metric>` :

```xml
<metric id="volume_z" kind="transform" domain="finance"
  op="zscore" inputs="volume" output="volume_z"
  dtype="timeseries" unit="sigma" window="48" />

<metric id="candidate_rank" kind="score" domain="finance"
  op="weighted_score" inputs="win_rate,pnl,drawdown,volume_z"
  output="rank" dtype="table"
  params='{"weights":[0.35,0.35,-0.2,0.1]}' />
```

Champs DSL : `id`, `kind`, `domain`, `op`, `inputs`, `output`, `dtype`, `unit`, `params`, `constraints`, `cache`, `proof`, `if`, `goal`, `description`, `weight`. `kind` est une grammaire fermee (`input`, `transform`, `aggregate`, `compare`, `score`, `select`, `simulate`, `optimize`, `validate`, `prove`, `export`). Les domaines et ops restent ouverts. `dtype/cache/proof` acceptent les valeurs connues ou `custom:<name>`. Forge normalise le graphe, refuse les doublons/cycles, hash chaque programme et reutilise les calculs identiques.

Readiness des programmes : `create`, `read { program_hash }` et `run` exposent `execution_readiness`. Forge distingue les metriques builtin executables immediatement, les metriques `custom_unresolved` conservees comme points d'extension content-addressed, et les metriques invalides sans `op`. Un programme mixte peut tourner : les metriques builtin calculent, les custom restent explicites dans les artefacts jusqu'a ajout d'un executor/template/kernel. L'agent ne doit pas presenter une metrique custom unresolved comme un resultat calcule.

Garde-fous tokens MCP :

- Chaque reponse outil contient `token_safety`.
- Chaque reponse outil contient aussi `workflow_guidance` : l'agent recoit le prochain appel recommande selon l'etat reel (`capabilities`, `plan_only`, programme cree, job pending/running/completed, log tail).
- Chaque reponse outil contient `tool_selection_policy` : les tools visibles, les regles de routage et les anti-patterns a eviter.
- `csv_included=false`, `source_content_included=false`, `full_log_included=false`, `artifact_content_included=false`.
- `agent_must_not_shell_read_user_inputs=true` et `agent_must_not_debug_forge_source_for_user_jobs=true`.
- Les manifests sont sanitises : champs lourds (`candles`, `rows`, `features`, `labels`, `predictions`, `logs`, `csv_content`, etc.) retires si presents.
- Les listes sont bornees a 50 items maximum.
- Les logs se lisent uniquement via `logs { job_id, cursor }` avec curseur et limite stricte.
- La regle produit reste : reponse MCP = index/resume/reference ; disque = donnees lourdes ; UI = affichage humain live.

Installation Codex locale officielle (`C:\Users\quent\.codex\config.toml`) :

```toml
[mcp_servers.Forge]
enabled = true
command = "C:\\scan-shared-target\\debug\\forge_mcp.exe"
cwd = "C:\\Users\\quent\\Documents\\GitHub\\Forge"
startup_timeout_sec = 20
tool_timeout_sec = 7200

[mcp_servers.Forge.env]
FORGE_STORE_DIR = "C:\\Users\\quent\\Documents\\GitHub\\Forge\\.forge-store"
FORGE_MCP_MODEL = "codex"
```

Nom d'affichage MCP : utiliser la cle de serveur `Forge` avec un F majuscule. Les clients MCP comme Claude/Codex affichent souvent la cle de config (`forge`) quand ils listent `/mcp`, avant meme de lire `serverInfo.name`. Pour migrer une config existante, renommer `[mcp_servers.forge]` en `[mcp_servers.Forge]` et `[mcp_servers.forge.env]` en `[mcp_servers.Forge.env]`, puis redemarrer Codex/Claude.

Installation doctrine agents Claude/Codex :

```powershell
# Windows PowerShell : detecte les sources globales C:\Users\<user>\.claude et .codex
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-forge-doctrine.ps1

# Applique le bloc doctrine Forge dans les fichiers agent globaux
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-forge-doctrine.ps1 -Apply

# macOS/Linux avec PowerShell 7+ : detecte ~/.claude et ~/.codex
pwsh -NoProfile -File scripts/install-forge-doctrine.ps1
pwsh -NoProfile -File scripts/install-forge-doctrine.ps1 -Apply
```

Le helper vise les sources globales des agents, pas le repo Forge : `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md`, `~/.codex/CLAUDE.md`, et sur macOS `~/Library/Application Support/Claude/CLAUDE.md` si le dossier existe. Il est idempotent : il insere/remplace uniquement le bloc marque `FORGE_MCP_DOCTRINE_START/END`. La phrase doctrine rappelle aux agents d'utiliser Forge MCP avant de lire des gros CSV/sources/logs ou de faire des calculs couteux dans le contexte LLM.

Le pipeline Alpha reverse trading actuel :

```text
OHLCV CSV
  → parse candles
  → pre-start analysis obligatoire (plan + features/atlas readiness)
  → raw Alpha features : VWAP, anchored VWAP, RSI, ATR, ADX, Stochastic
  → binary LONG/SHORT labels depuis SL/TP/horizon
  → dual-classifier synthesis par feature
  → pairing LONG/SHORT
  → holdout + walk-forward metrics
  → manifest + proof material + logs métier
```

Les anciennes notes ci-dessous documentent l'héritage atlas/Tauri pré-pivot. Elles ne doivent plus être lues comme architecture produit UI : le produit courant est **MCP-first, UI observer**.

## Pipeline historique (Φ.ν.8 — atlas-backed cross-session, pré-pivot MCP)

Le pipeline complet, du moment où l'utilisateur uploade un fichier
jusqu'à la mémoïsation cross-session de chaque sous-calcul. Les blocs
sont reliés par un `Arc<Atlas>` partagé.

```
┌──────────────────────────────────────────────────────────────────┐
│ FRONTEND (examples/forge_tauri_ui/ui)                            │
│   index.html + app.js + styles.css                               │
│                                                                  │
│   kindSelect.change  ┐                                           │
│   handleFiles        ├─→ refreshPlanPreview(kind, file, logFn)   │
│   alphaHandleFiles   ┘   └─→ tauri.invoke("inspect_program_map") │
│                                                                  │
│   logPlanReport(logFn, report) → emits to "forge-log" channel    │
└──────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│ TAURI BACKEND (examples/forge_tauri_ui/src-tauri/src/main.rs)    │
│                                                                  │
│   struct ForgeBackend {                                          │
│     node:          MonsterNode                                   │
│     atlas:         Arc<scan::atlas::Atlas>   ← shared with node  │
│     programs:      HashMap<&str, Hash>                           │
│     run_cache:     HashMap<(kind,mode,file), CachedRun>          │
│     inspect_cache: HashMap<(kind,file), ComputationPlanReport>   │
│   }                                                              │
│                                                                  │
│   #[tauri::command] inspect_program_map(kind, bytes_opt)         │
│   #[tauri::command] start_computation(bytes, kind, mode)         │
│   #[tauri::command] start_alpha_synthesis(csv_bytes, params)     │
│                                                                  │
│   ── Redundancy mining (synth path) ─────────────────────        │
│   enumerate_synth_candidates_d2(examples)                        │
│   enumerate_synth_candidates_d3(examples)                        │
│   cse_classify_candidates_with_reps(candidates)  → CseStats      │
│   analyze_subtree_redundancy(candidates)         → SubtreeStats  │
│   trace_classify_reps(reps, sample_inputs)       → TraceStats    │
│   synth_atlas_warm_estimate(node, reps, inputs)  → (peeks, hits) │
│   analyze_subwindow_redundancy(bar_count)        → SubwindowStats│
│                                                                  │
│   ── ComputationPlan (DNA path) ────────────────────────         │
│   ComputationPlan::build(node, raw_calls)                        │
│     pass 1 : FNV-1a (func || args) input dedup                   │
│     pass 2 : per-program CSE class (semantic_fingerprint)        │
│     pass 3 : node.peek_call (atlas_known)                        │
│     pass 4 : atlas.lookup_result (atlas_result_known)            │
└──────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│ ATLAS UNIFIÉ (src/atlas.rs — top-level, ~330 LoC)                │
│                                                                  │
│   pub struct Atlas {                                             │
│     path:    PathBuf,                                            │
│     file:    Mutex<File>,        ← append-only                   │
│     seen:    RwLock<HashSet<[u8;33]>>   ← kinds 1-4 (key only)   │
│     values:  RwLock<HashMap<(u8,[u8;32]),[u8;20]>> ← kinds 5-9   │
│   }                                                              │
│                                                                  │
│   API générique :                                                │
│     open(path) → io::Result<Self>                                │
│     record(kind, hash) → io::Result<bool>                        │
│     contains(kind, hash) → bool                                  │
│     record_with_value(kind, key, value_20B) → io::Result<bool>   │
│     lookup_with_value(kind, key) → Option<[u8;20]>               │
│     count_kind(kind) → usize                                     │
│     total() → usize                                              │
│                                                                  │
│   Helpers de packing :                                           │
│     result_key(func_bytes, input_bytes) → [u8;32]                │
│     feature_key(file_hash, feature_id, bar_index) → [u8;32]      │
│     trade_key(file_hash, bar, dir, sl, horizon) → [u8;32]        │
│     opmemo_key(op_byte, input_i64) → [u8;32]                     │
│     pack_f64 / pack_i64 / pack_trade / pack_u64 / unpack_*       │
│                                                                  │
│   Kinds (post M1.5+M2 collapse) :                                │
│     1-4 : CSE/TRACE/SUBTREE/PEEK — hash-only records             │
│           (33-byte) for upload-time analysis tracking            │
│     5   : RESULT — UNIQUE value-bearing kind, 53-byte records    │
│           (func_hash || input_hash) → 20-byte value              │
│     6-9 : FEATURE/TRADE/SCORE/OPMEMO legacy tags ; reader        │
│           still parses them for old files but new writes are     │
│           always RESULT (tags 6..=9 will disappear from atlas    │
│           files as old entries get re-warmed under RESULT)       │
└──────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│ MONSTERNODE (src/monster/) — runtime KASM hot path               │
│                                                                  │
│   pub struct MonsterNode {                                       │
│     store, governor, cache, programs, lru, oracles,              │
│     stats_atomic, op_memo, gpunode_runtime, ...                  │
│     atlas: RwLock<Option<Arc<Atlas>>>   ← attach_atlas() at boot │
│   }                                                              │
│                                                                  │
│   pub fn attach_atlas(arc: Arc<Atlas>)                           │
│   pub fn atlas() → Option<Arc<Atlas>>                            │
│   pub fn peek_call(func, args) → io::Result<bool>                │
│                                                                  │
│   ── apply (src/apply.rs) — Λ.0 singular operation ───────────   │
│     pub fn apply(node, func, input) -> Vec<u8>                   │
│       1. input_hash = Hash::for_blob(input)  (Λ.1 fix aliasing)  │
│       2. atlas.lookup_result(func || input_hash) → if hit, return│
│       3. kasm::execute, store output, atlas.record_result        │
│                                                                  │
│   ── dispatch_batch (src/monster/dispatch.rs) ───────────────    │
│     for each call (M1.1+M2 unified hashed-input keying) :        │
│       1. atlas.lookup_result(func || hash(args)) → if hit, Hit   │
│       2. CPU-fast bypass (auto-router v0/v1/v2)                  │
│       3. brain layer cascade via try_lookup_call                 │
│       4. bulk evaluator on miss (CUDA / WGPU / SIMD)             │
│            atlas.record_result on each computed result           │
│                                                                  │
│   ── dispatch_impl (src/monster/exec.rs) ─────────────────       │
│     Layer 0 : StaticOutput                                       │
│     Layer 1 : RAM cache (RamKey)                                 │
│     Layer 2 : wire-key (lazy)                                    │
│     Layer 3 : structural rule (HotPlan)                          │
│     Layer 4 : learned oracle                                     │
│     Layer 5 : disk memo (forge.cas)                              │
│     Layer 5b: atlas RESULT cross-session (Φ.ν.8 + M1.1+M2)       │
│     Layer 6 : execute_with_op_memo (slow lane)                   │
│            ↳ Hash64 : L1 op_memo + L2 atlas RESULT (M1.5+M2)     │
│                                                                  │
│   ── train.rs synthesize_i64 ────────────────────────────        │
│     push_binary(.., targets_fp, scratch, atlas) :                │
│       1. fill scratch (reusable Vec) with outputs                │
│       2. skip if all outputs constant (degenerate)               │
│       3. atlas.lookup_with_value(RESULT, score_key) → reuse loss │
│       4. else compute loss + atlas.record_with_value(RESULT, ..) │
└──────────────────────────────────────────────────────────────────┘
                                │ (callers from synth_strategy.rs)
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│ SYNTH STRATEGY (examples/forge_tauri_ui/.../synth_strategy.rs)   │
│                                                                  │
│   pub struct FeatureCache {                                      │
│     prefix_close, prefix_typical, prefix_typical_pv,             │
│     prefix_volume, prefix_gains, prefix_losses,                  │
│     prefix_tr, prefix_plus_dm, prefix_minus_dm                   │
│   }                                                              │
│                                                                  │
│   FeatureCache::build(bars)         ← O(N) prefix sums           │
│   FeatureCache.sma/rsi/atr/vwap/adx ← O(1) per bar               │
│   (M1.3 : persist_to_atlas dropped — was write-only waste)       │
│                                                                  │
│   simulate_trade_with_atlas(bars, i, dir, sl, horizon,           │
│                              atlas, file_hash) → TradeResult     │
│       1. atlas.lookup_with_value(RESULT, key) → if hit, return   │
│       2. else build is_hit[6] + pnl_horizon, run KASM trade      │
│          programs (Step 0.6) via compute_trade_kasm              │
│       3. atlas.record_with_value(RESULT, key, packed)            │
│                                                                  │
│   build_examples_in_range_masked_with_atlas(bars, range, cfg,    │
│                                              mask, atlas, fhash) │
│       → calls simulate_trade_with_atlas (FeatureCache in RAM)    │
│                                                                  │
│   eval_strategy_full → predict_label callback                    │
│       → routes via MonsterNode::call_one_i64 → dispatch_impl     │
│       → atlas RESULT lookup at Layer 5b                          │
└──────────────────────────────────────────────────────────────────┘
```

**Connections clés** :

- **`Arc<Atlas>` partagé** : `ForgeBackend::new` ouvre `forge.atlas`,
  appelle `node.attach_atlas(Arc::clone(&atlas))`. Backend ET MonsterNode
  voient le même état persistant.
- **Cross-session** : test `dispatch_batch_persists_results_across_sessions_via_atlas`
  prouve runtime que session 2 retourne le résultat de session 1 sans
  invoquer le bulk evaluator (assertion stricte `recorder.seen.len() == 0`).
  + 3 tests `apply::tests::*` qui exercent la singular operation Λ.0.
- **Sub-window** : `FeatureCache` (O(N) prefix sums + O(1) per query)
  est la voie canonique post-M1.3+ ; les anciennes boucles `compute_*`
  O(K) ont été supprimées (legacy path purgé, FeatureCache seule
  source de vérité).
- **Static skip** : `train.rs::push_binary` drop `Mul(x,0)≡0`, `Sub(x,x)≡0`,
  etc. avant scoring. Le synth ne dépense plus de slots beam sur les
  candidats provably-constant. Plus reusable scratch buffer (commit
  `ced75e9`, **1.43× synth speedup** mesuré).
- **inspect_program_map** : appelé depuis le frontend dès la sélection
  d'un programme (et a fortiori dès l'upload d'un fichier). Renvoie un
  `ComputationPlanReport` avec le décompte par couche : input dedup, CSE
  classes, sub-tree, trace, atlas peek, atlas RESULT déjà connus.
  Cache in-process via `inspect_cache` → second appel = ~1 ms.

**Doctrine §9 (filtre paranoïaque multi-échelle) appliquée** : le système
détecte la redondance à plusieurs échelles AVANT compute (CSE, trace,
sub-tree, sub-window, peek) au moment de l'inspection, et persiste TOUS
les résultats cross-session sous une seule clé `(func_hash, input_hash)`
dans `kind::RESULT` post-M1.5+M2. La cascade `lookup top-down avec early
exit` court-circuite toute session future qui retombe sur le même hash.

## Quickstart

```powershell
# Tous les tests (828 PASS attendus, post Wave 9)
cargo test --lib --release

# Suite validate-features (62 entries — KASM ISA + Wave 1-9 features)
cargo run --release --example lab_runner -- validate-features

# Bench miss path (mesure le hot path V7)
cargo run --release --example monster_miss_bench

# LAB — outil principal pour observer Forge sur des problèmes nouveaux
cargo run --release --example lab_runner -- 10000          # cible officielle ~4s
cargo run --release --example lab_runner -- analyze 10000  # latency p50/p95/p99 + breakdown

# UI observateur + serveur MCP (session 2026-05-06)
cargo tauri dev --bin forge-ui
cargo run --manifest-path examples/forge_tauri_ui/src-tauri/Cargo.toml --bin forge_mcp
```

## Modules `src/` (état V7 post Φ.μ.7.2 — 5 sous-dossiers max)

```
src/
├── lib.rs                pub mod declarations + re-exports
├── atlas.rs              Φ.ν.8 + M1.5+M2 — atlas unifié (~330 LoC, 1 active value-bearing kind RESULT, legacy tags 6..=9 readable)
├── apply.rs              Λ.0 singular operation `apply(node, func, input) → output` + M6 `apply_subtree` sub-expression memoization
├── codec.rs              pack_lossless + nanocube recipe NCB1 (Φ.μ.7)
├── store.rs              append-only forge.cas (γ.0)
├── memory.rs             MemoryGovernor
├── key.rs                CallKey
├── landauer.rs           Ω-6 thermodynamique
├── introspection.rs      Ω-Φ ghost storage (LiveSnapshot)
│
├── kasm/                 Ω-1 + Ω-3 — bytecode + numérique + tenseur
│   ├── types.rs          Op (66 variants — v0.x + v1.0 + v1.1 Vec + v1.2 self-host), Ty, Target, Node
│   ├── program.rs        Program struct, verify, hash_i64
│   ├── interpreter.rs    execute + execute_with_fractal + FractalDispatcher trait (Wave 8)
│   ├── optimizer.rs      canonicalize, simplify, semantic_fingerprint
│   ├── mlir.rs / kasm.td emit_mlir + spec TableGen
│   ├── jit.rs            x86-64 JIT (Windows)
│   ├── numeric/          Ω-3 Rational + Posit16/32 (Φ.μ.7.2 absorbé depuis top-level)
│   ├── tensor/           multi-dtype + NumericContract (Φ.μ.7)
│   ├── ssa.rs            Π.2 Wave 3 — Cranelift-style SSA IR + verifier + peephole + KASM lowering
│   ├── nanbox.rs         Σ.4/Π.6 Wave 2 — NaN-boxing Value (8B packed)
│   ├── threaded.rs       Π.5 Wave 2 — Forth threaded code dispatch (BTB warm fn pointers)
│   ├── columnar.rs       Π.9 Wave 4 — Q/Kdb+ column-oriented storage
│   ├── rank.rs           Π.10 Wave 4 — APL/J rank semantics + NumPy broadcasting
│   ├── rewrite.rs        Π.3 Wave 1 — Mathematica f[x_]:= pattern rewriting
│   ├── self_host.rs      Wave 8 — SelfHostingRuntime (Op::Fractal/Eval dispatcher + callee table)
│   ├── proof.rs          Π.14 Wave 9 — CompCert-style Proven<T, W> witness types
│   ├── fixed.rs          Π.16 Wave 11 — Fixed-point Q31.32 (HFT bit-exact)
│   ├── timestamp.rs      Π.17 Wave 11 — Timestamp/Duration arithmetic nanos UTC
│   ├── ohlcv.rs          Π.18 Wave 11 — OHLCV columnar layout (SMA/ATR/drawdown)
│   ├── errno.rs          Σ.14 Wave 11 — KasmErrno 4B compact error codes
│   ├── order_book.rs     Π.20 Wave 12 — Order book L2/L3 event-driven
│   ├── resampler.rs      Π.21 Wave 12 — Tick→Bar resampler streaming
│   ├── strategy.rs       Π.22 Wave 12 — Strategy graph DSL (Indicator + Action)
│   ├── execution.rs      Π.24 Wave 12 — VWAP/TWAP execution simulator
│   └── reservoir.rs      Π.19 Wave 13 — Reservoir sampling Knuth-Vitter
│
├── meta/                 Ω-4 — Term + kasm_embed + mlir_bridge + meta.td
│
├── agent/                Ω-7 + Ω-2 — symbolic + corpus + extract
│   ├── symbolic.rs       SymbolicAgent + 9 règles algébriques
│   ├── corpus.rs         Corpus SCAN-natif fuzz + η.0 lifter binaire
│   ├── term_pattern.rs   TermPattern matcher
│   ├── term_to_program.rs term_to_program inverse byte-exact
│   └── extract/          Ω-2 tracer + tensor_tracer (Φ.μ.7.2 absorbé)
│
├── godel/                Ω-5 — Gödel-machine
│   └── observer/hardware/criteria/runner + verifier+applicator (config)
│       + verifier_v2+applicator_v2 (program substitution)
│
└── monster/              Runtime — MonsterNode + hot paths
    ├── mod.rs            MonsterNode struct + attach_atlas/atlas accessors (Φ.ν.8)
    ├── cache.rs          RamKey align(64) + InlineCache L0 + StridePredictor
    ├── dispatch.rs       dispatch_batch + atlas RESULT lookup/write (Φ.ν.8)
    ├── exec.rs           dispatch_impl Layer 5b atlas RESULT + execute_with_op_memo
    │                       Hash64 L2 atlas OPMEMO (Φ.ν.8) + peek_call public
    ├── train.rs          synthesize_i64 + push_binary atlas SCORE memo (Φ.ν.8)
    ├── evolve.rs         lab-D : retrieval / glyph / ultra_glyph + kill-switch
    ├── hotplan.rs        HotPlan {Interpret, HashChain, AffineI64, StaticOutput}
    ├── atlas.rs          LiveAtlas v1 (offline retrieval atlas — DISTINCT du
    │                       top-level src/atlas.rs Φ.ν.8 qui mémo cross-session)
    ├── swarm.rs          Inter-node memo exchange
    ├── distill / oracle / stats
    ├── nnue.rs           Π.1 Wave 1 — NNUE Stockfish-style int8 oracle
    ├── seminaive.rs      Π.8 Wave 1 — Datalog seminaive incremental fixpoint
    ├── bump.rs           Σ.3 Wave 2 — Bump arena allocator lock-free
    ├── disruptor.rs      Π.4 Wave 2 — LMAX SPSC ring buffer
    ├── static_pool.rs    Π.7 Wave 2 — TigerBeetle static memory pool
    ├── lua_table.rs      Π.13 Wave 2 — Lua hybrid array/hash table
    ├── become_swap.rs    Π.11 Wave 5 — Erlang/OTP hot code swap registry
    ├── green_sched.rs    Π.12 Wave 5 — Go-style M:N green-thread scheduler
    ├── via_negativa.rs   Wave 6 — audit hot-path mut_self / box_dyn / arc_per_call
    ├── walkforward.rs    Π.23 Wave 13 — Walk-forward optimization parallel
    ├── mono_audit.rs     Σ.11 Wave 13 — Generics monomorphization audit
    ├── swiss_table.rs    Σ.12 Wave 13 — Swiss tables open-addressing
    ├── speed_ablation.rs Wave 14 — StackStr + ArenaItem + audit metrics
    ├── prefault.rs       Σ.21 Wave 15 — Boot prefault page-fault upfront
    ├── seqlock.rs        Σ.23 Wave 15 — Linux-kernel-style seqlock read sans lock
    ├── arena_lt.rs       Π.26 Wave 15 — Arena lifetime tracking (Mojo/Zig)
    ├── slab.rs           Π.29 Wave 15 — Slab allocator page-aligned
    ├── mmap_store.rs     Π.25 Wave 16 — Zero-copy CAS via Arc<Box<[u8]>>
    ├── intrusive_index.rs Π.27 Wave 16 — IntrusiveBlobIndex 16B/entry
    ├── huge_pages.rs     Σ.22 Wave 16 — HugePageBuffer 2MB hint API
    ├── swarm_cas.rs      Π.28 Wave 17 — Swarm shared Arc<Store> registry
    └── cow_snapshot.rs   Π.31 Wave 17 — CoW snapshot O(1) Arc::clone
```

**5 sous-dossiers `src/` exactement** : `agent/`, `godel/`, `kasm/`,
`meta/`, `monster/`. Règle dure : ne pas créer de 6e — fusionner d'abord.

**Modules supprimés en V7** : `numeric/{posit_lut,bigint,interval}`,
`meta/{typecheck,reduce,tactic,inductive,universe,bootstrap}`,
`agent/bandit_agent`, `scan_db` (absorbé dans `store.rs`).

**Note v1/v2 godel** : `applicator.rs`/`verifier.rs` (config patches,
chemin runner Codex) et `applicator_v2.rs`/`verifier_v2.rs` (program
substitution, chemin SymbolicAgent) ne sont **pas des doublons** — deux
applicateurs pour deux types de rewrites distincts.

## Doctrine V7 (durcie)

1. **Architecture ultra-compacte** — éviter les nouveaux modules ; fusionner quand possible.
2. **Zéro dépendance externe** — `std` + `sha2`, rien d'autre.
3. **Pas de gain massif = suppression** — couper le code non rentable.
4. **Fusion/hybrides** — rapprocher les modules qui se recouvrent.
5. **Via Negativa systématique** — chaque phase doit retirer du poids mort.
6. **Objectifs massifs, presque absurdes** — viser une cible démesurée, pas un petit progrès rassurant.
7. **Code inconnu, inconfortable, risqué** — rejeter la voie déjà maîtrisée quand elle n'apporte qu'un easy fix.

**Anti-easy-fix** : "ça compile vite", "c'est idiomatique" ne sont pas des arguments. Une solution prévisible est suspecte si elle n'ouvre aucun territoire nouveau.

**Logbook append-only** : [`CARNET.md`](CARNET.md) ne se réécrit pas, il s'ajoute. La trace est sacrée.

## Caps explicitement reportés

| Cap | Statut |
|---|---|
| Ω-7.1.x graph-NN réel | **Bloqué doctrine** (exige framework ML externe) |
| Ω-7.2.x corpus externe Linux/mathlib | **Phase η.0 livrée** (lifter binaire MSVC/GCC) |
| Ω-Φ.1 cross-process memory | **Repoussé phase γ.X** (mmap + memfd_create) |
| Ω-Φ.3 swarm bootstrap | **Repoussé phase γ.X** (mmap-shared CAS) |
| Ω-∞ auto-hébergement total | **Repoussé V∞** |
| Lab iter/sec ≥ 500 (promesse scientifique) | **✅ atteint × 8.6 en lab-D** (4 322 iter/sec) |

Voir [`ROADMAP.md`](ROADMAP.md) pour le plan détaillé.

## Cible long-terme

Chaque MonsterNode doit pouvoir héberger et entraîner un AGI scientifique en local : **< 100 MB RAM**, **2-5 G c/s**, **1-2 ns latence cache-hit chaud**, **0 dépendance externe**, **lab discovery ≥ 10 000 iter/sec** (cible V10 — l'originale ≥ 1000 a été dépassée ×4 en lab-D).

Murs visibles restants V7 (post Φ.μ, mesure 100k iter, 28/30 cibles à 0% miss) :

- **`wall_random_kasm` 61.5 %** — synthèse de KASM purement aléatoire, mur structurel sans pattern algébrique exploitable.
- **`wall_compose_clamp_div` 85 %** — composition `clamp(b/(c+x), 0, hi)`.
- **`wall_noisy_fsqrt_affine` 85 %** — sqrt-affine + bruit, recognizer bruit imparfait.
- **`wall_compound_invsqrt` 94 %** — `c/(d + a/sqrt(b·x+e))`.
- **Energy-aware verifier** — RAPL/PMU pas encore intégrés (phase ζ).

**Bottleneck atlas writes** : `Mutex<File>` sérialise tous les writes.
Mitigation **−11.2× wall-time** via `BufWriter` 256 KiB (commit
`72fbb1d`) : 866 ns/record observé en bench. Trajectoire vers shardé
mmap par kind tracée dans `ROADMAP.md` Φ.ν.9. Post-M1+M2 le runtime
n'utilise plus qu'un seul `kind::RESULT` côté écriture, simplifiant
encore la contention.

Voir [`ROADMAP.md`](ROADMAP.md) pour le détail des phases V8.
