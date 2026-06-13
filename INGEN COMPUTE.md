# InGen Compute - Pipeline Cible

InGen doit viser un moteur natif capable de rivaliser avec les moteurs AAA,
mais avec une difference centrale: Forge rend les calculs content-addressed
pour eviter de recalculer des fragments identiques a toutes les echelles.

## Hypothese

Unreal/Glacier gagnent par un moteur natif C++ + GPU bas niveau.

InGen doit gagner par:

```text
Rust moteur natif
+ Forge langage de calcul/verif
+ Monster execution massive
+ cache multi-echelle par hash
+ shaders Slang
+ RHI Vulkan / DirectX 12 / Metal
+ Banger comme editor/viewport
```

Forge ne remplace pas le GPU. Forge prepare, deduplique, prouve, bake et
compile les calculs que le GPU consomme ensuite a tres grande vitesse.

## Ce Qu On Abandonne

- TypeScript/WebGPU comme coeur du moteur 3D.
- Banger comme renderer principal porte par le front.
- L idee d un pipeline SDF pur qui refuserait meshes, voxels, surfels ou splats.
- Les calculs visuels refaits betement a chaque frame sans hash ni reuse.
- Les gros catalogues ActCode injectes dans le contexte LLM.
- Les outputs bruts lourds envoyes au LLM.

TypeScript peut rester pour l UI. WebGPU peut rester pour prototypes ou
previews. Le moteur profond doit etre natif.

## Contrat Canonique Des Deux Moteurs

InGen a deux moteurs, mais un seul bloc moteur profond:

```text
Rust Native Engine + Monster/Forge = Tandem Engine Block
```

Monster a un mode solo utile et deja existant:

```text
LLM -> /newcompute_ -> module Forge (src/kasm.rs)
-> MonsterPreparedCompute (src/monster.rs)
-> execution / reuse / proof
```

Ce mode solo sert a realiser des computes verifiables, content-addressed et
reutilisables, sans obliger le rendu ou les webviews a etre presents.

Le LLM garde le raisonnement: choix du domaine, classe math, objectif et
remplissage du contrat. Monster garde seulement l'execution et les refus
mecaniques. Les templates `/newcompute_` imposent donc un slot obligatoire
`workload_scale`; Monster verifie les nombres declares (`max_steps`,
`max_memory_mb`, `min_estimated_ops`, samples/sweeps/lanes ou tailles de
formes) avant de generer Forge. Si le contrat est trop petit, il retourne
`workload_too_small`. Il ne decide pas que le LLM devrait repondre directement;
il refuse simplement un faux compute.

Quand Monster travaille en tandem avec le moteur Rust natif, le bloc tandem a
deux sorties principales et aucune troisieme architecture parallele:

```text
Tandem Engine Block
|-- sortie A: native_tandem_render
|   `-- rendu moteur coeur Unreal-like pour Banger
|
`-- sortie B: native_tandem_dom_ram
    `-- memoryreading, cartographie et balisage RAM/DOM des webviews de l appli
