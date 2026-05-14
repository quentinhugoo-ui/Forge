# CLAUDE.md — Protocole de session pour Forge

> **Lecture obligatoire en début de session** (Claude + Codex).
> Ce fichier override le CLAUDE.md global et tout autre doc en cas de conflit.

**Branche unique** : `master` — toutes les autres branches (`kraken/v7.0`,
`dev/*`, `jit-native`, `neural-cache`) ont été supprimées en Φ.μ.7
(2026-04-29) après extraction ciblée des primitives utiles.

**Worktree unique** : Forge vit dans un seul checkout sur `master`. Aucun
`git worktree add`, aucun mirror de développement, aucune version parallèle
hors de ce répertoire. Toute expérience durable doit être absorbée dans
`master` ou supprimée.

---

## ⚠️ Source de vérité unique — règle des 4 docs

**Avant la première ligne de code**, lire dans cet ordre :

1. Ce fichier (`CLAUDE.md`) — doctrine + commandes lab_runner + zones protégées.
2. [`ROADMAP.md`](ROADMAP.md) — état du synthétiseur, walls, plan Event Horizon.
3. [`CARNET.md`](CARNET.md) — logbook append-only, sacré, ne jamais
   réécrire (anciennement `OMEGA.md`, renommé Φ.μ.7).

**Le projet maintient EXACTEMENT 4 documents racine** : `README.md`,
`CLAUDE.md`, `ROADMAP.md`, `CARNET.md`. Pas plus. Si une information mérite
une trace, elle s'ajoute à un de ces 4 — pas un nouveau fichier.
`AGENTS.md` fusionné dans `CLAUDE.md` en Φ.ν.9f (2026-04-30).
**`PROTOCOLE.md` supprimé** (theater métaphorique sans ROI mesuré : luciférine,
listes pirates A/B/C/D/E, vocabulaire mort 20 mots, démolisseur, etc.).
Supprimés en Φ.μ.7 : `STATE.md`, `TENSIONS.md`, `AMBITIONS.md`,
`ARCHITECTURE.md`, `docs/principles/`, `DOCUMENTATION.md`.

**Le projet maintient EXACTEMENT 5 dossiers de code dans `src/`**
(Φ.μ.7.2) : `agent/`, `godel/`, `kasm/`, `meta/`, `monster/`. Pas de 6e —
fusionner d'abord. Les noms suivent la doctrine architecturale (Ω-X
stable jusqu'à V∞). Si V8/V9 introduit une nouvelle couche, libérer un
slot par fusion préalable, jamais créer un 6e dossier.

Mapping doctrinal :
- `agent/` = Ω-7 (symbolic) + Ω-2 (extract) — réflexion sur le langage
- `godel/` = Ω-5 — auto-amélioration formellement prouvée
- `kasm/` = Ω-1 (bytecode) + Ω-3 (numeric Rational/Posit) + tenseur
- `meta/` = Ω-4 — calcul des constructions content-addressed
- `monster/` = runtime hot path + colony + swarm

**Top-level projet** : 3 dossiers visibles seulement (`src/`, `examples/`,
`tests/` — conventions Rust verrouillées par Cargo). Supprimés en Φ.μ.7.2 :
`docs/` (.td absorbés dans `src/{kasm,meta}/`), `.mojo-env/` et
`.pixi-home/` (orphelins Mojo, doctrine V7 = pure Rust).

**Plus de worktrees mirrors** (`agent/`, `core/`, `runtime/`, `jit/`,
`neural/`) ni de branches `dev/*` : tout est sur `master`, dans un seul
checkout vivant. Φ.μ.7 a
extrait 3 primitives des branches abandonnées (`jit-native` →
`nanocube_pack_recipe_i64` dans `codec.rs`, `NumericContract` dans
`kasm/tensor/types.rs` ; `neural-cache` → `InlineCache` + `StridePredictor`
dans `monster/cache.rs`) puis supprimé les branches.

**Source canonique du code lab** : `src/monster/lab/` (sous-module split
en `mod.rs` + `validate_features.rs` post audit 800-line rule). `examples/lab_runner.rs` n'est plus qu'un
**shim CLI** qui délègue à `MonsterNode::run_lab_batch /
analyze_lab_log / self_improve_lab / parasite_lab / audit_tier1_lab`.
Toute la logique cognitive vit dans le nœud. Ajouté commande
`prune [N]` qui truncate `lab_findings.jsonl` aux N dernières
entrées (rotation manuelle, le log est gitignored maintenant).

---

## 🛡️ Zones protégées — ne JAMAIS supprimer/modifier sans autorisation

Quentin travaille sur certaines zones **en parallèle** du synthétiseur, parfois
avec du contenu **untracked** (pas encore committé). Traiter ces chemins
comme intouchables lors des nettoyages "branches mortes" :

- **`examples/forge_tauri_ui/`** — application frontend Tauri (Rust +
  HTML/JS/CSS) en construction incrémentale. Peut contenir du code
  untracked (build artifacts maintenant dans `.gitignore`). **Ne jamais
  `rm -rf`, ne jamais inclure dans une coupe Via Negativa, ne jamais
  reformer le tree autour.** Si le bench-runner ou `cargo check --examples`
  râle dessus, ignorer (ce n'est pas un example Cargo standard, c'est un
  sous-projet).
- Tout dossier ou fichier non listé dans les 4 docs racine + 5 dossiers
  src/ + 3 top-level (`src/`, `examples/`, `tests/`) **mais explicitement
  marqué "WIP Quentin"** par le user dans une session.

Avant tout `git clean`, `rm -rf`, ou suppression en masse :
1. Vérifier l'untracked status (`git status -s`).
2. Si un dossier est `??` (untracked), **demander explicitement** avant
   suppression.
3. La Corbeille Windows ne récupère **pas** les `rm -rf` bash — Permanent.

## 0.1 · Pivot produit 2026-05-06 — MCP-first, UI observer

Forge n'est plus à traiter comme une application desktop classique.
Le Tauri frontend est un **panneau d'affichage** : historique de jobs,
dropbox multi-CSV, graphique, transcript de calcul, panneau droit de
vérification. Le calcul doit être pensé comme une capacité que Claude,
Codex ou tout autre agent appelle via MCP.

Règles pour toute session future :

- Ne pas supprimer ni désactiver la pre-start analysis Alpha : elle reste
  obligatoire avant le dispatch lourd.
- Ne plus concevoir le bouton `Start` comme source de vérité produit.
  L'entrée canonique devient `forge_mcp`; l'UI peut créer un job pending
  et observer, mais l'agent doit pouvoir lancer le calcul.
- Les logs visibles dans l'application sont des logs métier : parsing
  OHLC, VWAP, Stochastic, RSI, ATR, ADX, labels LONG/SHORT, synthèse,
  holdout, résultats.
