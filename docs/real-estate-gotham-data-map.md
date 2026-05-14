# Forge Agence Immo - Data Map Verrouillée

Ce document verrouille la cartographie des outils du mode Agence Immo et définit les données à collecter pour faire évoluer Forge vers un système d'intelligence immobilière de type Gotham: graphe d'entités, signaux faibles, preuve de source, calculs lourds KASM, mémoire sémantique et actions agentiques.

Principes:

- API autorisées, open data, données agence et fichiers locaux d'abord.
- Web public uniquement dans les limites robots, rate limits, conditions d'utilisation et sans contournement.
- Pas de login forcé, pas de CAPTCHA, pas de paywall bypass.
- Chaque information doit porter une source, une date, un hash, un niveau de confiance et une durée de fraîcheur.
- Le LLM ne doit pas ingérer les fichiers bruts: Forge produit des intel packs compacts, vérifiables et actionnables.

## Groupes De Datas À Attaquer

Cette section regroupe les données par familles opérationnelles. C'est la structure de travail pour construire les connectors, harvesters, parsers, tables locales, graph entities, métriques KASM et intel packs.

Priorités:

- P0: méga harvester public brut, indispensable avant toute donnée agence.
- P1: sources publiques/API qui enrichissent le socle géographique, marché et veille.
- P2: données publiques éloignées de l'immo mais puissantes une fois croisées.
- P3: signaux faibles avancés, utiles quand le graphe local est déjà fiable.
- INT: données internes agence à brancher plus tard, après le socle public brut.

## Phase 0 - Mega Harvester Public Et Brut

Avant de brancher les biens de l'agence, Forge doit construire une armée de collectors publics/API. Le but est de remplir un lac local de données brutes, sourcées, hashées et datées, dans énormément de domaines. Les données agence viendront ensuite se connecter à ce socle.

Artefacts opérationnels:

- `examples/forge_tauri_ui/source-registry/real-estate-public-sources.json`: registre seed des sources publiques officielles, URLs, formats attendus et parsers.
- `examples/forge_tauri_ui/source-registry/real-estate-parser-adapters.json`: registre des adapters SOTA visés: Magika, DuckDB, Tika, Docling, avec fallback natif hashé.
- `examples/forge_tauri_ui/scripts/real-estate-source-pipeline.mjs`: orchestrateur officiel audit/discovery/download/parser, avec `pipeline_run.json`, ledger et proof hash global.
- `examples/forge_tauri_ui/scripts/real-estate-source-audit.mjs`: auditeur sans dépendance qui valide le registre en mode plan-only ou teste les URLs en mode `--live`.
- `examples/forge_tauri_ui/scripts/real-estate-source-discovery.mjs`: collector source discovery qui extrait les ressources réelles exploitables et écrit `source_manifest.jsonl` dans le store Forge.
- `examples/forge_tauri_ui/scripts/real-estate-raw-downloader.mjs`: downloader brut content-addressed qui lit `source_manifest.jsonl`, télécharge les ressources filtrées, écrit les fichiers par hash et journalise `raw_downloads.jsonl`.
- `examples/forge_tauri_ui/scripts/real-estate-parser-router.mjs`: routeur de parsing qui lit `raw_downloads.jsonl`, choisit le parser par format et produit `normalized_events.jsonl`.

Principe d'ordre:

1. Collecter les données publiques brutes.
2. Normaliser adresses, parcelles, bâtiments, communes, zones et dates.
3. Créer des snapshots temporels par source.
4. Calculer des métriques KASM par zone, rue, bâtiment, marché, risque, demande et concurrence.
5. Seulement ensuite brancher CRM, biens agence, Google Workspace et dossiers internes.

### Armée De Collectors Publics Prioritaires

