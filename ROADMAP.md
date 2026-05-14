# ROADMAP - Forge Event Horizon

> Lecture obligatoire avant toute proposition strategique.
> Branche unique : `master`. Worktree unique. Doctrine pure Rust + `std` + `sha2`.
> Derniere mise a jour : 2026-05-09.

## Vision courte

Forge est maintenant un canvas agentique local adosse a un moteur MCP
content-addressed. Le produit vise trois plans qui doivent rester coherents :

1. **Compute verifiable** : les gros calculs, artefacts et preuves vivent sur disque et se reutilisent par hash.
2. **Canvas multi-LLM** : Codex, Gemini et Claude partagent la meme session, le meme Atlas et les memes outils Forge.
3. **Surface visuelle composable** : fichiers 2D/3D, visual programs, `planet_sphere`, geonodes et preuves compactes dans le chat.

## Spec 2026-05-09 - Forge Runtime OS v1 (deadline 2026-05-15)

Objectif de bloc : faire passer Forge d'un canvas agentique outille a un vrai runtime semantique local. Le but n'est plus seulement d'afficher des providers, des tools MCP et des graphes, mais de faire de Forge la couche de controle qui pense, route, valide, compile et memorise avant toute action.

### Vision de livraison au 2026-05-15

Au 15 mai, Forge doit livrer un premier vertical slice coherent ou :

- le LLM ne touche plus directement le bas niveau ;
- les actions passent par des capacites Forge haut niveau ;
- les sorties utiles sont compilees en structures semantiques et en nodes ;
- la memoire persistante est locale et exploitable ;
- un judge local valide avant action ou ecriture critique ;
- le runtime peut deja orchestrer plusieurs agents selon une topologie simple ;
- l'UI permet de voir ce qui a ete route, compile, valide et memorise.

### Les 15 objectifs en cascade

#### 1. 2026-05-09 - Figer le contrat produit "Forge Runtime"

**But**

Definir une source de verite unique pour ce que Forge devient : un runtime semantique applicatif, pas un simple chat avec plugins ni un assembleur de tools MCP.

**Plan d'action**

- Ecrire le manifeste court du runtime Forge.
- Nommer les briques obligatoires et leurs responsabilites.
- Definir la frontiere entre UI, runtime, outils, memoire et graphe.
- Geler le vocabulaire officiel (`capability`, `semantic compiler`, `judge`, `memory stack`, `virtual MCP server`, `world graph`).

**Requirements techniques**

- Un document canonique versionne dans le repo.
- Une liste des modules obligatoires du runtime.
- Une matrice "responsable / interdit / depend de".

#### 2. 2026-05-09 - Interdire le bas niveau au LLM

**But**

Supprimer le modele actuel ou un LLM peut raisonner a partir de primitives trop brutes. Forge doit exposer des intentions et des actions haut niveau, jamais des tuyaux de plomberie.

**Plan d'action**

- Lister tous les acces bas niveau existants.
- Classer chaque acces : interdit, encapsule, ou autorise sous mediation Forge.
- Introduire une policy runtime qui bloque les appels directs non conformes.
- Rediriger les flows critiques vers des wrappers haut niveau.

**Requirements techniques**

- Policy machine-lisible pour `shell`, `filesystem`, `network`, `sql`, `broker API`, `gpu`.
- Journalisation des violations.
- Fallback d'erreur explicite cote runtime.

#### 3. 2026-05-09 - Definir le schema de capacites Forge

**But**

Faire des capacites Forge la primitive centrale du systeme. Une capacite doit etre plus riche qu'un tool : elle transporte l'intention, les risques, les effets de bord et les preuves attendues.

**Plan d'action**

- Definir la structure JSON ou Rust canonique d'une capacite.
- Separarer `intent`, `inputs`, `constraints`, `outputs`, `side_effects`, `proof`.
- Ajouter des niveaux de risque et des contraintes d'execution.
- Definir la compatibilite avec MCP sans laisser MCP dicter le modele interne.

**Requirements techniques**

- Schema stable versionne.
- Validateur de schema.
- Support des capacites synchrones, asynchrones et composees.

#### 4. 2026-05-10 - Construire le Capability Registry local

**But**

Donner a Forge une table des matieres vivante de ce qu'il sait faire. Le runtime doit pouvoir chercher, scorer et router ses propres capacites sans dependre d'un LLM pour se souvenir.

**Plan d'action**