```

La sortie A prepare les pages, caches et preuves que le moteur Rust consomme
pour garder le viewport Banger fluide. La sortie B prepare les graphes DOM,
tables RAM, mutations, labels de haut niveau et preuves que le LLM consomme
sous forme de projections compactes, sans dump brut et sans bloquer la webview.

Etat reel de `/simulation_dynamics`: la premiere tranche promue n'est pas un
solveur multiphysique general. C'est un compute scientifique precis:
simulation electro-thermique 2D multi-step sur champs `tensor<f32,64x64>`,
avec `pde_stencil_step`, handoff `vector` pour rester dans Monster `MassMath`,
readback compact, typed result buffers et `proof_hash`. Le template thermique
doit declarer au minimum le champ de temperature, le champ/source thermique,
`dt`, nombre de pas, diffusivite, anisotropie thermique, `dx`, `dy`, seuil
separateur critique, convection, ambiance, profil de hotspot, courant,
resistance interne, coefficient temperature de resistance, masse, capacite
thermique, coefficient entropique, SOC, facteur thermique SOC, cinetique
Arrhenius, resistance de contact thermique, condition limite (`periodic`,
`adiabatic`, `dirichlet_ambient` ou `cooling_plate_edge`) et temperature de
plaque froide si presente. Le template peut aussi declarer une serie compacte
de points experimentaux temps/temperature maximale pour calibrer et valider le
cas precise.
Monster projette ensuite les diagnostics professionnels: champ final
`temperature_field_next` sous forme de buffer tensoriel hashable, temps simule,
temperature moyenne, maximum, hotspot, gradient thermique max, marge avant
runaway, temps estime avant seuil, ratio CFL, erreur de bilan energetique,
norme de residu, contributions Joule/entropique/Arrhenius et ranking de
sensibilite. La projection inclut aussi des series temporelles compactes
maximum/moyenne/marge runaway et une comparaison numerique
`cpu_f64_reference_vs_f32_quantized_candidate` avec erreurs L2, Linf et relative
max. La batterie de validation thermique ajoute `grid_dt_convergence`,
`energy_flux_breakdown`, `uncertainty_interval`, `analytic_reference_check`,
`fem_reference_check`, `experimental_reference_check`, `calibration_result`,
`monte_carlo_uncertainty`, `richardson_convergence`,
`material_parameter_provenance`, `gpu_execution_audit`,
`numerical_validity_score`, `engineering_decision_score`,
`validation_readiness_score` et `validation_battery_thermal`. Le benchmark
analytique verifie le stencil sur une solution sinusoidale fermee de l'equation
de chaleur pure. Le benchmark FEM independant compare le stencil principal a
une reference Q1 mass-lumped sur le meme cas thermique. Le check experimental
compare les points fournis dans le template a la serie simulee, la calibration
fait une recherche reduite source/convection, le Monte Carlo propage les
incertitudes declarees, et Richardson compare 32x32/64x64/128x128. Le verdict
doit rester honnete: avec reference analytique, FEM, convergence, incertitude
et donnees experimentales fournies par le contrat, Monster peut produire un
screening d'ingenierie decision-support; il ne pretend pas remplacer une
campagne laboratoire ou une certification finale.
Les unites physiques metier sont rendues dans `scientific_metrics`; le coeur
Forge garde seulement les unites que son validateur sait prouver aujourd'hui.

Le resultat rendu au LLM ne vient pas de `println!` de test. Monster expose une
projection runtime `MonsterNewComputeLlmResult`, relayee par le service natif:
statut d'execution, backend, lane, nombre de GPU, lanes executees, forme de
dispatch, bytes input/output/readback, diagnostics scalaires, buffers types par
hash, `output_hash`, `proof_hash`, `projection_hash`, limites et bloc
`compact_text`. Les donnees lourdes restent en buffers hashes.

## Architecture Cible

```text
InGen
  |-- Brain / State Kernel
  |-- Forge language ancien KASM
  |-- Monster compute
  |-- Native Rust Engine
  |-- Forge Compute Graph
  |-- Multi-scale Hash Cache
  |-- Render Graph
  |-- Slang Shader Compiler
  |-- InGen RHI
  |-- Vulkan backend
  |-- DirectX 12 backend
  |-- Metal backend
  `-- Banger UI / viewport / editor
```

Roles simples:

```text
Brain / State Kernel = memoire, scene, preuves, objets
Forge = langage des calculs hashables
Monster = usine de calcul massif
Rust Engine = moteur natif
Slang = shaders portables modernes
RHI = traducteur GPU
Vulkan/DX12/Metal = acces GPU natif
Banger = interface intelligente
```

## Pipeline Cible Resume En Arborescence