| Collector brut | Priorité | Sources libres/API à viser | Données brutes à récupérer | Pourquoi c'est stratégique |
|---|---:|---|---|---|
| `address_ban_collector` | P0 | API Adresse / BAN | Adresse normalisée, score, coordonnées, code INSEE, voie, commune | Clé de jointure de tout le système. Sans adresse fiable, pas de graphe solide. |
| `building_rnb_collector` | P0 | API RNB, exports RNB | ID bâtiment, emprise, statut, diff depuis date, tuiles/vector tiles si utile | Identifiant bâtiment stable pour croiser DPE, cadastre, risques, permis, biens. |
| `dvf_plus_collector` | P0 | API Données foncières, DVF+ open-data, exports data.gouv | Mutations, prix, date, type bien, commune, géolocalisation, indicateurs de prix | Base du moteur estimation, liquidité, anomalies, micro-marchés. |
| `cadastre_collector` | P0 | API Carto Cadastre, WFS/WMS IGN | Parcelles, divisions, centroïdes, géométries, commune, section, numéro | Relie mutation, bâtiment, terrain, potentiel division, contraintes foncières. |
| `dpe_ademe_collector` | P0 | API DPE logements, open data ADEME | Numéro DPE, date, classe énergie, GES, surface, bâti, chauffage, ECS, ventilation | Rénovation, passoires, décotes, coût d'usage, signaux vendeur. |
| `georisques_collector` | P0 | API Géorisques, bases Géorisques | Inondation, argile, radon, mouvements terrain, cavités, ICPE, SIS, CATNAT, PPR | Risque prix, assurabilité, objections, diagnostics, conformité. |
| `urbanisme_gpu_collector` | P0 | API Carto GPU, Géoportail Urbanisme | PLU, zonages, prescriptions, servitudes, documents d'urbanisme | Potentiel constructible, restrictions, changements de valeur. |
| `sitadel_permits_collector` | P0 | Sitadel data.gouv / SDES, PermisAPI si choix produit externe | Permis construire, démolir, aménager, déclarations préalables, dates, surfaces, logements | Détecte transformation de quartier, concurrence future, foncier actif. |
| `sirene_business_collector` | P1 | API Sirene / Recherche d'entreprises | Entreprises, établissements, ouvertures/fermetures, activité NAF, adresse, effectifs si disponibles | Vitalité économique, commerces, recrutement, attractivité, signaux locaux. |
| `insee_local_collector` | P1 | API INSEE données locales, fichiers INSEE | Population, ménages, revenus agrégés, âge, logements, mobilité résidentielle, chômage | Typologie demande, scoring quartier, scripts vendeurs/acquéreurs. |
| `transport_collector` | P1 | transport.data.gouv.fr, GTFS, API datasets | Arrêts, lignes, fréquences, horaires, mobilités partagées, parkings, bornes | Accessibilité réelle, prime transport, matching acquéreurs. |
| `jobs_market_collector` | P1 | API Offres d'emploi France Travail | Offres actives, métier, commune/département, contrat, secteur | Dynamique économique, tension recrutement agence, solvabilité zone. |
| `local_news_collector` | P1 | RSS presse locale, sites mairie, data.gouv, communiqués publics | Articles, titres, dates, lieux, thèmes, projets, incidents, commerces, écoles | Signaux faibles avant prix: quartier montant/faible, angles de prospection. |
| `public_procurement_collector` | P1 | BOAMP, marchés publics, data.gouv | Appels d'offres, travaux, montants, lieux, dates, maîtres d'ouvrage | Anticipe travaux publics, rénovation urbaine, équipements futurs. |
| `energy_price_collector` | P2 | ADEME, data.gouv, données énergie ouvertes, Enedis/GRDF ouvertes | Prix énergie, consommation locale, raccordements, énergie dominante | Impact DPE, coût d'usage, rénovation, objections acquéreur. |
| `weather_climate_collector` | P2 | meteo.data.gouv.fr, Météo-France open data, data.gouv | Températures, canicules, sécheresse, pluie, grêle, historiques locaux | Argile, confort été, risques travaux, assurance, valeur verte. |
| `education_services_collector` | P2 | Éducation nationale open data, annuaire services publics, data.gouv | Écoles, sectorisation si accessible, effectifs, équipements, services publics | Attractivité familles, matching acquéreurs, argumentaire quartier. |
| `health_services_collector` | P2 | FINESS, annuaires santé ouverts, data.gouv | Médecins, pharmacies, établissements, temps d'accès approximatif | Attractivité seniors/familles, zones sous-dotées. |
| `tourism_short_rental_collector` | P3 | data.gouv tourisme, offices publics, réglementation locale, sources autorisées | Flux tourisme, événements, règles meublés, saisonnalité | Investisseurs, locatif, tension marché, risque réglementaire. |
| `social_perception_collector` | P3 | Forums publics, avis publics, pétitions publiques, réseaux publics via API autorisée | Mentions, thèmes, polarité, lieux, fréquence, signaux de plaintes/enthousiasme | Perception quartier, réputation, signaux faibles très précoces. |

### Contrat Raw Collector

Chaque collector brut doit écrire:

| Sortie | Description |
|---|---|
| `raw_records` | Réponse brute ou fichier téléchargé, conservé localement, compressé si lourd. |
| `source_manifest` | URL/API, date de collecte, licence, limites, paramètres, statut HTTP, hash. |
| `normalized_events` | Événements minimaux: lieu, temps, type, source, entités possibles. |
| `geo_keys` | BAN, code INSEE, parcelle, ID bâtiment RNB, bbox, lat/lon. |
| `freshness` | Fréquence théorique, dernière collecte, prochaine collecte, stale_after. |
| `quality_score` | Complétude, précision géographique, fraîcheur, fiabilité source, erreurs. |
| `kasm_ready_metrics` | Vecteurs numériques prêts à être combinés massivement. |

### Premier Ordre D'Attaque Des Scrapers

