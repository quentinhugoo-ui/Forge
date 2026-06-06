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
                self.runtime_version.as_deref().unwrap_or("manual")
            )
        } else {
            format!(
                "webview unavailable: {}",
                self.error.as_deref().unwrap_or("unsupported platform")
            )
        }
    }

    pub fn synthetic_windows_capability() -> Self {
        Self {
            compile_time_capability: true,
            backend: "wry/WebView2".to_string(),
            builder_probe: "WebViewBuilder::new().with_html(local_fixture)".to_string(),
            child_attach_mode: "--webview-child-proof".to_string(),
            runtime_version: Some("synthetic".to_string()),
            child_view_required: true,
            fixture_path: "fixtures/webview_stage0.html".to_string(),
            focus_resize_proof_required: true,
            error: None,
        }
    }
}

pub fn run_webview_probe() -> WebViewProbe {
    #[cfg(windows)]
    {
        let _builder = wry::WebViewBuilder::new().with_html(WEBVIEW_STAGE0_FIXTURE);
        WebViewProbe {
            compile_time_capability: true,
            backend: "wry/WebView2".to_string(),
            builder_probe: "WebViewBuilder::new().with_html(local_fixture)".to_string(),
            child_attach_mode: "--webview-child-proof".to_string(),
            runtime_version: None,
            child_view_required: true,
            fixture_path: "fixtures/webview_stage0.html".to_string(),
            focus_resize_proof_required: true,
            error: None,
        }
    }

    #[cfg(not(windows))]
    {
        WebViewProbe {
            compile_time_capability: false,
            backend: "wry".to_string(),
            builder_probe: "not built on this platform".to_string(),
            child_attach_mode: "windows-only".to_string(),
            runtime_version: None,
            child_view_required: true,
            fixture_path: "fixtures/webview_stage0.html".to_string(),
            focus_resize_proof_required: true,
            error: Some("Stage 0 WebView2 probe is Windows-first".to_string()),
        }
    }
}

#[cfg(windows)]
const WEBVIEW_STAGE0_FIXTURE: &str = include_str!("../fixtures/webview_stage0.html");