- Construire un registre local des capacites, skills, agents, memories et adaptateurs.
- Indexer par nom, domaine, type d'effet, risque, cout et dependances.
- Exposer une recherche semantique locale et une recherche stricte par ID.
- Permettre l'ajout dynamique sans casser la compatibilite.

**Requirements techniques**

- Stockage local robuste.
- API de lookup rapide.
- Metadonnees minimales : id, version, domaine, prerequis, cout, risque, outputs.

#### 5. 2026-05-10 - Livrer le Semantic Router v1

**But**

Faire en sorte qu'une intention utilisateur soit traduite automatiquement vers la bonne combinaison de capacites, d'agents, de memoire et de topologie.

**Plan d'action**

- Construire le routeur d'intentions.
- Mapper intention -> capacite -> agent -> memoire -> topologie.
- Introduire un score multi-critere : cout, latence, confiance, specialite.
- Ajouter un fallback simple si la confiance est trop basse.

**Requirements techniques**

- Contrat d'entree/sortie pour le routeur.
- Scores et traces de routage.
- Possibilite de replay et de debug.

#### 6. 2026-05-10 - Livrer le Semantic Compiler v1

**But**

Compiler les sorties LLM en objets operables Forge. Le texte n'est plus la destination finale ; il devient une matiere a structurer.

**Plan d'action**

- Detecter les entites, metriques, relations, hypotheses et actions.
- Transformer ces sorties en structures intermediaires.
- Produire `geonodes`, `mini_geonodes`, `metric nodes`, `proof nodes`, `draft nodes`.
- Refuser l'ecriture directe dans le graphe sans validation.

**Requirements techniques**

- Pipeline d'extraction structuree.
- Score de confiance par objet.
- Support des modes `draft`, `validated`, `rejected`.

#### 7. 2026-05-11 - Construire la Persistent Memory v1

**But**

Faire de Forge un systeme qui se souvient de maniere utile, pas juste un historique de chat. La memoire doit devenir une infrastructure de travail.

**Plan d'action**

- Decouper la memoire en couches.
- Ajouter memoire de session, de tache, de strategie, d'agent et de graphe.
- Definir ce qui est ecrit automatiquement et ce qui demande validation.
- Rendre la memoire requetable par le router et par le judge.

**Requirements techniques**

- Stockage local durable.
- Indexation temporelle et semantique.
- Politique de retention et versioning.

#### 8. 2026-05-11 - Construire le Judge Pipeline v1

**But**

Eviter que Forge execute, ecrive ou compile n'importe quoi sur simple enthousiasme du LLM. Le judge devient le garde-fou interne.

**Plan d'action**

- Evaluer coherence, confiance, schema, risque et conflit d'etat.
- Faire des passes de critique avant commit.
- Produire un verdict : accept, draft, reject, needs_review.
- Conserver des preuves de jugement exploitables par l'UI.

**Requirements techniques**

- Contrat de verdict stable.
- Rules engine local.
- Trace des checks et motifs de rejet.

#### 9. 2026-05-11 - Construire l'Execution Sandbox v1

**But**

Encapsuler toute execution reelle dans un environnement controle, observable et rejouable. Forge doit pouvoir agir sans jamais perdre le controle de ce qui s'est passe.

**Plan d'action**

- Definir une sandbox pour code, fichiers, reseau et calcul.
- Mapper chaque capacite executable vers un profil de sandbox.
- Bloquer les sorties de sandbox qui ne respectent pas le contrat attendu.
- Journaliser les effets de bord.

**Requirements techniques**

- Profils de sandbox formels.
- Logs et artefacts content-addressed.
- Permissions explicites par capacite.

#### 10. 2026-05-12 - Transformer les tools existants en Virtual MCP Servers

**But**

Conserver l'interop MCP sans laisser MCP devenir la logique produit. Les tools existants doivent etre re-encapsules comme adaptateurs gouvernes par Forge.

**Plan d'action**

- Identifier les tools MCP critiques actuels.
- Les emballer derriere des capacites Forge.
- Garder MCP comme couche de transport et de distribution.
- Ajouter les metadonnees de registre et de policy.

**Requirements techniques**

- Adaptateurs compatibles MCP.
- Table de correspondance capability -> MCP server/tool.
- Compatibilite avec registry local puis registry externe plus tard.

#### 11. 2026-05-12 - Livrer les topologies multi-agents v1

**But**

Permettre a Forge d'orchestrer plusieurs intelligences de maniere explicite au lieu d'empiler des appels de modele. La topologie devient un objet de runtime.

**Plan d'action**