1. BAN + RNB: créer les clés géographiques.
2. DVF+ + Cadastre: créer le socle prix/parcelle.
3. DPE + Géorisques + Urbanisme GPU: créer le socle risques/énergie/contraintes.
4. Sitadel + marchés publics: créer les signaux de transformation future.
5. SIRENE + INSEE + Transport: créer le score d'attractivité locale.
6. News locale + mairie + RSS: créer le moteur de veille brute.
7. Météo/climat + énergie: créer les signaux éloignés à forte valeur.
8. Ensuite seulement: brancher CRM, biens agence, documents, appels, emails et Google Workspace.

| Groupe de datas | Priorité | Sources à viser | Données à extraire | Outils servis |
|---|---:|---|---|---|
| Biens agence | INT | CRM, exports agence, fichiers locaux, mandats, site agence | Adresse, type, surface, pièces, prix, statut, mandat, honoraires, photos, descriptions, historique prix, propriétaire CRM, visites, offres, documents liés | Mandat vendeur, Estimation, Rapport vendeur, Diffusion, Audit annonces, Matching acheteurs, Pipeline |
| Contacts et CRM | INT | CRM, formulaires site, emails exportés, appels, chatbot, répondeur IA | Prospects, vendeurs, acquéreurs, critères, budget, consentement, historique interactions, prochaine action, source lead, urgence, objections | Prospects, Vendeurs, Acquéreurs, Matching acheteurs, Répondeur IA, Chatbot site, Pipeline, Coaching équipe |
| Transactions et prix | P0 | DVF, fichiers notaire/agence, historiques internes, annonces vendues si disponibles | Prix vendu, date, surface, type, parcelle, délai vente, écart prix affiché/vendu, liquidité, anomalies, comparables | Estimation, Rapport vendeur, DVF, Marché & veille, KPI agence |
| Annonces et diffusion | P0 | Site agence, portails autorisés, exports portails, analytics site, Google Business Profile | Titres, textes, photos, prix, date publication, vues, clics, leads, appels, baisses prix, durée annonce, canal, ranking | Diffusion, Audit annonces, Performance diffusion, Concurrence, Rapport vendeur |
| Documents et conformité | INT | Dossier agence, mandats, diagnostics, compromis, pièces notaire, signatures électroniques | Dates expiration, pièces manquantes, incohérences, obligations légales, consentements, servitudes, annexes, preuves | Conformité, Diagnostics, Back-office, Notaires, Rapport vendeur |
| Données publiques foncières | P0 | Cadastre, Adresse, BAN, parcelles, PLU, urbanisme, permis | Parcelle, bâti, terrain, zonage, servitudes, divisions possibles, permis, contraintes, changement destination | Cadastre, Urbanisme, Estimation, Mandat vendeur, Travaux |
| Énergie et rénovation | P0 | DPE/ADEME, audits énergie, devis, aides publiques, prix énergie | Classe DPE, émissions, chauffage, coût usage, passoire, travaux nécessaires, aides, ROI rénovation, artisan disponible | DPE / ADEME, Diagnostics, Travaux, Estimation, Rapport vendeur |
| Risques et assurance | P0 | Géorisques, ERP, données bruit, pollution sols, sinistres agrégés, assureurs | Inondation, argile, radon, bruit, sismicité, pollution, coût assurance, assurabilité, risque travaux | Géorisques, Diagnostics, Assurances, Estimation, Rapport vendeur |
| Marché local et veille | P1 | Presse locale, mairie, RSS, marchés publics, événements, associations, commerces | News locales, travaux, commerces entrants/sortants, écoles, incidents publics, attractivité, signaux sociaux | Marché & veille, Veille locale, Concurrence, Estimation, Mandat vendeur |
| Concurrence et réputation | P1 | Sites concurrents, portails autorisés, Google Business Profile, avis publics, réseaux publics | Annonces, baisses prix, délai affichage, qualité annonces, avis, réponses, recrutement, spécialités zone | Concurrence, Réputation, Audit annonces, Diffusion, Coaching équipe |
| Financement et solvabilité marché | P1 | Taux publics, courtiers partenaires, dossiers acquéreurs consentis, banques via fichiers | Taux obtenus, capacité, apport, refus, délais banque, assurance emprunteur, risque financement | Courtiers, Acquéreurs, Matching acheteurs, Pipeline, Trésorerie |
| Back-office et finance agence | INT | Comptabilité, factures, commissions, charges, exports bancaires, planning actes | Commission prévue, marge, coût acquisition, charges, factures retard, trésorerie prévisionnelle, dates acte | Comptabilité, Fiscalité, Trésorerie, KPI agence, Pilotage agence |
| Planning et opérations | INT | Agenda, visites, tâches CRM, trajets, disponibilité équipe, météo | Visites, temps trajet, disponibilité, priorité, regroupement géographique, probabilité offre, retards | Planning visites, Pilotage agence, Pipeline, Performance commerciaux |
| Voix, appels et conversations | INT | Répondeur IA, téléphonie, transcriptions, chatbot, emails | Motif, urgence, sentiment, objection, résumé CRM, action suivante, transfert humain, scripts gagnants | Répondeur IA, Chatbot site, Prospects, Coaching équipe, Formation |
| Partenaires | INT | CRM partenaires, retours clients, délais dossiers, factures, avis publics | Notaires, courtiers, artisans, diagnostiqueurs, assureurs, délais, coût, fiabilité, erreurs, disponibilité | Partenaires, Notaires, Courtiers, Assurances, Travaux, Back-office |
| Équipe et RH | INT | CRM interne, tâches, appels anonymisés, résultats agents, offres emploi, onboarding | Performance, charge, relances oubliées, scripts, progression, recrutement, turnover, formation nécessaire | Recrutement, Onboarding, Formation, Coaching équipe, Performance commerciaux |
| Mobilité et accessibilité | P2 | Open data transport, trafic, parkings, temps trajet, travaux voirie | Temps vers centres, fréquence transport, bouchons, parking, nouvelles lignes, accessibilité réelle | Estimation, Veille locale, Rapport vendeur, Matching acheteurs |
| Écoles, santé et services | P2 | Open data écoles, santé, commerces, équipements publics, avis | Sectorisation, effectifs, médecins, pharmacies, équipements, commerces, qualité perçue | Estimation, Veille locale, Matching acheteurs, Rapport vendeur |
| Économie locale | P2 | SIRENE, annonces emploi, créations/fermetures entreprises, commerces, salaires agrégés | Dynamisme emploi, entreprises entrantes/sortantes, tension locale, pouvoir d'achat, demande locative | Marché & veille, Recrutement, Estimation, Pilotage agence |
| Climat et environnement | P2 | Météo historique, sécheresse, îlots chaleur, qualité air/eau, espaces verts | Confort été, risque argile, prime verte, nuisances, coûts futurs, objections probables | Géorisques, Diagnostics, Estimation, Assurances, Travaux |
| Fiscalité locale et coût de détention | P2 | Taxes foncières, budgets communes, charges copro si fichiers, données publiques | Taxe foncière, pression fiscale, coût de détention, dette commune, investissements futurs | Fiscalité, Estimation, Rapport vendeur, Trésorerie |
| Signaux sociaux et perception | P3 | Forums locaux publics, réseaux publics, avis, pétitions, associations, événements | Enthousiasme, plaintes, conflits locaux, perception quartier, sujets récurrents, polarité | Veille locale, Réputation, Concurrence, Rapport vendeur |
| Tourisme et location courte durée | P3 | Données tourisme, événements, plateformes autorisées, réglementation locale | Flux visiteurs, saisonnalité, pression locative, rentabilité potentielle, risque réglementaire | Estimation, Fiscalité, Marché & veille, Investisseur futur |