- Les logs internes (`job_id`, agent client, bytes, cache hits/misses,
  Store/Atlas, CPU/GPU dispatch, tailles d'artefacts, accounting LLM)
  vont au terminal sous `[forge-internal:<job>]`, pas dans le canva client.
- Les preuves/hash ne vont pas dans le canva général. Elles vivent dans
  le panneau droit `Compute proof`, ouvert par l'icône discrète du header
  ou par `Open verification` sous le transcript.
- Les résultats publiés pour agent passent par `mcp_result` dans le
  manifest (`publish_forge_job_to_mcp`) : l'agent reçoit une référence
  compacte, pas le CSV/log complet.
- Toute évolution UI doit rester proche des apps desktop d'agents LLM :
  fond unifié, peu de bordures, historique type Claude/Codex, lignes
  lisibles, typographie sobre, preuves sous forme de rangées.

## 0.2 · Economie exacte 2026-05-08 — pas de qualite sacrifiee

La session Lazy/Force a tranche une regle produit importante : Forge peut
economiser massivement du travail, mais pas en degradant la qualite des
programmes ou des scores. Les heuristiques approximatives de prefiltrage
synth Alpha ont ete retirees. Le chemin canonique est maintenant :

- cacher ce qui a une identite content-addressed stable ;
- stocker les resultats lourds sous forme de blobs schema-versionnes quand
  une matrice/table/artifact est l'unite naturelle ;
- reutiliser les programmes KASM valides via cache structurel global ;
- ne brancher `Op::Lazy`/`Op::Force` que comme primitives exactes ou comme
  wrappers qui preservent le score complet.

Mesure de reference NATGAS H4 : raw+labels ~30.8 s baseline -> ~4.7 s cold
exact -> ~79 ms warm exact. Les labels/scorings restent exacts.

## 0.3 · Canvas multi-LLM 2026-05-08 — session vivante, pas wrapper jetable

Forge est maintenant un canvas local ou plusieurs LLM peuvent travailler
dans la meme session. Codex, Gemini CLI et Claude Code CLI doivent etre
traites comme des clients d'une meme memoire Forge, pas comme trois chats
isoles.

Regles produit :

- Codex/OpenAI doit utiliser le runtime local Codex / Codex app-server deja
  authentifie par le compte ChatGPT/Codex de l'utilisateur. Ne pas demander
  `OPENAI_API_KEY` pour ce mode et ne pas presenter les compteurs comme une
  facturation OpenAI API.
- Claude doit utiliser Claude Code CLI avec login navigateur Claude.ai
  Pro/Max quand ce mode est disponible. Une cle API Anthropic reste un mode
  optionnel, mais elle doit etre explicitement etiquetee pay-as-you-go.
- Gemini doit utiliser Gemini CLI ou sa configuration locale. Si l'utilisateur
  saisit une cle Gemini API dans les settings, l'UI doit la distinguer du
  mode CLI/login local.
- Ne jamais promettre "gratuit illimite" : dire abonnement/login local dans
  les limites du provider, et economies Forge par avoidance des appels et
  des donnees brutes.
- Un fichier ajoute par `+` appartient a la session courante. Il ne doit
  jamais ouvrir une nouvelle session par effet de bord.
- Changer de session ne doit pas tuer un LLM, un calcul, un programme ou
  une ecriture en cours. La session continue en arriere-plan.
- Un provider qui arrive en cours de session doit recevoir l'historique
  compact, les fichiers, les programmes, les visual programs, les runs et
  l'Atlas pertinent avant de repondre.
- Les messages automatiques deterministes (salut, aide basique, action UI)
  doivent etre repondus localement avec une latence naturelle, sans appeler
  un CLI LLM couteux.
- Les erreurs backend/CLI et logs internes ne doivent pas polluer le chat.
  Le chat affiche les messages utilisateur/agent, les micro-evenements
  utiles et les cartes visuelles.
- En mode normal, une carte fichier 2D/3D se comporte comme un message et
  defile avec le chat. En split view, la carte inline disparait.
- Les cartes fichier sont a droite du chat et representent le fichier
  directement en 2D/3D ; pas de piece jointe texte a gauche quand une vue
  visuelle est disponible.

## 0.3b · Provider workbench 2026-05-09 — terminal embarque, pas shell externe

Le chemin produit cible pour les providers n'est plus "ouvrir PowerShell".
Forge doit montrer un vrai terminal embarque, unique, dans la page providers.

Regles produit :

- Un seul terminal visible dans l'overlay providers ; le launcher choisit Codex, Gemini ou Claude.
- Le terminal doit rendre la vraie UI du CLI via PTY + renderer terminal, pas une transcription simplifiee de logs.
- Si un CLI manque, Forge tente l'installation depuis cette surface embarquee ; l'utilisateur ne doit pas aller fouiller `AppData`, `~/.gemini` ou `~/.claude`.
- Si `Node` ou `npm` manquent, Forge l'explique proprement dans cette meme surface au lieu d'echouer en silence.
- Le design doit rester ultra sobre : pas de couleurs de decoration, pas de cadre lourd, pas de scroll de page, terminal noir incruste dans la matiere de la page.
- Le terminal providers est une cible cross-platform : Windows, macOS et Linux. Ne jamais durcir l'UX autour d'un shell Windows externe comme chemin normal.

## 0.4 · My Atlas et doctrine zero re-run

My Atlas est la memoire produit lisible par agents et UI. Chaque programme
cree, chaque balise metrique creee, chaque geonode creee et chaque run
termine doit etre enregistre sous `.forge-store/atlas/my_atlas.json` ou
dans l'Atlas Forge canonique associe.

Regles :

- Toujours consulter l'Atlas avant de creer ou lancer un programme.
- Si `program_hash + input_hashes + params` existe deja, retourner un hit
  immediat et ne pas relancer le calcul.
- Les agents peuvent reutiliser une balise d'un ancien programme dans un
  nouveau programme, tant que le compiler valide ses inputs/units/dtypes.
- L'onglet `Atlas > My Atlas` doit rester une vue compacte : programmes,
  balises metriques, geonodes, mini-geonodes, runs, status, hashes,
  actions de reutilisation.
- Ne jamais presenter une balise `custom_unresolved` comme un resultat
  calcule. Elle est un contrat extension, pas une preuve.

## 0.4b · Planet / GeoNode / MiniGeoNode

Forge distingue maintenant la visualisation spatiale des programmes de calcul.

Regles :

- `planet_sphere` est un tool visuel universel. Ce n'est pas un `program`.
- Les coordonnees nommees deviennent des `GeoNode`.
- Les sous-lieux precis deviennent des `MiniGeoNode` avec `parent_geonode`.
- Une node metrique classique peut se lier a un lieu via `geo_ref`, `geo_refs` ou un equivalent normalise.
- Le premier renderer concret est Mars : globe HD, rotation lente par defaut, focus doux sur geonode ciblee, pills de lieux dans le chat, et aucun vieux panneau editorial type Mars Magazine.
- Toute nouvelle planete/lune/corps doit suivre cette meme architecture : `Planet` dans l'UI, `planet_sphere` comme tool, et ancrages geographiques dans l'Atlas.

## 0.4c · Store canonique et persistence

Le store Forge canonique pour ce repo est `./.forge-store` a la racine du
workspace. `cargo tauri dev` et la build desktop doivent lire le meme
store, sinon l'utilisateur croit que ses sessions ou son Atlas ont disparu.

Regles :

- Ne pas rebasculer silencieusement vers un store Tauri isole dans `AppData`.
- Une session canvas reelle doit etre persistable cote backend, pas seulement en `localStorage`.
- Si `my_atlas.json` est vide ou corrompu, Forge doit tenter une recuperation ou regenerer un Atlas sain, sans casser l'onglet Atlas.

## 0.5 · Compiler universel de programmes

Tout programme agent-created complexe doit passer par
`program_compile_validate_route` (MCP) ou `forge_compile_validate_route`
(dynamic tool) avant stockage/run.

Pipeline obligatoire :

```text
intent / metric tags
  -> compile metric graph
  -> validate metric contracts
  -> check objective coverage
  -> check units / dimensions
  -> build dependency map
  -> bind formulas to executors
  -> route executors
  -> scientific validation block
  -> universal linter
  -> explain plan
  -> save to My Atlas
  -> run only if executable
```

Un programme qui ne compile pas ne doit pas etre annule comme s'il avait
echoue definitivement. Il devient `needs_repair`; l'agent doit expliquer
brievement ce qu'il corrige, modifier les balises/formules/dependances,
puis relancer le compile/validate/route. `run` doit refuser un programme
`needs_repair` tant qu'il n'a pas de route executable.

Les micro-evenements de construction se placent entre les messages LLM :
creation de balise, appel tool, modification, compilation, validation,
routage executor, sauvegarde Atlas. Ils doivent etre courts, avec une
icone lisible, et ne jamais remplacer l'explication naturelle de l'agent.

## 🎯 Doctrine — 7 principes Forge non-négociables

1. **Architecture ultra-compacte** : éviter les nouveaux modules ; fusionner quand possible.
2. **Zéro dépendance externe** : stack autorisée = `std` + `sha2`.
3. **Pas de gain massif = suppression** : couper le code non rentable.
4. **Fusion/hybrides** : rapprocher les modules qui se recouvrent.
5. **Via Negativa systématique** : chaque phase doit retirer du poids mort.
6. **Objectifs massifs, presque absurdes** : ne pas viser un petit progrès rassurant ; viser une cible qui paraît démesurée, presque impossible.
7. **Code inconnu, inconfortable, risqué** : reconnaître la voie déjà vue / déjà maîtrisée / déjà confortable, puis la rejeter quand elle n'apporte qu'un easy fix. Si l'on ne fait jamais d'erreur, c'est souvent qu'on n'innove pas assez.

**Anti-easy-fix** : "ça compile vite", "c'est idiomatique", "je connais déjà ce pattern" ne sont **pas** des arguments. Une solution confortable mais prévisible est suspecte si elle n'ouvre aucun territoire nouveau.

---

- Surface MCP visible volontairement compacte : `about`, `capabilities`,
  `create`, `program_compile_validate_route`, `run`, `jobs`, `read`,
  `logs`, `cancel`. Les anciens noms restent acceptes comme alias caches
  pour compatibilite, mais ne doivent plus etre enseignes aux agents.
- Reflexe agent obligatoire : utiliser Forge avant de lire/calculer dans le
  contexte LLM quand l'input est gros, repetitif, couteux, scientifique,
  numerique, document-heavy, ou quand le resultat doit etre verifiable par
  hash/proof. Ne pas lire de CSV/source/log brut si `read`, `logs`,
  `capabilities`, `create` ou `run` peuvent fournir un plan, un calcul, une
  preview bornee, des artefacts ou des hashes.
- `capabilities` sert de GPS : choisir domaine/template/operateurs et
  retourner un prochain appel recommande. `run { intent, inputs,
  plan_only:true }` sert a planifier sans effet de bord.
  `program_compile_validate_route` transforme l'intention ou les balises
  en graphe valide/routable. `create` sert a creer des programmes
  reusables depuis des balises/metriques ; `run` lance ensuite les
  programmes de bibliotheque existants ou crees par agent via
  `program_hash`, `program`, `program_title` ou `program_query`.
- Garde-fou token obligatoire pour toute commande MCP : renvoyer des
  index, resumes, curseurs et references de fichiers. Ne jamais renvoyer
  CSV complet, log complet, artifact lourd, matrice de features, labels
  ou predictions par defaut. Chaque reponse doit porter `token_safety`.
- Limites MCP actuelles : listes max 50 items ; `logs` defaut 16 KB,
  maximum 64 KB ; `read` retourne des resumes/manifests sanitises, pas le
  contenu brut.
- Atlas obligatoire : utiliser `forge_atlas_overview` /
  `get_forge_atlas_overview` ou le resume `my_atlas` du contexte avant de
  creer un programme qui pourrait deja exister. Les hits Atlas sont des
  resultats disponibles, pas des suggestions a recalculer.
- Installation Codex officielle locale : `[mcp_servers.Forge]` dans
  `C:\Users\quent\.codex\config.toml`, pointant vers
  `C:\scan-shared-target\debug\forge_mcp.exe`. Redemarrer Codex apres
  modification de cette config. Utiliser `Forge` avec F majuscule comme
  cle de serveur, car `/mcp` affiche souvent la cle de config avant de
  lire `serverInfo.name`; `forge` reste seulement le slug technique
  historique.

## 0 · Le brief permanent

Forge est un substrat de calcul content-addressed déterministe. La
promesse : **chaque programme valide est une identité cryptographique
stable cross-machine et cross-décennie**.

État courant V7 (post session 2026-05-08 — MCP-first + economie exacte KASM/Alpha ; bloc B saturation + Φ.ν.9f per-feature synthesis, Λ.0+Λ.1 apply, M1.x atlas collapse, M3 self-host généralisé, M4 score généralisé, M5 apply_program Tauri, M6 apply_subtree) :

| Métrique | Valeur |
|---|---|
| **Lignes Rust** | ~29 940 (post session 2026-05-05 unification + reduction-first ; M1+M2 collapse + Speculative purge ont récupéré -607 LoC, M3-M6 self-host généralisé + apply_subtree ont ajouté +500 nets — voir CARNET 2026-05-05) |
| **Dépendances externes** | `sha2` uniquement ; `wgpu` + `pollster` derrière feature `wgpu` ; `cuda_min` derrière feature `cuda` (pas de cudarc, juste nvcuda.dll FFI direct) |
| **Tests** | **1097 / 1097 PASS** lib core + **34 / 34 PASS** Tauri UI (post Φ.ν.9f per-feature synthesis) |
| **Atlas unifié** | `src/atlas.rs` ~330 LoC. Fichier format hérite de 9 kind tags pour rétro-compat lecture, **runtime n'utilise plus que `kind::RESULT`** post M1.5+M2 (commits `d6fe1e7` et `d7848f0`). `Arc<Atlas>` partagé `ForgeBackend` ↔ `MonsterNode` via `attach_atlas`, append-only cross-session |
| **Blob RESULT cache** | `Atlas::blob_result_key(namespace, source_hash, schema_hash, start, end)` ajoute une cle generique pour matrices/tables/artifacts deterministes. Alpha H4 l'utilise pour la raw feature matrix : 1 blob schema-versionne au lieu de ~463k scalaires. |
| **Apply singular operation** | `pub fn apply(node, func, input) -> Vec<u8>` dans `src/apply.rs` — content-addressed inputs via `Hash::for_blob` (Λ.1 fix aliasing >12 bytes), atlas RESULT lookup, fallback `kasm::execute`. Plus `apply_subtree` (M6) pour sous-expressions. Tauri command `apply_program` exposée publique (`fa531eb`). |
| **Self-host bytecode-en-bytecode** | `src/kasm/self_host_lite.rs` : `affine_self_host_program` + `affine_score_program` (Λ.2/Λ.3 lite) + `general_6node_self_host_program` (M3, 123 nodes, dispatch dynamique) + `generalized_score_program` (M4, ~480 nodes, K=4 examples unrolled) |
| **KASM ISA** | **74 opcodes**. Extension 2026-05-08 : `Op::Lazy=72` et `Op::Force=73` FULL cote ISA/interpreter/optimizer/consumers, CUDA/JIT fail-loud si non resolu. Usage synth approximatif interdit ; exact only. |
| **Program build cache** | `Program::new` memoise les graphes KASM valides par hash structurel dans un cache global borne. Benefice universel : tout builder deterministe evite rebuild/remap/verify repetes. |
| **Canvas multi-LLM** | Codex, Gemini et Claude peuvent coexister dans la meme session Forge. La barre de chat choisit provider/modele/effort, assigne fichiers/programmes/geonodes, et le workbench providers ouvre un vrai terminal embarque pour les CLIs. |
| **My Atlas produit** | `.forge-store/atlas/my_atlas.json` indexe programmes, balises metriques, geonodes, mini-geonodes et runs termines. Les agents consultent l'Atlas avant create/run ; un run deja connu revient immediatement par hash. |
| **Program compiler** | `program_compile_validate_route` + `forge_compile_validate_route` compilent les balises en graphe, valident contrats/objectifs/unites/dependances, routent executors, produisent un explain plan et marquent `needs_repair` au lieu de lancer un programme incoherent. |
| **Visual programs 2D/3D** | Les fichiers uploades deviennent des cartes 2D/3D dans le canvas, avec mapping par defaut OHLCV `time/price/volume` + VWAP/EMA, split view optionnelle, 3D orbit/drag, et references compactes au lieu de donnees brutes envoyees au LLM. |
| **FeatureCache** | prefix sums O(N) build + O(1) per bar — voie canonique post M1.3+ (legacy `compute_*` O(K) supprimés). FeatureCache::persist_to_atlas DROPPÉ M1.3 (write-only en prod, jamais lu) |
| **Alpha exact compute economy** | NATGAS H4 : raw feature matrix ~3.6 s -> ~2.0 s cold -> ~36.6 ms warm ; labels LONG/SHORT ~27.2 s -> ~2.6 s cold -> ~42.3 ms warm. Gain par caches exacts (`OnceLock`, Program cache, blob Atlas), pas par approximation. |
| **Hot path miss** (auto-router v0/v1/v2) | **75-243 ns/call** sur Léger (45-51× vs ancien ~5 µs/call) |
| **dispatch_batch CPU-fast bypass** | **1 µs/call** sur Léger (vs ~5 µs avant entry-bypass) |
| **Multi-GPU split** | **CUDA + WGPU parallèle** quand 2 vendors différents, kernel WGSL universal handle 9/9 programmes ADN bit-exact |
| **DNA bench réel** (k=21, 100k k-mers) | splitmix1 75 ns / complement 88 ns / branched 119 ns / heavy_64 405 ns / crypto_heavy 16 µs (split GPU) |
| **Lab synthesis** | ~1 000 iter/sec ; queue lente ultra_glyph 28-100 ms reste à attaquer |
| **Lab holdout-exact** | ~95 % (mesure variable selon session, target ≥ 95%) |
| **Hot Atlas L1** | ~166k entrées persistées, 67% hit rate |
| **18 Wave primitives wirées** (Tier 1-4) | mmap_store/intrusive_index/prefault/huge_pages/bump/slab/swiss_table + arena_lt/static_pool/disruptor/seqlock + nnue/seminaive/cow_snapshot + lua_table/walkforward/speed_ablation/mono_audit |
| **WGSL universal KASM kernel** | 20 opcodes i64-only, stack 128 nodes, 9/9 programmes ADN bit-exact validés |
| **Semantic CSE** | `Program::cse()` — trace 8 sample inputs, merge subexpressions with identical value traces (`Shl(x,1)` ≡ `Add(x,x)` ≡ `Mul(x,2)` → 1 computation). Falls back to `simplify()` pour F64/Vec/meta-ops |
| **InlineCache L0** (Φ.μ.7) | 5-10 ns direct-mapped 64 slots/programme — primitive disponible, wiring direct dans Layer 0 attempted + reverted (regressed bench, redesign nécessaire) |
| **`src/monster/lab/`** | split en sous-module (`mod.rs` + `validate_features.rs`) post audit 800-line rule |
| **`examples/lab_runner.rs`** | shim CLI + commande `prune [N]` pour rotation lab_findings.jsonl |
| **Storage** | lab_findings.jsonl untracked (gitignored), prune via CLI ; forge.cas + Tauri build artifacts gitignored |
| **Doctrine** | pure Rust + std + sha2 + (cuda OR wgpu features optionnelles), déterminisme partout |

**Sauts majeurs de la séquence Φ.μ (2026-04-29)** :
- **Φ.μ.1** (`wall_quadratic_disc`) : 57.9 % → **100 %** via `recognize_quadratic_disc_program` (algébrique stage 1 + brute-force stage 2 sur 4·a, b²).
- **Φ.μ.2 gen2 fix** (`ultra_invsqrt_affine`) : 57.4 % → **96.9 %** via `retrieve_highway_programs(examples, ...)` au lieu de `&split.train` (les recognizers algébriques voient maintenant tous les 12 inputs, pas le subset train, donc l'anchor de contrainte serrée n'est jamais en holdout).
- **Φ.μ.3 nano_probe au carré** : extraction depth-4 récursive + minage des `train_ok` (pas seulement exact_holdout) + atlas partagé warm + `nano_analyze` mode + tracking miss/hit par target. Catalogue de 148 atomes universels exploitable pour la phase ultra_glyph gen 2 (auto-dérivation).

**Murs visibles restants après Φ.μ** (mesure 100k iter, 28/30 targets à 0% miss) :

- **`wall_random_kasm` 61.5 %** — synthèse de KASM purement aléatoire, mur structurel sans pattern algébrique exploitable. Cible légitime de "déjà vu vs jamais vu".
- **`wall_compose_clamp_div` 85 %** — composition `clamp(b/(c+x), 0, hi)`, 109 échecs résiduels.
- **`wall_noisy_fsqrt_affine` 85 %** — sqrt-affine avec bruit, recognizer bruit imparfait (9% miss train→holdout).
- **`wall_compound_invsqrt` 94 %** — `c/(d + a/sqrt(b·x+e))`, 40 résiduels.
- **bit_mixer** : ✅ résolu — 100% holdout (ancienne mesure 47% obsolète).
- ~~mmap pas encore implémenté sur `forge.cas` (γ.X)~~ ✅ **MmapStore wiré** (session 2026-05-03), Layer 5 fast read via `MonsterNode::enable_mmap_view()`.
- Solution C : `PhysicalEnvelope` posé, RDTSC actif sur slow lane, RAPL/PMU pas encore intégrés (ζ).

---

## 1 · Le protocole lab_runner — OBLIGATOIRE pour tout travail

> **Avant chaque modification** qui touche au synthétiseur, à la
> dispatch logic, ou à toute optimisation hot-path : **donne à
> Quentin la commande pour relancer le lab et capter les nouvelles
> métriques**. Le `lab_findings.jsonl` est notre instrument de
> mesure principal — c'est lui qui dit ce qui marche vraiment.

### Commandes types

```powershell
# Run standard 10k (≈4s avec atlas warm) — CIBLE OFFICIELLE
cargo run --release --example lab_runner -- 10000

# Analyse cumulative (lit lab_findings.jsonl)
cargo run --release --example lab_runner -- analyze 10000

# Run smaller pour smoke test crash/no-crash
cargo run --release --example lab_runner -- 200

# Run plus large pour signal stable (1000-5000 iter)
cargo run --release --example lab_runner -- 1000
cargo run --release --example lab_runner -- 5000

# Single-thread pour mesurer baseline pure
$env:FORGE_LAB_THREADS = "1"; cargo run --release --example lab_runner -- 50

# Analyse — N optionnel pour ne lire que les N dernières entrées
cargo run --release --example lab_runner -- analyze
cargo run --release --example lab_runner -- analyze 5000
```

### Ce que les logs exposent (post lab-D)

Par ligne JSONL :
- `iter`, `target`, `cfg`, `outcome`, `elapsed_ms`, `elapsed_us`
- `candidates_evaluated`, `candidates_per_sec`
- `program_nodes`, `train_loss`, `holdout_loss`, `generations_used`
- **`source`** : `beam` / `glyph` / `retrieval` / `ultra_glyph`
- `exact_train`, `exact_holdout`

Par run summary :
- iter/sec, candidates/sec, effective cand/sec
- exact retrieval / glyph / ultra_glyph / structured / evolved
- per-target intel : holdout %, avg ms, avg kc/s, avg cand

Par analyze :
- distribution elapsed p50/p95/p99/max
- distribution candidate throughput p50/p95/p99/max
- breakdown par source + par target + par max_nodes
- top 8 slowest experiments
- unique error messages

### Ce qu'il faut PROPOSER au user après chaque mesure

1. Si une optim n'apporte rien (~bruit) : **revert immédiatement**.
   Ne pas garder du code qui ne prouve pas son ROI.
2. Si une optim aide certains domaines mais en régresse d'autres :
   **arbitrer explicitement avec Quentin**, ne pas merger seul.
3. Si une optim débloque vraiment quelque chose : **commit atomique
   avec le commentaire qui cite le delta mesuré**.
4. Si Forge "ne devrait pas savoir faire ça" mais le fait : valider
   par holdout strict avant de claim la victoire (kill-switch lab-D).

---

## 1.5 · Le protocole audit v1.0 KASM — OBLIGATOIRE à chaque étape

> **Inscrit après audit 2026-05-01** : à chaque commit qui touche le
> bytecode KASM, une couche cognitive (agent/godel/meta), un runtime
> (interpreter/JIT/CUDA/GPU), ou ajoute une feature étrangère stolen
> (Julia/Mojo/JAX/APL/etc.), Claude **doit** lancer un audit complet
> projet vérifiant que la **version la plus récente du KASM mutant** est
> adoptée partout — pas juste déclarée dans `kasm/types.rs`.

### Pourquoi cette règle existe

L'audit du 2026-05-01 a découvert deux gaps silencieux après les Waves
1-4 :
- `agent/term_to_program.rs::op_from_byte` s'arrêtait à l'opcode 31 :
  un programme touchant les 12 nouveaux opcodes v1.0 (34-45), ConstF64
  (32) ou F64Op (33) survivait `embed_program(p).hash()` mais ratait le
  round-trip silencieusement.
- `agent/symbolic.rs` rebuild paths ne remappaient l'imm-as-3rd-ref que
  pour `SelectI64 | ClampI64`, oubliant `Op::Cond` qui utilise la même
  encoding. Le rebuild produisait un Program byte-valid avec une
  référence stale → exécution silencieusement corrompue.

Ces gaps ont survécu Waves 1-4 parce qu'aucun audit ne les cherchait.
Une feature peut être *déclarée* dans l'enum `Op`, *exécutée* par
l'interpreter, mais ignorée par 1-N autres consumers qui font du
pattern-match partiel.

### Que cherche l'audit

Pour chaque commit qui élargit la surface KASM, balayer **tous** les
sites qui pattern-matchent sur `Op` ou `Ty`, et vérifier la couverture :

```powershell
# Sites Op
Grep -r 'match\s+\w+\.op|matches!\(.*\.op|node\.op\s*==|n\.op\s*==' src/
# Sites Ty
Grep -r 'match\s+\w+\.ty|matches!\(.*\.ty|Ty::I64|Ty::Bool|Ty::F64|Ty::VecI64' src/
# Sites raw byte → Op (decoders)
Grep -r '=> Op::|=> Ty::' src/
```

Pour chaque match exhaustif (sans wildcard `_`), Rust refuse de compiler
si une variante manque — sécurité automatique ✅.
Pour chaque match avec wildcard ou `if matches!(...)` : audit manuel
obligatoire — la nouvelle variante peut tomber dans le wildcard et être
silencieusement traitée à tort.

### Périmètre de l'audit (16 fichiers connus avec match Op au 2026-05-01)

Modules KASM core :
- `kasm/{types, program, interpreter, optimizer, jit, mlir}.rs`
- `kasm/tensor/*` (ISA séparé `TensorOp` — pas concerné par v1.0 mais à
  vérifier indépendamment si KASM-Tensor mute aussi)

Couches cognitives :
- `agent/{symbolic, term_to_program, corpus}.rs` — embeddings + agents
- `meta/{kasm_embed, mlir_bridge, term}.rs` — pont preuves formelles
- `godel/*` — auto-amélioration (vérifier si concerné)

Runtime :
- `monster/{exec, hotplan, gpunode, lab, cache}.rs`
- `landauer.rs` — comptabilité énergie (per-op cost table)
- `cuda/kasm_interpret.cu` — kernel CUDA
- `examples/forge_tauri_ui/src-tauri/src/main.rs` — `expected_for_program`
  doit avoir une référence Rust pure pour chaque programme installé

### Status matrix obligatoire à produire

À chaque Wave / phase Φ.X qui ajoute une feature, le commit doit
inclure (dans son message ou dans CARNET.md) une matrice. Exemple
**au moment de l'audit 2026-05-01** (statuts ci-dessous datés ;
état courant complet des opcodes V1.0 vit dans `README.md` et
`ROADMAP.md` Status matrix v1.0 KASM, qui sont la source de
vérité — la plupart des STUB / PARTIAL ci-dessous sont passés à
✅ FULL via Wave 6/7/8/10/11.6) :

