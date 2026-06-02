# MIGRATION_FRONT.md — Front Forge : TypeScript → Rust (Dioxus/WASM)

> Document unique : contexte SOTA + objectifs + plan. Instantané daté **2026-06-02**.
> Doctrine : circuit plus court, un seul langage vérifié, interface étroite + vérif + rollback à chaque étape.
> Live objectives → `ROADMAP.md` une fois la migration lancée. Sources code lues en local :
> `DioxusLabs/dioxus` (0.7 stable, master 0.8-alpha), `leptos-rs/leptos` (0.8.19 stable, 0.9-alpha).

---

## 1. Objectif de migration

Aujourd'hui Forge vit en **deux mondes** :

```
Backend Rust (Tauri, brain, kasm, godel, collection_os)
        ↕  frontière sérialisée (commandes Tauri, JSON, tauri-bridge.ts)
Front TypeScript + HTML + CSS  (≈47 fichiers ui/src/**, shell + sections)
```

Cette frontière est la principale source de **complexité, duplication de types et bugs runtime**.

**But** : supprimer la frontière. Écrire l'interface **dans le même langage que le cœur (Rust)** ; un compilateur
(Dioxus → WASM) génère le HTML/CSS/JS que le navigateur exige. On n'écrit plus HTML/CSS à la main ; ils
deviennent une sortie de build, jamais une source.

Objectifs mesurables (la définition de « fini ») :

| Mur poussé | Cible concrète et vérifiable |
|---|---|
| **Une seule archi** | 0 fichier `.ts` applicatif sous `ui/src/` ; tout en `.rs`. |
| **Puissance / rapidité** | Logique compilée WASM ; réactivité fine (seul le nœud modifié change). |
| **Moins de code** | Types partagés back↔front (plus de double déclaration) ; `tauri-bridge.ts` supprimé. |
| **Moins d'erreurs** | Contrats vérifiés par le compilateur : mauvais champ = build cassé, pas un bug en prod. |
| **Design / animation** | CSS scopé par composant généré ; WGSL/WebGPU réutilisés via `wgpu`. |
| **Bundle maîtrisé** | `main.wasm` initial petit ; sections lourdes chargées à la demande (wasm-split). |

**Non-objectif** : réécrire le 3D from scratch. `banger/ingen-render` est déjà **WebGPU + WGSL** → se porte par
`wgpu`, shaders réutilisés tels quels. `alpha3d-webgl.ts` (WebGL2, seul reliquat) migre vers `wgpu` au passage.

---

## 2. Contexte SOTA (juin 2026) — l'essentiel pour décider

**Socle WASM.** WebAssembly 3.0 (fin 2025) est standard : GC, Memory64, exceptions. Pour Rust, le gain clé est
**reference-types** (moins de glue JS). Threads WASM en stage 3 (nightly + flags COOP/COEP) — levier futur pour
pousser du calcul hors du thread UI. La cible est mûre : le vrai mur est **taille du binaire + latence de
démarrage + qualité du pont WASM↔GPU**, pas « est-ce que ça tourne ».

**Dioxus** (0.7 stable, clos 23/01/2026 ; master 0.8-alpha) — choix retenu.
- VirtualDOM **template-based** : `rsx!` génère des templates statiques à la compile, seul le dynamique est diffé.
- État : **`generational-box`** donne des signaux **`Copy + 'static`** (pas de clone/lifetime). `Signal`, `Memo`,
  et surtout **`Store`** = réactivité **par champ** (écrire `store[1].name` ne réveille que ce chemin).
- **`wasm-split`** (`#[wasm_split]`) : vrai code-splitting WASM → charge banger/splat-loader à l'ouverture seulement.
  C'est l'argument n°1 pour une UI lourde. Contrainte : points de split = fonctions `async`, graphe statique.
- **Subsecond hot-patch** : recompile que les fonctions changées. Top en desktop, **limité sur web**, et un
  **changement de struct casse le patch** (rebuild).