## Modèle D'Ingestion Par Groupe

Chaque groupe doit produire les mêmes contrats pour rester branchable partout:

| Étape | Sortie attendue |
|---|---|
| Source registry | Source, type, méthode d'accès, fréquence, conformité, coût, fraîcheur, propriétaire de la donnée. |
| Extracteur | Records bruts locaux, jamais envoyés au LLM, avec source hash et timestamp. |
| Normalisation | Entités canoniques: bien, contact, zone, annonce, événement, document, partenaire, agent, source. |
| Entity resolution | Déduplication adresse/contact/bien/source avec score de confiance. |
| Metric snapshots | Vecteurs numériques KASM: prix, risque, délai, demande, concurrence, timing, qualité, coût. |
| Intel pack | Résumé compact, vérifiable, orienté action pour LLM et UI. |
| Alertes | Priorités du jour, opportunités, risques, tâches à pousser au CRM. |

## Ordre D'Attaque Recommandé

1. P0 BAN + RNB + DVF + Cadastre.
2. P0 DPE/ADEME + Géorisques + Urbanisme GPU + permis/Sitadel.
3. P1 SIRENE + INSEE + transport + emploi.
4. P1 News locale + mairie + marchés publics + concurrence publique.
5. P2 météo/climat + énergie + écoles/santé/services + fiscalité locale.
6. P3 signaux sociaux publics + tourisme + location courte durée autorisée.
7. INT seulement ensuite: biens agence, CRM, documents, appels, emails, Google Workspace, back-office.

## Outils Verrouillés

### Production immo

| Outil | Données à collecter |
|---|---|
| Mandat vendeur | CRM vendeurs, historique contacts, estimations envoyées, relances, biens similaires vendus, annonces concurrentes proches, délais de vente, baisses de prix, signaux DPE/travaux/succession/division disponibles légalement. |
| Estimation | DVF, annonces actives et expirées, prix/m2, délai moyen, DPE, surface, terrain, étage, bruit, risques, transports, écoles, commerces, permis proches, concurrence directe. |
| Rapport vendeur | Comparables DVF, annonces concurrentes, prix affiché vs prix vendu, demande acheteur CRM, forces/faiblesses quartier, timing marché, risques diagnostics, stratégie prix. |
| Diagnostics | DPE/ADEME, ERP/Géorisques, amiante/plomb si fichier agence, assainissement, termites, bruit, radon, argile, inondation, contraintes copropriété si documents fournis. |
| Conformité | Mandats, diagnostics expirés, mentions annonce, RGPD CRM, preuves de consentement, pièces notaire, servitudes, cadastre, urbanisme. |
| Diffusion | Site agence, portails autorisés, Google Business Profile, réseaux sociaux, photos, titres, descriptions, prix, leads, appels, taux de contact, position concurrentielle. |
| Audit annonces | Qualité texte/photo, mots clés, cohérence surface/pièces/DPE, prix vs marché, CTA, lisibilité mobile, doublons, concurrence directe, baisse de visibilité. |
| Performance diffusion | Impressions, clics, leads, appels, messages, coût par lead, taux visite, taux mandat, délai avant baisse prix, canal gagnant, horaires et jours performants. |