- Implementer au minimum les topologies : sequentiel, parallele, hierarchique, judge/debate.
- Definir les regles de passage de contexte entre agents.
- Tracer quelle topologie a ete choisie et pourquoi.
- Permettre un fallback mono-agent.

**Requirements techniques**

- Scheduler simple.
- Contrat d'echanges inter-agents.
- Etat partage limite et controle.

#### 12. 2026-05-13 - Brancher le dialogue inter-LLM sous controle Forge

**But**

Faire parler les agents entre eux sans laisser naitre un chaos conversationnel opaque. Le runtime Forge doit rester le mediateur et l'archiviste de chaque echange.

**Plan d'action**

- Definir un protocole d'echange interne Forge.
- Normaliser les messages inter-agents.
- Enregistrer qui parle, pourquoi, avec quelle capacite et quel contexte.
- Eviter les boucles, la dilution de contexte et la perte de responsabilite.

**Requirements techniques**

- Message envelope interne.
- IDs de tour et de causalite.
- Budget de contexte par agent.

#### 13. 2026-05-13 - Connecter le Semantic Compiler au World Graph

**But**

Faire du graphe Forge une surface vivante et calculable. Toute sortie semantique utile doit pouvoir devenir un node, une relation ou une mise a jour de metrique.

**Plan d'action**

- Definir les kinds minimaux du world graph.
- Mapper les sorties compilees vers les kinds de nodes.
- Ajouter des upserts, des drafts et des merges prudents.
- Garder les preuves et la provenance a chaque ecriture.

**Requirements techniques**

- Schemas de nodes et d'edges.
- IDs stables et strategie de merge.
- Provenance : source, agent, modele, timestamp, preuve.

#### 14. 2026-05-14 - Construire la Control UI v1

**But**

Rendre le runtime visible. L'utilisateur doit pouvoir comprendre ce que Forge est en train de faire, avec quels agents, quelles capacites, quelle memoire et quel verdict.

**Plan d'action**

- Ajouter une surface de controle du runtime.
- Afficher topologie active, capacites appelees, memoire mobilisee et nodes produits.
- Rendre visibles les verdicts du judge.
- Montrer les etats `draft`, `validated`, `rejected`.

**Requirements techniques**

- Etat frontend derive du runtime.
- Panneaux ou overlays lisibles sans casser le format canonique Forge.
- Journaux compacts et non bavards.

#### 15. 2026-05-15 - Livrer le vertical slice Forge Runtime OS v1

**But**

Assembler toutes les briques precedentes dans un premier flux complet. Ce n'est pas la version finale de Forge, mais la premiere preuve que le noyau semantique fonctionne de bout en bout.

**Plan d'action**

- Prendre une intention utilisateur representative.
- La router via le Semantic Router.
- Executer une ou plusieurs capacites dans la sandbox.
- Compiler le resultat en nodes.
- Le faire juger avant commit.
- L'enregistrer en memoire et l'afficher dans l'UI de controle.

**Requirements techniques**

- Demo ou scenario canonique rejouable.
- Logs et preuves compacts.
- Etats finaux coherents entre runtime, graphe, memoire et UI.

### Definition du succes au 2026-05-15

Le bloc est considere livre si :

- Forge route une intention sans exposer le bas niveau au LLM ;
- au moins une capacite haut niveau pilote une execution reelle ;
- le resultat est compile semantiquement ;
- le judge peut accepter, refuser ou mettre en draft ;
- la memoire persistante retient le contexte utile ;
- au moins une topologie multi-agent fonctionne ;
- l'UI montre clairement le pipeline runtime.

### Priorites absolues si le temps manque

1. `Forge Runtime contract`
2. `Capability Registry`
3. `Semantic Router`
4. `Semantic Compiler`
5. `Persistent Memory`
6. `Judge Pipeline`
7. `Execution Sandbox`

### Objectifs rayes / remplaces

- ~~Faire de MCP la primitive produit centrale.~~ Remplace par un runtime Forge avec MCP comme couche d'interop.
- ~~Laisser les LLM manipuler shell, fichiers et APIs brutes pour gagner du temps.~~ Remplace par des capacites Forge haut niveau et une sandbox.
- ~~Traiter le graphe comme une sortie visuelle optionnelle.~~ Remplace par un world graph alimente par compilation semantique.
- ~~Empiler plusieurs LLM sans topologie ni mediation centrale.~~ Remplace par des topologies d'orchestration explicites et tracees.
- ~~Conserver la memoire comme un simple historique de chat.~~ Remplace par une memoire persistante multi-couches.