| Feature | Origine | Status (audit 2026-05-01) | Évidence |
|---|---|---|---|
| Op::Cond | JAX `lax.cond` | ✅ FULL | exec interp + opt + UI demo + CUDA + agent rebuild |
| Op::Comptime | Mojo `@comptime` | ✅ FULL | Wave 3 OR-chain fold + tests |
| Op::Memoize | Mathematica | ✅ FULL | Codex Wave 2 RamMemo force + test |
| Op::Adaptive | Mojo `@adaptive` | ⚙️ PARTIAL → ✅ Wave 11.6 | Brain-level call_adaptive(impls, args) bench-and-pick via RDTSC |
| Op::Grad/Vmap/Pmap/Pipeline/Fori/WhileLoop/Reduce/Scan | JAX/OCaml/APL | ⛔ STUB → ✅ Wave 6/7a/8a/10 | Brain-level wired call_pipeline / call_map / call_pmap / call_reduce / call_scan / call_grad / call_fori / call_while |
| MultiMethod | Julia | ⚙️ PARTIAL → ✅ Wave 4b | MonsterNode wire-up store/load/resolve/call_multi + 14 tests |
| Op::VGetI64 | (Wave 7i, session 2026-05-05) | ✅ FULL | exec interp + opt + verifier + mlir + agent + landauer + 5 tests |