### Marché et veille

| Outil | Données à collecter |
|---|---|
| Marché & veille | News locales, urbanisme, commerces, transports, écoles, sécurité, emploi local, entreprises, permis, annonces concurrentes, taux crédit, saisonnalité, événements locaux. |
| DVF | Mutations, prix, date, type de bien, surface, parcelle, adresse approximative, volume ventes, dispersion prix, liquidité, anomalies par rue/quartier. |
| Cadastre | Parcelles, surface terrain, bâti/non bâti, limites, divisions possibles, proximité foncier, cohérence adresse/parcelle, densité. |
| DPE / ADEME | Classe énergie, émissions, surface, chauffage, année construction si disponible, fréquence passoires, potentiel rénovation par zone. |
| Géorisques | Inondation, argile, retrait-gonflement, radon, sismicité, pollution sols, bruit, feux, mouvements terrain, anciennes industries. |
| Urbanisme | PLU, zonage, servitudes, permis construire/démolir, projets publics, travaux, restrictions, densification, changements de destination. |
| Veille locale | Mairie, presse locale, travaux publics, écoles, commerces, transports, incidents, entreprises, fermetures, événements, attractivité quartier. |
| Concurrence | Annonces agences, prix, baisses, durée publication, photos, positionnement, avis Google, recrutement, volume mandats, spécialités quartier. |
| Réputation | Avis Google, réponses agence, sentiment, plaintes récurrentes, notes concurrents, réseaux sociaux, mentions locales, qualité relation client. |

### Contacts

| Outil | Données à collecter |
|---|---|
| Prospects | CRM, formulaires, emails entrants, appels, pages consultées, demandes estimation, budget, zone, urgence, historique relances, origine lead. |
| Vendeurs | Estimations passées, propriétaires CRM, ancienneté bien, DPE faible, travaux, interactions email/appel, signaux de timing. |
| Acquéreurs | Budget, financement, apport, critères, zones, biens vus, refus, urgence, capacité, calendrier, comportement portail/site. |
| Matching acheteurs | Biens agence, critères acquéreurs, score compatibilité, budget réel, temps de réaction, historique visites, probabilité offre. |
| Répondeur IA | Appels entrants, motifs, transcriptions, horaires, objections, qualification, urgence, transfert humain, résumé CRM, taux résolution. |
| Chatbot site | Conversations site, questions fréquentes, biens consultés, estimation demandée, rendez-vous, abandons formulaire, conversion. |
| Partenaires | Notaires, courtiers, diagnostiqueurs, artisans, assureurs, délais, qualité, zones couvertes, coût, taux transformation, avis. |

### Pilotage agence

| Outil | Données à collecter |
|---|---|
| Pilotage agence | Pipeline global, mandats, CA prévisionnel, leads, visites, offres, délais, charge équipe, alertes marché, priorités du jour. |
| Pipeline | Statuts prospects/vendeurs/acquéreurs, prochaines actions, probabilité conversion, valeur mandat, blocages, temps depuis dernier contact. |
| KPI agence | CA, commissions, taux mandat, taux exclusivité, délai vente, taux transformation lead, performance canal, performance agent. |
| Planning visites | Agenda, disponibilités, adresses, temps trajet, météo, priorité acheteur, probabilité offre, regroupement géographique. |
| Coaching équipe | Appels, emails, scripts, objections, taux réponse, rythme relance, performance par agent, besoins formation. |
| Performance commerciaux | Leads traités, mandats signés, offres, relances, rapidité, qualité CRM, taux exclusivité, CA estimé, zones fortes/faibles. |

### Back-office

| Outil | Données à collecter |
|---|---|
| Back-office | Documents, tâches admin, échéances, conformité, signatures, factures, contrats, workflows bloqués. |
| Comptabilité | Commissions, factures, charges, fournisseurs, rentabilité mandat, coût acquisition lead, prévision CA. |
| Fiscalité | Taxe foncière, frais notaire, plus-value, LMNP, IFI, fiscalité travaux, résidence principale/secondaire selon infos fournies. |
| Trésorerie | Entrées prévues, commissions à venir, délais signatures, charges fixes, scénarios pessimiste/normal/optimiste. |
| Courtiers | Taux, capacité emprunt, refus financement, délais banque, profils acheteurs, apport, assurance emprunteur. |
| Assurances | PNO, habitation, emprunteur, sinistres zone, risques naturels, coûts, contraintes. |
| Notaires | Délais, compromis, actes, successions, servitudes, pièces manquantes, retards, historique partenaires. |
| Travaux | Devis, artisans, coûts rénovation, aides, DPE avant/après, ROI, délais, disponibilité entreprises. |

