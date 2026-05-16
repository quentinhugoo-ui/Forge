declare global {
  interface Window {
    ForgeWebExplorerConfig?: unknown;
  }
}

(function () {
  "use strict";

  const freeze = <T>(value: T): Readonly<T> => Object.freeze(value);

  const config = freeze({
    storageKeys: freeze({
      realEstateMode: "forge.realEstate.mode.v1",
    }),
    dropzone: freeze({
      default: freeze({
        title: "Drop any file",
        sub: "Heavy compute in any domain — data, code, medical imaging, genomics, anything. The LLM stays out of files and math, saving massive tokens.",
        aria: "Upload files for Forge compute",
      }),
      realEstate: freeze({
        title: "Dépose n'importe quel fichier immobilier",
        sub: "Calcul lourd pour l'immobilier — données, code, photos de biens, cadastre, DPE, CRM, n'importe quoi. Le LLM reste hors des fichiers et des calculs, économisant énormément de tokens.",
        aria: "Importer des fichiers agence immobilière",
      }),
    }),
    webExplorerSuggestions: freeze({
      default: freeze([
        "filtre moi tous les mails de Sophie et retrouve celui ou elle parle du voyage des prochaines vacances",
        "reconstruis la chronologie complete de mes reservations voyage a partir des confirmations, factures et changements d'horaires",
        "retrouve toutes les conversations avec les recruteurs depuis janvier et classe les opportunites par urgence, salaire et prochaine action",
        "analyse trois ans de factures recues par mail et regroupe les abonnements qui se renouvellent automatiquement avec leur cout annuel",
        "trouve les pieces jointes contractuelles importantes, compare les versions et signale celles qui semblent remplacees ou obsoletes",
        "identifie tous les mails restes sans reponse qui peuvent bloquer un projet et prepare une liste priorisee de relances courtes",
        "resume les longs threads avec mon equipe, extrait les decisions prises et retrouve les fichiers ou liens mentionnes dans chaque decision",
        "cherche les mails de support client lies a des bugs recurrents, regroupe les symptomes et propose les tickets a creer",
        "retrouve les preuves d'achat, garanties et numeros de serie pour mes appareils, puis classe-les par date d'expiration de garantie",
        "audite mes newsletters et notifications pour detecter les sources inutiles, les doublons et les envois qui meritent un filtre automatique",
      ]),
      realEstate: freeze([
        "scanne une ville entière avec DVF, DPE, permis, annonces et Google Maps; sors les zones où ouvrir une agence, recruter ou concentrer la prospection",
        "croise mon CRM, les appels, les visites et les relances pour prédire les 50 prochaines actions qui peuvent créer mandat, offre ou rendez-vous cette semaine",
        "ingère 50 000 mutations DVF, 12 000 DPE et nos anciennes estimations; segmente vendeurs, acquéreurs et investisseurs par intention probable",
        "analyse les demandes acquéreurs, budgets, refus de visites et critères récurrents; détecte les biens à rentrer pour combler la demande non servie",
        "construis un répondeur IA ElevenLabs pour qualifier les appels entrants, détecter urgence, budget, adresse, projet vendeur ou acquéreur et créer la fiche CRM",
        "audite toutes les conversations CRM et emails agence; détecte les prospects chauds oubliés, les promesses non tenues et les relances à fort ROI",
        "compare nos annonces à celles des concurrents: photos, prix, texte, délais, baisses, exclusivités, avis clients; liste leurs forces, faiblesses et angles d'attaque",
        "veille chaque matin sur les actualités locales: écoles, commerces, transports, urbanisme, sécurité, entreprises; transforme-les en arguments de prospection",
        "cartographie les propriétaires bailleurs, biens énergivores, vacances locatives et tensions locatives pour proposer gestion, vente ou rénovation",
        "surveille les permis de construire, changements de PLU, projets publics et transactions voisines; alerte quand une rue devient stratégiquement intéressante",
        "génère un chatbot agence qui répond aux acquéreurs, qualifie les vendeurs, propose des créneaux de visite et escalade au négociateur avec résumé CRM",
        "analyse les avis Google, appels perdus et messages entrants; détecte les objections récurrentes et propose un plan qualité pour augmenter le taux de conversion",
        "crée un score de recrutement négociateur par secteur: vivier local, agences concurrentes, volumes de transactions, potentiel mandat et charge commerciale",
        "prépare un plan d'ouverture sur trois villes: potentiel vendeur, demande acquéreur, concurrence, loyers commerciaux, recrutement et retour sur investissement",
        "simule l'impact de 5 stratégies commerciales sur 12 mois: pige, estimation offerte, mandat exclusif, gestion locative, partenariat notaire et budget pub",
        "classe les mandats actuels par probabilité de vente, risque de perte, action corrective, repositionnement prix, argumentaire et prochain appel",
        "détecte les acquéreurs dormants dans le CRM qui correspondent aux nouveaux biens rentrés, puis prépare les messages personnalisés et les preuves de fit",
        "construis une veille concurrence hebdomadaire: parts de voix, nouvelles annonces, baisses de prix, recrutements, avis, quartiers dominés et opportunités faibles",
        "transforme les données de visites en modèle: objections, pièces bloquantes, prix psychologique, probabilité d'offre et recommandations pour le vendeur",
        "crée un cockpit dirigeant: pipeline mandats, pipeline acquéreurs, productivité négociateurs, appels perdus, sources de leads, marge et signaux à traiter",
      ]),
    }),
    chatPlaceholderIdeas: freeze({
      realEstate: freeze([
        "Scanne une ville entière avec DVF, DPE, annonces et Google Maps pour détecter les zones à fort potentiel agence",
        "Croise CRM, appels, visites et relances pour prioriser les 50 actions commerciales les plus rentables cette semaine",
        "Analyse les demandes acquéreurs et budgets dormants pour recommander les biens à rentrer en priorité",
        "Construis un répondeur IA ElevenLabs qui qualifie vendeur, acquéreur, urgence, budget, adresse et prochaine action CRM",
        "Compare nos annonces avec celles des concurrents et liste leurs forces, faiblesses, baisses de prix et angles d'attaque",
        "Veille chaque matin sur urbanisme, écoles, transports, commerces et actualité locale pour générer des arguments terrain",
        "Score les mandats actuels par probabilité de vente, risque de perte, repositionnement prix et prochain appel utile",
        "Détecte les acquéreurs dormants qui matchent les nouveaux biens et prépare les messages personnalisés",
        "Crée un cockpit dirigeant: pipeline mandats, pipeline acquéreurs, appels perdus, sources de leads et marge",
        "Simule 5 stratégies commerciales sur 12 mois: pige, estimation offerte, exclusivité, gestion locative et partenariats",
        "Cartographie les propriétaires bailleurs, biens énergivores et vacances locatives pour proposer vente ou gestion",
        "Classe trois villes pour une ouverture d'agence selon concurrence, potentiel vendeur, demande acquéreur et recrutement",
      ]),
    }),
    toolSections: freeze(["google", "trading", "extract", "datasets", "score", "act", "create"]),
    realEstateToolLabels: freeze({
      google: "Google",
      extract: "Extraire",
      datasets: "Données",
      score: "Scoring",
      act: "Agir",
      create: "Créer",
    }),
    realEstateToolCopy: freeze({
      google_connect_secure: freeze({ label: "/connexion", summary: "Connexion sécurisée au compte Google de l'agence, sans exposer les identifiants au LLM." }),
      gmail_filter: freeze({ label: "/gmail_agence", summary: "Retrouver les échanges vendeurs, estimations, relances et pièces utiles dans la boîte agence." }),
      web_extract: freeze({ label: "/extraire_agence", summary: "Extraire signaux agence depuis page visible, Google Maps, DVF, DPE, annonces, CRM, concurrence ou actualité locale." }),
      extract_results: freeze({ label: "/resultats_locaux", summary: "Structurer résultats, fiches, liens, adresses, contacts, preuves et sources locales." }),
      extract_prices: freeze({ label: "/prix_marche", summary: "Extraire prix, baisses, délais, frais, surfaces, demandes acquéreurs et indices de tension marché." }),
      extract_forms: freeze({ label: "/formulaires", summary: "Cartographier formulaires, champs et étapes avant qualification, prise de contact ou action CRM." }),
      proof_pack: freeze({ label: "/preuves", summary: "Capturer URL, horodatage, hash DOM, références visuelles et preuves minimales." }),
      crawl_task: freeze({ label: "/crawler_zone", summary: "Explorer une zone de façon bornée et retourner des lignes structurées par adresse ou source." }),
      dedupe_sources: freeze({ label: "/dedoublonner", summary: "Dédupliquer propriétaires, biens, pages, annonces et signaux quasi identiques." }),
      source_trust: freeze({ label: "/fiabilite", summary: "Scorer fraîcheur, autorité, contradictions et risques des sources utilisées." }),
      rank_options: freeze({ label: "/prioriser_actions", summary: "Classer prospects, mandats, acquéreurs, relances, villes ou recrutements avec preuves et confiance." }),
      compare_offers: freeze({ label: "/comparer_zones", summary: "Comparer rues, micro-quartiers, concurrents et typologies selon prix, liquidité, demande et potentiel." }),
      fee_detector: freeze({ label: "/risques", summary: "Détecter frais, pièges, incohérences, sources fragiles et points à valider humainement." }),
      itinerary_score: freeze({ label: "/score_zone", summary: "Scorer une zone selon accessibilité, écoles, services, flux, concurrence et attractivité locale." }),
      web_act: freeze({ label: "/plan_action", summary: "Préparer un plan d'action commercial, CRM, répondeur, chatbot ou veille avec validations humaines." }),
      fill_form: freeze({ label: "/preparer_formulaire", summary: "Préremplir les champs connus depuis des données validées, sans soumettre d'étape sensible." }),
      checkout_draft: freeze({ label: "/brouillon_demande", summary: "Préparer une demande, prise de contact ou réservation, puis arrêter avant engagement." }),
      human_confirm: freeze({ label: "/validation_humaine", summary: "Exiger une validation explicite avant connexion, envoi, dépôt, achat ou action irréversible." }),
      create: freeze({ label: "/creer", summary: "Créer une entité Atlas réutilisable pour une zone, campagne ou segment vendeur." }),
      program_create: freeze({ label: "/programme", summary: "Créer ou lancer un programme de calcul réutilisable pour scoring et extraction agence." }),
      metric_create: freeze({ label: "/metrique", summary: "Créer une métrique d'extraction, de scoring ou de qualité de source." }),
      visualprogram_create: freeze({ label: "/carte_visuelle", summary: "Créer une Lens visuelle sur les vendeurs, zones, preuves et scores." }),
      geo_create: freeze({ label: "/geo", summary: "Créer un GeoNode pour une ville, quartier, rue ou adresse détectée." }),
      minigeo_create: freeze({ label: "/micro_geo", summary: "Créer un MiniGeoNode pour un immeuble, commerce, école ou point local précis." }),
    }),
    actionCopy: freeze({
      default: freeze({
        extract: freeze({ label: "Extract", title: "Extract", aria: "Extract web data" }),
        score: freeze({ label: "Score", title: "Score", aria: "Score web options" }),
        act: freeze({ label: "Act", title: "Act", aria: "Prepare web actions" }),
      }),
      realEstate: freeze({
        extract: freeze({ label: "Extraire", title: "Extraire les signaux agence", aria: "Extraire les signaux agence" }),
        score: freeze({ label: "Scorer", title: "Scorer les priorités agence", aria: "Scorer les priorités agence" }),
        act: freeze({ label: "Agir", title: "Préparer les actions agence", aria: "Préparer les actions agence" }),
      }),
    }),
  });

  window.ForgeWebExplorerConfig = config;
})();

export {};