```text
InGen 3D Engine Cible
|-- 1. Brain / State Kernel
|   |-- memoire scene
|   |-- objets
|   |-- intentions utilisateur
|   |-- preuves / hashes
|   `-- contexte LLM
|
|-- 2. Scene Graph IA-First
|   |-- objets nommes
|   |-- transforms
|   |-- materiaux
|   |-- lumieres
|   |-- cameras
|   |-- relations
|   `-- representation choisie
|
|-- 3. Forge Compute Graph
|   |-- langage Forge ancien KASM
|   |-- calculs decoupes
|   |-- micro / mini / small / medium / large
|   |-- hash stable par fragment
|   |-- verification
|   `-- preuves
|
|-- 4. Monster Compute
|   |-- prepare un manifeste MonsterPreparedCompute
|   |-- route mass_math / native_tandem_render / native_tandem_dom_ram
|   |-- execute les calculs lourds
|   |-- calcule seulement les cache-miss
|   |-- stocke resultats hashes
|   |-- reuse calculs identiques
|   |-- travaille en arriere-plan
|   |-- ne bloque pas chaque frame
|   |-- produit artefacts native-ready
|   `-- dialogue en tandem avec Rust Engine
|
|   Etat reel juin 2026
|   |-- /newcompute_ ouvre le template universel Monster
|   |-- le LLM ecrit un module Forge
|   |-- Monster parse, verifie, hash et prepare MonsterPreparedCompute
|   |-- Monster extrait primitiveOps depuis l IR Forge
|   |-- Monster genere des kernels WGSL/RHI pour execution massive
|   |-- Monster route sur GPU via Rust RHI / wgpu
|   |-- Monster peut utiliser plusieurs adaptateurs GPU compatibles
|   |-- Monster produit readback compact, output_hash et proof_hash
|   |-- /simulation_dynamics execute deja une tranche PDE thermique 2D:
|   |   tensor<f32,64x64>, pde_stencil_step, multi-step electro-thermal,
|   |   champ final hashable, mean/max/hotspot/gradient/CFL/energie/residu,
|   |   Joule/entropique/Arrhenius/threshold/sensibilite,
|   |   reference experimentale, calibration, Monte Carlo, Richardson,
|   |   audit GPU, scores numerique et decisionnel,
|   |   typed buffers et proof hash sur MassMath GPU
|   `-- les artefacts render/DOM sont les deux sorties du tandem natif
|
|-- 5. Representation Hybride
|   |-- SDF
|   |   `-- objets LLM / maths / proceduraux
|   |-- Mesh
|   |   `-- personnages / rigs / assets classiques
|   |-- Voxels
|   |   `-- volumes / terrain / acceleration
|   |-- Surfels
|   |   `-- lumiere indirecte / radiance cache
|   `-- Gaussian Splats
|       `-- scans photorealistes / decors captures
|
|-- 6. Virtual / Fake Geometry
|   |-- Micro-style
|   |   `-- vraie geometry virtuelle dense
|   |-- Crimson-like
|   |   `-- fake geometry / culling / imposteurs
|   |-- meshlets
|   |-- SDF bricks
|   |-- voxel pages
|   |-- splat clusters
|   `-- cache Forge par fragment
|
|-- 7. Material Graph
|   |-- physical materials
|   |-- eau
|   |-- verre
|   |-- metal
|   |-- peau
|   |-- vegetation
|   `-- neural/material cache possible
|
|-- 8. Lighting / Radiance Cache
|   |-- Solaris-style
|   |-- GI dynamique
|   |-- surfels
|   |-- probes
|   |-- screen traces
|   |-- world traces
|   |-- ray tracing
|   `-- cache lumiere hashe
|
|-- 9. Native Rust Engine
|   |-- boucle frame
|   |-- ressources GPU
|   |-- streaming
|   |-- scheduling
|   |-- scene runtime
|   |-- asset runtime
|   |-- demande calculs a Monster
|   |-- consomme artefacts native-ready / GPU-ready
|   `-- garde le temps reel fluide
|
|-- 10. Render Graph
|   |-- visibility
|   |-- shadows
|   |-- geometry
|   |-- materials
|   |-- lighting
|   |-- reflections
|   |-- volumes
|   |-- post-process
|   `-- capture LLM interne
|
|-- 11. Shader System
|   |-- Slang
|   |-- shader source unique
|   |-- compilation Vulkan
|   |-- compilation DirectX 12
|   |-- compilation Metal
|   `-- neural shaders futur
|
|-- 12. InGen RHI
|   |-- equivalent RHI Unreal
|   |-- Vulkan backend
|   |-- DirectX 12 backend
|   |-- Metal backend
|   `-- GPU natif bas niveau
|
|-- 13. Path Tracing / Neural Rendering
|   |-- RTX Kit-like
|   |-- path tracing progressif
|   |-- ReSTIR
|   |-- denoising
|   |-- upscaling
|   |-- ray reconstruction
|   `-- neural radiance cache
|
|-- 14. Banger UI / Editor
|   |-- viewport
|   |-- scene collection
|   |-- chat LLM
|   |-- selection objet
|   |-- preview objet
|   |-- tools
|   `-- controle du moteur natif
|
`-- 15. CodeAct / LLM Tooling
    |-- /newcompute_
    |-- /selectcompute_
    |-- /newobject_
    |-- /compute_<name>_
    |-- futurs /scene_
    |-- futurs /material_
    |-- futurs /light_
    |-- futurs /geometry_
    |-- futurs /render_
    |-- futurs /simulate_
    `-- router compact, pas 500 tools visibles
```