### Équipe

| Outil | Données à collecter |
|---|---|
| Recrutement | Offres concurrentes, salaires, profils publics candidats, écoles, alternance, tension emploi local, réputation employeur. |
| Onboarding | Checklist arrivée, formations, accès outils, scripts, portefeuille attribué, progression, erreurs fréquentes. |
| Formation | Appels anonymisés, objections, scripts gagnants, points faibles équipe, cas réels, veille juridique/marché. |

## Couches De Données Gotham

### Graphe d'entités central

Forge doit relier toutes les données autour d'un graphe:

- Bien: adresse, parcelle, caractéristiques, historique prix, DPE, risques, annonces, visites, offres.
- Propriétaire/vendeur: seulement données CRM et consenties, historique relation, timing, préférences, objections.
- Acquéreur: critères, capacité, urgence, comportement, interactions, probabilité de conversion.
- Zone: quartier, rue, micro-marché, prix, liquidité, risques, infrastructures, demande.
- Agence concurrente: annonces, zones fortes, vitesse de vente, avis, recrutement, style commercial.
- Partenaire: notaire, courtier, assureur, artisan, diagnostiqueur, performance et fiabilité.
- Équipe agence: charge, performance, spécialités, formation, disponibilité.
- Événement local: permis, travaux, fermeture/ouverture commerce, nouvelle école, incident, transport, article de presse.

Chaque relation doit porter: `source`, `source_hash`, `observed_at`, `freshness`, `confidence`, `legal_basis`, `entity_resolution_score`.

### Données très éloignées mais précieuses

Ces données paraissent hors immobilier, mais deviennent puissantes une fois croisées avec DVF, CRM, DPE et annonces.

| Domaine | Données | Intel possible |
|---|---|---|
| Météo et climat | Sécheresse, canicule, pluie extrême, grêle, vent, historique sinistres | Risque argile, coût assurance, confort été, travaux futurs, objections vendeur/acquéreur. |
| Énergie | Prix électricité/gaz/fioul, aides rénovation, coût chauffage par typologie | Impact des DPE faibles, urgence rénovation, angle de négociation, ciblage travaux. |
| Mobilité | Temps transport, fréquence bus/train, nouvelles lignes, bouchons, parkings | Attractivité réelle par profil, prime/décote micro-zone, timing avant hausse de demande. |
| Écoles | Sectorisation, effectifs, ouvertures/fermetures, options, réputation publique | Demande familles, saisonnalité, argumentaire vendeur, tension par quartier. |
| Santé | Médecins, pharmacies, hôpitaux, déserts médicaux, temps d'accès | Attractivité seniors/familles, potentiel locatif, objections. |
| Emploi | Offres d'emploi, ouvertures/fermetures entreprises, plans sociaux, salaires locaux | Solvabilité acheteurs, migration entrante, pression locative, zones montantes/faibles. |
| Commerce | Ouvertures/fermetures, vacance commerciale, avis, fréquentation visible | Vitalité quartier, attractivité rue, risque de déclassement. |
| Sécurité publique | Statistiques agrégées, incidents publics, éclairage, nuisances | Risque d'objection, stratégie prix, veille réputation quartier. |
| Tourisme | Saisonnalité, locations courte durée, événements, flux visiteurs | Potentiel investisseur, tension locative, risque réglementaire. |
| Réseaux sociaux locaux | Mentions quartier, plaintes, événements, enthousiasme, communautés | Signaux faibles avant que les prix ne bougent. |
| Urbanisme indirect | Marchés publics, appels d'offres, permis, chantiers, rénovation urbaine | Détection précoce des rues qui vont changer de valeur. |
| Fiscalité locale | Taxe foncière, budgets municipaux, dette commune, investissements | Coût de détention, attractivité, pression future. |
| Démographie | Âge, ménages, revenus agrégés, mobilité résidentielle, naissances | Typologie de demande, scripts commerciaux, produits à rentrer. |
| Culture et sport | Clubs, équipements, événements, lieux culturels | Attractivité émotionnelle, argumentaire quartier. |
| Connectivité | Fibre, 5G, zones blanches, télétravail possible | Attractivité cadres, locatif, résidences secondaires. |
| Agriculture/environnement | Qualité air/eau, pesticides, espaces verts, îlots de chaleur | Risques d'objection, prime verte, qualité de vie. |

## Datas Supplémentaires Ciblées Par Outil

Cette section définit les datas additionnelles à viser pour pousser chaque outil au niveau "Gotham immobilier". La priorité n'est pas seulement de collecter plus, mais de capturer les variations temporelles: ce qui change, à quelle vitesse, dans quelle zone, et avec quel effet probable sur les mandats, les prix, les acquéreurs et la concurrence.