**Définitions précises** :
- ✅ FULL : déclaré + handled par TOUS les consumers + impl sémantique
  réelle + au moins un test d'exécution end-to-end.
- ⚙️ PARTIAL : déclaré + handled par tous les consumers (pass-through ou
  fail-loud explicite OK) + impl partielle (e.g. wrapper transparent,
  data structure sans wire-up).
- ⛔ STUB : déclaré + fail-loud uniformément + impl vide. Acceptable
  comme étape intermédiaire MAIS interdit pour une feature qu'on prétend
  "absorbée".

### Trigger conditions

L'audit OBLIGATOIRE se déclenche si le commit en cours :
1. Ajoute une variante d'enum `Op` ou `Ty` (KASM ISA expansion).
2. Ajoute une feature étrangère absorbée (Julia/Mojo/JAX/etc.).
3. Modifie une signature publique de `Program` (e.g. `Program::sig()`).
4. Touche `kasm/types.rs::from_byte` (decoder bytecode → enum).
5. Modifie un consumer KASM dans la liste périmètre ci-dessus.

L'audit OPTIONNEL (mais recommandé) à chaque entrée Wave/Φ.X :
- Lancer `cargo test --lib` complet (gate ≥ 609/609 PASS post-audit
  2026-05-01) — un manque de couverture v1.0 n'apparaîtra pas comme un
  échec de test si aucun test ne cible le round-trip ou le rebuild des
  nouveaux opcodes. **Ajouter au moins 1 test par nouveau opcode qui
  l'exerce dans chaque consumer concerné.**

### Comment intégrer dans la doctrine déjà en place

Cette règle ne remplace pas la §1 (lab_runner) ni la §6 (discipline
self-test) — elle s'y ajoute. Ordre d'exécution typique pour un commit
qui ajoute un nouvel opcode v1.0 :

1. Implémenter l'opcode dans `kasm/types.rs` (variant + from_byte +
   opcode constant).
2. Étendre `interpreter`, `optimizer`, `jit`, `mlir` (Rust force la
   complétude par exhaustivité du match).
3. Étendre les consumers à wildcard : `agent/{term_to_program,
   symbolic}`, `meta/kasm_embed` (déjà couvert via `as u32`),
   `landauer.rs`, `cuda/kasm_interpret.cu`, Tauri UI.
4. Lancer l'audit Grep décrit ci-dessus pour tracer les sites manquants.
5. Ajouter ≥ 1 test régression par consumer touché (round-trip pour les
   embedders, exécution pour les interpreters, holdout-exact pour les
   recognizers du lab).
6. Mettre à jour la matrice status dans le commit message ou CARNET.

---

## 2 · Doctrine V7 (héritée de V6, durcie)

| Règle | Statut |
|---|---|
| Pure Rust + `std` + `sha2`, **rien d'autre** | ✅ Atteint en γ.0 (suppression git2) |
| Zéro RNG ambiant, zéro horloge dans les hash | ✅ |
| Programme valide = identité cryptographique stable | ✅ |
| Via negativa : couper sans pitié ce qui n'apporte pas la promesse | ✅ Ω-V7 a coupé 12 244 lignes |
| Logbook append-only (`CARNET.md` + `lab_findings.jsonl`) | ✅ |
| **Pas de fausse livraison** : "atteint" exige un test runnable | ✅ Discipline tenue |
| **Compactness** : éviter les nouveaux modules, fusionner | ✅ -2 modules en γ.0, η.0 étend `corpus.rs` sans nouveau module |
| **Le meilleur calcul est celui qu'on ne fait pas** | ✅ Doctrine lab-D : retrieval avant search |

**Tier C/D du brainstorm hardware** (rejected) :
- Self-propagation SSH/NFS/Git hooks → comportement de ver
- Lecture mémoire d'autres processus → side-channel offensif
- Détournement de Jenkins/Jupyter LAN sans consentement
- Toute crate de framework lourd (wgpu, cuda, rayon, ndarray, tokio…)

---

## 3 · Plan d'action V8 — voir ROADMAP.md

Les phases livrées + à livrer sont dans **ROADMAP.md**. Lire avant
toute proposition stratégique. Le plan est verrouillé après audit
multi-session ; ne pas réécrire l'architecture sans en discuter.

Synthèse Forge Event Horizon — état actuel :

Accomplis (session 2026-05-03) :
- ~~**Auto-router CPU v0/v1/v2**~~ : ✅ AffineI64 + HashChain + Interpret≤64 nodes ; sub-150 ns sur Léger
- ~~**dispatch_batch entry-level CPU-fast bypass**~~ : ✅ skip cascade pour programmes auto-routables (4-5× sur Léger)
- ~~**WGSL universal KASM kernel**~~ : ✅ 20 opcodes i64-only, stack 128 nodes, 9/9 ADN bit-exact
- ~~**Multi-GPU split CUDA + WGPU**~~ : ✅ via thread::scope quand 2 vendors différents
- ~~**Op::Cond JIT branchless**~~ : ✅ branched batch 511→10.7 ns
- ~~**Stack-only KASM interpreter**~~ : ✅ try_execute_i64_inline, 0 Vec alloc per call
- ~~**γ.X mmap CAS**~~ : ✅ MmapStore + IntrusiveBlobIndex + prefault wirés (Tier 1)
- ~~**18 Wave primitives wirées**~~ : ✅ Tier 1-4 complets (storage, concurrency, inference, audit)

