# SOTA_FRONT_RUST.md — État de l'art front Rust/WASM (juin 2026)

> Briefing issu d'une lecture directe des dépôts (clones locaux) + recherches web datées.
> Sources code : `DioxusLabs/dioxus` (master = `0.8.0-alpha.0`), `leptos-rs/leptos` (`0.8.19` stable, `0.9.0-alpha`).
> Objectif : décider l'archi front Forge en connaissance réelle, pas sur slogans.

---

## 1. La couche socle : WebAssembly + toolchain (état juin 2026)

- **WebAssembly 3.0** (fin 2025) est la vraie rupture : **GC**, **Memory64**, **exception handling**, JS String
  Builtins, désormais standard. Conséquence directe : les langages GC (Kotlin, Dart, Scala…) n'embarquent plus
  leur runtime mémoire. Rust n'a pas de GC, donc en bénéficie surtout via **reference-types** (moins de glue JS).
- **reference-types** : permet à WASM d'appeler des APIs avec des valeurs JS natives → réduit la colle générée par
  `wasm-bindgen`. Active par défaut sur les toolchains récentes.
- **panic=unwind** sur cible wasm émet désormais le *modern exception handling* par défaut (nightly ≥ 2026-05-06).
- **Gouvernance** : l'org `rustwasm` est archivée ; `wasm-bindgen` et `wasm-pack` passent à de nouveaux
  mainteneurs. L'outil **`wasm-bindgen`** reste la fondation ; `trunk` et la CLI `dx` (Dioxus) sont les
  orchestrateurs courants. Pour Forge sur Tauri, on n'utilise PAS trunk : Tauri sert le bundle.

**À retenir** : la cible WASM est mûre. Le mur n'est plus « est-ce que ça tourne » mais « taille du binaire,
latence de démarrage, et qualité du pont WASM↔JS↔GPU ». C'est précisément ce que les deux frameworks adressent.

---

## 2. Dioxus — architecture réelle (lecture du code)

### Modèle
- VirtualDOM **template-based** : la macro `rsx!` génère des `Template` **statiques à la compilation** ; seuls
  les nœuds dynamiques sont diffés au runtime. Peu d'allocations, updates ciblées.
- Renderers branchables via le trait **`WriteMutations`** → même arbre de composants sur web (WASM), desktop
  (wry/tao), **natif GPU (Blitz + Vello/wgpu)**, liveview (WebSocket), SSR.
- Web : interpréteur **Sledgehammer** (protocole binaire compact) côté JS, piloté par wasm-bindgen.

