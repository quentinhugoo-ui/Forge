# Migration Front

Source de verite de la refonte native InGen.

Date de mise a jour: 2026-06-06.

## Etat Actuel

Migration Front est en phase de coupure finale.

- Shell actif: `examples/ingen_native_front`.
- Services natifs partages: `examples/ingen_native_services`.
- UI: Rust + Slint.
- Rendu: wgpu pour les surfaces moteur.
- Etat UI: kernel Rust deterministe dans `examples/ingen_native_front/src/state.rs`.
- Audit de coupure: `cargo run --manifest-path examples\ingen_native_front\Cargo.toml -- --cutover-audit`.

L'ancien arbre applicatif a ete supprime du depot. Les services qui y restaient ont ete extraits, retires ou remplaces par des adaptateurs Rust natifs.

## Objectif Produit

Livrer InGen comme application native compacte:

```text
Intention utilisateur
-> etat Rust verifiable
-> Slint
-> wgpu / services natifs
-> preuve compacte
```

Le front n'est plus pilote par un shell navigateur. Le chemin normal de lancement est le binaire natif.

## Regles De Coupure

- Ne pas recreer d'arbre applicatif obsolete.
- Ne pas ajouter de shell UI pilote par un moteur navigateur.
- Ne pas ajouter de runtime npm pour l'app produit.
- Ne pas ajouter de source applicative basee sur documents navigateur, feuilles de style globales ou scripts client.
- Toute nouvelle surface UI doit entrer par `examples/ingen_native_front/ui/app.slint`, `tokens.slint`, `state.rs`, `services.rs` ou un module Rust natif explicitement relie.
- Toute dependance d'affichage externe doit rester un peripherique contenu, jamais le shell global.

## Checklist Restante

- [x] Shell Rust + Slint actif.
- [x] Parite visuelle Forge premiere vue portee dans Slint.
- [x] Etat UI rejouable et hashable.
- [x] Banger projete par surface native.
- [x] Services Banger natifs extraits dans `examples/ingen_native_services`.
- [x] Ancien arbre applicatif supprime.
- [x] Audit de coupure mis a jour pour la retraite complete.
- [x] Repasser `cargo check --manifest-path examples\ingen_native_front\Cargo.toml --tests`.
- [x] Repasser `cargo test --manifest-path examples\ingen_native_front\Cargo.toml --lib`.
- [x] Repasser `cargo run --manifest-path examples\ingen_native_front\Cargo.toml -- --cutover-audit`.
- [ ] Commit + push de la coupure finale.

## Definition De Fini

La migration est finie quand:

- le binaire natif demarre sans l'ancien arbre;
- l'audit de coupure retourne pret;
- les tests du front natif passent;
- les docs ne decrivent plus l'ancienne architecture comme une cible vivante;
- le commit de suppression est pousse sur GitHub.
