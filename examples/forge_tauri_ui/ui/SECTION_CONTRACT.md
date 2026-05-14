# Forge UI Section Contract

Chaque nouvelle section doit respecter ce contrat avant d'appeler le backend Tauri.

1. Charger les modules shell/config (`forge-section-registry.js`, `forge-tauri-bridge.js`, `forge-boot.js`, `forge-window-controls.js`, `forge-webexplorer-config.js`) avant `app.js`.
2. Enregistrer la section avec `ForgeSectionRegistry.register`.
3. Marquer la section active/inactive a l'ouverture et a la fermeture.
4. Passer les commandes Tauri sensibles par `ForgeTauriBridge.invoke`.
5. Ajouter `requiresActiveSection: true` pour les commandes qui creent ou presentent une webview native.
6. Garder les commandes shell critiques, dont les controles de fenetre et le hardware scan, en `bootSafe: true`.
7. Ne jamais creer de webview native, lancer de moteur lourd, ou demarrer un polling global pendant le boot.
8. Declarer la section dans `SECTION_OWNERSHIP.json`.
9. Lancer `node examples/forge_tauri_ui/scripts/forge-ui-section-audit.mjs` avant de valider une nouvelle section UI.
10. Lancer `node examples/forge_tauri_ui/scripts/forge-ui-smoke.mjs` avant de valider une nouvelle section UI.

Objectif: une section peut disparaitre, etre inactive, ou echouer sans casser le shell, les boutons de fenetre, la detection CPU/GPU, l'historique ou les assets.