## ✅ Livre 2026-05-09 - BOOM workspace Blender/Vectary + Slicer + KASM spatial

Objectif de session : transformer BOOM d'un simple mode visuel en vrai workspace 3D local pilotable par LLM, avec panel de scene, import mesh, modes d'edition, slicer et couche KASM au-dessus du mesh runtime.

### Objectifs remplis

- ✅ Activation BOOM stable depuis la titlebar, avec bouton actif/inactif, ouverture/fermeture propre et retour a `New session`.
- ✅ Viewport BOOM plein ecran transparent avec matrice 3D type Blender, grille profonde, gizmo allegé et origine recalee sur le canvas Forge.
- ✅ Navigation viewport : orbite + pan au clic droit maintenu, comportement corrige pour suivre la direction du geste.
- ✅ Panel gauche hybride : Outliner simplifie type Blender en haut, inspector plus Vectary-like en bas.
- ✅ Workflow explicite `Design -> Slicer` dans le panel gauche.
- ✅ Import 3D via bouton `+` du chat et drag-and-drop sur la matrice.
- ✅ Import frontend direct pour `OBJ`, `STL`, `PLY`, `OFF`, `glTF`, `GLB`.
- ✅ Normalisation backend vers `GLB` pour formats plus larges (`FBX`, `DAE`, `3DS`, `3MF`, `USD/USDZ`, etc.) via Blender quand il est disponible.
- ✅ Affichage du mesh importe dans la scene et dans l'Outliner, avec selection synchronisee viewport <-> panel.
- ✅ Picking composant `Object / Vertex / Edge / Face` avec surbrillance viewport.
- ✅ Barre de modes viewport + stack de modifiers `Mirror`, `Array`, `Inflate`, `Bevel`, `Subdivide`, `Solidify`.
- ✅ Mode Slicer branche a un vrai preview de couches dans le viewport.
- ✅ Premiere detection imprimantes/profils cote backend Tauri pour le mode Slicer.
- ✅ Couche KASM topologique au-dessus du mesh : `cell / coordinate / vertex / edge / face / object / modifier`.
- ✅ Coordonnees XYZ, cellules spatiales, requetes de voisinage, regions hash-addressed et seeds geonodes symboliques exposes au runtime.
- ✅ Contrat UI universel BOOM : chaque bouton, champ et parametre visible recoit un hash stable et un nom d'outil logique pilotable par LLM.
- ✅ Panneau droit `Verification / Console` avec console `Rust / KASM`, sans deplacer le chat natif Forge.
- ✅ Injection de contexte BOOM dans la console : objet actif, selection, region, graphe KASM et contrat UI hash-addressed.

### Objectifs rayes / remplaces

- ~~Traiter BOOM comme un simple canvas 3D decoratif a cote du chat.~~ Remplace par un vrai workspace scene + slicer + KASM.
- ~~Faire du panel gauche un simple placeholder vide.~~ Remplace par Outliner + inspector utiles.
- ~~Lire les objets 3D comme des blobs opaques non adressables.~~ Remplace par topologie et regions KASM.
- ~~Considerer Blender/Vectary et Slicer comme deux produits separes.~~ Remplace par un pipeline unique : concevoir puis imprimer.
- ~~Laisser l'UI BOOM muette pour le LLM.~~ Remplace par un contrat hash/tool explicite.

### Reste ouvert

1. Ajouter la vraie selection de region par box/lasso/volume dans le viewport, pas seulement a partir de la selection composant active.
2. Brancher des outils de modelisation sur regions KASM (`extrude`, `inset`, `delete`, `bridge`, etc.).
3. Brancher l'execution reelle de la console BOOM : `Rust` vers runtime/programmes Forge, `KASM` vers outils MCP/scene graph live.
4. Ajouter un systeme de rigging BOOM inspire d'Auto-Rig Pro, mais clean-room et pilote par KASM/MCP.
5. Completer le mode Slicer avec export machine, preview plus riche (infill/walls/supports) et workflow print end-to-end.

## ⏳ Spec 2026-05-09 - BOOM Rig Graph (clean-room, inspire d'Auto-Rig Pro)

Objectif du prochain bloc : faire de BOOM un vrai mode rigging agentique. Le but n'est pas de copier du code proprietaire, mais de reproduire la logique produit d'un rig generator moderne dans l'architecture Forge : scene KASM adressable, outils MCP formels, regeneration deterministe, et pilotage complet par LLM.

### Principe produit