Phrase cle:

```text
Unreal = mesh-first + C++ + GPU natif
InGen = SceneGraph-first + Forge anti-recalcul + Rust natif + GPU bas niveau + LLM natif
```

Tandem moteur:

```text
Rust Native Engine = temps reel, GPU, frame loop, streaming
Forge / Monster = calculs lourds, preuves, cache, artefacts native-ready

Rust demande.
Forge decrit et hash.
Monster execute ou reuse.
Rust consomme.
GPU rend.
```

Regle d architecture:

```text
Rust Native Engine + Monster/Forge forment un seul bloc tandem.
Ne pas agrandir l architecture avec des moteurs paralleles.
Ajouter seulement des lanes et des connexions sur ce bloc.
```

Le bloc tandem porte deux lanes principales:

```text
Tandem Engine Block
|-- Lane A: rendu Unreal-like
|   |-- scene graph
|   |-- virtual/fake geometry
|   |-- material graph
|   |-- radiance cache
|   |-- render graph
|   |-- RHI Vulkan/DX12/Metal
|   `-- Banger viewport/editor
|
`-- Lane B: memoryreading RAM/DOM webviews
    |-- Google Web section
    |-- native webviews
    |-- DOM map
    |-- RAM/DOM high-level reading
    |-- mutation/state graph
    |-- proof/hash projections
    `-- compact LLM context
```

Interdiction:

```text
pas de second moteur web
pas de second moteur DOM
pas de second moteur rendering
pas de second moteur compute
pas de nouvelle architecture pour chaque section
```

Les futures sections se branchent sur le bloc tandem:

```text
Banger -> Tandem Engine Block -> Lane A
Google Web -> Tandem Engine Block -> Lane B
autres sections -> Tandem Engine Block -> lane existante ou nouvelle lane justifiee
```

## Pipeline Vise

```text
1. LLM ou utilisateur demande une scene
   |
2. Brain / State Kernel garde intention, objets, preuves
   |
3. Scene Graph structure la scene
   |
4. Forge Compute Graph decoupe la scene en calculs
   |
5. Chaque calcul recoit un hash stable
   |
6. Cache lookup: deja calcule ou non ?
   |
7. Monster calcule seulement les cache-miss
   |
8. Monster produit des artefacts native-ready
   |
9. Rust Engine consomme les artefacts sans bloquer la frame
   |
10. Slang compile les shaders necessaires
   |
11. Render Graph organise les passes
   |
12. InGen RHI envoie au GPU via Vulkan/DX12/Metal
   |
13. GPU rend l image
   |