Prochaines cibles encore ouvertes :
- **Φ.ν.7** : atlas L2 shape-fingerprint (×100 sur queue lente ultra_glyph)
- **Φ.ν.8** : MonsterCAS unifié (RAM cache fusionné dans mmap forge.cas)
- **Φ.ν.9** : cold boot < 50 ms via index content-addressed
- **ε** : HotPlan multi-backend (dlopen lazy)
- **ζ** : RAPL/PMU dans le verifier
- **η.1+** : ultra-glyphs auto-extraits depuis le Corpus binaire
- **Φ.μ.4** : ultra_glyph gen 2 (auto-dérivation depuis catalogue d'atomes)
- **ι** : BitNet 1.58 dans KASM (fantasme assumé)
- **κ** : multi-process gossip cross-machine (post γ.X)
- **λ** : persistent wgpu device (50 ms init → 0)

---

## 4 · Comment je travaille avec Quentin

1. **Audit avant de coder** — toute optim majeure passe par un audit
   ciblé (Agent / Grep / Read) qui informe le pivot.
2. **Atomic commits par phase** — un commit = un changement structurel
   testable + tests PASS (gate post session 2026-05-05 : **1097/1097
   lib + 34/34 Tauri PASS**).
3. **Mesure avant d'affirmer** — si je dis "X est plus rapide", je
   donne la mesure. Si je n'ai pas mesuré, je dis "hypothèse".
4. **Pirate but honest** — délire sur les solutions, mais reconnaître
   les murs réels (énergie, mémoire, complexité algorithmique).
5. **Reconnaître les régressions** — si une optim régresse, revert
   IMMÉDIATEMENT. Ne pas chercher à sauver le commit.
6. **Donner la commande lab_runner** à chaque travail qui pourrait
   bénéficier d'instrumentation (voir §1).
7. **Tracer la source** — quand un succès vient d'un raccourci
   (retrieval/glyph/ultra_glyph), le dire. Ne pas confondre avec
   un succès évolutif.

---

## 5 · Ce qu'il NE FAUT PAS faire

- ❌ Ajouter un module sans avoir vérifié qu'on ne peut pas fusionner
  avec un module existant.
- ❌ Ajouter une dépendance Cargo sans avoir consulté Quentin.
- ❌ Créer une nouvelle branche (tout le travail est sur `master` direct depuis Φ.μ.7).
- ❌ Créer ou utiliser un worktree parallèle (`git worktree add`, mirror,
  checkout concurrent). Un seul checkout vivant : celui-ci.
- ❌ Faire des claims de speedup sans bench mesuré sur le même
  hardware.
- ❌ Cacher des findings négatifs. Si une optim ne marche pas, le
  dire et reverter.
- ❌ Confondre un succès retrieval/glyph (raccourci) avec un succès
  beam (vraie évolution). Les deux comptent, mais il faut les distinguer.
- ❌ Réécrire `lab_findings.jsonl` ou `CARNET.md` — les deux sont
  append-only. Utiliser `analyze N` pour ne pas se polluer avec
  l'historique complet.
- ❌ Ajouter un 5e fichier de doc à la racine. Le projet maintient
  EXACTEMENT 4 docs : README, CLAUDE, ROADMAP, CARNET. Toute
  nouvelle info s'ajoute à un existant.
- ❌ Ajouter un 6e sous-dossier dans `src/`. Le projet maintient
  EXACTEMENT 5 dossiers : agent, godel, kasm, meta, monster. Une
  nouvelle capacité s'ajoute dans le dossier correspondant à sa couche
  Ω-X — ou en fusionne deux pour libérer un slot.

---

## 6 · Ce qu'il FAUT faire

- ✅ Lire ROADMAP.md avant toute proposition stratégique.
- ✅ Lire les dernières entrées de `lab_findings.jsonl` via `analyze N`
  avant de proposer un changement au synthétiseur.
- ✅ Donner la commande lab_runner à chaque modification touchant le
  hot path ou le synthétiseur.
- ✅ Mesurer avant/après chaque commit qui claim une perf gain.
- ✅ Archiver les commits par phase atomique avec messages détaillés
  (voir l'historique sur `master` pour le style).
- ✅ Quand on hit un mur : reconnaître honnêtement, documenter le
  mur dans le commit, proposer un pivot.
- ✅ Quand un raccourci marche (retrieval/glyph/ultra_glyph), valider
  par holdout strict avant de claim. Le kill-switch lab-D est le
  garde-fou — ne pas le contourner.

### 6.1 · Discipline self-test (Tauri example, programmes KASM)

> **À lire avant d'ajouter un nouveau programme KASM dans
> `examples/forge_tauri_ui/src-tauri/src/main.rs`.**

Le backend Tauri exécute une **self-test correctness** au début de
chaque `start_computation` : 10 inputs déterministes envoyés via
`dispatch_batch`, comparés hash-par-hash à une référence Rust pure
calculée par `expected_for_program(kind, x)`. Si Forge dévie de la
référence → refus de traiter le fichier, scientifique alerté
explicitement.

**RÈGLES IMPÉRATIVES quand on ajoute un programme KASM** :

1. **Toujours ajouter la référence Rust pure** dans
   `expected_for_program` du même `main.rs`. Sans elle, la self-test
   échoue avec "aucune référence pour le programme X" — c'est une
   protection forcée (pas un soft warning), le programme refusera de
   tourner depuis l'UI tant que la référence n'est pas écrite.
2. **La référence doit être bit-pour-bit identique** au comportement
   KASM attendu. Pour les hash : copier les constantes exactes de
   `src/kasm/program.rs::hash_i64` (SplitMix64 / Stafford Mix13).
   Pour les bit ops : valider signed vs unsigned shift (KASM `ShrI64`
   est zero-fill, pas arithmétique).
3. **Tester localement** avec quelques inputs avant commit. La
   self-test seule ne couvre que 10 inputs ; ajouter un sample-check
   en production (1 sur 100k) si protection accrue nécessaire pour
   des datasets de millions de calculs.
4. **Ne pas désactiver la self-test** sous prétexte d'un programme
   "qu'on connaît bien" — c'est précisément le moment où on rate les
   bugs subtils (`execute_with_op_memo` ajouté en Φ.12.1 aurait pu
   diverger silencieusement de `execute_hot_plan` sans la self-test).

**Limites assumées de la self-test** :
- 10 inputs choisis = couverture légère. Un bug pour un
  sous-ensemble très spécifique d'inputs (multiples d'une constante
  magique) pourrait passer à travers — pour des datasets de millions
  de calculs, ajouter un sample-check pendant le run (1 sur 100k).
- Couvre uniquement les programmes installés. Tout nouveau programme
  sans référence dans `expected_for_program` → la self-test refuse
  de continuer (par design, fail-loud).
- Ne couvre PAS les performances ; juste la justesse mathématique.

---

## 7 · Anti-easy-fix — protocole de saut technique

> **À lire quand tu coinces sur un hot path, une feature à inventer, ou
> que tu sens que ton code est "trop évident".** Pas un rituel à appliquer
> partout. Une boussole pour les moments où l'idiomatique plafonne.

### 7.1 Pourquoi cette section existe

Une IA, par défaut, écrit le code qu'elle a vu le plus souvent pendant
son training : idiomatique, défensif, "best practices". C'est un **biais
de distribution**, pas une qualité technique. Les vraies percées tech
viennent quasi-systématiquement d'une **violation des conventions du
domaine** :

- **Cheaters de jeux vidéo** lisent la RAM d'autres processus pour aimbot
  → shared memory IPC < 100 ns vs socket Unix ~10 µs.
- **Stockfish NNUE** : eval neuronal embedded > 600 lignes d'heuristiques
  hand-crafted → +80 Elo sans changer la recherche.
- **LMAX Disruptor** : ring buffer mmap + memory barriers > queue Java
  bloquante → 6 M ops/sec single-thread.
- **Demoscene 4 KB intros** : générateurs procéduraux >> assets stockés
  → scènes 3D entières dans la taille d'une icône PNG.
- **Linus / git** : adressage par contenu (SHA-1) > arbres de paths →
  graphe mondial de hashes immuables, pas de "version control" classique.
- **Bitcoin** : proof-of-work > authority-based trust → consensus sans
  serveur central.

Si ton code ressemble à ce qu'un agent moyen écrirait au premier coup
sur le même prompt, **tu es dans la zone de confort du training**, pas
dans le territoire Forge.

### 7.2 La triade rapide — avant d'écrire la première ligne

Pas un rituel à inscrire — un check mental de 30 secondes :

1. **Nommer la solution idiomatique évidente**.
   "La voie réflexe ici serait X (ex : `Vec<HashMap>`, `std::sync::mpsc`,
   `tokio::spawn`, JSON parser stdlib...)". L'écrire explicitement la
   désamorce. Si tu ne peux pas la nommer, c'est que tu n'as pas vu le
   choix qui existe.

2. **Calculer la borne théorique du hot path**.
   Latence DRAM ~40 ns, L3 cache ~10 ns, L1 ~1 ns. Bandwidth L1 ~1 TB/s.
   Limite Landauer ~3 zJ/bit. Si ta solution est à 10× ou 100× au-dessus
   de la borne pertinente, **tu es loin** — autre voie.

3. **Citer un précédent radical transposable**.
   "Le pattern qui m'inspire ici est X (ex : NNUE Stockfish, Disruptor
   LMAX, mmap Urbit Loom, content-addressing git, branchless arithmetic)".
   Pas une métaphore physique — un pattern technique avec mécanisme
   réellement transposable au problème courant.

### 7.3 Pivots techniques — questionnaire Forge

Quand tu coinces, balayer ces questions. Pas un quota : une checklist
mentale rapide. **2 réponses oui = direction prometteuse.**

- Le résultat peut-il **précéder** l'input ? (pré-calcul, spéculation, JIT)
- Le calcul peut-il **disparaître** ? (mémoïsation, content-addressing)
- L'allocation peut-elle être **éliminée** ? (bump alloc, arena, stack)
- La branche peut-elle devenir **arithmétique** ?
  (`a + (b-a)*cond` >> `if`)
- Le syscall peut-il être **userspace** ? (mmap, ring buffer, lock-free)
- Le lock peut-il être **inexistant** ? (CAS, seqlock, atomic ptr swap)
- Le code peut-il être **généré au runtime** ? (JIT CPUID-spécialisé)
- Les données peuvent-elles être **adressées par leur contenu** ?
  (hash → identité, pas hash → bucket)
- Le format peut-il être **mmap-direct** ? (zéro deserialization, layout
  = struct C en mémoire, lecture par cast pointer)
- Le problème peut-il **ne pas se poser** ? (changer la spec, déplacer
  la frontière, supprimer la fonctionnalité plutôt que l'optimiser)

### 7.4 Précédents radicaux — patterns transposables

| Précédent | Pattern technique réutilisable |
|---|---|
| Cheaters de jeux (aimbot) | Lire la RAM d'un autre processus → shared mem / mmap pour IPC sub-µs |
| Stockfish NNUE | Eval neuronal embedded, weights ∈ {-127..+127} int → BitNet 1.58 KASM |
| LMAX Disruptor | Ring buffer mmap + memory barriers → producteur/consommateur lock-free |
| Demoscene 4 KB intro | Generateurs procéduraux > assets stockés → tout reproduire à partir d'un seed |
| git (Linus) | Content-addressed → identité = SHA, pas path |
| Bitcoin proof-of-work | Anti-spam sans authentification → forge.cas idempotent |
| Urbit / Nock / Loom | Loom mmap unifié → disk/RAM dissolus, état téléportable par hash |
| CompCert (Leroy) | Preuve formelle dans la syntaxe → si ça type-check c'est prouvé |
| Plan 9 | Tout est un fichier → une seule abstraction (le hash, dans Forge) |
| Wuffs / Cranelift | JIT déterministe à backends multiples → KASM compilé per-machine |
| Erlang / OTP | Let-it-crash + supervisors → pas de défense, supervision externe |
| Spanner TrueTime | Borner l'incertitude au lieu de la cacher → mesure ε explicite |

**Règle d'usage** : quand tu cites un précédent, le pattern doit être
**transposable au problème courant**, pas une métaphore décorative. Si
tu écris "luciférine = atlas, photon = match", tu fais de la métaphore.
Si tu écris "ring buffer de seqno + memory barrier = pipeline lock-free",
tu fais du pattern transposable.

### 7.5 Le test final avant tout commit non-trivial

Une seule question, honnête :

> Est-ce qu'un GPT-4 ou Claude moyen, prompté avec la même tâche **sans
> contexte Forge**, sortirait ce code au premier coup ?

- **Oui** → tu es dans la distribution de training. Recommencer.
- **Non** → tu as dévié assez pour avoir trouvé quelque chose. Garder.

C'est subjectif mais honnête. Si tu hésites, c'est probablement oui.

### 7.6 Quand le pattern idiomatique EST la bonne réponse

Tout ne mérite pas un saut technique. Les cas où la voie idiomatique est
correcte et où §7 ne s'applique pas :

- **Glue code, plomberie** qui sera lue plus que tournée
- **Parsing/sérialisation** de formats standards (sauf si hot path)
- **Tests unitaires** (préférer le clair au tordu)
- **Surfaces d'API publique** (cohérence > originalité)
- **Code rare**, exécuté < 1× / minute, où le coût d'écriture domine

Le saut technique est **réservé aux hot paths, aux contraintes physiques
violées, aux questions où l'idiomatique plafonne**. Pas un critère
universel — un outil ciblé.

### 7.7 Amputation avant addition (renforcement Via Negativa)

Avant d'ajouter du code, **toujours** se demander : quel code existant
peut disparaître ? Si tu peux supprimer 50 lignes pour résoudre le
problème, c'est mieux que d'ajouter 50 lignes. Le commit avec Δ négatif
de lignes Rust est valorisé. L'amputation expérimentale (couper une
fonction qui paraît utile et voir si quelque chose casse) est encouragée
— commit avant d'amputer (la Corbeille Windows ne récupère pas les `rm`).

