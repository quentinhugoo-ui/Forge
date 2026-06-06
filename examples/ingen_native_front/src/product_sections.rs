use crate::services::ServiceSnapshot;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductSectionState {
    pub section_id: String,
    pub title: String,
    pub status: String,
    pub metric_lines: Vec<String>,
    pub card_lines: Vec<String>,
    pub action_lines: Vec<String>,
    pub webview_required: bool,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductSectionsManifest {
    pub schema: String,
    pub source: String,
    pub sections: Vec<ProductSectionState>,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProductSectionProjection {
    pub active_section: String,
    pub title: String,
    pub status: String,
    pub metric_lines: String,
    pub card_lines: String,
    pub action_lines: String,
    pub proof_summary: String,
    pub manifest_hash: String,
    pub projection_hash: String,
}

pub fn build_product_sections_manifest(
    snapshot: &ServiceSnapshot,
    banger_frame_hash: &str,
    atlas_projection_hash: &str,
) -> ProductSectionsManifest {
    let mut sections = vec![
        section(
            "forge",
            "Forge / Compute",
            &snapshot.monster_status,
            vec![
                format!("provider={}/{}", snapshot.provider.provider, snapshot.provider.model),
                format!("jobs={}", snapshot.jobs.len()),
                "compute_templates=ready".to_string(),
                "memory_projection=native".to_string(),
            ],
            vec![
                "Monster queue is rendered by Slint status and proof cards.".to_string(),
                "Compute templates remain Rust-owned; no browser command bridge.".to_string(),
                format!("latest_service_proof={}", short_hash(&snapshot.proof_hash)),
            ],
            vec![
                "new compute".to_string(),
                "select compute".to_string(),
                "open proof ledger".to_string(),
            ],
            false,
        ),
        section(
            "alpha",
            "Alpha",
            &snapshot.brain_status,
            vec![
                "canvas=slint-native".to_string(),
                "brain_projection=evidence-aware".to_string(),
                "webview_required=false".to_string(),
            ],
            vec![
                "Alpha receives the same native state and proof bus as Forge.".to_string(),
                "Visual parity remains gated by the Tauri oracle inventory.".to_string(),
            ],
            vec!["open canvas".to_string(), "run local proof".to_string()],
            false,
        ),
        section(
            "trading",
            "Trading",
            &snapshot.trading_status,
            vec![
                "market_status=direct".to_string(),
                "chart_summary=native-card".to_string(),
                "timeframes=1m/5m/1h/1d".to_string(),
                "provider_status=direct-rust".to_string(),
            ],
            vec![
                "NATGAS_USD paper workspace is native-state backed.".to_string(),
                "Backtest and alert cards are product adapters, not WebView DOM.".to_string(),
            ],
            vec![
                "refresh market".to_string(),
                "run backtest".to_string(),
                "arm alert".to_string(),
            ],
            false,
        ),
        section(
            "real-estate",
            "Real Estate",
            &snapshot.real_estate_status,
            vec![
                "onboarding=native".to_string(),
                "zone_scoring=ready".to_string(),
                "resolver=direct-service-boundary".to_string(),
                "mode_state=slint".to_string(),
            ],
            vec![
                "Zone scoring, resolver summaries and panels have native slots.".to_string(),
                "Domain data stays behind Rust service adapters.".to_string(),
            ],
            vec![
                "score zone".to_string(),
                "open resolver".to_string(),
                "show panels".to_string(),
            ],
            false,
        ),
        section(
            "banger",
            "Banger",
            &snapshot.banger_status,
            vec![
                format!("frame={}", short_hash(banger_frame_hash)),
                "viewport=wgpu-native".to_string(),
                "browser_canvas=false".to_string(),
            ],
            vec![
                "Banger is already a native viewport surface.".to_string(),
                "Direct external texture import remains gated by Slint/wgpu type parity.".to_string(),
            ],
            vec!["frame proof".to_string(), "scene graph".to_string()],
            false,
        ),
        section(
            "webexplorer",
            "WebExplorer",
            &snapshot.webexplorer_status,
            vec![
                format!("atlas_ui={}", short_hash(atlas_projection_hash)),
                "webview=isolated-peripheral".to_string(),
                "app_shell_webview=false".to_string(),
            ],
            vec![
                "The page is inspected as a RAM DOM object in native Slint.".to_string(),
                "WebView remains a contained peripheral, not the app shell.".to_string(),
            ],
            vec!["inspect atlas".to_string(), "refresh capture".to_string()],
            true,
        ),
        section(
            "diagnostics",
            "Diagnostics / Proofs",
            &format!("service_proof={}", short_hash(&snapshot.proof_hash)),
            vec![
                "proof_cards=native".to_string(),
                "section_switch_replay=true".to_string(),
                "old_front_dependency=false".to_string(),
            ],
            vec![
                "All product section manifests are content-addressed.".to_string(),
                "Non-web sections explicitly reject WebView as a dependency.".to_string(),
            ],
            vec!["copy proof".to_string(), "open report".to_string()],
            false,
        ),
    ];

    for item in &mut sections {
        item.proof_hash = section_hash(item);
    }
    let mut manifest = ProductSectionsManifest {
        schema: "ingen.native_front.stage9_product_sections.v1".to_string(),
        source: "examples/ingen_native_front/src/product_sections.rs".to_string(),
        sections,
        proof_hash: String::new(),
    };
    manifest.proof_hash = stable_hash(&(&manifest.schema, &manifest.source, &manifest.sections));
    manifest
}

pub fn product_section_projection(
    manifest: &ProductSectionsManifest,
    active_section: &str,
) -> ProductSectionProjection {
    let section = manifest
        .sections
        .iter()
        .find(|item| item.section_id == active_section)
        .or_else(|| manifest.sections.iter().find(|item| item.section_id == "forge"))
        .expect("product sections manifest has forge section");
    let mut projection = ProductSectionProjection {
        active_section: section.section_id.clone(),
        title: section.title.clone(),
        status: section.status.clone(),
        metric_lines: section.metric_lines.join("\n"),
        card_lines: section.card_lines.join("\n\n"),
        action_lines: section
            .action_lines
            .iter()
            .map(|action| format!("> {action}"))
            .collect::<Vec<_>>()
            .join("\n"),
        proof_summary: format!(
            "section={} manifest={} webview_required={}",
            short_hash(&section.proof_hash),
            short_hash(&manifest.proof_hash),
            section.webview_required
        ),
        manifest_hash: manifest.proof_hash.clone(),
        projection_hash: String::new(),
    };
    projection.projection_hash = stable_hash(&(
        &projection.active_section,
        &projection.title,
        &projection.status,
        &projection.metric_lines,
        &projection.card_lines,
        &projection.action_lines,
        &projection.proof_summary,
        &projection.manifest_hash,
    ));
    projection
}

fn section(
    section_id: &str,
    title: &str,
    status: &str,
    metric_lines: Vec<String>,
    card_lines: Vec<String>,
    action_lines: Vec<String>,
    webview_required: bool,
) -> ProductSectionState {
    ProductSectionState {
        section_id: section_id.to_string(),
        title: title.to_string(),
        status: status.to_string(),
        metric_lines,
        card_lines,
        action_lines,
        webview_required,
        proof_hash: String::new(),
    }
}

fn section_hash(section: &ProductSectionState) -> String {
    stable_hash(&(
        &section.section_id,
        &section.title,
        &section.status,
        &section.metric_lines,
        &section.card_lines,
        &section.action_lines,
        section.webview_required,
    ))
}

fn stable_hash<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize product section hash input");
    format!("{:x}", Sha256::digest(bytes))
}

fn short_hash(hash: &str) -> String {
    hash.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::fake_service_snapshot;

    #[test]
    fn product_sections_cover_all_stage9_targets() {
        let manifest = build_product_sections_manifest(
            &fake_service_snapshot(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        let ids = manifest
            .sections
            .iter()
            .map(|section| section.section_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "forge",
                "alpha",
                "trading",
                "real-estate",
                "banger",
                "webexplorer",
                "diagnostics"
            ]
        );
        assert_eq!(manifest.proof_hash.len(), 64);
        assert!(manifest
            .sections
            .iter()
            .filter(|section| section.section_id != "webexplorer")
            .all(|section| !section.webview_required));
    }

    #[test]
    fn product_section_projection_is_deterministic() {
        let manifest = build_product_sections_manifest(
            &fake_service_snapshot(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        );
        let first = product_section_projection(&manifest, "trading");
        let second = product_section_projection(&manifest, "trading");

        assert_eq!(first, second);
        assert_eq!(first.active_section, "trading");
        assert!(first.metric_lines.contains("timeframes"));
        assert!(first.action_lines.contains("run backtest"));
        assert_eq!(first.projection_hash.len(), 64);
    }
}