- Le workflow global BOOM reste `Design -> Slicer`.
- Le rigging vit **dans `Design`**, comme sous-mode specialise de la scene.
- Le chat reste dans la barre native Forge.
- La console Rust/KASM du panneau droit devient le poste d'orchestration du rig.
- Le systeme doit rester regenerable : on edite une couche de reference, puis Forge regenere `Control`, `Deform`, `Retarget`, `Export`.

### Premier scope livre vise

1. Construire la specification `BOOM Rig Graph`.
2. Creer un premier `Humanoid Reference Rig`.
3. Ajouter `Generate Rig` avec :
   - spine
   - arms
   - legs
   - IK/FK
   - twist bones
4. Ajouter ensuite `Retarget`.
5. Finir par `Export`.

### Graphe KASM BOOM Rig

Le rig n'est pas un blob Blender opaque. C'est un graphe KASM adresse par hash, superpose a la scene mesh.

#### Node kinds

- `rig.blueprint`
  - Root logique du rig.
  - Porte `rigHash`, `version`, `species`, `symmetryMode`, `unitScale`.
- `rig.reference_bone`
  - Os de placement initial editable a la main ou par detection.
  - Champs : `name`, `side`, `headXYZ`, `tailXYZ`, `roll`, `parent`, `mirrorOf`, `semantic`.
- `rig.reference_marker`
  - Landmark haut niveau (`hips`, `chest`, `neck`, `wrist_l`, `ankle_r`, etc.).
  - Sert a l'auto-placement humanoid.
- `rig.module`
  - Module de generation (`spine`, `arm_l`, `arm_r`, `leg_l`, `leg_r`, `hand_l`, `head`, etc.).
  - Porte les options de generation par membre.
- `rig.control_bone`
  - Os de controle animateur.
  - Champs : `shape`, `color`, `space`, `ikFkGroup`, `pickerGroup`.
- `rig.deform_bone`
  - Os de deformation skinnant le mesh.
  - Champs : `bindRole`, `stretch`, `twistIndex`, `segment`.
- `rig.mechanism_bone`
  - Os utilitaire interne pour solveurs, pole vectors, reverse foot, soft IK, etc.
- `rig.constraint`
  - Contrainte orientee graphe.
  - Types initiaux : `copy_transform`, `copy_rotation`, `copy_location`, `aim`, `pole`, `ik`, `fk`, `stretch`, `limit`, `space_switch`.
- `rig.ik_chain`
  - Groupe solveur IK.
  - Champs : `root`, `mid`, `tip`, `pole`, `softness`, `stretch`, `preferredAngle`.
- `rig.fk_chain`
  - Groupe de controles FK coherents.
- `rig.twist_chain`
  - Chaine de repartition de torsion.
  - Champs : `driver`, `targets`, `count`, `distribution`.
- `rig.skin_cluster`
  - Liaison mesh -> deform bones.
  - Porte les poids ou references de poids.
- `rig.retarget_profile`
  - Profil source/target pour remap animation.
- `rig.retarget_map`
  - Mappage os source -> controles ou deform bones cibles.
- `rig.export_profile`
  - Configuration d'export runtime (`engine`, `format`, `leafBones`, `axisPreset`, `embedActions`).
- `rig.validation_report`
  - Rapport verifiable de generation/export.

#### Edges / relations

- `blueprint -> reference_bone`
- `reference_bone -> module`
- `module -> control_bone`
- `module -> deform_bone`
- `module -> mechanism_bone`
- `control_bone -> constraint`
- `deform_bone -> skin_cluster`
- `ik_chain -> control_bone / mechanism_bone`
- `twist_chain -> deform_bone`
- `retarget_profile -> retarget_map`
- `export_profile -> validation_report`

#### Invariants KASM

- Chaque bone, module, contrainte, chaine IK/FK/twist et profil d'export recoit un hash stable.
- Les coords `head/tail/roll` sont elles-memes derivees des hashes KASM XYZ/cells deja presentes dans BOOM.
- `Generate Rig` ne mute pas brutalement la reference : il cree une nouvelle couche `control/deform/mechanism` regenerable.
- Les poids skin, maps de retarget et profils export sont content-addressed et diffables.
- Toute commande UI du mode Rig doit exposer un `data-kasm-hash` et un `data-boom-mcp-tool`.

### Humanoid Reference Rig v1

Premier preset a livrer :

