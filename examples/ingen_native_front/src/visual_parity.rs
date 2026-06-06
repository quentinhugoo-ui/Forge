use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForgeFirstViewportParity {
    pub oracle: String,
    pub default_section: String,
    pub canvas_title: String,
    pub chat_placeholder: String,
    pub locked_geometry: Vec<String>,
    pub hidden_on_forge: Vec<String>,
    pub passed: bool,
}

pub fn forge_first_viewport_parity() -> ForgeFirstViewportParity {
    let source = include_str!("../ui/app.slint");
    let locked_geometry = vec![
        "window=1535x786".to_string(),
        "titlebar=38px".to_string(),
        "left_panel=(8,44,279,height-54)".to_string(),
        "workspace_header=(287,44,width-287,52)".to_string(),
        "canvas=(287,96,width-287,height-96)".to_string(),
        "chat=(center+143,height-156,780x99)".to_string(),
        "chat_command_square=83px+8px_inset".to_string(),
    ];
    let hidden_on_forge = vec![
        "DropCanvas migration action strip".to_string(),
        "SectionStatusDock".to_string(),
        "extra generated recents".to_string(),
    ];
    ForgeFirstViewportParity {
        oracle: "examples/ingen_native_front/ui/generated_assets/layer_forge.png".to_string(),
        default_section: "forge".to_string(),
        canvas_title: "Drop any file".to_string(),
        chat_placeholder: "Run a Monte C".to_string(),
        passed: source.contains("preferred-width: 1535px;")
            && source.contains("preferred-height: 786px;")
            && source.contains("no-frame: true;")
            && source.contains("x: 8px;")
            && source.contains("y: 44px;")
            && source.contains("width: 279px;")
            && source.contains("x: (root.width - 780px) / 2 + 143px;")
            && source.contains("placeholder-text: \"Run a Monte C\";")
            && source.contains("visible: root.active_section != \"shell\" && root.active_section != \"forge\";")
            && source.contains("visible: root.active_section == \"forge\" || root.active_section == \"shell\";"),
        locked_geometry,
        hidden_on_forge,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NativeUiState;

    #[test]
    fn forge_first_viewport_parity_contract_passes() {
        let parity = forge_first_viewport_parity();
        assert!(parity.passed);
        assert_eq!(parity.default_section, "forge");
        assert_eq!(parity.canvas_title, "Drop any file");
        assert!(parity
            .locked_geometry
            .contains(&"chat=(center+143,height-156,780x99)".to_string()));
    }

    #[test]
    fn default_projection_opens_forge_oracle_surface() {
        let projection = NativeUiState::default().projection();
        assert_eq!(projection.active_section, "forge");
        assert_eq!(projection.section_title, "New session");
        assert_eq!(projection.canvas_title, "Drop any file");
        assert!(projection.canvas_hint.starts_with("Heavy compute"));
    }
}
