# MIGRATION_FRONT.md — Front Forge : TypeScript → Rust (Dioxus/Leptos)

> Statut : proposition / plan. Live objectives → `ROADMAP.md` une fois lancé.
> Doctrine : circuit plus court, un seul langage vérifié, interface étroite + vérif + rollback à chaque étape (Via Negativa).

---

## 1. Objectif

Aujourd'hui Forge vit en **deux mondes** :

```
Backend Rust (Tauri, brain, kasm, godel, collection_os)
        ↕  frontière sérialisée (commandes Tauri, JSON, tauri-bridge.ts)
Front TypeScript + HTML + CSS  (≈47 fichiers ui/src/**, shell + sections)
```

Cette frontière est la principale source de **complexité, duplication de types et bugs runtime** : tout
contrat traverse une couche non vérifiée par le compilateur, et chaque section réimplémente en TS une
logique qui existe déjà en Rust.

**But** : supprimer la frontière. Écrire l'interface **dans le même langage que le cœur (Rust)**, et
laisser un compilateur (Dioxus ou Leptos → WASM) générer le HTML/CSS/JS que le navigateur exige.

Gains visés, mesurables :

| Mur poussé | Cible concrète |
|---|---|
| **Une seule archi** | 0 fichier `.ts` applicatif ; tout en `.rs`. HTML/CSS générés, jamais écrits à la main. |
| **Puissance / rapidité** | Logique compilée WASM (proche natif) ; Leptos = réactivité fine, seul le nœud DOM modifié est touché. |
| **Moins de code** | Types partagés back↔front (plus de double déclaration). Suppression de `tauri-bridge.ts` comme couche de traduction. |
| **Moins d'erreurs** | Contrats vérifiés par le compilateur Rust : un mauvais champ = build cassé, pas un bug chez l'utilisateur. |
| **Design / animation** | CSS conservé en coulisses (scoped par composant) ; WGSL/WebGPU réutilisés via `wgpu`. |

**Non-objectif** : réécrire le 3D from scratch. WebGPU/WGSL (banger/ingen-render) se porte par `wgpu`,
shaders WGSL réutilisés tels quels.

---

## 2. Décision : Dioxus ou Leptos ?

- **Dioxus** — multi-cible (web/desktop/mobile/TUI), modèle proche de React (RSX), virtual DOM. S'intègre
  comme contenu de la webview Tauri **ou** remplace la webview par son propre renderer. Friction la plus faible.
- **Leptos** — réactivité fine (pas de virtual DOM), perf brute max, SSR. Idéal web-first.

**Recommandation Forge : Dioxus.** Raisons : on reste sur la **coquille Tauri existante** (backend, fenêtre,
sécurité, commandes), on remplace seulement le *contenu* de la webview ; le modèle RSX est lisible ; le
multi-cible garde la porte ouverte (desktop natif sans webview plus tard). Leptos reste plan B si un profilage
montre que la réactivité fine est nécessaire sur une section chaude (ex. trading temps réel).

> Décision verrouillée à l'**Étape 1** par un POC mesuré, pas par préférence.

---

## 3. Principe de migration : par section, derrière un gate, jamais big-bang

On NE réécrit PAS les 47 fichiers d'un coup. On migre **section par section**, chaque section vit derrière
un drapeau `forge_front=rust|ts`, l'ancienne reste rollback immédiat tant que la nouvelle n'a pas passé la vérif.
Le `tauri-bridge.ts` reste l'unique frontière pendant la transition (cohabitation TS legacy + Rust neuf).

Ordre de migration choisi (du moins risqué au plus risqué) :

1. shell statique (sidebar, window-controls, fenêtres, routing) — peu d'état, beaucoup de DOM.
2. sections « formulaire/données » : real-estate, webexplorer.
3. trading (état temps réel, candidat Leptos local si besoin).
4. banger / alpha3d (WebGPU/WGSL via `wgpu`) — gardé pour la fin, le plus de surface GPU.

---

## 4. Les 10 étapes