14. Banger affiche, edite et renvoie contexte au LLM
```

## Etape 1 - Construire Le Nouveau Moteur

La premiere etape n est pas d ajouter plus de features dans le front actuel.
Elle est de deplacer le coeur du rendu vers un moteur natif.

```text
Rust Native Engine
-> Render Graph
-> Slang shaders
-> InGen RHI
-> Vulkan / DirectX 12 / Metal
-> GPU natif
```

Objectif de cette etape:

- ~~sortir TypeScript/WebGPU du role de coeur moteur pour la lane Banger
  backend~~: Banger dispose maintenant d un service Rust natif direct,
  consommable par l Electron shell et les surfaces natives sans passer par un
  renderer browser.
- garder Banger comme interface/editor,
- ~~creer une vraie boucle frame native~~: le moteur Rust natif produit des
  frames `wgpu` offscreen et une boucle frame avec timeline/proof hash.
- ~~gerer ressources GPU, shaders, passes, buffers et streaming~~: la tranche
  actuelle couvre residency heap, streaming manifest/proof, render graph
  schedule proof, shader pipeline proof, RHI report et viewport contract.
- ~~rendre une scene simple avec hashes/proofs de pipeline~~: le test GPU rend
  une scene offscreen, prouve texture/frame/render-graph/pipeline/RHI/viewport
  et verifie un viewport custom redimensionne avec orbit/pan/zoom/modes et
  `fit_mode=scene`.

Etat reel Banger, 2026-06-06:

- Monster reste intact et prepare les handoffs `native_tandem_render`.
- `src/kasm.rs` et `src/monster.rs` ne sont pas modifies.
- Le moteur Rust natif Banger consomme ces handoffs, cree la residency GPU,
  execute un render graph `wgpu`, produit un render target hashable, un contrat
  viewport Electron/native-ready et une boucle frame prouvee.
- Le viewport natif expose maintenant un cadrage verifiable: bounds derives du
  Hybrid Scene Graph Banger, focus, rayon, padding, FOV, distance camera,
  `fit_bounds_hash` et `viewport_fit_hash`. Le vieux fit direct par slots est
  remplace par des noeuds hybrides artifact-derived avec representation,
  transform, AABB/sphere et proof hash.
- Le render target Banger porte `TEXTURE_BINDING` en plus de
  `RENDER_ATTACHMENT` et `COPY_SRC`; chaque frame expose un bridge texture
  natif avec route d import, fallback et proof hash.
- InGen RHI expose maintenant une matrice de feature gates dans `rhiReport`:
  bindless resource arrays, mesh shader path, ray query path, shader
  precompile cache, compute scale floor et backend parity. Chaque gate contient
  status, features/limits manquants, fallback route, promotion rule et proof
  hash.
- Limite technique restante: la texture n est pas encore promue en surface
  produit interactive. Le prochain gate doit stabiliser le partage frame/texture
  avec le meme device/queue et verifier le chemin Electron/native host.
- Limite Scene Graph restante: le graphe hybride existe et pilote le fit, mais
  ses noeuds restent derives des artefacts Monster/native. La prochaine
  promotion doit donner l autorite a de vrais objets editables, transforms
  parent/enfant et choix de representation scene-first.
- Limite AAA restante: les gates RHI existent et sont verifies, mais les chemins
  production ne sont pas encore promus: DX12 n est pas compile dans ce profil
  Windows, les blobs de pipeline cache ne sont pas persistes, et mesh/ray
  restent des routes conditionnelles derriere la matrice.
- Tranche pipeline cache en cours: Banger a maintenant un manifeste
  content-addressed par adapter/driver/features/shader library et des entrees
  de pipeline rattachees au `shaderPipeline`. Cette tranche n est pas promue:
  la verification Cargo passe, mais la persistence de blobs driver
  `wgpu::PipelineCache` n est pas encore cablee.

Cette etape pose le corps du moteur. Sans elle, le reste reste un prototype.

## Etape 2 - Representation Hybride IA-First

Unreal est mesh-first: le triangle mesh est la base, puis Micro, voxels,
surfels, materials et caches gravitent autour.

Etat reel, 2026-06-06:

- Banger emet un `hybridSceneGraph` par frame native: noeuds, type de
  representation (`sdf`, `voxel`, `meshlet`, `surfel`, `material_graph`,
  `gaussian_splat` ou `native_artifact`), transform, AABB monde, sphere,
  politique de residency, mix de representations, `bounds_hash`, `graph_hash`
  et `proof_hash`.
- Banger expose aussi un manifeste d objets editables:
  `banger_build_scene_object_manifest`. Il verifie ids uniques, parents
  existants, absence de cycle, transforms locaux/monde, choix de
  representation, AABB/sphere, mix de representations et proof hashes.
- Le viewport fit consomme maintenant les bounds du Hybrid Scene Graph au lieu
  de recalculer un proxy directement depuis les slots GPU.
- Verification: `cargo check --manifest-path
  examples\forge_tauri_ui\src-tauri\Cargo.toml --bin forge-ui` passe, et
  `cargo test --manifest-path examples\forge_tauri_ui\src-tauri\Cargo.toml
  --bin forge-ui banger::tests::renders_native_offscreen_frame_artifact_when_gpu_is_available`
  passe avec assertions sur `hybridSceneGraph`. Les tests
  `banger_scene_graph::tests` passent aussi sur le target local
  `.codex-target\banger-scene-tests`.

InGen ne doit pas etre mesh-first ni SDF-only. InGen doit etre:

```text
Scene Graph first
+ Forge first
+ representation hybride
```

Le LLM ne manipule pas les vertices, les millions de splats ou les buffers GPU.
Il manipule une scene structuree:

```text
object_id
role
transform
material
representation
Forge contract
proof refs
cache refs
```

Ensuite le moteur choisit la bonne representation.

```text
SDF = objets generes par LLM, maths, formes procedurales
mesh = personnages, rigs, animations, assets classiques
voxel = volumes, terrain, fog, caches, acceleration
surfel = lumiere indirecte, radiance cache, GI
gaussian splat = scans photorealistes, decors captures
```

Regle simple:

```text
LLM controle le Scene Graph.
Forge controle les calculs hashables.
Rust Engine controle les assets GPU.
GPU rend l image.
```

Exemples:

```text
ocean procedural
-> SDF / field / shader params
-> Forge cache spectres et parametres stables
-> GPU calcule le delta temps reel
```

```text
personnage anime
-> mesh + skeleton + material graph
-> proxy SDF pour selection/collision/contexte LLM
-> GPU rend le mesh
```

```text
falaise scannee
-> gaussian splats pour rendu photorealiste
-> proxy voxel/SDF pour collision et selection
-> surfels pour lumiere
```

```text
foret dense
-> instances mesh ou voxels
-> Forge/PCG hash les regles de distribution
-> renderer reutilise les chunks identiques
```

Cette etape est le pont entre notre avantage LLM/SDF et la realite AAA:

```text
SDF pour creer et comprendre.
Mesh pour animer et produire.
Voxel pour massifier.
Surfel pour eclairer.
Splat pour capturer le reel.
```

## Etape 3 - InGen Virtual/Fake Geometry

Cette etape est l equivalent conceptuel de Micro, mais adapte a InGen.

Unreal/Micro pousse la virtual geometry dense. Crimson Desert/BlackSpace semble
pousser une strategie plus agressive de fake geometry, culling, imposteurs et
reduction de vertices pour les grands mondes.

InGen doit prendre les deux:

```text
Virtual Geometry quand il faut du vrai detail.
Fake Geometry quand l illusion suffit.
Forge cache quand un detail revient plusieurs fois.
```

Representations gerees:

```text
meshlets
SDF bricks
voxel pages
splat clusters
impostors
billboards
distance proxies
procedural instances
```

Pipeline:

```text
Scene Graph
-> choisir representation visible
-> decouper en clusters/pages/proxies
-> hasher chaque fragment avec Forge
-> reutiliser les fragments deja calcules
-> streamer seulement ce qui est utile a la camera
-> envoyer au GPU
```

Objectif:

- afficher des mondes denses,
- eviter le rendu inutile,
- supprimer les details invisibles,
- remplacer les details lointains par des illusions controlees,
- garder une preuve/hash de chaque fragment reusable.

Equivalent simple:

```text
Micro         -> vraie geometry virtuelle
Crimson-like  -> fake geometry agressive
InGen         -> virtual/fake geometry content-addressed
```

## Etape 4 - InGen Radiance Cache

Cette etape est l equivalent conceptuel de Solaris.

But:

```text
calculer la lumiere directe et indirecte
sans tout recalculer depuis zero a chaque frame
```

InGen doit utiliser:

```text
surfels
probes
screen traces
world traces
SDF traces
ray tracing
radiance cache
shadow cache
```

Pipeline:

```text
Scene visible
-> echantillons de surface / surfels
-> traces lumiere
-> cache de radiance par zone
-> reuse par hash quand scene/lumiere/camera changent peu
-> rendu final
```

Objectif:

- lumiere indirecte dynamique,
- ombres douces,
- reflets,
- GI reutilisable,
- cache multi-frame et multi-scene,
- preuve/hash des resultats stables.

Equivalent simple:

```text
Solaris -> InGen Surfel/Radiance Cache
```

## Etape 5 - Path Tracing Et Neural Rendering

Cette etape est le chemin RTX Kit / Omniverse-like.

Elle ne remplace pas le rendu temps reel. Elle ajoute un mode qualite et des
techniques de reconstruction.

Blocs vises:

```text
path tracing progressif
ray tracing hardware
ReSTIR / many-light sampling
denoising temporel
upscaling
ray reconstruction
neural materials
neural texture compression
neural radiance cache
```

Regle:

```text
temps reel = renderer rapide
mode qualite = path tracing progressif
neural = reconstruction/compression/approximation quand verifier possible
```

Forge intervient pour:

- dedupliquer les samples et caches,
- garder les preuves de bake,
- memoriser les radiance caches,
- comparer les approximations neural/classiques,
- refuser les chemins non deterministes quand ils ne sont pas bornes.

## La Couche Anti-Recalcul

Le point differentiant d InGen est le cache Forge multi-echelle.

```text
micro  = formule, bruit, normale, petite fonction SDF
mini   = materiau, patch de surfels, petit champ
small  = brique SDF, voxel page, meshlet, splat group
medium = chunk terrain, foret, radiance cache local
large  = biome, monde, simulation, scene complete
```

Chaque fragment a:

```text
input_hash
program_hash
type_hash
unit_hash
result_hash
proof_hash
cost
backend
```

Si le meme fragment revient, InGen reutilise le resultat au lieu de recalculer.

Exemples:

```text
10 000 rochers partagent le meme bruit fractal
-> Forge hash identique
-> Monster calcule une fois
-> le renderer reutilise 10 000 fois
```

```text
ocean anime
-> Forge bake spectres, champs, parametres stables
-> GPU calcule seulement le delta temps reel
```

## Representations 3D

InGen ne doit pas etre prisonnier d une seule representation. Cette section
est la regle courte de l Etape 2.

```text
SDF = objets proceduraux et LLM-friendly
voxel = volumes, terrain, caches, acceleration
surfel = lumiere indirecte et radiance cache
mesh = personnages, rigs, assets classiques
gaussian splat = decors scannes photorealistes
```

Le LLM manipule le Scene Graph et les contrats Forge, pas les vertices.

## Pipeline Visuel AAA

Le renderer natif doit viser ces blocs:

```text
Virtual Geometry
SDF / voxel acceleration
mesh / splat import
material graph physique
many-light sampling
soft shadows
radiance cache
ray tracing / raymarching / path tracing
temporal accumulation
denoising
upscaling
post-process
```

Equivalent conceptuel:

```text
Micro         -> InGen Virtual Geometry
Solaris       -> InGen Surfel/Radiance Cache
Substrate     -> InGen Material Graph
MegaLights    -> InGen Many-Light Sampling
PCG           -> Forge/LLM Procedural Graph
RDG/RHI       -> InGen Render Graph + RHI
RTX Kit       -> Path tracing + neural rendering path
Crimson-like  -> InGen Fake Geometry / aggressive culling
```

## Regle Forge

Forge doit etre utilise quand un calcul est:

- repetable,
- couteux,
- partageable,
- verifiable,
- reutilisable par hash,
- utile a plusieurs frames, objets ou scenes.

Forge ne doit pas etre utilise pour remplacer l execution pixel par pixel du
GPU dans les passes temps reel. Le GPU rend; Forge evite que le moteur arrive
au GPU avec du travail deja connu.

## Regle Banger

Banger devient l editor et le viewport d InGen:

```text
Banger UI
-> selection, scene collection, chat, preview, controle LLM
-> Native Rust Engine pour rendu lourd
-> Brain/State Kernel pour contexte et preuves
```

Banger n est plus le moteur profond. Banger est la surface intelligente du
moteur InGen.

## Apart - Google Web Et Navigation Native

Plus tard, InGen devra aussi porter une section Google Web pour que le LLM
navigue ultra efficacement dans le navigateur web natif.

Cette direction doit etre prise en compte dans l architecture moteur:

```text
Google Web section
-> navigateur web natif
-> lecture haut niveau RAM / DOM
-> cartographie DOM
-> balisage memoire
-> preuves / hashes
-> contexte compact pour le LLM
```

Le tandem Rust Native Engine + Monster doit aussi servir ici:

```text
Rust Native Engine = observation native, UI, browser surface, frame/state loop
Monster / Forge = analyse massive, hash, dedup, preuve, cartes RAM/DOM
```

Objectif:

- lire et structurer le DOM sans dump brut,
- cartographier les noeuds, relations, mutations et etats visibles,
- baliser la RAM/DOM a haut niveau,
- dedupliquer les observations repetitives,
- fournir au LLM un contexte compact et actionnable,
- garder les preuves et les hashes des observations.

Cette section n est pas le moteur 3D, mais elle partage la meme doctrine:

```text
observer beaucoup localement,
hash ce qui se repete,
ne donner au LLM que des projections compactes,
laisser Rust porter le temps reel,
laisser Monster porter le calcul massif.
```

## Sources De Direction

- Unreal Engine 5.7: Micro Foliage, MegaLights, Substrate, PCG.
- NVIDIA RTX Kit: path tracing, neural rendering, RTX Mega Geometry.
- Slang: shader unique vers Vulkan, DirectX, Metal et autres backends.
- wgpu/Dawn: preuve qu une abstraction GPU multi-backend est viable, mais InGen
  doit viser une RHI native plus ambitieuse pour le moteur profond.
- StableHLO: operations explicites, typage strict, semantique verifiable.
- Triton: kernels GPU par blocs/lanes, proche du metal sans ecrire CUDA partout.
- Futhark: `map`, `reduce`, `scan`, `filter` comme base de calcul parallele.

## Monster SOTA Actuel

Monster n est plus un faux planificateur de calcul. Le chemin reel est:

```text
LLM -> /newcompute_ -> template Monster -> Forge source
-> Forge IR -> primitiveOps -> GPU batch plan
-> Rust RHI / wgpu -> dispatch GPU -> readback -> hash/preuve
```

Ce qui fonctionne:

- calcul massif via `MonsterPreparedCompute::execute_mass_compute`,
- generation de kernels WGSL depuis les primitives Forge,
- couverture de lowering pour tout le vocabulaire primitif universel,
- execution GPU reelle testee avec readback, `output_hash` et `proof_hash`,
- sharding sur plusieurs GPU compatibles au lieu de supposer un seul GPU,
- cache/reuse par fragments hashes,
- exposition de `primitiveOps` dans le JSON `/newcompute_`.

Limite importante:

Les primitives simples executent directement. Les primitives complexes
(`fft`, `svd`, graphes, sparse solve, AD, PDE/ODE, crypto) ont une premiere
semantique massive deterministe, mais pas encore des kernels industriels
specialises. Le pipeline est reel; tous les algorithmes ne sont pas encore au
niveau Unreal/RTX/compute scientifique.

Objectifs restants pour que Monster soit complet:

1. kernels specialises FFT/IFFT/RFFT, sparse, graphes, SVD/QR/Cholesky/eigen;
2. solveurs ODE/PDE, autodiff JVP/VJP, reductions deterministes;
3. vrais buffers resultats types: arrays, tensors, fields, tables, graphs;
4. tests differentiels CPU/GPU avec cas analytiques par famille primitive;
5. politique multi-GPU: score adapter, budget memoire, chunking, retry, merge;
6. cache persistant de kernels par hash IR + primitiveOps + ABI + adapter;
7. modes numeriques robustes: f32/f64, NaN traps, bounds traps, RNG stable;
8. artefacts render natifs: SDF bricks, meshlets, voxels, surfels, materials;
9. artefacts DOM/RAM: graphes/tables de cartographie sans bloquer le navigateur;
10. suppression ou extraction de tout ancien helper Monster hors pipeline Forge.

## Phrase Finale

InGen ne doit pas etre Unreal en WebGPU.

InGen doit etre:

```text
un moteur natif Rust + Forge anti-recalcul + GPU bas niveau,
ou le LLM controle une scene structuree
et ou chaque calcul reutilisable devient un artefact hashe.
```