### État réactif (le vrai différenciateur ergonomique)
- **`generational-box`** : donne des sémantiques **`Copy + 'static`** aux références. On passe les signaux
  partout sans clone ni galère de lifetime. Validité garantie par **compteur de génération** (pas de
  use-after-free, pas d'overhead par accès). C'est un mini-GC dont la durée de vie est liée au **scope du
  composant**, pas au scope lexical Rust.
- Primitives : `Signal<T>` (mutable, souscription auto via `.read()`, `.peek()` sans souscrire), `Memo<T>`
  (dérivé paresseux, skip si `PartialEq` égal), `GlobalSignal` (singleton lazy), `ReadSignal/WriteSignal`
  (type-erased).
- **`Store<T>`** (`#[derive(Store)]`) : réactivité **par champ** via un arbre de chemins. Écrire `store[1].name`
  ne réveille que les abonnés du chemin `[1]` et `[1, name]`, pas `[0]`/`[2]`. Idéal pour structures imbriquées
  (≈ l'état des sections Forge : real-estate, trading).

### Différenciateurs lourds pour Forge
- **`wasm-split`** (`#[wasm_split(feature)]`) : **code splitting WASM réel**. Découpe le binaire en
  `main.wasm` + `module_*.wasm` + `chunk_*.wasm` partagés, chargés à la demande via table de fonctions
  indirecte. → on charge la section *banger 3D* / *splat-loader* seulement quand l'utilisateur l'ouvre.
  C'est l'argument n°1 pour une UI lourde comme la tienne. Contrainte : points de split = fonctions `async`,
  graphe statique connu à la compilation.
- **Subsecond hot-patching** : recompile **seulement les fonctions modifiées** (thin build) → patch dylib →
  table de saut `APP_JUMP_TABLE` mise à jour à chaud. Marche desktop/mobile ; **web = rechargement de module
  limité**. Limites importantes : **changement de struct non supporté** (taille/alignement → crash), seul le
  *tip crate* est patché (pas les libs de l'espace de travail).
- **Native renderer** (Blitz + Vello, GPU via wgpu) : expérimental ; rend le CSS sur GPU **sans navigateur**.
  Mais pour tes scènes **WebGPU/WGSL custom** (ingen-render), tu ne passes PAS par Vello : tu embarques une
  **surface `wgpu`** dans un `<canvas>` (Dioxus dit explicitement « Embed Dioxus in Bevy, WGPU »). Tes shaders
  WGSL restent réutilisés tels quels.
- Fullstack intégré sur **Axum** : `#[server]` génère client (sérialisation) + handler serveur. WebSockets,
  SSE, streaming, upload, middleware. Pertinent si un jour le back web de Forge se rapproche du front.
- Bundle web optimisé : compression `.wasm`, minification, génération `.avif` ; « hello world » ~50 ko, apps
  démo < 50 ko possibles.

### Verdict Dioxus
Le plus aligné « une seule archi multi-cible » + **wasm-split** + embarquement **wgpu** + coexistence Tauri.
Modèle RSX proche de React = friction faible. C'est le choix par défaut recommandé pour Forge.

---

## 3. Leptos — architecture réelle (lecture du code)

### Modèle
- **Réactivité fine SANS virtual DOM** : un signal qui change met à jour **un seul nœud texte / une classe /
  un attribut**, sans réexécuter le composant. C'est le modèle SolidJS porté en Rust.
- Système réactif = **une structure runtime unique** ; créer un signal donne un identifiant `Copy + 'static`
  (index dans une slotmap arena). Même idée que generational-box. La durée de vie des données est liée au
  **scope réactif** (≈ GC lié à l'UI).
- Macro `view!` : optimise SSR en **chaînes statiques compilées** → Leptos se dit ~3-4× plus rapide que les
  autres frameworks Rust sur le rendu HTML.
- Framework en **couches indépendantes** (`reactive_graph`, `leptos_dom`, `tachys`, `leptos_router`,
  `leptos_meta`, `server_fn`) : chaque couche est remplaçable. Très modulaire.

### Fonctions serveur & fullstack
- `server_fn` (framework-agnostique) : transforme une fonction « serveur uniquement » en endpoint REST ad hoc
  + code client d'appel. Dégradation gracieuse via `<form>`, POST/GET URL-encodés. WebSockets via server fns
  (0.8). Erreurs typées via `FromServerFnError` (0.8).
- **Islands** + **islands-router** (0.8) : rendu majoritairement statique, hydratation seulement des « îlots »
  interactifs → JS/WASM minimal expédié. Pertinent pour des pages surtout-contenu (pas le cas du shell Forge,
  très interactif).

### Verdict Leptos
Gagne sur la **perf brute web** et l'empreinte (pas de VDOM, hydratation islands). Moins « multi-cible » que
Dioxus (web-first). **Plan B Forge** : à réserver à une section chaude au profil temps réel (ex. trading) si le
profilage montre que le VDOM de Dioxus coûte. Mixer les deux dans la même app n'est pas souhaitable → choix
global Dioxus, Leptos seulement si un mur perf précis l'exige.

---

## 4. Tableau de décision (collé à Forge)

| Critère Forge | Dioxus | Leptos |
|---|---|---|
| Une seule archi, déjà Rust/Tauri | ✅ coexiste avec Tauri, embarque wgpu | ✅ mais web-first |
| UI lourde (3D, splat, sections) | ✅ **wasm-split** natif | ⚠️ pas d'équivalent aussi intégré |
| Sections WebGPU/WGSL existantes | ✅ surface wgpu dans canvas, WGSL réutilisé | ✅ idem (canvas) |
| État imbriqué par section | ✅ **Stores** par champ | ✅ signaux fins |
| Perf rendu HTML pur | ✅ template diff | ✅✅ pas de VDOM (plus rapide) |
| Vélocité dev | ✅ Subsecond (web limité) | ✅ hot-reload `view!` |
| Friction équipe ex-React | ✅ RSX proche React | ⚠️ modèle Solid à apprendre |
| Maturité (juin 2026) | 0.7 stable, 0.8-alpha | 0.8 stable, 0.9-alpha |

**Décision : Dioxus comme socle. Leptos = plan B ciblé.** À reverrouiller par le POC mesuré (MIGRATION_FRONT.md, Étape 1).

---

## 5. Implications concrètes pour la migration Forge

1. **wasm-split = pilier de l'Étape 8** (banger/alpha3d) et des grosses sections : chaque section devient un
   chunk chargé à la demande. Mesurer la taille de `main.wasm` dès l'Étape 1.
2. **GPU** : garder les shaders **WGSL** d'`ingen-render` ; les exposer via une surface **`wgpu`** montée dans
   un canvas Dioxus. Ne pas confondre avec le native renderer Vello (qui sert le CSS, pas tes scènes).
3. **État** : porter l'état des sections en **Stores** (réactivité par champ) plutôt qu'un gros signal global
   → moins de re-renders, plus proche du budget perf Banger.
4. **Coexistence Tauri** : on garde la coquille Tauri (fenêtre, sécurité, commandes). Dioxus rend le *contenu*
   de la webview en WASM. Pont commandes Tauri typé via crate partagé (MIGRATION_FRONT.md, Étape 2).
5. **Hot-patch** : utile en dev desktop, **limité sur web** ; ne pas en faire une dépendance du workflow.
   Attention : un changement de `struct` invalide le patch → rebuild. Ne bloque rien, mais à savoir.
6. **Toolchain** : pas de trunk (Tauri sert le bundle) ; `wasm-bindgen` + `dx`/CLI pour build/optim
   (compression wasm, split). Cibler WASM 3.0 / reference-types pour réduire la glue JS.

---

## 6. Ce qu'il reste à vérifier empiriquement (le POC tranche)

- Taille `main.wasm` (gzip) du shell minimal + 1 section.
- Latence premier rendu dans la webview Tauri (vs UI TS actuelle).
- FPS d'une scène `wgpu`/WGSL embarquée dans Dioxus sur RTX 3050 6 Go (budget perf Banger).
- Ergonomie réelle du pont commandes Tauri typé (Dioxus `use_server_future` vs invoke direct).
- Confort Subsecond en dev desktop sur ce repo (tip crate vs workspace).

---

## 7. Décision d'archi CACHÉE : garder Tauri, ou le supprimer ?

Trouvaille majeure de l'approfondissement : **Dioxus desktop utilise `wry` + `tao` — exactement la même
techno de webview que Tauri.** Forge a donc deux chemins, et c'est un vrai choix doctrinal (circuit court) :

| | **Chemin A — garder Tauri** | **Chemin B — supprimer Tauri** |
|---|---|---|
| Archi | Coquille Tauri + Dioxus rendu WASM dans la webview | Dioxus desktop seul (wry/tao en direct) |
| Frontière | Commandes Tauri (IPC) conservées, typées | **Plus d'IPC** : accès natif Rust direct |
| Middlemen | Tauri reste un intermédiaire | **Un acteur entier supprimé** |
| Risque | Faible, incrémental, rollback facile | Élevé : on perd plugins/écosystème Tauri |
| Doctrine Forge | OK | **Plus aligné** (« supprimer les middlemen », circuit le plus court) |

**Recommandation nuancée** : migrer en **Chemin A d'abord** (sécurité, incrémental — c'est ce que décrit
`MIGRATION_FRONT.md`), puis, une fois toutes les sections en Dioxus et l'IPC réduit à presque rien, **évaluer
le Chemin B** comme une suppression finale de Tauri (Étape 9 bis). On ne tranche pas B à l'aveugle : on y arrive
quand l'IPC ne sert presque plus à rien. C'est exactement « supprimer les nœuds obsolètes avant d'en ajouter ».

> Dioxus 0.8 (en cours, sans date) vise justement des **APIs natives de première classe** (caméra, géoloc,
> stockage, OAuth) + un sous-ensemble « 1.0 » stable pour les auteurs de libs. Ça renforce le Chemin B à terme.

## 8. Benchmarks (où ça se situe réellement)

- **Leptos** : réactivité fine façon SolidJS → updates chirurgicales, **proche du vanilla JS** (no-framework)
  sur `js-framework-benchmark`. Le plus rapide des Rust sur le DOM pur.
- **Dioxus** : après l'optimisation **template-diffing**, **à parité avec SolidJS**, devant Sycamore et Yew.
  Son VirtualDOM « fiber-like » batch intelligemment ; léger surcoût vs Leptos, négligeable en pratique.
- **Les deux battent React** largement, tout en restant proches du JS natif. Référence officielle :
  `krausest.github.io/js-framework-benchmark`.

**Lecture Forge** : la perf de rendu DOM n'est PAS ton mur (tes murs sont le GPU/WGSL et le poids du bundle).
Donc le choix Dioxus vs Leptos se joue sur archi/ergonomie/splitting, pas sur 5 % de benchmark DOM.

## 9. Les autres prétendants (vérifié, pour ne rien rater)

- **Sycamore** — réactivité fine façon SolidJS, axé perf. Sérieux mais écosystème/fullstack plus mince que
  Leptos. Pas d'avantage décisif pour Forge.
- **Yew** — le plus ancien, modèle React, stable, mais **VirtualDOM plus lourd** (diffing complet). En retrait
  perf. Non retenu.
- **Floem / Xilem** — pistes GPU-natives (hors navigateur), encore immatures, hors cible « webview Tauri ».
  À surveiller, pas à adopter.
- **Conclusion** : le duo **Dioxus / Leptos** reste le SOTA pertinent pour Forge. Rien de plus mûr à l'horizon.

## 10. WASM multithreading (levier futur pour tes sections lourdes)

- Le multithreading WASM est en **stage 3** et expédié par tous les runtimes JS majeurs.
- Côté Rust : `wasm-bindgen` le supporte mais exige **nightly + bons flags** (threads via `SharedArrayBuffer`,
  en-têtes COOP/COEP). Pas trivial.
- **Lecture Forge** : pertinent à terme pour pousser du calcul (banger, KASM côté front) sur plusieurs threads
  WASM au lieu de bloquer le thread UI. À garder en réserve, pas une dépendance de la migration initiale.

## 11. Faits de version (instantané juin 2026)

- **Dioxus 0.7** : version stable finale (cycle clos **23 jan 2026**) — apporte Dioxus **Native** (moteur HTML/CSS
  **Blitz**, collab Firefox/Google/Servo/Bevy + **Vello** GPU), **hot-patch Rust** (Subsecond), **wasm-split**,
  bibliothèque de composants (façon shadcn/Radix), détection Tailwind auto. Master = **0.8.0-alpha**.
- **Dioxus 0.8** (en cours, sans date) : APIs natives (caméra/géoloc/stockage/OAuth), cross-platform, « 1.0 »
  d'un sous-ensemble pour libs.
- **Leptos** : **0.8.19** stable (islands-router, server fns WebSockets, erreurs typées), **0.9.0-alpha**
  (19 mai 2026) surtout nettoyage/semver-breaking, pas de grosse nouveauté.
- **WebAssembly 3.0** standard (GC, Memory64, exceptions) ; **reference-types** réduit la glue JS ; threads WASM
  stage 3.

---

> Ce doc est un instantané. Quand la migration avance, fusionner les conclusions dans `ROADMAP.md` et
> compresser/supprimer ce fichier (un doc n'est pas une archive).
