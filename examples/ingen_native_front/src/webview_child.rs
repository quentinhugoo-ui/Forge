use crate::AppWindow;
use ingen_native_front::{
    webexplorer_fixture_html, webexplorer_initialization_script, WebExplorerPolicy,
};
use std::{cell::RefCell, time::Duration};

use slint::ComponentHandle;

#[cfg(windows)]
thread_local! {
    static CHILD_PROOF: RefCell<Option<ChildProofState>> = const { RefCell::new(None) };
}

#[cfg(windows)]
struct ChildProofState {
    webview: wry::WebView,
    _bounds_timer: slint::Timer,
}

#[cfg(windows)]
pub fn maybe_attach_webview_child(window: &AppWindow, policy: WebExplorerPolicy) {
    if !std::env::args().any(|arg| arg == "--webview-child-proof") {
        return;
    }

    use wry::WebViewBuilder;

    let slint_window_handle = window.window().window_handle();
    let initial_bounds = child_bounds(window);
    let navigation_policy = policy.clone();
    let result = WebViewBuilder::new()
        .with_html(webexplorer_fixture_html())
        .with_initialization_script(webexplorer_initialization_script())
        .with_devtools(false)
        .with_navigation_handler(move |url| {
            let decision = navigation_policy.decide_navigation(&url);
            eprintln!(
                "webexplorer_navigation allowed={} reason={} proof={}",
                decision.allowed, decision.reason, decision.proof_hash
            );
            decision.allowed
        })
        .with_bounds(initial_bounds)
        .build_as_child(&slint_window_handle);

    match result {
        Ok(webview) => {
            eprintln!("webview_child=attached");
            let bounds_timer = slint::Timer::default();
            let weak = window.as_weak();
            bounds_timer.start(
                slint::TimerMode::Repeated,
                Duration::from_millis(250),
                move || {
                    if let Some(window) = weak.upgrade() {
                        sync_child_bounds(&window);
                    }
                },
            );
            CHILD_PROOF.with(|slot| {
                *slot.borrow_mut() = Some(ChildProofState {
                    webview,
                    _bounds_timer: bounds_timer,
                });
            });
            window.set_webview_status(
                format!(
                    "wry/WebView2 isolated child attached; policy={} bounds sync and focus cycle scheduled",
                    policy.proof_hash
                )
                .into(),
            );
            schedule_focus_cycle(window.as_weak());
        }
        Err(error) => {
            window.set_webview_status(format!("wry/WebView2 child proof failed: {error}").into());
        }
    }
}

#[cfg(not(windows))]
pub fn maybe_attach_webview_child(_window: &AppWindow) {}

#[cfg(windows)]
fn sync_child_bounds(window: &AppWindow) {
    let bounds = child_bounds(window);
    CHILD_PROOF.with(|slot| {
        if let Some(state) = slot.borrow().as_ref() {
            if let Err(error) = state.webview.set_bounds(bounds) {
                window.set_webview_status(format!("wry/WebView2 bounds sync failed: {error}").into());
            }
        }
    });
}

#[cfg(windows)]
fn schedule_focus_cycle(window: slint::Weak<AppWindow>) {
    slint::Timer::single_shot(Duration::from_millis(700), {
        let window = window.clone();
        move || {
            let result: Result<(), String> = CHILD_PROOF.with(|slot| {
                slot.borrow()
                    .as_ref()
                    .map(|state| state.webview.focus().map_err(|error| error.to_string()))
                    .unwrap_or_else(|| Err("WebView child proof missing".to_string()))
            });
            if let Some(window) = window.upgrade() {
                match result {
                    Ok(()) => {
                        eprintln!("webview_child=focus_ok");
                        window.set_webview_status(
                            "wry/WebView2 focus() accepted; focus_parent() scheduled".into(),
                        );
                    }
                    Err(error) => {
                        eprintln!("webview_child=focus_failed error={error}");
                        window.set_webview_status(format!("wry/WebView2 focus failed: {error}").into());
                    }
                }
            }
        }
    });

    slint::Timer::single_shot(Duration::from_millis(1400), move || {
        let result: Result<(), String> = CHILD_PROOF.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|state| state.webview.focus_parent().map_err(|error| error.to_string()))
                .unwrap_or_else(|| Err("WebView child proof missing".to_string()))
        });
        if let Some(window) = window.upgrade() {
            match result {
                Ok(()) => {
                    eprintln!("webview_child=focus_parent_ok");
                    window.set_webview_status(
                        "wry/WebView2 focus_parent() accepted; manual visual focus proof still required".into(),
                    );
                }
                Err(error) => {
                    eprintln!("webview_child=focus_parent_failed error={error}");
                    window.set_webview_status(format!("wry/WebView2 focus_parent failed: {error}").into());
                }
            }
        }
    });
}

#[cfg(windows)]
fn child_bounds(window: &AppWindow) -> wry::Rect {
    use wry::dpi::{LogicalPosition, LogicalSize};

    let physical = window.window().size();
    let logical = physical.to_logical(window.window().scale_factor());
    let width = (logical.width - 560.0).clamp(280.0, 520.0);
    let height = (logical.height - 260.0).clamp(180.0, 340.0);

    wry::Rect {
        position: LogicalPosition::new(236.0, 82.0).into(),
        size: LogicalSize::new(width, height).into(),
    }
}
