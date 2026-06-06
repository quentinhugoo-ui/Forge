use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebViewProbe {
    pub compile_time_capability: bool,
    pub backend: String,
    pub builder_probe: String,
    pub child_attach_mode: String,
    pub runtime_version: Option<String>,
    pub child_view_required: bool,
    pub fixture_path: String,
    pub focus_resize_proof_required: bool,
    pub error: Option<String>,
}

impl WebViewProbe {
    pub fn summary(&self) -> String {
        if self.compile_time_capability {
            format!(
                "{} fixture={} runtime={}",
                self.backend,
                self.fixture_path,
                self.runtime_version.as_deref().unwrap_or("native")
            )
        } else {
            format!(
                "external-web unavailable: {}",
                self.error.as_deref().unwrap_or("disabled")
            )
        }
    }

    pub fn synthetic_windows_capability() -> Self {
        Self {
            compile_time_capability: true,
            backend: "native-external-web-stub".to_string(),
            builder_probe: "native policy probe".to_string(),
            child_attach_mode: "disabled-until-contained-runtime".to_string(),
            runtime_version: Some("synthetic".to_string()),
            child_view_required: false,
            fixture_path: "native-webexplorer-fixture".to_string(),
            focus_resize_proof_required: false,
            error: None,
        }
    }
}

pub fn run_webview_probe() -> WebViewProbe {
    WebViewProbe {
        compile_time_capability: true,
        backend: "native-external-web-stub".to_string(),
        builder_probe: "native policy probe".to_string(),
        child_attach_mode: "disabled-until-contained-runtime".to_string(),
        runtime_version: None,
        child_view_required: false,
        fixture_path: "native-webexplorer-fixture".to_string(),
        focus_resize_proof_required: false,
        error: None,
    }
}