- GPU : pour tes scènes WGSL custom, on embarque une **surface `wgpu`** dans un `<canvas>` (Dioxus dit « Embed in
  Bevy/WGPU »). À ne pas confondre avec leur renderer natif Vello (qui ne sert qu'à dessiner le CSS).
- Fullstack Axum (`#[server]`), bundle web optimisé (compression wasm, ~50 ko hello world).

**Leptos** (0.8.19 stable, 0.9-alpha) — plan B ciblé.
- **Réactivité fine SANS virtual DOM** (modèle SolidJS) : la plus rapide des Rust sur le DOM pur (≈ vanilla JS).
- Islands + islands-router (JS minimal expédié), server fns. Plus **web-first**, moins multi-cible.
- À réserver à une section temps réel (ex. trading) si le profilage montre que le VDOM de Dioxus coûte.

**Benchmarks** : Dioxus ≈ SolidJS (après optim template-diffing), devant Sycamore/Yew ; Leptos ≈ vanilla JS ;
les deux écrasent React. **Mais la perf DOM n'est PAS ton mur** (tes murs = GPU/WGSL + poids bundle), donc le
choix se joue sur archi/splitting, pas sur 5 % de benchmark.

**Autres prétendants** (vérifiés) : Sycamore (sérieux, fullstack plus mince), Yew (VDOM trop lourd),
Floem/Xilem (GPU-natif, immatures). **Rien de plus mûr que Dioxus/Leptos** pour Forge.

---

## 3. Décision

- **Socle : Dioxus.** Raisons concrètes : coexiste avec Tauri, **wasm-split** pour l'UI lourde, embarque `wgpu`
  (WGSL réutilisés), `Store` par champ pour le budget perf Banger, RSX proche React = friction minimale.
- **Leptos : plan B** sur une section chaude précise, si un profilage le justifie. Pas de mix global.
- **Décision reverrouillée par le POC mesuré (Étape 1)**, pas par préférence.

### Décision d'archi à trancher PLUS TARD : garder Tauri ou le supprimer ?

Dioxus desktop tourne sur **wry/tao = la même techno que Tauri**. Deux chemins :

| | Chemin A — garder Tauri | Chemin B — supprimer Tauri |
|---|---|---|
| Archi | Coquille Tauri + Dioxus WASM dans la webview | Dioxus desktop seul (wry/tao direct) |
| Frontière | Commandes Tauri (IPC) typées | **Plus d'IPC**, accès natif Rust direct |
| Doctrine | OK | **Plus aligné** (supprime un acteur entier) |
| Risque | Faible, incrémental | Élevé (perte écosystème/plugins Tauri) |

**On migre en Chemin A d'abord** (incrémental, rollback facile). Une fois toutes les sections en Dioxus et l'IPC
réduit à presque rien, **évaluer le Chemin B** (Étape 9 bis) comme suppression finale de Tauri. Dioxus 0.8 (APIs
natives caméra/géoloc/stockage/OAuth) renforce ce chemin à terme.

---

## 4. Principe de migration : par section, derrière un gate, jamais big-bang

On NE réécrit PAS les 47 fichiers d'un coup. Migration **section par section**, chaque section derrière un
drapeau `forge_front=rust|ts` ; l'ancienne reste rollback immédiat tant que la nouvelle n'a pas passé la vérif.
`tauri-bridge.ts` reste l'unique frontière pendant la cohabitation. Ordre choisi (du moins au plus risqué) :

1. shell statique (sidebar, window-controls, routing) — peu d'état, beaucoup de DOM.
2. sections data : real-estate, webexplorer.
3. trading (état temps réel, candidat Leptos local si besoin).
4. banger / alpha3d (WebGPU/WGSL via `wgpu`) — gardé pour la fin.

---

## 5. Les 10 étapes (objectifs définis : But / Actions / Vérif / Rollback)

On ne passe à la suivante que si la vérif passe. À chaque Φ, l'app reste livrable.

### Étape 1 — POC mesuré + choix gravé
- **But** : trancher sur preuve ; valider Rust→WASM dans la webview Tauri.
- **Actions** : crate WASM isolée `examples/forge_tauri_ui/front-rs/`. Une page : bouton + animation CSS + une
  donnée lue via une commande Tauri existante. Build Dioxus (et un essai Leptos pour comparer).
- **Vérif** : taille `main.wasm` (gzip), temps de premier rendu, fluidité animation sur RTX 3050 6 Go. Tableau commité.
- **Rollback** : supprimer `front-rs/`, rien d'autre touché.

### Étape 2 — Pont typé Rust↔Rust (tuer la traduction JSON)
- **But** : remplacer l'`invoke` non typé de `tauri-bridge.ts` par un client Rust aux types **partagés** avec le back.
- **Actions** : extraire les structs de commande Tauri dans un crate commun `forge-ipc` (req/resp) ; le front l'importe.
- **Vérif** : `cargo check` sur `forge-ipc` + front + back ; renommer un champ casse les trois → frontière vérifiée.
- **Rollback** : pont additif ; `tauri-bridge.ts` legacy reste en place.

### Étape 3 — Design system en Rust (plus de CSS à la main)
- **But** : centraliser couleurs/espacements/typo/animations en **design tokens** Rust + styles scopés par composant.
- **Actions** : porter `ui/styles.css` en tokens (constantes) + CSS scoping Dioxus (`asset!`/`stylance`).
- **Vérif** : pixel-diff avant/après sur le shell ; pas de régression > seuil.
- **Rollback** : `styles.css` conservé tant que tout n'est pas migré.

### Étape 4 — Migrer le SHELL (coquille, routing, sidebar, fenêtres)
- **But** : porter `shell/{boot,sidebar,window-controls,click-router,shell-machine,section-registry}.ts`.
- **Actions** : routing en Rust (enum de sections + état) ; le shell monte les sections Rust, sinon héberge la section TS legacy.
- **Vérif** : `forge-ui-smoke.mjs`, `forge-ui-section-audit.mjs`, navigation manuelle complète.
- **Rollback** : `forge_front=ts` recharge le shell TS.

### Étape 5 — Section data : real-estate
- **But** : valider le pattern de section complète (état + panneaux + outils + onboarding).
- **Actions** : porter `real-estate/{runtime-context,onboarding,language,mode,panel}-runtime.ts` en composants
  Dioxus + `Store`. Outils via le pont typé (Étape 2).
- **Vérif** : parcours onboarding complet ; comportement vs version TS ; `section-audit`.
- **Rollback** : section servie en TS via le gate.

### Étape 6 — webexplorer + contrats d'ownership
- **But** : porter webexplorer en respectant `SECTION_OWNERSHIP.json` et le contrat de bridge natif.
- **Actions** : porter `webexplorer/config.ts` + actions natives via le pont typé.
- **Vérif** : `forge-tauri-bus-audit.mjs --strict`, `forge-surface-manifest.mjs --check`.
- **Rollback** : gate section.

### Étape 7 — trading (état temps réel)
- **But** : valider une section haute fréquence ; décider Dioxus vs îlot Leptos.
- **Actions** : porter `trading/{state,catalog,controller,surface}.ts`. Si jank, isoler le flux temps réel
  (réactivité fine : signaux Dioxus optimisés ou Leptos local).
- **Vérif** : FPS/latence de mise à jour vs TS ; budget perf respecté.
- **Rollback** : gate section.

### Étape 8 — banger + alpha3d : GPU vers `wgpu`
- **But** : déplacer le rendu 3D en Rust, **WGSL réutilisé tel quel**, WebGPU → `wgpu`.
- **Actions** : porter `banger/ingen-render.ts` (déjà WebGPU/WGSL) sur `wgpu` ; migrer `alpha3d-webgl.ts`
  (WebGL2) vers `wgpu` au passage (suppression d'une dette). Respecter le budget perf Banger (cache GI hashé,
  résolution interne cappée). Chaque grosse section = un chunk `wasm-split`.
- **Vérif** : parité visuelle ; FPS sur RTX 3050 6 Go ≥ actuel ; pas de fuite VRAM.
- **Rollback** : gate section.

### Étape 9 — Suppression du legacy TS (Via Negativa)
- **But** : une fois toutes les sections vertes, **supprimer** le monde TS, pas le documenter.
- **Actions** : retirer `ui/src/**/*.ts` applicatifs, `tauri-bridge.ts`, le build TS, `vendor-three`, le lock JS
  manuel devenu sans objet, les scripts d'audit TS. Mettre à jour `AGENTS.md` (UI Discipline) et `SECTION_CONTRACT.md`.
- **Vérif** : build complet Rust/WASM ; aucune référence morte (`rg`) ; app end-to-end, toutes sections.
- **Rollback** : tag git `pre-front-rust-removal` avant suppression ; `git revert` possible.

### Étape 9 bis — (optionnel) Évaluer la suppression de Tauri (Chemin B)
- **But** : si l'IPC ne sert presque plus, supprimer Tauri et passer en Dioxus desktop natif.
- **Actions** : remplacer la coquille Tauri par `dioxus-desktop` (wry/tao) ; accès natif Rust direct sans IPC.
- **Vérif** : parité fonctionnelle (fenêtre, raccourcis, accès fichiers/natifs) ; pas de régression sécurité.
- **Rollback** : conserver la coquille Tauri tant que la parité n'est pas prouvée.

### Étape 10 — Verrouillage + doctrine à jour
- **But** : graver la nouvelle archi pour qu'aucun futur agent ne réintroduise du TS.
- **Actions** : check CI `cargo check` front+back+ipc en une passe ; règle `AGENTS.md` « front = Rust/Dioxus ».
  Mettre à jour `FORGE_RUNTIME_ARCHITECTURE.md`. Compresser/supprimer ce doc (un doc n'est pas une archive).
- **Vérif** : CI rouge sur une PR test qui ajoute un `.ts`.
- **Rollback** : N/A.

---

## 6. Risques honnêtes

- **Écosystème** : moins de composants prêts qu'en npm. Mitigation : peu de deps UI externes côté Forge ; la
  doctrine interdit déjà les middlemen.
- **Poids WASM** : surveiller `main.wasm` (Étape 1 = garde-fou) ; `wasm-opt`, lazy-load par section.
- **Debug** : DevTools moins riches sur WASM. Mitigation : logs typés via le pont, tests déterministes en Rust.
- **Hot-patch web limité** ; un changement de struct casse le patch. Ne pas en dépendre.
- **Temps** : migration réelle. Le découpage par gate garantit qu'à chaque étape l'app reste livrable.

---

## 7. À trancher empiriquement (le POC, Étape 1)

- Taille `main.wasm` (gzip) du shell minimal + 1 section.
- Latence premier rendu dans la webview Tauri vs UI TS actuelle.
- FPS d'une scène `wgpu`/WGSL embarquée dans Dioxus sur RTX 3050 6 Go.
- Ergonomie du pont commandes Tauri typé (`use_server_future`/invoke).
- Confort Subsecond en dev desktop sur ce repo (tip crate vs workspace).

> Ce doc est un instantané. Quand la migration avance, fusionner les conclusions dans `ROADMAP.md` et
> compresser/supprimer ce fichier.