### Production immo

| Outil | Datas supplémentaires à viser |
|---|---|
| Mandat vendeur | Ancienneté de détention estimée, historique estimations non converties, biens hérités/successions via CRM/notaire, signaux de travaux lourds, vacance probable, pression fiscale locale, baisse de pouvoir d'achat quartier. |
| Estimation | Elasticité prix par micro-zone, délai de vente par segment, décote bruit/risque/DPE, prime école/transport, prix psychologique local, comparaison photo/standing, tension acquéreur réelle CRM. |
| Rapport vendeur | Arguments personnalisés par profil vendeur, objections probables, scénario prix haut vs prix marché, probabilité baisse future, coût d'attente, fenêtre optimale de mise en vente. |
| Diagnostics | Probabilité de défaut par âge/type bâtiment, coût moyen travaux par poste, rareté artisans, aides disponibles, risque de contre-visite, impact DPE sur délai de vente. |
| Conformité | Dates d'expiration documents, incohérences mandat/annonce/diagnostics, checklist notaire, risques RGPD, pièces manquantes par dossier, preuve de consentement. |
| Diffusion | Ranking portail, taux de scroll, qualité photo IA, heure/jour optimal publication, performance mots-clés, duplication annonce, cannibalisation entre canaux. |
| Audit annonces | Score de désirabilité texte/photo, angle émotionnel, niveau de preuve, friction CTA, comparaison annonces vendues vite, détection annonce qui fatigue. |
| Performance diffusion | Attribution canal vers mandat/offre, coût lead qualifié, lead decay, saturation d'audience, canal fort par typologie, temps idéal avant modification prix/texte/photo. |

### Marché et veille

| Outil | Datas supplémentaires à viser |
|---|---|
| Marché & veille | Chantiers publics, marchés publics, commerces entrants/sortants, fermetures classes, sécurité agrégée, transports futurs, signaux sociaux locaux. |
| DVF | Revente rapide, anomalies prix, clusters de rues liquides, biens sous/surpayés, évolution par typologie, comparaison avec annonces expirées. |
| Cadastre | Potentiel division, densification possible, parcelles atypiques, contraintes accès, foncier voisin, cohérence bâti/parcelle. |
| DPE / ADEME | Distribution DPE par rue, passoires ciblables, coût rénovation probable, impact aides, zones où DPE pénalise le plus le prix. |
| Géorisques | Historique sinistres agrégé, argile + météo sécheresse, bruit routier/rail/aérien, pollution sols, coût assurance probable. |
| Urbanisme | PLU versionné, permis futurs, recours, zones de densification, changement destination, projets transports/voirie, calendrier travaux. |
| Veille locale | Presse hyperlocale, comptes mairie, associations, écoles, commerces, forums locaux, événements, avis citoyens, incidents récurrents. |
| Concurrence | Vitesse publication, fréquence baisse prix, qualité visuelle, avis récents, agents recrutés/perdus, spécialités par rue, mandats longs. |
| Réputation | Sentiment avis, thèmes négatifs récurrents, délai réponse, réputation concurrents par service, mentions réseaux, litiges publics éventuels. |

### Contacts

| Outil | Datas supplémentaires à viser |
|---|---|
| Prospects | Source exacte du lead, intention implicite, temps de réponse, pages consultées, mots utilisés, urgence déduite, probabilité d'achat/vente. |
| Vendeurs | Cycle de vie propriétaire, interactions anciennes, changements familiaux uniquement si CRM, travaux évoqués, refus précédents, prix attendu vs marché. |
| Acquéreurs | Capacité réelle, taux obtenu, flexibilité critères, vitesse décision, objections, biens rejetés, probabilité offre, risque financement. |
| Matching acheteurs | Similarité comportementale, compromis acceptables, biens presque bons, arbitrage prix/zone/surface, timing visite optimal. |
| Répondeur IA | Motifs d'appel, émotion/urgence, objections, horaires pics, transfert nécessaire, scripts gagnants, résumé CRM automatique. |
| Chatbot site | Parcours avant message, abandon formulaire, questions fréquentes, biens consultés, score intention, friction UX, sujet qui convertit. |
| Partenaires | Délais réels, taux erreur, prix, satisfaction client, disponibilité, zone couverte, business généré, fiabilité par dossier. |

### Pilotage agence

| Outil | Datas supplémentaires à viser |
|---|---|
| Pilotage agence | Prévision CA, risque pipeline, charge équipe, top priorités, valeur dormante CRM, zones sous-exploitées, mandats à sauver. |
| Pipeline | Probabilité étape suivante, blocage principal, temps mort, prochaine meilleure action, valeur espérée, risque abandon. |
| KPI agence | KPI par source/canal/agent/zone, marge réelle, qualité CRM, délai moyen, taux exclusivité, coût d'acquisition mandat. |
| Planning visites | Distance/temps trajet, météo, urgence acquéreur, regroupement géographique, probabilité offre, disponibilité vendeur. |
| Coaching équipe | Analyse appels/emails, objections perdues, scripts performants, temps réponse, relances oubliées, comparaison top performer. |
| Performance commerciaux | Conversion par étape, valeur portefeuille, spécialité zone/type bien, charge mentale, relances manquées, qualité notes CRM. |