- `root`
- `hips`
- `spine_01`, `spine_02`, `chest`
- `neck`, `head`
- `clavicle_l/r`
- `upperarm_l/r`, `lowerarm_l/r`, `hand_l/r`
- `thigh_l/r`, `calf_l/r`, `foot_l/r`, `toe_l/r`

Capacites attendues :

- symetrie gauche/droite
- landmark placement manuel
- auto-placement humanoid basique a partir du mesh import
- edition de `head`, `tail`, `roll`
- regen propre sans casser les hashes de la reference inchangée

### Generate Rig v1

`Generate Rig` prend le `Humanoid Reference Rig` et produit les couches suivantes :

#### Spine

- controle `root`
- bloc `hips`
- chaine `spine -> chest -> neck -> head`
- mecanismes de courbure et orientation propre

#### Arms

- IK/FK bras
- pole vector coude
- controle main
- clavicle linking
- option stretch

#### Legs

- IK/FK jambes
- pole vector genou
- controles `foot` et `toe`
- base reverse-foot mecanique

#### IK/FK

- chaque membre expose un groupe `ik_fk_blend`
- blend hash-addressed et pilotable par outil
- changement de mode rejouable et scriptable

#### Twist bones

- generation `upperarm_twist_*`, `forearm_twist_*`, `thigh_twist_*`, `calf_twist_*`
- repartition parametrable
- derivee de `twist_chain`

### Retarget

Le retarget est une couche a part, jamais confondue avec le rig de reference.

#### Inputs

- `source skeleton`
- `target BOOM control rig`
- `retarget profile`

#### Sorties

- `rig.retarget_map`
- `animation remap plan`
- `validation_report`

#### Fonctions v1

- auto-map par noms/semantiques
- correction d'echelle
- mode `in-place`
- exclusion de bones auxiliaires
- preview de correspondance dans le panel

### Export

Le rig exporte un profil runtime, pas juste un fichier.

#### Formats cibles v1

- `GLB`
- `FBX`

#### Profils v1

- `generic_runtime`
- `unity_humanoid`
- `unreal_skeleton`

#### Checks avant export

- hierarchie valide
- axes / roll tolerables
- controle bones exclus si besoin
- deform bones complets
- bind pose presente
- actions/animations referencees correctement

### Tools MCP BOOM Rig

#### Reference

- `boom.rig.reference.create_humanoid`
- `boom.rig.reference.detect_landmarks`
- `boom.rig.reference.place_bone`
- `boom.rig.reference.mirror_bone`
- `boom.rig.reference.add_module`
- `boom.rig.reference.remove_module`

#### Generate

- `boom.rig.generate.control_rig`
- `boom.rig.generate.spine`
- `boom.rig.generate.arm`
- `boom.rig.generate.leg`
- `boom.rig.generate.ik_fk`
- `boom.rig.generate.twist_chain`
- `boom.rig.generate.validate`

#### Skin

- `boom.rig.skin.bind_mesh`
- `boom.rig.skin.transfer_weights`
- `boom.rig.skin.normalize_weights`
- `boom.rig.skin.mirror_weights`

#### Retarget

- `boom.rig.retarget.create_profile`
- `boom.rig.retarget.auto_map`
- `boom.rig.retarget.apply_clip`
- `boom.rig.retarget.validate`

#### Export

- `boom.rig.export.profile_create`
- `boom.rig.export.preview`
- `boom.rig.export.glb`
- `boom.rig.export.fbx`
- `boom.rig.export.validate`

### Structure du panel gauche

Le panel gauche reste hybride Blender/Vectary, mais le rig y ajoute une strate claire.

#### Top level workflow

- `Design`
- `Slicer`

#### Sous-mode de `Design`

- `Model`
- `Rig`

#### Outliner haut

- `Scene Collection`
- `Collection`
- `Mesh`
- `Armature`
  - `Reference`
  - `Control`
  - `Deform`
  - `Mechanism`

#### Inspector bas en mode `Rig`

- `Reference`
  - preset humanoid
  - landmarks
  - symetrie
  - edition bone head/tail/roll
- `Generate`
  - spine
  - arms
  - legs
  - IK/FK
  - twist
  - validation
- `Skin`
  - bind
  - weights
  - mirror
  - normalize
- `Retarget`
  - source rig
  - auto-map
  - profile
  - preview
- `Export`
  - format
  - engine profile
  - checks
  - export button

### Pipeline canonique

1. `Reference`
   - creer ou detecter un humanoid reference rig
   - ajuster les landmarks et les bones de reference
2. `Control`
   - generer les controles animateur et les chaines IK/FK
