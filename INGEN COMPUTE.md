# Banger: Ingénierie Computationnelle & Géométrie Implicite

Ce document définit la vision et l'architecture technique pour transformer le moteur **Banger** d'un simple visualiseur 3D en une plateforme de synthèse générative pilotée par la physique (Computational Engineering).

## 1. Le Changement de Paradigme : Du Mesh au SDF

L'ingénierie traditionnelle repose sur le **Mesh** (triangles), une représentation *discrète* et *statique*. L'ingénierie computationnelle exige une représentation *continue* et *dynamique* : le **SDF** (Signed Distance Field).

### Pourquoi abandonner le Mesh pour la conception ?
*   **Rigidité Topologique** : Modifier un mesh complexe (ajouter un canal interne, fusionner deux pièces) est instable et produit des erreurs géométriques.
*   **Coût de Calcul** : L'optimisation de forme sur des millions de triangles est exponentiellement lente.
*   **Perte de Précision** : Les arrondis et les structures organiques sont approximés par des facettes plates.

### L'Avantage du SDF (Signed Distance Fields)
Un SDF est une fonction mathématique $f(p) \rightarrow d$ où $p$ est un point dans l'espace et $d$ la distance à la surface la plus proche.
*   **Opérations Booléennes Parfaites** : L'union, l'intersection et la soustraction sont des opérations mathématiques simples (min/max).
*   **Fusions Organiques (Smooth Blending)** : On peut fusionner deux formes avec une transition fluide ("veineuse") par simple interpolation.
*   **Topologie Fluide** : La matière peut se séparer ou se rejoindre sans casser le modèle. La topologie "émerge" de la fonction.

## 2. Le Rôle de Rust et KASM

L'objectif est de remplacer Python par un pipeline **Rust natif + KASM bytecode** pour garantir performance et sécurité.