Chaque étape = **But / Actions / Vérification / Rollback**. On ne passe à la suivante que si la vérif passe.

### Étape 1 — POC mesuré + choix gravé
- **But** : trancher Dioxus vs Leptos sur preuve, pas sur goût ; valider que Rust→WASM tourne dans la webview Tauri.
- **Actions** : créer `examples/forge_tauri_ui/front-rs/` (crate WASM isolée). Une page : un bouton, une
  animation CSS, une donnée lue via une commande Tauri existante. Build en Dioxus, puis en Leptos.
- **Vérif** : taille du `.wasm` (gzip), temps de premier affichage, fluidité de l'animation sur RTX 3050 6 Go.
  Tableau comparatif commité dans ce doc.
- **Rollback** : supprimer `front-rs/`, rien d'autre touché. Coût nul.

### Étape 2 — Pont typé Rust↔Rust (tuer la traduction JSON manuelle)
- **But** : remplacer l'appel non typé `invoke("cmd", {...})` de `tauri-bridge.ts` par un module Rust où
  les types des commandes sont **partagés** avec le backend.
- **Actions** : extraire les structs de commande Tauri dans un crate commun `forge-ipc` (req/resp). Le front
  Rust importe ce crate. Génération d'un client typé par-dessus `tauri-sys` (ou `wasm-bindgen` + `invoke`).
- **Vérif** : `cargo check` sur `forge-ipc` + front + backend ; un champ renommé casse les trois → preuve que
  la frontière est désormais vérifiée par le compilateur.
- **Rollback** : le pont Rust est additif ; le `tauri-bridge.ts` legacy reste en place et fonctionnel.

### Étape 3 — Système de design en Rust (tokens, pas de CSS à la main)
- **But** : ne plus jamais écrire de CSS brut applicatif. Centraliser couleurs/espacements/typo/animations.
- **Actions** : porter `ui/styles.css` en **design tokens** (constantes Rust) + styles scopés par composant
  (Dioxus `asset!`/CSS scoping ou `stylance`). Animations = classes générées depuis les tokens.
- **Vérif** : capture d'écran avant/après pixel-diff sur le shell ; aucune régression visuelle > seuil.
- **Rollback** : `styles.css` conservé tant que toutes les sections ne sont pas migrées.

### Étape 4 — Migrer le SHELL (coquille, routing, sidebar, fenêtres)
- **But** : porter `shell/{boot,sidebar,window-controls,click-router,shell-machine,section-registry}.ts`.
- **Actions** : réimplémenter le routing de sections en Rust (enum de sections + état). Le shell Rust monte les
  sections : celles déjà portées en Rust, sinon un conteneur qui héberge la section TS legacy (iframe/slot).
- **Vérif** : `forge-ui-smoke.mjs`, `forge-ui-section-audit.mjs`, navigation manuelle entre toutes les sections.
- **Rollback** : drapeau `forge_front=ts` → recharge le shell TS d'origine.

### Étape 5 — Première section data : real-estate
- **But** : valider le pattern de section complète (état + panneaux + outils + onboarding) en Rust.
- **Actions** : porter `sections/real-estate/{runtime-context,onboarding,language,mode,panel}-runtime.ts` en
  composants Dioxus + signaux. Réutiliser le pont typé (Étape 2) pour les outils.
- **Vérif** : parcours onboarding complet ; comparaison comportement vs version TS ; `section-audit`.
- **Rollback** : section servie en TS via le gate ; le shell Rust héberge la version legacy.

### Étape 6 — webexplorer + contrats d'ownership
- **But** : porter webexplorer en respectant `SECTION_OWNERSHIP.json` et le contrat de bridge natif.
- **Actions** : porter `sections/webexplorer/config.ts` + actions natives ; faire passer les actions par le
  pont typé Rust au lieu de `tauri-bridge.ts`.
- **Vérif** : `forge-tauri-bus-audit.mjs --strict`, `forge-surface-manifest.mjs --check`.
- **Rollback** : gate section ; ownership inchangé.