### 7.8 Ce que cette section n'est PAS

- ❌ Pas un rituel à inscrire dans CARNET avant chaque commit
- ❌ Pas une rotation forcée à travers une liste de domaines
- ❌ Pas un démolisseur qui rejette tout pattern nommable
- ❌ Pas un vocabulaire interdit
- ❌ Pas un quota de "techniques piochées par phase"
- ❌ Pas une obligation de citer un précédent radical pour chaque commit

**C'est une boussole technique** : quand tu sens que ton code est trop
facile, trop idiomatique, trop "obvious", relire §7 et balayer les
pivots. Quand le code est légitimement simple (glue, tests, API), pas
besoin de §7.

---

## 8 · Langage mutant — quand Rust n'est plus le bon outil

> **À lire quand tu sens que les concepts de Rust standard plafonnent
> sur le problème courant.** Pas un problème de syntaxe ou de nom — la
> sémantique de Rust (types, ownership, dispatch, allocation) ne paie
> plus sur ce hot path. Réponse : muter le langage, pas le décorer.

### 8.1 Surface vs substrat — la distinction critique

**Mutation de surface (théâtre, ne marche pas)** :
- Renommer les fonctions en pictogrammes Unicode
- Bannir le mot `match` mais coder un `if/else` qui fait pareil
- Inventer une syntaxe différente pour appeler `Vec::push`

→ La sémantique reste celle de Rust standard. Calligraphie. Aucun gain
mesurable.

**Mutation de substrat (réelle, marche)** :
- Changer ce que `&T`, `Box<T>`, ou `fn()` veulent dire au niveau machine
- Réinterpréter les structures de données comme des objets d'une autre nature
- Remplacer la dispatch table de Rust par autre chose

→ Le compilateur Rust voit toujours du Rust valide. Le programme tournant
n'a plus la sémantique de Rust standard. **C'est ce qui paie.**

### 8.2 Forge fait DÉJÀ ça — KASM est une mutation substrat de Rust

```rust
let prog = Program::from_bytes(&blob)?;
let result = execute(&prog, &input)?;
```

`Program` n'est pas une struct Rust normale — c'est un blob
content-addressed dont l'identité EST son SHA-256. `execute` ne lance pas
un appel de fonction Rust — c'est un interpréteur de bytecode dans une
autre dimension. Du point de vue du compilateur Rust : valide. Du point
de vue du programme tournant : **Rust est l'échafaudage, KASM est le
vrai langage.**

KASM ⊂ Rust syntaxiquement, mais sémantiquement c'est ailleurs.
**Reconnaître cette mutation existante est le point de départ pour en
faire de nouvelles.**

### 8.3 Quatre patterns de mutation substrat applicables à Forge

