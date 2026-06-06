use crate::AppWindow;
use ingen_native_front::WebExplorerPolicy;

pub fn maybe_attach_webview_child(window: &AppWindow, policy: WebExplorerPolicy) {
    window.set_webview_status(
        format!(
            "external web runtime disabled during native cutover; policy={}",
            policy.proof_hash
        )
        .into(),
    );
}