### Étape 7 — trading (état temps réel)
- **But** : valider une section à forte fréquence de mise à jour ; décider Dioxus vs îlot Leptos.
- **Actions** : porter `sections/trading/{state,catalog,controller,surface}.ts`. Si le profilage montre du jank,
  isoler le flux temps réel dans un composant à réactivité fine (Leptos local ou signaux Dioxus optimisés).
- **Vérif** : FPS/latence de mise à jour mesurés vs version TS ; budget perf respecté.
- **Rollback** : gate section.

### Étape 8 — banger + alpha3d : porter le GPU vers `wgpu`
- **But** : déplacer le rendu 3D en Rust. **WGSL réutilisé tel quel**, WebGPU → `wgpu` (même API).
- **Actions** : porter `sections/banger/ingen-render.ts` (déjà WebGPU/WGSL) sur `wgpu` côté Rust/WASM ;
  migrer le reliquat `alpha3d-webgl.ts` (WebGL2) directement en `wgpu` au passage (suppression d'une dette).
  Respecter le budget perf Banger (cache GI hashé, résolution interne cappée — cf. mémoire projet).
- **Vérif** : parité visuelle des scènes ; FPS sur RTX 3050 6 Go ≥ version actuelle ; pas de fuite VRAM.
- **Rollback** : gate section ; le renderer TS WebGPU reste dispo.

### Étape 9 — Suppression du legacy (Via Negativa)
- **But** : une fois toutes les sections vertes, **supprimer** le monde TS, pas le documenter.
- **Actions** : retirer `ui/src/**/*.ts` applicatifs, `tauri-bridge.ts`, le build TS, `vendor-three`,
  le lock JS manuel devenu sans objet, et les scripts d'audit spécifiques au pipeline TS. Mettre à jour
  `AGENTS.md` (section UI Discipline) et `SECTION_CONTRACT.md`.
- **Vérif** : build complet Rust/WASM ; aucune référence morte (`rg` sur les chemins supprimés) ; app lancée
  end-to-end, toutes sections.
- **Rollback** : tag git `pre-front-rust-removal` avant suppression ; `git revert` possible.

### Étape 10 — Verrouillage + doctrine à jour
- **But** : graver la nouvelle archi pour qu'aucun futur agent ne réintroduise du TS.
- **Actions** : nouveau check CI `cargo check` front+back+ipc en une passe ; règle dans `AGENTS.md` :
  « front = Rust/Dioxus uniquement ». Mettre à jour `FORGE_RUNTIME_ARCHITECTURE.md`. Compresser/supprimer ce
  doc une fois la migration terminée (un doc n'est pas une archive).
- **Vérif** : CI verte sur une PR test qui ajoute un `.ts` → doit échouer.
- **Rollback** : N/A (étape de consolidation).

---

## 5. Risques honnêtes

- **Écosystème** : moins de composants prêts-à-l'emploi qu'en npm. Mitigation : peu de dépendances UI externes
  côté Forge aujourd'hui, et la doctrine interdit déjà les middlemen inutiles.
- **Poids WASM** : surveiller la taille du `.wasm` (Étape 1 = garde-fou). `wasm-opt`, lazy-load par section.
- **Outillage debug** : DevTools moins riches sur WASM. Mitigation : logs typés via le pont Rust, tests
  déterministes côté logique (déjà en Rust).
- **Temps** : migration réelle, pas un week-end. Le découpage par gate garantit qu'à **chaque étape l'app
  reste livrable**, jamais cassée globalement.

---

## 6. Définition de « fini »

- 0 `.ts` applicatif sous `ui/src/`.
- Types back↔front partagés, vérifiés au compilateur.
- Parité visuelle + perf ≥ actuel sur toutes les sections (RTX 3050 6 Go).
- `tauri-bridge.ts` et le pipeline TS supprimés.
- `AGENTS.md` / `FORGE_RUNTIME_ARCHITECTURE.md` à jour ; ce doc compressé ou supprimé.