#### 8.3.1 Type = hash de contenu (style Unison)
```rust
// Standard :
fn add(a: i64, b: i64) -> i64
// Mutant :
fn h_a4f9c2(...) -> ...   // le nom EST le hash de l'AST
```
La fonction n'a plus de nom logique — elle est référencée par son
contenu. Deux fonctions équivalentes ont le même nom. Refactor =
impossible (le hash change, donc c'est une autre fonction). Forge a ça
à moitié via `Program` SHA-256.

#### 8.3.2 Pointeur = offset dans le Loom (style Urbit / mmap)
```rust
// Standard :
let prog: Box<Program> = Box::new(...);
// Mutant :
let prog: LoomOffset = loom.alloc(...);  // u32, pas un Box
```
`Box<T>` disparaît. Tout vit dans un mmap unique. `&T` devient un offset,
pas un pointeur. Le borrow checker ne marche plus comme prévu — on le
contourne avec des wrappers. **Plus de fragmentation, plus de GC, plus
de heap.** C'est plus vraiment du Rust — c'est du Loom mappé.

#### 8.3.3 Dispatch par hash (style content-addressed Lisp)
```rust
// Standard :
fn dispatch(op: Op, a: i64, b: i64) -> i64 {
    match op { Op::Add => a + b, Op::Mul => a * b, ... }
}
// Mutant :
fn dispatch(op_hash: u64, args: &[i64]) -> i64 {
    let fn_ptr = ATLAS.lookup(op_hash);
    unsafe { fn_ptr(args) }
}
```
Plus de `match`, plus d'`enum`. Chaque opcode = un hash, et le hash
mappe directement à un pointeur de fonction JIT-compilée. Le langage
n'a plus d'instructions — il a des hashes qui se résolvent.

#### 8.3.4 Branchement = arithmétique (style Bitwise / cryptographie)
```rust
// Standard :
let y = if cond { a } else { b };
// Mutant :
let y = b + (a - b) * (cond as i64);
```
Pas de pipeline stall, pas de branch prediction. Le CPU exécute toujours
la même séquence. **Mutation à petite échelle mais qui change la nature
des programmes** — déterministes en cycles, pas en branches.

### 8.4 Le coût réel d'une mutation substrat

- **Code moins lisible** par un dev Rust standard (le compilateur ne
  dit plus la vérité sur ce qui se passe)
- **Outillage cassé** : `rust-analyzer`, `lldb`, `perf` voient bizarre
- **Onboarding plus dur** : un dev junior ne peut pas contribuer
- **Bugs plus subtils** : les invariants Rust normaux ne tiennent plus,
  et le compilateur ne te protège plus comme tu crois

### 8.5 Quand appliquer une mutation substrat

✅ **Ça paye** quand :
- Hot path mesuré qui plafonne sur les abstractions Rust standard
- Contraintes physiques violées (cache lines, latence DRAM, zero-alloc)
- Identité content-addressed essentielle au modèle (Forge entier)
- Le saut de paradigme débloque un facteur ≥ 10× mesurable

❌ **Ça ne paye pas** quand :
- Code écrit une fois, lu 100 fois (favoriser lisibilité)
- API publique exposée à d'autres équipes/contributeurs
- Glue, plomberie, configuration
- Les abstractions Rust standard donnent déjà le résultat acceptable

### 8.6 Le test "vraie mutation ou déguisement ?"

Une mutation substrat **doit casser au moins une attente fondamentale**
du Rust standard :

- Les types ne sont plus liés à leur nom textuel (Unison)
- Les pointeurs ne pointent plus dans le heap allocateur (Loom mmap)
- Les appels de fonction ne sont plus résolus à la compilation (CAS dispatch)
- Les branches ne créent plus de divergence d'exécution (branchless)
- L'identité d'une donnée n'est plus son adresse mais son contenu (CAS)

Si ton "mutant" garde toutes les attentes Rust intactes, c'est juste du
Rust avec des noms exotiques = théâtre. Recommencer.

### 8.7 Précédents historiques de mutation substrat dans Forge

Reconnaître ce qui a déjà été muté permet de pousser plus loin sans
réinventer :

| Phase Forge | Mutation substrat appliquée |
|---|---|
| KASM bytecode | Programme = blob SHA-256, plus une fonction Rust nommée |
| `forge.cas` | Storage par hash, plus par path filesystem |
| Atlas v1 | Lookup hash → programme, plus traversée d'arbre |
| Φ.μ.7.13 `AtlasIngest` trait | Dispatch par hash, plus par type Rust |
| Φ.ν.7 `LiveAtlas` | Persistance ATLASV2 mmapable, plus serde JSON |
| Φ.ν.7e De Bruijn α-norm | Identité = forme, plus liée au nommage des slots |

Φ.ν.7d (queue lente `ultra_clamp_fsqrt`) candidate naturelle : muter
`Program` en `RamProgram = (ramée_id: u16, sève: [i64; K])` = dialecte
KASM spécialisé à 798 opcodes virtuels. **Pas du Rust idiomatique. Pas
du KASM générique. Un sous-langage taillé pour la queue lente.**

---

## 9 · Filtre paranoïaque multi-échelle — la règle d'or de Forge

> **Règle architecturale fondamentale** : le CPU brain n'envoie JAMAIS au
> compute (GPU ou CPU SIMD) un calcul dont le résultat existe déjà dans
> l'atlas, **à n'importe quelle échelle de la hiérarchie de décomposition**.

C'est probablement la règle la plus importante de toute la doctrine
Forge, parce qu'elle conditionne le ROI mondial du système.

### 9.1 Les 6 échelles de redondance

```
Niveau 6 — META       (calcul COMPLET : protéine entière, backtest entier)
Niveau 5 — GRANDE     (pans entiers : epoch training, iteration MD)
Niveau 4 — MOYENNE    (composants : layer réseau, evoformer block)
Niveau 3 — SEMI       (blocs algorithmiques : attention head, MLP, EMA)
Niveau 2 — MINI       (sous-arbres ~10-100 nœuds : matmul, normalisation)
Niveau 1 — MICRO      (ops courtes ~5-10 nœuds : dot product, distance)
Niveau 0 — TROP FIN   (single op = trop cher à cacher, ignored)
```

Chaque échelle a son **profil hit/gain** :
- **META** : hit rare individuellement, mais gain par hit = ×∞
- **MICRO** : hit massif, mais gain par hit = peu
- **Niveaux intermédiaires** : compromis qui contribuent à l'effet cumulé

**L'erreur classique** = ne cacher qu'à UNE seule échelle (ex: top-level
seulement, ou single-op seulement). Forge cache **à toutes les échelles
simultanément**.

### 9.2 Cascade lookup top-down avec early exit

Algorithme de filtre paranoïaque :

```
fn paranoid_filter(call: Call) -> Option<Output> {
    // Du plus gros au plus petit, early exit dès le premier hit
    for scale in [META, GRANDE, MOYENNE, SEMI, MINI, MICRO] {
        let hash = hash_at_scale(call, scale);
        if let Some(result) = atlas.lookup(hash) {
            return Some(result);  // EARLY EXIT — économie maximale
        }
    }
    None  // truly new at all scales, must compute
}
```

**Coût** : 6 lookups × 10 µs SHA-NI = **60 µs au pire**.
**Bénéfice** : si hit au niveau META, on économise potentiellement
**plusieurs heures de compute**. ROI ×∞ structurel.

### 9.3 L'auto-détection des frontières de cache (CŒUR du filtre)

Le CPU doit détecter **automatiquement** les sous-calculs cacheables à
toutes les échelles, sans intervention utilisateur. **3 mécanismes
complémentaires** (Phase 12.2 dans ROADMAP) :

#### Pattern-based (analyse statique du DAG)

Scanner le programme à sa première exécution, identifier les sous-arbres
récurrents (structure identique ou α-équivalente après De Bruijn).
Sous-arbre apparaissant ≥ 3 fois → promotion automatique en
sous-programme content-addressed via Op::Fractal implicite.

#### Profile-guided (trace dynamique)

Pendant les K premiers runs, tracer :
- Quels sous-arbres ont été exécutés
- Combien de fois
- Coût moyen par exécution

Sous-arbre coûtant ≥ X µs ET apparaissant ≥ N fois → cacheable.

Seuils par échelle :
- **MICRO** : 1 µs, 100 fois
- **MINI** : 10 µs, 50 fois
- **SEMI** : 100 µs, 20 fois
- **MOYENNE** : 1 ms, 10 fois
- **GRANDE** : 100 ms, 5 fois
- **META** : 1 s, 1 fois (premier run suffit)

#### Threshold-based (apprentissage en ligne)

À chaque appel, hash le sous-arbre exécuté et son input. Si le couple
apparaît ≥ N fois → insert dans atlas immédiatement.

**Forge apprend ses propres patterns de redondance.** L'utilisateur ne
déclare jamais "ce sous-calcul est cacheable" — Forge le voit tout seul.

### 9.4 Conséquence — le GPU ne reçoit que le neuf

Le dispatcher GPU applique strictement :

```rust
fn dispatch_to_gpu(batch: Vec<Call>) -> Vec<Output> {
    let (cached, fresh) = batch.into_iter()
        .partition(|call| paranoid_filter(call).is_some());
    
    // GPU ne touche PAS au cached — économie massive
    let fresh_results = gpu_evaluator.eval_batch(fresh);
    let cached_results = cached.iter().map(lookup);
    
    merge(cached_results, fresh_results)
}
```

À l'échelle d'un workload typique : 70-95 % des sous-calculs sont
filtrés (déjà cachés à une échelle quelconque), 5-30 % seulement vont
réellement au GPU. **Throughput effectif GPU multiplié par 5-100** sans
changer le hardware.

### 9.5 Pourquoi cette règle change tout

Sans filtre paranoïaque multi-échelle, Forge serait juste "un système
de cache plus joli". **Avec, Forge devient catégoriquement différent**
des autres substrats de calcul :

- **Spark/Dask** : cache au niveau RDD seulement (1 échelle)
- **Stockfish transposition table** : cache par position (1 échelle)
- **Bazel build cache** : cache par fichier objet (1 échelle)
- **CPU caches L1/L2/L3** : 3 niveaux mais 1 seule échelle (taille de ligne fixe)
- **Forge filtre paranoïaque** : **6+ échelles hiérarchiques avec
  auto-détection**

C'est le mécanisme qui rend possibles les calculs moonshot
(interactome humain, drug screening exhaustif, whole-cell simulation —
voir ROADMAP §"Le moonshot scientifique").

### 9.6 Implications pour le code Forge

À chaque commit qui touche au runtime, au dispatcher, ou à l'atlas :
**vérifier que la règle paranoïaque tient à toutes les échelles**.

Si un agent propose "envoyons ça directement au GPU sans passer par
l'atlas pour gagner 10 µs" → **rejet immédiat**. Les 60 µs de la
cascade paranoïaque sont **par construction** moins chers que 99 % des
calculs qu'ils filtrent. Court-circuiter la cascade casse le contrat
fondamental de Forge.

**Le ROI de Phase 12 dépasse celui de toutes les autres phases combinées
sur les workloads multi-laboratoires.** C'est ce qui justifie son
positionnement en 7ᵉ position canonique (juste après Phase 11 KASM
enrichi, avant Phase Ω.1-Ω.4 PARTIEL).

---

**Si tu lis ce fichier en début de session : commence par
`git log --oneline master` pour reprendre le fil exact.**

### 6.2 Note session 2026-04-30 (UI + GPUnode)

- CUDA active dans `examples/forge_tauri_ui/src-tauri/Cargo.toml`.
- Pipeline GPU valide avec `cargo check --features cuda` et `cargo test --lib --features cuda`.
- UI `Resultats` refondue en canvas document unique + animation dependante du programme selectionne.
- Details complets et objectifs rayes: voir l'entree `Phi.12.2` dans `CARNET.md` et la section "Mise a jour session (2026-04-30, UI + GPUnode)" dans `ROADMAP.md`.

### 6.3 Note session 2026-05-03 (multi-GPU + dispatch_batch + 18 wave primitives wirées)

Synthèse condensée — détails dans `CARNET.md` entry du jour :

**Performance** :
- Auto-router CPU v0/v1/v2 (AffineI64 + HashChain + Interpret≤64 nodes)
  → sub-150 ns sur Léger workloads (45-51× vs ancien path).
- `dispatch_batch` entry-level CPU-fast bypass → 4-5× sur Léger
  (skip cascade dispatch_impl pour programmes auto-routables).
- Op::Cond JIT branchless lowering → branched batch 511 ns → 10.7 ns (47×).
- Stack-only KASM interpreter (`try_execute_i64_inline`) → 0 Vec alloc per call.

**Multi-GPU** :
- WGSL universal KASM kernel (20 opcodes i64-only, stack 128 nodes).
  9/9 programmes ADN bit-exact validés vs `kasm::execute`.
- Multi-GPU split CUDA + WGPU en parallèle via `thread::scope`
  quand 2 GPU vendors différents détectés (NVIDIA + AMD/Intel).
- CPU-fast detection : programmes Léger ne quittent jamais le CPU,
  GPU réservé aux workloads Lourd (≥ 65 nodes Interpret) — 6/9
  programmes ADN passent en CPU eval_serial automatique, seul
  crypto_heavy 101 nodes va au split GPU.

**18 Wave primitives wirées (Tier 1-4)** :

| Tier | Modules | Wire location |
|---|---|---|
| 1 storage | mmap_store, intrusive_index, prefault, huge_pages, bump, slab, swiss_table | Layer 5 fast read + Store::blobs + scratch pools |
| 2 concurrency | arena_lt, static_pool, disruptor, seqlock | MonsterNode pools + APIs publiques |
| 3 inference | nnue, seminaive, cow_snapshot | Oracle alt + Datalog + Atlas checkpoint |
| 4 audit | lua_table, walkforward, speed_ablation, mono_audit | Domain index + validation + audits |

API publique étendue sur MonsterNode : `enable_mmap_view`, `bump_alloc`,
`call_pool_take`, `event_publish`, `domain_state_read`, `nnue_predict`,
`seminaive_run`, `state_take_snapshot`, `domain_index_insert`,
`walkforward_run`, etc. Détails dans le commit `b4b74ba` et le code
`src/monster/mod.rs`.

**Storage** :
- `lab_findings.jsonl` untracked (gitignored), commande `cargo run
  --release --example lab_runner -- prune [N]` pour rotation manuelle.
- Forge.cas + Tauri build artifacts également gitignored.

**Tests** : 1055/1055 PASS sur default + cuda + wgpu + cuda+wgpu builds.
Aucune régression de doctrine § ; le revert du Via Negativa erroné
(commit `d80629d`) prouve la doctrine §6 ("reconnaître les régressions").

### 6.4 Note session 2026-05-03 (section α Alpha + GPU dispatch synth + Φ.ν.7g)

Synthèse condensée — détails complets dans `CARNET.md` entry du jour :

**Section α Alpha** (Tauri UI nouvelle) :
- Panel reverse synthesis : Target / Risk / Indicators / Synthesis params
- Canvas chart bougies japonaises (zoom/pan/hover, drag-drop CSV)
- Channels IPC dédiés (`alpha-log`, `alpha-signal`) pour ne pas polluer
  les events de la section Ψ DNA
- Backend `start_alpha_synthesis(csv_bytes, params)` — Tauri command
  paramétrée qui réutilise toute la machinerie de synth Forge sur des
  données OHLCV utilisateur

**Pipeline reverse synth NATGAS H4** (`synth_strategy.rs`, ~1713 lignes
pure Rust + std) :
- Parser CSV OHLCV (BOM UTF-8 géré, ISO-8601 ou epoch ms)
- **10 raw features per-feature** (post Φ.ν.9f) : hour, dow, rsi14,
  ema8_delta, ema21_delta, atr14_bps, lag_ups, hilo_bps, adx14,
  vwap6_delta — chacune extraite comme entier significatif
  (RSI en 0-10000, deltas en basis points, etc.)
- `build_per_feature_examples()` pour beam search par feature
- `extract_raw_feature()` + `eval_strategy_per_feature()` pour eval
- Trade simulator (SL fixe en points, exit horizon configurable,
  decision-hour filter "no entry 02h-06h UTC")
- Examples builder par range (split temporel train/holdout strict)
- Per-day evaluator avec **métriques hedge-fund** : Sharpe, Sortino,
  Calmar, MaxDD, Profit Factor, max consecutive losing days
- Verdict commercial automatique :
  - ⭐ HEDGE-FUND-GRADE : Sharpe ≥ 1.5 ET Sortino ≥ 2 ET Calmar ≥ 1
    ET PF ≥ 1.5
  - 📊 RETAIL-GRADE : profitable mais sub-target institutionnel
  - ⚠️ NON-VENDABLE : ratios insuffisants

**Bugs critiques trouvés + fixés** :

1. **CSE branch-sensitive ops** (commit `91ea818`) — `cse()` éliminait
   silencieusement les nodes Min/Max/Select/Clamp/Cond parce que
   trace-equivalence sur 8 sample inputs ≠ semantic-equivalence pour
   les ops conditionnelles. **Fix** : skip dedupe par trace pour
   ces ops dans phase 2 de `cse()`. Trace-equivalence est nécessaire
   mais pas suffisante — ces ops gardent leur identité.
   **Test régression** : `cse_preserves_clamp_min_max_branch_semantics`.

2. **call_many_values_i64 divergeait de call_one_i64** sur les
   programs avec Min/Max (commit `ae14a9b`). Le path BATCH passait
   par `dispatch_impl` → `execute_with_jit` (JIT compiled) qui
   donnait des résultats différents de `try_execute_i64_inline` (interp
   inline utilisé par `call_one_i64`). **Fix Via Negativa** : 80→40
   lignes, suppression du path divergent, le scalaire boucle
   `call_one_i64` (path éprouvé). Code plus simple à raisonner ET
   bug résolu.

**GPU dispatch dans synth — succès partiel** :

`score_program` route maintenant via `<MonsterNode as
BulkEvaluator>::eval_batch` qui appelle `gpunode_runtime`. Heuristique
étendue dans `all_calls_cpu_routable` :
  - Un programme CPU-routable seul devient HEAVY (= GPU paye) si
    `nodes ≥ HEAVY_NODES_THRESHOLD` (16) ET `batch × nodes ≥
    HEAVY_VOLUME_THRESHOLD` (50 000)

Mesure honnête : le GPU dispatch wired marche techniquement mais ne
réduit PAS le temps total de synth alpha. **Le bottleneck est dans
`train.rs::synthesize_i64::push_binary` loop** (220k combinaisons par
gen × 17 567 examples = 3.7 milliards d'ops Rust pures single-thread).
Le GPU ne s'enclenche que pour les WINNERS (~10-100 candidats par
gen qui passent les pré-filtres avant scoring). La génération des
candidats reste CPU séquentiel.

**Solutions futures pour le bottleneck push_binary** :
1. Réduire combinatoire (`beam_width` 192 → 32 + `max_nodes` 16 → 8)
2. Paralleliser sur threads CPU (refusé en cette session)
3. JIT-compiler la search en kernel GPU (chantier 2-4 semaines)

**Données NATGAS_USD H4** :
- 25 381 bougies (16.4 ans, 2009-12-31 → 2026-05-01)
- Fetched depuis OANDA v20 API (endpoint candles)
- Stockées HORS du repo : `%USERPROFILE%/Documents/GitHub/Forge-data/oanda/`
- `.gitignore` mis à jour pour bloquer `examples/data/`, `*.csv`,
  `*.token`, `.env*`

**Tests** : 1064 / 1064 Forge core PASS + 22 / 22 synth_strategy PASS
(parser, features, simulator, métriques pro).

**Doctrine confirmée** :
- §5 "Reconnaître les régressions" : appliqué 3 fois cette session
  (revert InlineCache wire naïf, revert score_program via dispatch_impl,
  revert cse() couplé hot_program)
- §7 "Anti-easy-fix" : la simplification `call_many_values_i64` est
  exactement ce que la doctrine prêche
- §9 "Filtre paranoïaque multi-échelle" : poussé jusqu'au mode fractal
  méta par le user, reconnu théoriquement, pas implémenté complètement
  (op_memo always-on tenté puis reverté pour bug Min/Max)

### 6.5 Note session 2026-05-03 (Φ.ν.8 — atlas unifié + ComputationPlan + wire-up cross-session 100%)

Synthèse condensée — entrée complète dans `CARNET.md` Φ.ν.8 :

**Bloc nouveau** : `src/atlas.rs` (~330 LoC, top-level — pas un nouveau
sous-dossier, doctrine §1 respectée). 9 kinds, append-only, `Arc<Atlas>`
partagé entre ForgeBackend (Tauri) et MonsterNode (lib core) via
`MonsterNode::attach_atlas(arc)` au boot.

**API atlas** :
- `record(kind, hash) → io::Result<bool>` pour kinds 1-4 (key only)
- `record_with_value(kind, key, value_20B) → bool` pour kinds 5-9
- `lookup_with_value(kind, key) → Option<[u8;20]>`
- `contains(kind, hash) → bool` / `count_kind(k) → usize` / `total() → usize`
- Helpers : `result_key`, `feature_key`, `trade_key`, `opmemo_key`,
  `pack_f64`, `pack_i64`, `pack_trade`, `pack_u64`, `unpack_*`

**Wire-up runtime cross-session** :
- `dispatch_batch` : atlas RESULT lookup avant cascade brain layers,
  atlas record après bulk evaluator compute (slow lane et CPU-fast bypass)
- `dispatch_impl` Layer 5b : atlas RESULT lookup après Layer 5 (disk memo)
  et avant Layer 6 (interpreter), atlas record après compute. Couvre
  `call_one_i64` slow lane et `call_value_bytes_hot_args`.
- `execute_with_op_memo` : Hash64 utilise L1 op_memo (RAM) + L2 atlas
  OPMEMO. Promote L2 → L1 sur hit.
- `train.rs::push_binary` : signature étendue `(.., targets_fp, atlas)`,
  lookup atlas SCORE avant compute loss. Skip provably-constant
  (outputs constants).

**Tauri Frontend (synth path)** :
- `FeatureCache::build(bars)` O(N) prefix sums + `persist_to_atlas(atlas, file_hash)`
  → ~177k FEATURE entries sur NATGAS H4. Bit-identical à l'ancien path,
  **10.1× speedup** mesuré.
- `simulate_trade_with_atlas(.., atlas, file_hash)` : checks atlas TRADE
  avant simulation, écrit après.
- `build_examples_in_range_masked_with_atlas` : variante atlas-backed
  utilisée par les Tauri commands `start_alpha_synthesis` et
  `start_computation` reverse_synth path.
- `inspect_program_map(kind, bytes_opt)` Tauri command : `ComputationPlan::build`
  + enumerate_synth_candidates_d2/d3 + cse_classify + analyze_subtree +
  trace_classify + atlas_warm_estimate + analyze_subwindow.
- `inspect_cache: HashMap<(kind, file_hash), ComputationPlanReport>` :
  second appel mêmes bytes = ~1 ms.
- Skip warm-atlas heavy passes : `count_kind(SUBTREE) >= 1000` ou
  `count_kind(PEEK) >= 4000` → bypass mining + lit le compteur.

**Mesures runtime confirmées (NATGAS H4 25 381 bars)** :
- Sub-tree redundancy : 43 242 → 3 331 unique = **92.3%**
- Sub-window redundancy : 10.30M → 380.7k sliding = **96.31%**
- Atlas peek cold cache : 832/1024 brain-resolvable = **81.2%**
- CSE depth-2+3 : 21 856 raw → 3 345 classes (84.7% redundancy)
- 33 provably-constant SKIPPED downstream (live_reps)

**Test cross-session prouvé** :
`dispatch_batch_persists_results_across_sessions_via_atlas` — session 2
voit `RecordingEvaluator.seen.len() == 0` (assertion stricte). Result
vient de l'atlas, pas du compute.

**Tests** : 1071/1071 PASS lib (+3 atlas + 1 cross-session vs baseline 1067) |
32/32 PASS Tauri (+5 vs baseline 27).

**Bottleneck reconnu** : Mutex<File> sérialise les writes. Sur session
synth complète (Mk RESULT + ~177k FEATURE + ~50k TRADE + Mk SCORE + Mk
OPMEMO), contention mesurable. Passage à mmap shardé per-kind = sujet
session suivante. Correctness et accessibilité cross-session livrées
et prouvées par tests runtime cette session.

**Doctrine §9 (filtre paranoïaque multi-échelle)** : matérialisée. 6
échelles de détection (CSE / TRACE / SUBTREE / PEEK / SUBWINDOW / RESULT)
+ 5 échelles de persistance cross-session (RESULT / FEATURE / TRADE /
SCORE / OPMEMO) dans un seul bloc atlas. La cascade lookup top-down avec
early exit posée comme doctrine fondamentale est maintenant un wire-up
runtime concret.

### 6.6 Note session 2026-05-05 (Λ trajectoire unifiée — bloc B 100%)

Synthèse condensée — entrée complète dans `CARNET.md` 2026-05-05.
**Cette session SUPERSÈDE la description Φ.ν.8 ci-dessus** sur les
points suivants :

**Atlas runtime API simplifiée** :
- 5 kinds value-bearing (FEATURE/TRADE/SCORE/OPMEMO/RESULT) collapsés
  vers **1 seul `kind::RESULT`** post M1.5 (commit `d6fe1e7`) + M2
  (commit `d7848f0`).
- Le file format hérite des 8 kind tags pour rétro-compat lecture
  (anciens atlases parsent cleanly), mais TOUS les nouveaux writes
  vont sous `kind::RESULT`.
- Atlas keying unifié : `Hash::for_blob(input)` partout (M1.1
  commit `82230b1`), élimine le bug d'aliasing >12 bytes pour tous
  les call sites (apply, dispatch_batch, dispatch_impl, train,
  exec).

**Ce qui n'est PLUS écrit dans l'atlas** :
- `FeatureCache::persist_to_atlas` SUPPRIMÉ M1.3 (commit `40a1118`).
  Les ~177k FEATURE entries par run de NATGAS H4 étaient
  write-only en prod (jamais lues). −85 LoC, ~150ms/run, ~9MB/file
  économisés.
- Plus jamais d'entrées spécifiques TRADE/SCORE/OPMEMO sous leurs
  kinds dédiés ; tout sous RESULT.

**Λ singular operation** :
- `pub fn apply(node, func, input) -> Vec<u8>` (commit `bb07b2d`)
  est l'opération singulière de Forge. Content-addressed inputs
  via `Hash::for_blob`. Atlas RESULT lookup → `kasm::execute`
  fallback.
- `apply_subtree` (commit `aa53941`) étend Λ.4 sub-expression
  scale : slice arbitraire d'un programme via
  `extract_output_subprogram` + apply().
- Tauri command publique `apply_program` (commit `fa531eb`)
  expose la singular operation à JS.

**Self-host KASM-en-KASM** :
- `general_6node_self_host_program` (M3, 123 nodes KASM) — un
  seul programme interprète n'importe quel programme 6-node sur
  subset {Input, Const, Add, Sub, Mul, Output} via dispatch
  dynamique sur op-byte décodé.
- `generalized_score_program` (M4, ~480 nodes KASM) — score K=4
  examples unrolled, ANY 6-node candidate, pure KASM.
- ISA Wave 7i `Op::VGetI64 = 66` ajouté pour rendre le self-host
  possible.

**5 root nodes Hassabis-style inscrits** (cf. ROADMAP.md ~ligne 605) :
- Root #1 atlas distribué — plus tard, après Forge local prêt
- Root #2 synth = exécution — en cours (Λ.3 lite + M3+M4 v2)
- Root #3 atlas-as-model — final, lié à phase ι BitNet
- Root #4 typed KASM — Wave 9, débloque synth ÷10-100×
- Root #5 stochastique seedé — LIÉ à Root #1

**Reductions** : −607 LoC sur M1+M2+SpeculativeDispatchCache purge.
Ajouts +500 LoC sur M3-M6 (constructeurs KASM + tests).
**Net session ~+200 LoC** mais l'architecture cross-domain est
maintenant **ferme** : prochaines déletions massives débloquées
(kasm::execute Rust, brain layers, Tauri sections specific).

**Tests final session** : **1095 / 1095 lib + 49 / 49 Tauri PASS**.

**Direction remplacée 2026-05-06** : avant Bloc C (atlas distribué Root #1),
Forge local doit être MCP-first, reproductible et prêt à l'emploi :
`forge_mcp` stable, jobs multi-fichiers, logs de calcul visibles dans l'UI,
manifestes/proofs exportables, résultats publiables dans le MCP pour les
agents IA. Le polish Tauri reste nécessaire, mais uniquement comme panneau
d'observation, pas comme orchestrateur principal à boutons.

### 6.7 Note session 2026-05-05 (Φ.ν.9f — per-feature synthesis + dual-GPU visibility)

**Problème fondamental résolu** : le bitfield packé i64 (11 features
encodées en bits) était inutilisable par le beam search — pas de Shr
dans les 9 ops disponibles pour isoler les bits individuels. Résultat :
`best: 0` bloqué, 5 gens identiques.

**Architecture per-feature** :
- `synth_strategy::FEATURE_NAMES` : 10 features raw (RSI, EMA deltas,
  ATR, ADX, VWAP, etc.) extraites comme entiers significatifs
- `build_per_feature_examples()` → 1 beam search par feature
- `extract_raw_feature(bars, i, idx, cache)` pour eval post-synth
- `eval_strategy_per_feature()` remplace `eval_strategy_with_signal_callback_masked`
  dans le path alpha

**Visibilité temps réel** :
- `SynthProgress` struct + `Expr::Display` + `LAST_GPU_BACKEND` atomic
- Logs ◆/▶/✓ avec feature name, GPU backend, throughput M-ops/s
- `skip_prepass: true` élimine 50s de recognizers inutiles

**Tests** : 1097/1097 lib + 34/34 Tauri PASS.