### KASM comme Langage de Description Physique
Au lieu d'importer un fichier `FBX` ou `STL`, l'ingénieur (ou l'agent) fournit un programme **KASM**.
*   **Input** : Contraintes physiques (vecteurs de force, zones de chaleur, ancres mécaniques).
*   **Logique** : Le bytecode KASM décrit comment la matière doit réagir.
*   **Output** : Une fonction de distance que le GPU peut évaluer en temps réel.

### Banger comme Solveur de Champ
Banger (en Rust) devient l'hôte qui exécute ce champ :
1.  **JIT Compilation** : Le programme KASM est transformé en un Compute Shader (`wgpu`).
2.  **GPU Evaluation** : Le GPU calcule la forme optimale à 60 FPS.
3.  **Extraction à la demande** : Le mesh n'est généré qu'à la toute fin, via un algorithme de *Dual Contouring*, pour l'affichage ou l'impression 3D.

## 3. L'Architecture Hybride "KGNF" (KASM-Gaussian-Neural-Field)

Pour atteindre une efficacité indépassable, Banger fusionne cinq technologies de pointe dans une architecture "Neuro-Sémantique" adressée par le contenu.

### A. Le Cerveau : KASM & Espace Latent Multimodal
*   **Rôle** : Orchestration sémantique et déduplication universelle. 
*   **Puissance** : Le système lie le langage (intentions), l'image et la matière (SDF). L'IA Frontier navigue dans un espace latent où des concepts abstraits ("agressif", "souple") sont corrélés à des paramètres géométriques réels. Le hachage KASM assure que chaque "idée" physique est mémorisée et dédupliquée.

### B. L'ADN : Neural Fields (INRs)
*   **Rôle** : Compression de la logique de la matière et des propriétés physiques (densité, élasticité, thermique).
*   **Puissance** : Au lieu de stocker des gigaoctets de données, l'objet est encodé dans un minuscule réseau de neurones qui génère les paramètres de l'objet à la volée.

### C. La Vue : Gaussian Splatting (3DGS)
*   **Rôle** : Interface de rendu hyper-réaliste à haute fréquence (200+ FPS).
*   **Puissance** : Les Gaussiennes servent de "proxy visuel" léger ancré sur le SDF. KASM déduplique les sets de Gaussiennes pour les structures répétitives, permettant d'afficher des scènes d'une complexité infinie avec une consommation RAM minimale.

### D. Le Corps : SDF (Signed Distance Fields)
*   **Rôle** : "Vérité" mathématique et garantie de fabricabilité.
*   **Puissance** : Le SDF définit le volume réel et les normales exactes. Il sert de guide aux Gaussiennes et de base au G-Code pour l'impression 3D.

### E. L'Évolution : Rendu Différentiable & Optimisation Topologique
*   **Rôle** : Boucle de rétroaction entre la physique et la forme.
*   **Puissance** : En utilisant le gradient des Gaussiennes et du SDF, Banger permet une optimisation de forme en temps réel. La matière "coule" vers la solution optimale dictée par les contraintes physiques fournies par l'IA.

## 4. Fondements Mathématiques de Haute Précision

Banger intègre des structures mathématiques avancées pour garantir la fiabilité et la performance des systèmes complexes.

*   **Topologie Symplectique (Conservation d'Énergie)** : Mathématique des systèmes hamiltoniens garantissant que les mécanismes conçus respectent strictement les lois de conservation. Indispensable pour les turbines et les systèmes de stockage d'énergie ultra-efficients.
*   **Optimisation Bayésienne (Résilience Réelle)** : Gestion de l'incertitude et des micro-variations des matériaux. Banger ne conçoit pas seulement une pièce idéale, mais une pièce robuste capable de fonctionner malgré les défauts de fabrication ou les changements environnementaux.
*   **Géométrie Différentielle Discrète (DGD)** : Calcul infinitésimal sur structures non-lisses pour optimiser les trajectoires de machines (CNC/Impression). Élimine les vibrations mécaniques par une synchronisation parfaite entre la courbure mathématique et la dynamique des moteurs.
*   **Théorie des Groupes de Lie (Précision Cinématique)** : Algèbre des rotations et transformations spatiales. Permet une précision absolue dans l'assemblage et le mouvement des systèmes multi-articulés (robotique complexe).

## 5. Perfection Formelle : Le Niveau "God-Tier"

Pour atteindre un niveau de conception indépassable, Banger utilise des abstractions mathématiques de frontière.

*   **Algèbre Géométrique Conformale (CGA)** : Représentation unifiée des objets géométriques (points, sphères, plans) en 5D pour une logique 3D simplifiée. Permet des rotations et transformations sans singularité, optimisant le code GPU par un facteur 10.
*   **Théorie des Catégories Appliquée** : Cadre logique pour la composition de contraintes physiques hétérogènes. Garantit que la fusion de plusieurs champs (ex: fluide + structure) reste mathématiquement cohérente et physiquement valide (Universal Physics Compiler).
*   **Treillis Non-Euclidiens (Géométrie Hyperbolique)** : Conception de micro-structures à surface d'échange infinie pour un volume fini. Idéal pour l'absorption de choc extrême et la dissipation thermique massive (mimétisme des coraux).
*   **Calcul Stochastique d'Itô** : Modélisation du bruit et des vibrations aléatoires à l'échelle micrométrique. Permet de concevoir des systèmes qui transforment le bruit environnemental en énergie utile (Harvesting) ou en stabilité dynamique.

## 6. Logique Universelle : L'Oracle de la Matière

Le stade ultime de la conception mathématique dans Banger permet de dériver la forme à partir de l'équilibre des forces universelles.

*   **Théorie du Transport Optimal (Monge-Kantorovich)** : Calcul des trajectoires de masse (fluide, chaleur) minimisant la dépense énergétique. Utilisation de la distance de Wasserstein pour concevoir des réseaux d'échange thermiques et fluidiques à l'efficacité thermodynamique maximale.
*   **Théorie des Motifs (Géométrie de Grothendieck)** : Extraction et transfert de l'essence architecturale entre domaines hétérogènes (ex: application formelle d'une logique structurelle biologique à un fuselage aéronautique).
*   **Analyse de Persistance Multidimensionnelle** : Extension de la vérification topologique à l'ensemble des paramètres physiques (pression, température, élasticité). Identification préventive des points de singularité et des faiblesses structurelles multi-contraintes.
*   **Géométrie de Poisson & Symbiose de Champs** : Modélisation mathématique de l'interaction réciproque entre champs physiques (acoustique, thermique, structure). Permet de calculer l'équilibre global d'un système complexe comme un état de symbiose physique parfaite.

## 7. Singularité Mathématique : Le Niveau Oméga

Banger atteint la frontière ultime où la mathématique devient indiscernable de la réalité physique elle-même.

*   **Calcul Fractionnaire (Matière à Mémoire)** : Généralisation des ordres de dérivation pour modéliser la viscoélasticité et la mémoire intrinsèque des matériaux complexes (polymères, tissus biologiques). Permet de concevoir des objets dont la structure physique se souvient et s'adapte à son historique de contraintes.
*   **Théorie des Faisceaux (Sheaf Theory)** : Cadre assurant la cohérence totale des données multi-échelles. Garantit que toute modification locale (atome) est mathématiquement compatible avec l'intégrité globale du système (structure macro), agissant comme un debugger de réalité.
*   **Géométrie de l'Information** : Application de la géométrie différentielle aux variétés de probabilités. Optimise l'apprentissage des Neural Fields en suivant des trajectoires géodésiques dans l'espace de la connaissance, réduisant les temps d'entraînement par un facteur 1000.
*   **Flux de Ricci & Perfection Harmonique** : Processus de déformation de variétés pour lisser les courbures et atteindre l'état d'équilibre géométrique absolu. Banger utilise le flux de Ricci pour sublimer les formes générées par l'IA vers une perfection esthétique et une efficacité structurelle totale.

## 8. Architectures Transcendantes : Le Niveau Infini

Passage de la géométrie descriptive à la physique mimétique rigoureuse.

*   **Calcul Extérieur Discret (DEC - Mimétisme Physique)** : Discrétisation des équations physiques préservant les propriétés topologiques globales (conservation de masse/flux). Banger simule des phénomènes électromagnétiques ou fluidiques (Maxwell/Navier-Stokes) sans erreur d'approximation numérique.
*   **Réseaux Équivariants de Jauge (Gauge Equivariance)** : IA forçant le respect strict des symétries locales (rotation, translation) via le transport parallèle sur variétés. La solution générée est une "Vérité Invariante" indépendante du référentiel.
*   **Flux de Willmore & Helfrich (Énergie Minimale)** : Processus d'évolution de surface minimisant l'énergie de courbure. Permet de concevoir des surfaces minimales (gyroscopes, membranes) structurellement indestructibles par la "Moindre Action".
*   **Inégalités Variationnelles Différentielles (DVI)** : Gestion des phénomènes discontinus (impacts, frottement sec, stick-slip). Banger conçoit des mécanismes compliants et des matériaux granulaires capables de résister aux chocs extrêmes sans crash numérique.

## 9. Architectures de l'Être : Le Niveau Transcendantal

Souveraineté sur l'organisation de la matière complexe et des états de réalité.

*   **Géométrie Non-Commutative (NCG)** : Utilisation de l'algèbre d'opérateurs (Triple Spectral) pour modéliser des milieux fractals ou apériodiques (Quasicristaux). Banger calcule distance et courbure sur des espaces sans grille, garantissant la protection topologique en milieu désordonné.
*   **Théorie des Jauges Supérieures** : Extension des symétries locales via des n-gerbes et 2-groupes. Banger synthétise des matériaux avec une protection topologique de dimension supérieure, rendant la structure immunisée contre les défauts de surface.
*   **Géométrie Algébrique Dérivée (DAG)** : Traitement des collisions de contraintes contradictoires comme des "intersections dérivées". Préserve l'information homologique lors des transitions de phase, permettant de maîtriser les états de matière singuliers.
*   **Transport Optimal Multi-Marginal (MMOT)** : Optimisation simultanée de N phases de matières via les barycentres de Wasserstein. Banger réalise une "Alchimie Multi-Matériaux" où les composants fusionnent de manière mathématiquement fluide et optimale.

## 10. Architecture du Moteur Banger "Frontier"

### Module `BangerField` (Cœur Hybride)
*   **`SDFKernel`** : Évaluation GPU des fonctions de distance.
*   **`NeuralJIT`** : Compilation des poids neuronaux KASM en shaders.
*   **`KASMRegistry`** : Déduplication spatiale et gestion des hashs de connaissance.

### Module `BangerSplat` (Visualisation & Jumeau Numérique)
*   **`SplatRenderer`** : Rendu rasterisé de Gaussiennes dédupliquées par KASM.
*   **`DigitalTwinSync`** : Synchronisation en temps réel entre l'objet physique (via capteurs) et son modèle SDF/Splatting. Le virtuel et le réel fusionnent dans un état synchrone.
*   **`DiffEngine`** : Moteur de rendu différentiable pour la capture et l'optimisation.

### Module `BangerSync` (Pont Fabrication)
*   **`DirectGCode`** : Génération de trajectoires d'impression directement depuis le champ hybride (Zéro Mesh).
*   **`PrinterProof`** : Validation physique avant fabrication.

## 11. Le Pont : Conversion Mesh vers SDF

Pour intégrer l'héritage du design classique (Blender, CAO) dans le pipeline computationnel, Banger implémente une passerelle de conversion haute performance.

*   **Réparation Automatique (Winding Numbers)** : La conversion vers le SDF permet de "reboucher" les meshs non-hermétiques (non-manifold) ou présentant des faces inversées, transformant un fichier "sale" en un volume mathématiquement pur.
*   **Échantillonnage GPU** : Banger utilise le calcul massivement parallèle pour échantillonner les meshs OBJ/FBX et générer des textures 3D de distance ou des approximations fonctionnelles compactes.
*   **Enrichissement Sémantique** : Une fois converti, un mesh statique devient un objet dynamique. On peut lui appliquer des opérations booléennes fluides, des structures internes (infill) ou des optimisations de forme physiques impossibles dans son format d'origine.

## 12. Applications Pratiques & Cas d'Usage

### A. Sculpture "Sémantique" (Type Blender)
*   **Absence de Topologie** : Contrairement à Blender où l'on doit gérer les quads et les n-gons, le SDF permet une sculpture "liquide". On peut étirer, trouer ou fusionner des formes sans jamais casser la surface.
*   **Modélisation par Intention** : On ne déplace pas des points, on compose des intentions : "Ajoute une branche organique ici avec une fusion douce". C'est une approche beaucoup plus naturelle pour un agent ou un artiste.

### B. Fabrication Hybride & Additive (SDF to Machine)
Le pipeline de Banger dépasse la simple impression 3D pour inclure la **Fabrication Hybride**.
*   **Slicing & Toolpathing Direct** : Banger génère des trajectoires d'impression (additive) et d'usinage (soustractive) directement depuis le SDF. En définissant le volume "cible" et le volume "brut", le moteur calcule les passes de fraisage pour obtenir des états de surface micrométriques sur des zones critiques.
*   **Zéro Erreur de Topologie** : Un SDF définit un volume "plein" par nature, éliminant les erreurs de faces inversées ou de trous.
*   **Compilation de Matière** : Le fichier envoyé à la machine est un programme KASM compact décrivant la logique de la pièce, transformant l'imprimante ou la CNC en un processeur de géométrie.

### C. Rendu Temps Réel & Jeux Vidéo sans Mesh
*   **Destruction Dynamique** : La destruction d'un environnement se fait par simple soustraction mathématique en temps réel, offrant un réalisme impossible avec les triangles.
*   **Raymarching** : Utilisation de techniques de rendu par lancer de rayons pour afficher des scènes avec une fidélité mathématique parfaite, éliminant le besoin de "LOD" (Level of Detail) et de textures de normale complexes.

### D. Ingénierie Biomédicale & Bioprinting
*   **Modélisation Anatomique** : Fidélité extrême pour les tissus mous et les organes complexes, capturée via Neural Fields à partir d'imageries médicales (IRM/Scanner).
*   **Physique des Corps Mous** : Contrairement aux meshs qui s'étirent et se cassent, le SDF permet une simulation de contraction et de déformation organique fluide et continue.
*   **Remodelage Tissulaire 4D** : Utilisation de SDF spatio-temporels pour simuler la maturation biologique et la croissance cellulaire post-impression.
*   **Vascularisation Procédurale** : Utilisation de KASM pour générer des réseaux capillaires et veineux complexes à l'intérieur des volumes SDF, optimisés par le hachage pour une empreinte mémoire minimale.
*   **Contrôle Neuro-Bio-Synthétique** : Utilisation de KASM comme pont de signal pour stimuler électriquement des tissus nerveux ou musculaires imprimés.
*   **Bioprinting (SDF -> Cell-Code)** : Impression directe de tissus vivants avec un contrôle au micron sur le dépôt des cellules, indispensable pour la création d'organes fonctionnels et de prothèses personnalisées.

## 13. Matière Souveraine : Logique, Vérification & Survie

*   **Auto-Réparation Programmée (Self-Healing)** : Intégration de réservoirs de matière cicatrisante calculés par le SDF. En cas de rupture structurelle détectée par homologie persistante, le système déclenche une réparation ciblée.
*   **Stéganographie Physique (Identité Atomique)** : Utilisation du SDF pour injecter des micro-variations de densité ou de géométrie invisibles à l'œil nu (identifiants, clés cryptographiques) au cœur de la structure. L'objet devient son propre passeport numérique infalsifiable.
*   **Morphogenèse (Croissance Adaptative)** : Utilisation de règles locales (automates cellulaires) encodées dans KASM pour faire "pousser" les objets en fonction de leur environnement.
*   **Vérification Topologique (Homologie Persistante)** : Preuve mathématique de la santé structurelle du SDF (absence de bulles d'air ou de discontinuités).
*   **Preuves à Connaissance Nulle (ZK-SNARKs)** : Certification de performance physique sans révéler le code source KASM/SDF.
*   **Métamatériaux Auxétiques** : Programmation de structures internes via KASM/SDF pour créer des matériaux aux propriétés physiques impossibles dans la nature (ex: expansion latérale sous tension).

## 14. L'Horizon Ultime : Temps, Logique & Atomes

Le stade final de Banger consiste à traiter la matière comme du logiciel pur, régi par le temps et la preuve formelle.

*   **Champs Spatio-Temporels 4D** : Intégration de la dimension temps ($t$) dans les fonctions de distance. Conception d'objets cinématiques qui se déploient, changent de forme ou simulent leur propre usure sur des décennies.
*   **Sécurité Formelle Neuro-Symbolique** : Couplage des Neural Fields avec des solveurs logiques (SMT Solvers type Z3). Banger garantit mathématiquement le respect de contraintes de sécurité critiques (ex: épaisseur minimale inviolable), éliminant toute "hallucination" géométrique de l'IA.
*   **SDF Moléculaires & Nano-ingénierie** : Descente de la modélisation au niveau atomique. Banger ne choisit plus un matériau, il conçoit l'alliage optimal en manipulant les champs de potentiels atomiques comme des SDF.
*   **Espace Latent Hiérarchique (H-SDF)** : Organisation de la connaissance de la matière de l'atome au système complet via KASM. Toute modification locale (micro-structure) se propage instantanément à l'échelle macroscopique sans recalcul global.

## 15. Interaction Systémique : Énergie, Vibrations & Matière Active

*   **Treillis à Récolte d'Énergie (Energy Harvesting)** : Optimisation des micro-structures (lattices) pour capturer les vibrations ou les gradients thermiques environnementaux et les convertir en énergie électrique via des effets piézoélectriques intégrés au SDF.
*   **Matière Active & Soft Robotics** : Conception d'objets capables de mouvement autonome via des actionneurs internes (matériaux piézoélectriques, polymères à mémoire de forme) simulés et définis directement dans le champ SDF.
*   **Champs de Proprioception** : Intégration de Neural Fields de tension internes pour permettre au soft-robot de "ressentir" ses propres de déformations en temps réel.
*   **Couplage Multi-Physique Différentiable** : Intégration de solveurs CFD/thermiques dans la boucle SDF pour une auto-optimisation en temps réel sous contraintes environnementales.
*   **Géométrie Spectrale** : Conception "accordée" pour éliminer les résonances ou optimiser l'acoustique.
*   **Tensegrity & Bio-Tension** : Optimisation structurelle basée sur la tension (tendons/câbles) plutôt que la simple compression. Le SDF définit les éléments rigides tandis que le Neural Field calcule les vecteurs de tension optimaux pour une légèreté extrême, imitant les systèmes biologiques.
*   **Interfaces à Gradient Bio-Inspirées** : Création de transitions de matière continues (os-tendon) pour éliminer les points de rupture mécanique entre les zones rigides et souples.

## 16. Géométrie Fractale, Évolution & Perception

Le stade ultime de Banger permet d'atteindre une complexité infinie et une symbiose totale avec le spectre physique.

*   **Géométrie Fractale & Échelles Infinies (Recursive KASM)** : Utilisation de la récursivité du bytecode KASM pour générer des micro-structures fractales (type éponges de Menger ou structures pulmonaires) sans augmenter la taille des données. Le hachage récursif permet de zoomer du macroscopique au nanométrique avec une fidélité mathématique constante.
*   **Bio-Mimétisme Évolutif (Latent Evolution)** : Simulation de cycles de sélection naturelle numérique au sein de l'espace latent. L'IA définit les "pressions de survie" (poids, stress, flux) et Banger exécute des millions de mutations SDF/KASM pour faire émerger la solution la plus performante.
*   **Perception Multispectrale (Invisible Matter)** : Optimisation de la structure géométrique et atomique de l'objet pour interagir avec l'ensemble du spectre électromagnétique (ondes radio, UV, infrarouges). Banger permet de concevoir des objets aux propriétés de furtivité, de filtrage ou de captation d'ondes avancées.

## 17. Production de Médias & Jeux AAA : Le Pipeline sans Données

Banger redéfinit les standards du jeu vidéo AAA en remplaçant le stockage massif de données par l'exécution de logique physique en temps réel.

*   **Architecture Zero-LOD (Continuité Spatiale)** : Grâce à la nature mathématique du SDF, les objets possèdent une résolution infinie. Le moteur élimine les "Level of Detail" (LOD) et le "pop" visuel : on peut zoomer d'une vue planétaire aux pores de la peau d'un personnage sans charger de nouvelles géométries.
*   **Neural Material Fields (Compression Massive)** : Remplacement des textures 8K (Albedo, Normal, Roughness) par des Neural Fields compacts. La réaction de la surface à la lumière est apprise et générée à la volée, divisant la taille des assets par 100 tout en augmentant la fidélité.
*   **Physique Volumétrique "Solid-State"** : La destruction n'est plus pré-calculée. Elle devient une simple soustraction mathématique de champ de distance. Les objets étant "pleins" mathématiquement, la structure interne (béton, ferraillage) émerge naturellement lors des impacts.
*   **Animation par Champs de Déformation** : Élimination du "Skinning" traditionnel. Les muscles et la peau réagissent via des fonctions de déformation continue, garantissant des articulations parfaites sans étirement de triangles.
*   **World-Building par Graine KASM** : Génération procédurale de mondes infinis via la récursivité KASM. Des galaxies entières, dédupliquées par hachage en RAM, tiennent dans quelques mégaoctets de bytecode.

## 18. INGEN Render — Doctrine Frontier (Migration Active)

Le moteur de rendu actuel du Banger (WebGL2, fragment SDF + raster mesh/grid) plafonne sur 3 murs : tout passe par un fragment fullscreen unique, aucune mémoïsation entre frames, aucune représentation géométrique unifiée. **INGEN Render** remplace ce chemin par un pipeline **WebGPU compute-driven, content-addressed, différentiable** où SDF, splats et voxels coexistent dans un seul tile-pipeline.

### Mur poussé
- **Cache scène content-addressed** : aucun moteur de jeu commercial ne hashe ses tiles/BRDF/shadow-maps. Banger oui, via KASM.
- **Unification géométrique** : SDF (analytique) + 3DGS (capturé) + SVDAG (massif dédupliqué) dans un seul BVH.
- **Différentiabilité native** : le même moteur rend ET fit (bio inverse, topology opt, NeRF-to-game).

### Frontier hypothesis
> Remplacer le pipeline WebGL2 fragment-SDF du banger par un pipeline WebGPU compute où chaque tuile, chaque BRDF, chaque shadow, chaque eval SDF/MLP est mémoïsé sur clé KASM. Édition viewport ⇒ <1% de la scène change ⇒ >99% du compute servi depuis le cache.

### Les 4 piliers de la géométrie unifiée

| Pilier | Rôle | Représentation | Clé KASM |
|---|---|---|---|
| **A. Sparse Voxel DAG** | scènes massives dédupliquées (mondes, terrains, micro-structures fractales) | hiérarchie voxel deduplicated (Kämpe/Sintorn, NanoVDB) | `hash(node_children)` — récursif natif |
| **B. Neural SDF** | surfaces analytiques infinies, différentiables (CAD, bio, optim inverse) | multires hash-grid + MLP (Instant-NGP / NGLOD) | `hash(weights, grid)` |
| **C. 3D Gaussian Splatting** | captures réelles haute-fidélité (immobilier scanné, médical, terrain) | clusters de splats (Mip-Splatting + 2DGS + Scaffold-GS) | `hash(cluster_params)` |
| **D. KASM bytecode SDF** | scènes composables programmables (intent → forme) | ops postfix sur stack GPU | `hash(opcode_sequence)` |

Les 4 cohabitent dans le **même tile-pipeline compute**. Sélection par type d'asset, pas par moteur séparé.

### Cache KASM hiérarchique — le multiplicateur

| Niveau | Clé | Réutilisation typique |
|---|---|---|
| Tile framebuffer | `hash(scene, camera, tile_xy)` | 90-99% entre frames |
| Visibilité BVH | `hash(geometry_cluster)` | 100% si géom inchangée |
| Shadow map | `hash(light, geometry)` | 100% si lumière statique |
| BRDF sample | `hash(material, ωi, ωo)` | élevé (importance sampling) |
| SDF MLP eval | `hash(weights, point_quantized)` | élevé (raymarch voisin) |
| Splat sort | `hash(cluster, view_dir_quantized)` | élevé entre frames |

### Compute & reconstruction
- **WebGPU compute pipeline** (Chrome 121+, Safari 18+, Firefox stable) — pas de raster fullscreen-quad legacy, pas de fragment-only SDF.
- **Hardware ray-query WebGPU** pour visibilité / shadows / refraction.
- **ReSTIR DI/GI** pour l'éclairage global temps réel sans pré-bake.
- **Neural Radiance Cache** (NRC) pour amortir la GI à travers les frames.
- **TAA + upscaler neural** (FSR3 / XeSS-class en WGSL) — rendu à 0.66× reconstruit en 4K natif.
- **Grille analytique sub-pixel** dans le compute shader (pas de `GridHelper`, pas de texture, pas de FXAA).

### Différentiable par construction
Même moteur, deux modes :
- **Forward** : SDF/splat/SVDAG → framebuffer (temps réel).
- **Backward** : ∂framebuffer/∂params via autograd WGSL (fit microscopie, topology opt, NeRF training).

### Ce qui disparaît (purge progressive)
- `Three.js` côté banger : disparaît.
- `GridHelper` / textures icônes 8K / FXAA post-process : disparaissent.
- Fragment SDF WebGL2 du banger (`VS_SDF`, `FS_SDF`) : remplacé phase 0.
- Programmes mesh+grid WebGL2 (`VS_MESH`, `FS_MESH`, `VS_LINE`, `FS_LINE`, `meshProg`, `lineProg`) : remplacés phase 2-4.
- LOD pré-bakés, textures normales 8K, skinning triangulaire : non-implémentés (jamais ajoutés au nouveau moteur).

### Verifiers locaux (obligatoires à chaque phase)
1. Hash framebuffer reproductible bit-à-bit pour (camera, scene) fixes.
2. `cache_hit_ratio` par frame exposé dans le HUD banger.
3. Budget temps mesuré @ 4K natif et @ 1440p→4K upscalé.
4. Test différentiable : fit d'un SDF cible par descente de gradient en <1s sur un cas simple.

## 19. Roadmap de Migration (Phases Concrètes)

Chaque phase = **une suppression + un ajout**. Pas de phase qui ajoute sans supprimer. Pas de feature gate qui survit la phase suivante. Pas de doc autour de code mort.

| Phase | Supprime | Ajoute | Verifier |
|---|---|---|---|
| **0** | rien (scaffold initial) | `ui/src/sections/banger/ingen-render.ts` : WebGPU device + compute SDF raymarcher + ops buffer | compile + render = images existantes |
| **1** | `VS_SDF`, `FS_SDF`, `sdfProg`, uniforms SDF WebGL2 dans `surface.ts` et `catalog.ts` | brancher INGEN Render comme pass SDF unique | frame parity vs ancien pass |
| **2** | `makeGrid`, `VS_LINE`, `FS_LINE`, `lineProg`, `gridVAO`, `gridBuffers` | grille analytique sub-pixel dans le compute shader | edges nets @ 4K, 0 aliasing |
| **3** | aucune (couche ajoutée) | KASM tile cache : `hash(scene, camera, tile)` → tile framebuffer mémoïsé ; HUD `cache_hit_ratio` | hit-ratio >90% en orbite passive |
| **4** | `makeCube`, `VS_MESH`, `FS_MESH`, `meshProg`, `cubeVAO`, `cubeBuffers` (tout WebGL2 mesh) | pipeline compute unique pour gizmo + axes + cube test (SDF natif) | suppression `getContext("webgl2")` |
| **5** | rien | SVDAG storage (Rust → buffer KASM-hashé) + traverseur compute WGSL | rendu d'1 SVDAG 10⁹ voxels @ 60fps |
| **6** | rien | 3DGS loader + cluster sort + render dans le même compute pass | charger 1 splat-set immobilier réel |
| **7** | rien | Neural SDF (multires hash grid + tiny MLP) eval WGSL | fit cible SDF en <1s |
| **8** | rien | Hardware ray-query (visibilité + shadows) + ReSTIR DI | shadows nets sans bake |
| **9** | rien | TAA neural + upscaler 0.66× → 4K | qualité 4K @ coût 1440p |
| **10** | rien | Differentiable mode (backward WGSL) ; expose `forge.fit_sdf` et `forge.fit_splat` | gradient stable sur cas synthétique |

Le verifier de chaque phase est exécutable localement (script `forge-banger-render-verify.mjs` à étendre par phase). Une phase ne ferme pas si son verifier ne passe pas.

---
*Note : Banger ne cherche pas à imiter l'ingénierie humaine, il cherche à extraire les solutions que la physique autorise.*