### Back-office

| Outil | Datas supplémentaires à viser |
|---|---|
| Back-office | Dossiers bloqués, échéances, signatures manquantes, relances admin, pièces expirées, risques avant compromis. |
| Comptabilité | Rentabilité par mandat, coût canal, commission attendue, charges par activité, marge par agent, factures en retard. |
| Fiscalité | Taxe foncière, plus-value probable, frais notaire, LMNP, aides travaux, coût détention, fiscalité locale future. |
| Trésorerie | CA probabilisé, dates acte, commissions à risque, scénarios cash, dépendance gros mandats, trous de trésorerie. |
| Courtiers | Taux réels obtenus, délai banque, refus, apport moyen, capacité acheteurs, qualité courtier, risque financement offre. |
| Assurances | Coûts par zone, sinistres agrégés, risques naturels, PNO, emprunteur, impact géorisques sur assurabilité. |
| Notaires | Délais par étude, pièces souvent manquantes, dossiers bloqués, successions, servitudes, retards compromis/acte. |
| Travaux | Prix artisans locaux, disponibilité, devis historiques, aides, ROI DPE, délais, matériaux, risque dépassement budget. |

### Équipe

| Outil | Datas supplémentaires à viser |
|---|---|
| Recrutement | Offres concurrentes, salaires, profils publics, écoles, tension locale, turnover concurrents, réputation employeur. |
| Onboarding | Temps montée en compétence, erreurs fréquentes, scripts appris, accès outils, progression CRM, premiers leads traités. |
| Formation | Appels anonymisés, objections réelles, cas perdus/gagnés, scripts top agents, lacunes par personne, veille juridique. |

## Signaux Faibles À Calculer

Forge ne doit pas seulement afficher des données. Il doit calculer des signaux.

- Signal vendeur probable: DPE faible + propriétaire CRM ancien + hausse charges énergie + estimation non convertie + baisse de liquidité quartier.
- Signal mandat urgent: héritage/succession dans CRM + bien vacant + taxe/charges élevées + marché encore liquide.
- Signal prix trop haut: annonce > percentile local + peu de leads + concurrence plus fraîche + baisse taux appels.
- Signal quartier montant: travaux publics + nouveaux commerces + amélioration transport + hausse recherche web + DVF encore bas.
- Signal quartier fragile: fermetures commerces + bruit/trafic + baisse avis + délais vente qui montent + prix affichés qui baissent.
- Signal acquéreur chaud: visites répétées + financement prêt + réaction rapide + biens sauvegardés cohérents + peu d'objections.
- Signal concurrence vulnérable: annonces longues + avis négatifs récents + baisses nombreuses + recrutement absent + faible qualité diffusion.
- Signal partenaire rentable: délais courts + taux transformation élevé + peu d'erreurs + bons retours clients.
- Signal recrutement: concurrents recrutent + hausse annonces emploi + zones sous-couvertes par l'agence.
- Signal trésorerie: concentration commissions futures + retards compromis + baisse pipeline neuf.

## Données À Ne Pas Collecter

Pour rester solide légalement et commercialement:

- Pas de données personnelles hors base légale ou consentement.
- Pas de scraping derrière login ou CAPTCHA.
- Pas d'enrichissement propriétaire avec données sensibles non nécessaires.
- Pas de scoring discriminatoire sur critères protégés.
- Pas de données de solvabilité individuelle sans consentement explicite.
- Pas de conservation brute quand un hash, une preuve et un extrait suffisent.

## Sorties Attendues

Chaque collector doit produire:

- `source_evidence`: preuve, URL ou fichier, date, hash.
- `entity_events`: événements normalisés liés au graphe.
- `metric_snapshots`: métriques prêtes pour KASM.
- `intel_pack`: synthèse actionnable pour le LLM.
- `alerts`: priorités du jour.
- `audit_log`: preuve que la collecte respecte le périmètre autorisé.

## Priorité De Construction

1. Brancher les vraies sources open data: DVF, DPE/ADEME, Géorisques, Cadastre/Adresse, SIRENE, data.gouv.
2. Ajouter RSS/news locales et pages mairie en crawler public borné.
3. Ajouter ingestion fichiers agence: CRM CSV, mandats, annonces, diagnostics, appels, emails exportés.
4. Créer le graphe d'entités local.
5. Produire les premiers signaux vendeur, estimation, diffusion et concurrence.
6. Ajouter scoring KASM multi-scenarios.
7. Publier un intel pack quotidien par zone, par commercial et par outil.
8. Brancher actions agentiques: relance, rapport, alerte, tâche CRM, brouillon email/appel.
