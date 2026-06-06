use crate::webexplorer::webexplorer_fixture_html;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasNode {
    pub atlas_ref: String,
    pub frame_path: String,
    pub backend_node_id: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    pub tag: String,
    pub role: String,
    pub ax_name: String,
    pub ax_value: String,
    pub text_value: String,
    pub attributes: BTreeMap<String, String>,
    pub bounds: AtlasBounds,
    pub style_subset: BTreeMap<String, String>,
    pub resource_refs: Vec<String>,
    pub evidence_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasResource {
    pub atlas_ref: String,
    pub kind: String,
    pub url: String,
    pub evidence_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasCoverageReport {
    pub dom_node_count: usize,
    pub ax_node_count: usize,
    pub layout_node_count: usize,
    pub styled_node_count: usize,
    pub resource_count: usize,
    pub dom_ratio: u8,
    pub ax_ratio: u8,
    pub layout_ratio: u8,
    pub style_ratio: u8,
    pub resource_ratio: u8,
    pub blind_spots: Vec<String>,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasManifest {
    pub schema: String,
    pub source: String,
    pub raw_hash: String,
    pub normalized_hash: String,
    pub node_count: usize,
    pub resources: Vec<AtlasResource>,
    pub nodes: Vec<AtlasNode>,
    pub coverage: AtlasCoverageReport,
    pub proof_hash: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AtlasUiProjection {
    pub selected_index: usize,
    pub tree_lines: String,
    pub selected_summary: String,
    pub ax_summary: String,
    pub layout_summary: String,
    pub resource_lines: String,
    pub action_candidates: String,
    pub blind_spot_lines: String,
    pub proof_summary: String,
    pub search_summary: String,
    pub projection_hash: String,
}

pub fn capture_fixture_webatlas() -> AtlasManifest {
    capture_webatlas_from_html("webexplorer.fixture.stage6", webexplorer_fixture_html())
}

pub fn atlas_ui_projection(manifest: &AtlasManifest, selected_index: usize) -> AtlasUiProjection {
    let selected_index = selected_index.min(manifest.nodes.len().saturating_sub(1));
    let selected = manifest.nodes.get(selected_index);
    let tree_lines = manifest
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let marker = if index == selected_index { ">" } else { " " };
            let indent = "  ".repeat(node.frame_path.matches('/').count().saturating_sub(1));
            format!(
                "{marker} {index:02} {indent}<{}> {}",
                node.tag,
                if node.ax_name.is_empty() {
                    node.role.as_str()
                } else {
                    node.ax_name.as_str()
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let selected_summary = selected
        .map(|node| {
            format!(
                "{}\n{}\nchildren={} evidence={}",
                node.atlas_ref,
                node.frame_path,
                node.children.len(),
                node.evidence_hash
            )
        })
        .unwrap_or_else(|| "no node selected".to_string());
    let ax_summary = selected
        .map(|node| {
            format!(
                "role={} name={} value={}",
                node.role,
                empty_marker(&node.ax_name),
                empty_marker(&node.ax_value)
            )
        })
        .unwrap_or_default();
    let layout_summary = selected
        .map(|node| {
            format!(
                "x={} y={} w={} h={} styles={}",
                node.bounds.x,
                node.bounds.y,
                node.bounds.width,
                node.bounds.height,
                node.style_subset.len()
            )
        })
        .unwrap_or_default();
    let resource_lines = manifest
        .resources
        .iter()
        .map(|resource| format!("{} {} {}", resource.kind, resource.url, resource.evidence_hash))
        .collect::<Vec<_>>()
        .join("\n");
    let action_candidates = manifest
        .nodes
        .iter()
        .filter(|node| matches!(node.role.as_str(), "button" | "textbox" | "link"))
        .map(|node| format!("{} {} {}", node.role, node.atlas_ref, empty_marker(&node.ax_name)))
        .collect::<Vec<_>>()
        .join("\n");
    let blind_spot_lines = manifest.coverage.blind_spots.join("\n");
    let proof_summary = format!(
        "manifest={}\nnormalized={}\ncoverage={}\nDOM={} AX={} layout={} style={} resource={}",
        manifest.proof_hash,
        manifest.normalized_hash,
        manifest.coverage.proof_hash,
        manifest.coverage.dom_ratio,
        manifest.coverage.ax_ratio,
        manifest.coverage.layout_ratio,
        manifest.coverage.style_ratio,
        manifest.coverage.resource_ratio,
    );
    let search_summary = format!(
        "interactive={} resources={} nodes={}",
        manifest
            .nodes
            .iter()
            .filter(|node| matches!(node.role.as_str(), "button" | "textbox" | "link"))
            .count(),
        manifest.resources.len(),
        manifest.nodes.len(),
    );
    let mut projection = AtlasUiProjection {
        selected_index,
        tree_lines,
        selected_summary,
        ax_summary,
        layout_summary,
        resource_lines,
        action_candidates,
        blind_spot_lines,
        proof_summary,
        search_summary,
        projection_hash: String::new(),
    };
    projection.projection_hash = hash_json(&(
        projection.selected_index,
        &projection.tree_lines,
        &projection.selected_summary,
        &projection.ax_summary,
        &projection.layout_summary,
        &projection.resource_lines,
        &projection.action_candidates,
        &projection.blind_spot_lines,
        &projection.proof_summary,
        &projection.search_summary,
    ));
    projection
}

pub fn capture_webatlas_from_html(source: &str, html: &str) -> AtlasManifest {
    let raw_hash = sha256_hex(html.as_bytes());
    let mut nodes = parse_html_nodes(html);
    let resources = collect_resources(&nodes);
    attach_resource_refs(&mut nodes, &resources);
    let normalized_hash = hash_json(&(&nodes, &resources));
    let coverage = coverage_report(&nodes, &resources);
    let mut manifest = AtlasManifest {
        schema: "ingen.webatlas.manifest.v1".to_string(),
        source: source.to_string(),
        raw_hash,
        normalized_hash,
        node_count: nodes.len(),
        resources,
        nodes,
        coverage,
        proof_hash: String::new(),
    };
    manifest.proof_hash = hash_json(&(
        &manifest.schema,
        &manifest.source,
        &manifest.raw_hash,
        &manifest.normalized_hash,
        manifest.node_count,
        &manifest.coverage.proof_hash,
    ));
    manifest
}

fn empty_marker(value: &str) -> &str {
    if value.is_empty() {
        "none"
    } else {
        value
    }
}

fn parse_html_nodes(html: &str) -> Vec<AtlasNode> {
    let mut nodes = vec![node(
        "document",
        None,
        "document".to_string(),
        BTreeMap::new(),
        0,
        0,
    )];
    let mut stack = vec![0usize];
    let mut cursor = 0usize;
    let bytes = html.as_bytes();

    while cursor < html.len() {
        let Some(tag_start_rel) = html[cursor..].find('<') else {
            push_text(&mut nodes, *stack.last().unwrap_or(&0), &html[cursor..]);
            break;
        };
        let tag_start = cursor + tag_start_rel;
        push_text(&mut nodes, *stack.last().unwrap_or(&0), &html[cursor..tag_start]);
        let Some(tag_end_rel) = html[tag_start..].find('>') else {
            break;
        };
        let tag_end = tag_start + tag_end_rel;
        let raw = html[tag_start + 1..tag_end].trim();
        cursor = tag_end + 1;

        if raw.is_empty() || raw.starts_with('!') || raw.starts_with('?') {
            continue;
        }
        if let Some(close) = raw.strip_prefix('/') {
            let close = close.split_whitespace().next().unwrap_or_default();
            while stack.len() > 1 {
                let idx = stack.pop().unwrap();
                if nodes[idx].tag == close {
                    break;
                }
            }
            continue;
        }

        let self_closing = raw.ends_with('/') || is_void_tag(raw);
        let clean = raw.trim_end_matches('/').trim();
        let (tag, attrs) = parse_tag(clean);
        if tag.is_empty() {
            continue;
        }
        let parent = *stack.last().unwrap_or(&0);
        let depth = stack.len();
        let idx = nodes.len();
        nodes.push(node(&tag, Some(parent), html_path(&nodes, parent, &tag), attrs, idx, depth));
        nodes[idx].parent = Some(nodes[parent].atlas_ref.clone());
        let child_ref = nodes[idx].atlas_ref.clone();
        nodes[parent].children.push(child_ref);
        if !self_closing {
            stack.push(idx);
        }

        if bytes.get(cursor).is_none() {
            break;
        }
    }

    for item in &mut nodes {
        item.role = role_for(&item.tag, &item.attributes).to_string();
        item.ax_name = ax_name_for(item);
        item.ax_value = ax_value_for(item);
        item.style_subset = style_subset_for(item);
        item.evidence_hash = node_evidence_hash(item);
    }
    nodes
}

fn node(
    tag: &str,
    parent: Option<usize>,
    frame_path: String,
    attributes: BTreeMap<String, String>,
    index: usize,
    depth: usize,
) -> AtlasNode {
    let atlas_ref = format!("atlas:{:016x}", stable_u64(&format!("{tag}:{index}:{depth}")));
    AtlasNode {
        atlas_ref,
        frame_path,
        backend_node_id: format!("fixture-node-{index}"),
        parent: parent.map(|parent| format!("fixture-parent-{parent}")),
        children: Vec::new(),
        tag: tag.to_string(),
        role: String::new(),
        ax_name: String::new(),
        ax_value: String::new(),
        text_value: String::new(),
        attributes,
        bounds: AtlasBounds {
            x: 24 + (depth as i32 * 34),
            y: 20 + (index as i32 * 42),
            width: 720u32.saturating_sub((depth as u32) * 42).max(120),
            height: match tag {
                "input" | "button" => 32,
                "h1" => 38,
                "document" | "html" | "body" | "main" => 220,
                _ => 64,
            },
        },
        style_subset: BTreeMap::new(),
        resource_refs: Vec::new(),
        evidence_hash: String::new(),
    }
}

fn push_text(nodes: &mut [AtlasNode], idx: usize, text: &str) {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return;
    }
    if !nodes[idx].text_value.is_empty() {
        nodes[idx].text_value.push(' ');
    }
    nodes[idx].text_value.push_str(&normalized);
}

fn is_void_tag(raw: &str) -> bool {
    matches!(
        raw.split_whitespace().next().unwrap_or_default(),
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "source" | "track" | "wbr"
    )
}

fn parse_tag(raw: &str) -> (String, BTreeMap<String, String>) {
    let mut parts = raw.splitn(2, char::is_whitespace);
    let tag = parts.next().unwrap_or_default().to_ascii_lowercase();
    let attrs_raw = parts.next().unwrap_or_default();
    (tag, parse_attrs(attrs_raw))
}

fn parse_attrs(raw: &str) -> BTreeMap<String, String> {
    let mut attrs = BTreeMap::new();
    let mut cursor = 0usize;
    while cursor < raw.len() {
        while raw[cursor..].starts_with(char::is_whitespace) {
            cursor += 1;
            if cursor >= raw.len() {
                return attrs;
            }
        }
        let key_start = cursor;
        while cursor < raw.len()
            && !raw.as_bytes()[cursor].is_ascii_whitespace()
            && raw.as_bytes()[cursor] != b'='
        {
            cursor += 1;
        }
        let key = raw[key_start..cursor].trim().to_ascii_lowercase();
        while cursor < raw.len() && raw.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let mut value = String::new();
        if cursor < raw.len() && raw.as_bytes()[cursor] == b'=' {
            cursor += 1;
            while cursor < raw.len() && raw.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor < raw.len() && matches!(raw.as_bytes()[cursor], b'"' | b'\'') {
                let quote = raw.as_bytes()[cursor];
                cursor += 1;
                let value_start = cursor;
                while cursor < raw.len() && raw.as_bytes()[cursor] != quote {
                    cursor += 1;
                }
                value = raw[value_start..cursor].to_string();
                cursor = (cursor + 1).min(raw.len());
            } else {
                let value_start = cursor;
                while cursor < raw.len() && !raw.as_bytes()[cursor].is_ascii_whitespace() {
                    cursor += 1;
                }
                value = raw[value_start..cursor].to_string();
            }
        }
        if !key.is_empty() {
            attrs.insert(key, value);
        }
    }
    attrs
}

fn html_path(nodes: &[AtlasNode], parent: usize, tag: &str) -> String {
    if parent == 0 {
        return format!("/document/{tag}");
    }
    format!("{}/{}", nodes[parent].frame_path, tag)
}

fn role_for(tag: &str, attrs: &BTreeMap<String, String>) -> &'static str {
    if attrs.contains_key("role") {
        return "explicit";
    }
    match tag {
        "h1" | "h2" | "h3" => "heading",
        "main" => "main",
        "section" => "region",
        "input" => "textbox",
        "button" => "button",
        "a" => "link",
        "img" => "image",
        "html" | "body" | "document" => "document",
        _ => "generic",
    }
}

fn ax_name_for(node: &AtlasNode) -> String {
    node.attributes
        .get("aria-label")
        .cloned()
        .or_else(|| node.attributes.get("title").cloned())
        .or_else(|| {
            if matches!(node.tag.as_str(), "button" | "h1" | "h2" | "h3") {
                Some(node.text_value.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

fn ax_value_for(node: &AtlasNode) -> String {
    node.attributes
        .get("value")
        .cloned()
        .unwrap_or_else(|| node.text_value.clone())
}

fn style_subset_for(node: &AtlasNode) -> BTreeMap<String, String> {
    let mut subset = BTreeMap::new();
    if let Some(style) = node.attributes.get("style") {
        for part in style.split(';') {
            if let Some((key, value)) = part.split_once(':') {
                let key = key.trim().to_ascii_lowercase();
                if matches!(
                    key.as_str(),
                    "display" | "position" | "width" | "height" | "color" | "background"
                ) {
                    subset.insert(key, value.trim().to_string());
                }
            }
        }
    }
    if node.tag == "body" {
        subset.insert("color-scheme".to_string(), "dark".to_string());
    }
    subset
}

fn collect_resources(nodes: &[AtlasNode]) -> Vec<AtlasResource> {
    let mut resources = Vec::new();
    for node in nodes {
        let maybe = match node.tag.as_str() {
            "img" => node.attributes.get("src").map(|url| ("image", url.clone())),
            "script" => node.attributes.get("src").map(|url| ("script", url.clone())),
            "link" => node.attributes.get("href").map(|url| ("link", url.clone())),
            "style" => Some(("style", "inline-style".to_string())),
            "meta" if node.attributes.contains_key("http-equiv") => {
                Some(("policy", "content-security-policy".to_string()))
            }
            _ => None,
        };
        if let Some((kind, url)) = maybe {
            let evidence_hash = hash_json(&(kind, &url, &node.atlas_ref));
            resources.push(AtlasResource {
                atlas_ref: format!("resource:{evidence_hash}"),
                kind: kind.to_string(),
                url,
                evidence_hash,
            });
        }
    }
    resources
}

fn attach_resource_refs(nodes: &mut [AtlasNode], resources: &[AtlasResource]) {
    for node in nodes {
        for resource in resources {
            let matches_node = match node.tag.as_str() {
                "style" => resource.kind == "style",
                "meta" => resource.kind == "policy",
                "img" => resource.kind == "image",
                "script" => resource.kind == "script",
                "link" => resource.kind == "link",
                _ => false,
            };
            if matches_node {
                node.resource_refs.push(resource.atlas_ref.clone());
            }
        }
        node.evidence_hash = node_evidence_hash(node);
    }
}

fn coverage_report(nodes: &[AtlasNode], resources: &[AtlasResource]) -> AtlasCoverageReport {
    let dom_node_count = nodes.len();
    let ax_node_count = nodes
        .iter()
        .filter(|node| !node.role.is_empty() && node.role != "generic")
        .count();
    let layout_node_count = nodes.iter().filter(|node| node.bounds.width > 0 && node.bounds.height > 0).count();
    let styled_node_count = nodes.iter().filter(|node| !node.style_subset.is_empty()).count();
    let resource_count = resources.len();
    let mut blind_spots = Vec::new();
    blind_spots.push("runtime JavaScript mutations require WebView2 execution capture in a later pass".to_string());
    blind_spots.push("native screenshot/crop refs are represented by layout bounds until the visual crop store is wired".to_string());
    blind_spots.push("full platform accessibility tree is approximated from roles/labels for the fixture".to_string());
    let mut report = AtlasCoverageReport {
        dom_node_count,
        ax_node_count,
        layout_node_count,
        styled_node_count,
        resource_count,
        dom_ratio: ratio(dom_node_count, dom_node_count),
        ax_ratio: ratio(ax_node_count, dom_node_count),
        layout_ratio: ratio(layout_node_count, dom_node_count),
        style_ratio: ratio(styled_node_count, dom_node_count),
        resource_ratio: if resource_count > 0 { 100 } else { 0 },
        blind_spots,
        proof_hash: String::new(),
    };
    report.proof_hash = hash_json(&(
        report.dom_node_count,
        report.ax_node_count,
        report.layout_node_count,
        report.styled_node_count,
        report.resource_count,
        &report.blind_spots,
    ));
    report
}

fn ratio(part: usize, total: usize) -> u8 {
    if total == 0 {
        0
    } else {
        ((part * 100) / total).min(100) as u8
    }
}

fn node_evidence_hash(node: &AtlasNode) -> String {
    hash_json(&(
        &node.atlas_ref,
        &node.frame_path,
        &node.backend_node_id,
        &node.parent,
        &node.children,
        &node.tag,
        &node.role,
        &node.ax_name,
        &node.ax_value,
        &node.text_value,
        &node.attributes,
        &node.bounds,
        &node.style_subset,
        &node.resource_refs,
    ))
}

fn stable_u64(value: &str) -> u64 {
    let hash = Sha256::digest(value.as_bytes());
    u64::from_le_bytes(hash[0..8].try_into().expect("hash slice"))
}

fn hash_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).expect("serialize web atlas input");
    sha256_hex(&bytes)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_webatlas_is_deterministic() {
        let first = capture_fixture_webatlas();
        let second = capture_fixture_webatlas();

        assert_eq!(first, second);
        assert_eq!(first.schema, "ingen.webatlas.manifest.v1");
        assert!(first.node_count >= 8);
        assert_eq!(first.proof_hash.len(), 64);
    }

    #[test]
    fn fixture_webatlas_contains_ax_layout_and_policy_resource() {
        let manifest = capture_fixture_webatlas();

        assert!(manifest
            .nodes
            .iter()
            .any(|node| node.role == "textbox" && node.ax_name == "Stage 0 focus probe"));
        assert!(manifest.nodes.iter().all(|node| node.bounds.width > 0));
        assert!(manifest.resources.iter().any(|resource| resource.kind == "policy"));
        assert!(manifest.coverage.ax_ratio > 30);
        assert!(!manifest.coverage.blind_spots.is_empty());
    }

    #[test]
    fn atlas_ui_projection_selects_and_hashes_nodes() {
        let manifest = capture_fixture_webatlas();
        let projection = atlas_ui_projection(&manifest, 12);

        assert_eq!(projection.selected_index, 12);
        assert!(projection.tree_lines.contains("<input>"));
        assert!(projection.ax_summary.contains("textbox"));
        assert!(projection.action_candidates.contains("button"));
        assert_eq!(projection.projection_hash.len(), 64);
    }
}