3. `Deform`
   - produire les deform bones, twist bones et clusters de skin
4. `Retarget`
   - mapper une animation source sur le rig BOOM
5. `Export`
   - valider, choisir le profil runtime, exporter

### Etat de livraison attendu pour ce bloc

- un `Humanoid Reference Rig` minimal mais propre
- un `Generate Rig` v1 regenerable
- des nodes/edges KASM explicites pour tout le rig
- des tools MCP formels pour chaque etape
- un panel `Rig` coherent dans le mode `Design`
- aucune logique magique opaque : chaque etape doit etre hashable, rejouable et pilotable par console/LLM

## ✅ Livre 2026-05-09 - Provider workbench, Planet/GeoNode et store canonique

Objectif de session : sortir Forge d'un etat transitoire ou les providers
ouvraient encore des shells externes, ou `cargo tauri dev` pouvait lire un
autre store que la build courante, et ou le globe Mars etait encore pense
comme une integration speciale plutot qu'une vraie brique produit.

### Objectifs remplis

- ✅ Workbench providers livre : une seule surface providers dans Forge, un seul terminal embarque, et un lanceur sobre pour Codex, Gemini et Claude.
- ✅ Vrai terminal integre : passage a une base PTY + xterm vendored pour afficher la vraie UI des CLIs au lieu d'un faux log texte.
- ✅ Cible cross-platform explicite : le chemin terminal est pense pour Windows, macOS et Linux, pas seulement pour un shell Windows.
- ✅ Bootstrap provider integre : installation/login/lancement Gemini et Claude dans Forge si le CLI manque, avec verification Node/npm au lieu d'une bidouille manuelle dans les dossiers.
- ✅ Store canonique verrouille : `./.forge-store` a la racine du repo devient la source de verite pour `cargo tauri dev` et pour la build desktop.
- ✅ Recuperation sessions/Atlas : migration et auto-repair du store, restauration de l'historique, persistence backend des conversations canvas, et retour de la detection CPU/GPU.
- ✅ Atlas produit elargi : nouveaux kinds `geonode` et `mini_geonode`, visibles par l'UI et par les LLM via l'overview Atlas.
- ✅ Architecture spatiale clarifiee : `planet_sphere` devient un tool visuel universel, distinct des `programs`, et les lieux deviennent des `GeoNode` / `MiniGeoNode`.
- ✅ Globe Mars nettoye : integration du globe HD avec zoom/focus geonode, pills de lieux dans le chat, sans reprendre les anciens panneaux editoriaux de Mars Magazine.

### Objectifs rayes / remplaces

- ~~Ouvrir PowerShell ou un terminal externe pour connecter Gemini ou Claude.~~ Remplace par un workbench providers embarque dans Forge.
- ~~Traiter Mars comme une mini-app autonome / Lens speciale hors modele Atlas.~~ Remplace par `planet_sphere` + `GeoNode` / `MiniGeoNode` reutilisables.
- ~~Laisser `cargo tauri dev` lire un store AppData separe de la build courante.~~ Remplace par `./.forge-store` canonique dans le repo.
- ~~Garder les conversations canvas uniquement en `localStorage`.~~ Remplace par une persistence backend dans les manifests de job quand le chat vit vraiment dans Forge.

### Reste ouvert

1. Verifier visuellement le vrai rendu PTY Gemini/Claude/Codex sur Windows, macOS et Linux, pas seulement la compile et le wiring.
2. Generaliser `planet_sphere` a d'autres corps que Mars, puis ajouter un renderer spatial pour les ancrages `ra/dec`.
3. Ajouter le mini-resolver qui cree automatiquement une GeoNode/MiniGeoNode inconnue a partir d'un lieu mentionne par l'utilisateur ou le LLM.
4. Finaliser la mise en page providers pour garantir le no-scroll absolu sur toutes les tailles de fenetre utiles.

## ✅ Livre 2026-05-08 - Canvas multi-LLM, My Atlas et programmes universels

### Objectifs remplis

- ✅ Chat canvas nettoye : presentation plus proche de Codex/Claude, suppression des messages parasites.
- ✅ Barre multi-agent : selection Codex / Gemini / Claude / All, modele, effort, stop, microphone, ajout de fichier et choix de programme depuis la barre de chat.
- ✅ Coexistence providers : Codex, Gemini et Claude partagent la meme session Forge, les memes descriptions d'outils et les memes strategies d'economie de tokens.
- ✅ Sessions persistantes : changer de session ne doit pas arreter un LLM, une reflexion ou un calcul en cours.
- ✅ Fichiers en canvas : un fichier joint devient une carte 2D/3D a droite du chat, pas une piece jointe texte.
- ✅ Default visual program : tant qu'un LLM ne cree pas une map specialisee, Forge produit une traduction litterale 2D/3D du fichier.
- ✅ My Atlas UI et backend : programmes, metric tags et runs sauvegardes, avec hits instantanes par hash.
- ✅ `program_compile_validate_route` + `forge_compile_validate_route` livres.
- ✅ Programmes invalides non annules : etat `needs_repair` au lieu d'un faux succes ou d'un abandon brutal.
- ✅ Micro-evenements de creation : compilation, validation, routage et sauvegarde Atlas interleaves dans le chat.
- ✅ Logs de calcul separes : interne dans le terminal, mathematique lisible dans le canvas.
- ✅ Calcul en arriere-plan : le chat reste utilisable pendant les runs.

### Objectifs rayes / remplaces

- ~~Specialiser Forge sur quelques programmes trading/backtesting.~~ Remplace par des programmes universels a metriques libres.
- ~~Laisser les LLM lire les CSV/logs/cartes 3D pour interpreter les donnees.~~ Remplace par contexte compact, artefacts et preuves.
- ~~Afficher les logs internes Forge dans le chat.~~ Remplace par cartes live lisibles et preuve laterale.
- ~~Annuler un programme des qu'il a une erreur de construction.~~ Remplace par `needs_repair`.

### Reste ouvert

1. Brancher plus d'executors reels pour les metriques encore `custom_unresolved`.
2. Renforcer les contrats de visual programs pour des domaines hors finance.
3. Ajouter des tests UI automatises sur les flows critiques.

## ✅ Livre 2026-05-08 - Economie de compute exacte KASM + Alpha

### Objectifs remplis

- ✅ `Op::Lazy` / `Op::Force` ajoutes a l'ISA KASM sans approximation.
- ✅ `Program::new` a maintenant un cache structurel global et borne.
- ✅ `Atlas::blob_result_key(...)` ajoute comme cle generique RESULT pour les blobs.
- ✅ Alpha H4 stocke la raw feature matrix en blob verifie au lieu de centaines de milliers de scalaires.
- ✅ Les programmes KASM des labels trade sont reutilises via `OnceLock`.

### Mesures NATGAS H4

| Etape | Avant | Apres cold exact | Apres warm exact |
|---|---:|---:|---:|
| Raw feature matrix | ~3.6 s + ~463k writes | ~2.0 s + 1 blob | ~36.6 ms |
| Labels LONG/SHORT | ~27.2 s | ~2.6-2.7 s | ~42.3 ms |
| Raw + labels | ~30.8 s | ~4.7 s | ~79 ms |

### Reste ouvert

1. Debloquer certains runs Alpha frais qui coincent encore au premier beam CUDA.
2. Etendre `blob_result_key` aux autres pipelines lourds.
3. Garder `Lazy/Force` strictement sur des chemins exacts.

## ✅ Livre 2026-05-06 - Pivot Forge MCP Compute

### Objectifs remplis

- ✅ `forge_mcp` comme point d'entree agent-first.
- ✅ UI Tauri ramenee au role d'observateur/client.
- ✅ Surface MCP compacte : `about`, `capabilities`, `create`, `program_compile_validate_route`, `run`, `jobs`, `read`, `logs`, `cancel`.
- ✅ Garde-fous `token_safety` : resumes, curseurs et references au lieu de gros blobs dans le contexte LLM.
- ✅ UI Programs branchee sur la bibliotheque content-addressed.
- ✅ Visual mapping 3D et proof panel relies aux artefacts et a leurs hashes.

### Reste ouvert

1. Annulation cooperative plus profonde dans les boucles CPU/GPU.
2. `proof.json` encore plus riche en replay et versioning.
3. Liaison fine point 3D -> metric/proof/artifact associe.

## Focus immediat

1. Ajouter la vraie selection de region BOOM par box/lasso/volume et brancher les premiers outils de modelisation region-aware.
2. Construire la console BOOM d'orchestration (chat + tool + query + script) pour manipuler scene, slicer et KASM depuis le meme point d'entree.
3. Stabiliser la page providers et verifier le vrai rendu PTY sur Windows, macOS et Linux.
4. Finir le mini-resolver spatial et l'upsert automatique des geonodes inconnues.
5. Continuer d'ajouter des executors concrets sans casser la compacite MCP ni la doctrine content-addressed.
