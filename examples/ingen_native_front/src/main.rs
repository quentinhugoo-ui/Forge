#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use ingen_native_front::{
    atlas_ui_projection, build_cutover_audit_report, build_obsolete_front_manifest,
    banger_frame_image, build_stage0_report, local_service_command, local_service_snapshot,
    product_section_projection,
    render_banger_viewport_frame, spawn_fake_long_job, stage0_report_json, AtlasManifest,
    BangerViewportFrame, NativeAgentCard, NativeAgentCardKind, NativeServiceCommand,
    NativeSessionSummary, NativeStateKernel, NativeUiEvent, ProductSectionsManifest,
    ServiceSnapshot, ServiceStreamEvent, WebExplorerPolicy,
};
use slint::winit_030::WinitWindowAccessor;
use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Duration};

slint::include_modules!();

mod webview_child;

fn main() -> Result<(), slint::PlatformError> {
    if std::env::args().any(|arg| arg == "--obsolete-front-report") {
        let manifest = build_obsolete_front_manifest();
        println!(
            "{}",
            serde_json::to_string_pretty(&manifest)
                .expect("serialize obsolete front manifest")
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--cutover-audit") {
        let audit = build_cutover_audit_report();
        println!(
            "{}",
            serde_json::to_string_pretty(&audit)
                .expect("serialize cutover audit report")
        );
        if !audit.cutover_ready {
            std::process::exit(2);
        }
        return Ok(());
    }

    let report = build_stage0_report();
    if std::env::args().any(|arg| arg == "--report") {
        println!("{}", stage0_report_json(&report));
        return Ok(());
    }

    slint::BackendSelector::new()
        .backend_name("winit".to_string())
        .with_winit_window_attributes_hook(|attributes| attributes.with_decorations(false))
        .select()?;

    let window = AppWindow::new()?;
    window
        .window()
        .with_winit_window(|winit_window| winit_window.set_decorations(false));
    let state = Rc::new(RefCell::new(NativeStateKernel::default()));
    let atlas_manifest = Rc::new(report.web_atlas.clone());
    let atlas_selected_index = Rc::new(RefCell::new(report.web_atlas_ui.selected_index));
    let product_sections = Rc::new(report.product_sections.clone());
    let streams: Rc<RefCell<Vec<mpsc::Receiver<ServiceStreamEvent>>>> =
        Rc::new(RefCell::new(Vec::new()));

    apply_report(&window, &report);
    apply_service_snapshot(&mut state.borrow_mut(), &local_service_snapshot());
    let banger_frame = render_banger_viewport_frame();
    apply_banger_viewport_frame(&window, &mut state.borrow_mut(), &banger_frame);
    apply_atlas_ui_projection(&window, &atlas_manifest, *atlas_selected_index.borrow());
    apply_state_projection(&window, &state.borrow());
    apply_product_section_projection(&window, &product_sections, window.get_active_section().as_str());
    window.set_chat_text("".into());
    let child_proof_mode = std::env::args().any(|arg| arg == "--webview-child-proof");
    if child_proof_mode {
        window.show()?;
    }
    webview_child::maybe_attach_webview_child(&window, WebExplorerPolicy::default_locked());
    start_service_stream_poller(&window, Rc::clone(&state), Rc::clone(&streams));

    let weak = window.as_weak();
    let state_for_refresh = Rc::clone(&state);
    let atlas_for_refresh = Rc::clone(&atlas_manifest);
    let atlas_index_for_refresh = Rc::clone(&atlas_selected_index);
    let product_sections_for_refresh = Rc::clone(&product_sections);
    window.on_refresh_probes(move || {
        if let Some(window) = weak.upgrade() {
            let report = build_stage0_report();
            apply_report(&window, &report);
            apply_service_snapshot(&mut state_for_refresh.borrow_mut(), &local_service_snapshot());
            let banger_frame = render_banger_viewport_frame();
            apply_banger_viewport_frame(&window, &mut state_for_refresh.borrow_mut(), &banger_frame);
            apply_atlas_ui_projection(&window, &atlas_for_refresh, *atlas_index_for_refresh.borrow());
            apply_state_projection(&window, &state_for_refresh.borrow());
            apply_product_section_projection(
                &window,
                &product_sections_for_refresh,
                window.get_active_section().as_str(),
            );
            eprintln!("{}", stage0_report_json(&report));
        }
    });

    let weak = window.as_weak();
    let state_for_nav = Rc::clone(&state);
    let product_sections_for_nav = Rc::clone(&product_sections);
    window.on_navigate(move |section| {
        if let Some(window) = weak.upgrade() {
            state_for_nav.borrow_mut().dispatch(NativeUiEvent::Navigate {
                section: section.to_string(),
            });
            apply_state_projection(&window, &state_for_nav.borrow());
            apply_product_section_projection(
                &window,
                &product_sections_for_nav,
                window.get_active_section().as_str(),
            );
        }
    });

    let weak = window.as_weak();
    let state_for_imported_control = Rc::clone(&state);
    window.on_activate_imported_control(move |label| {
        if let Some(window) = weak.upgrade() {
            let label = label.to_string();
            let section = window.get_active_section().to_string();
            let (title, body) = if label == "promotion gate" {
                (
                    "Promotion blocked".to_string(),
                    format!(
                        "Native section '{section}' captured the promotion gate, but design parity is still false. The Tauri visual reference remains protected."
                    ),
                )
            } else if section == "banger" && label.contains("Banger") {
                let banger_frame = render_banger_viewport_frame();
                apply_banger_viewport_frame(
                    &window,
                    &mut state_for_imported_control.borrow_mut(),
                    &banger_frame,
                );
                (
                    "Banger native viewport".to_string(),
                    format!(
                        "frame={} bridge={} contract={}",
                        banger_frame.frame_hash,
                        banger_frame.slint_texture_bridge.proof_hash,
                        banger_frame.viewport_contract_hash
                    ),
                )
            } else {
                let result = local_service_command(NativeServiceCommand::CaptureControl {
                    section: section.clone(),
                    label: label.clone(),
                });
                (
                    "Native control captured".to_string(),
                    format!(
                        "{} proof={}",
                        result.message, result.proof_hash
                    ),
                )
            };
            state_for_imported_control
                .borrow_mut()
                .dispatch(NativeUiEvent::OpenModal { title, body });
            apply_state_projection(&window, &state_for_imported_control.borrow());
        }
    });

    let weak = window.as_weak();
    window.on_window_close(move || {
        if let Some(window) = weak.upgrade() {
            let _ = window.window().hide();
        }
    });

    let weak = window.as_weak();
    window.on_window_minimize(move || {
        if let Some(window) = weak.upgrade() {
            window.window().set_minimized(true);
        }
    });

    let weak = window.as_weak();
    window.on_window_toggle_maximize(move || {
        if let Some(window) = weak.upgrade() {
            let native_window = window.window();
            native_window.set_maximized(!native_window.is_maximized());
        }
    });

    let weak = window.as_weak();
    let state_for_next = Rc::clone(&state);
    let product_sections_for_next = Rc::clone(&product_sections);
    window.on_navigate_next(move || {
        if let Some(window) = weak.upgrade() {
            state_for_next.borrow_mut().dispatch(NativeUiEvent::NavigateNext);
            apply_state_projection(&window, &state_for_next.borrow());
            apply_product_section_projection(
                &window,
                &product_sections_for_next,
                window.get_active_section().as_str(),
            );
        }
    });

    let weak = window.as_weak();
    let atlas_for_prev = Rc::clone(&atlas_manifest);
    let atlas_index_for_prev = Rc::clone(&atlas_selected_index);
    window.on_atlas_previous_node(move || {
        if let Some(window) = weak.upgrade() {
            let mut index = atlas_index_for_prev.borrow_mut();
            *index = index.saturating_sub(1);
            apply_atlas_ui_projection(&window, &atlas_for_prev, *index);
        }
    });

    let weak = window.as_weak();
    let atlas_for_next_node = Rc::clone(&atlas_manifest);
    let atlas_index_for_next_node = Rc::clone(&atlas_selected_index);
    window.on_atlas_next_node(move || {
        if let Some(window) = weak.upgrade() {
            let mut index = atlas_index_for_next_node.borrow_mut();
            let max = atlas_for_next_node.nodes.len().saturating_sub(1);
            *index = (*index + 1).min(max);
            apply_atlas_ui_projection(&window, &atlas_for_next_node, *index);
        }
    });

    let weak = window.as_weak();
    let state_for_chat = Rc::clone(&state);
    let streams_for_chat = Rc::clone(&streams);
    window.on_send_chat(move || {
        if let Some(window) = weak.upgrade() {
            let draft = window.get_chat_text().to_string();
            state_for_chat
                .borrow_mut()
                .dispatch(NativeUiEvent::ChatDraftChanged {
                    draft: draft.clone(),
                });
            state_for_chat.borrow_mut().dispatch(NativeUiEvent::SendChat);
            if !draft.trim().is_empty() {
                let result = local_service_command(NativeServiceCommand::SubmitChat {
                    draft: draft.trim().to_string(),
                });
                for job in result.emitted_jobs {
                    state_for_chat
                        .borrow_mut()
                        .dispatch(NativeUiEvent::JobUpserted { job });
                }
                state_for_chat
                    .borrow_mut()
                    .dispatch(NativeUiEvent::ProofBadgeRaised {
                        badge: format!("service-command-proof={}", result.proof_hash),
                    });
                streams_for_chat
                    .borrow_mut()
                    .push(spawn_fake_long_job(draft.trim().to_string()));
            }
            apply_state_projection(&window, &state_for_chat.borrow());
            window.set_chat_text(state_for_chat.borrow().chat_draft().into());
        }
    });

    let weak = window.as_weak();
    let state_for_modal = Rc::clone(&state);
    window.on_close_modal(move || {
        if let Some(window) = weak.upgrade() {
            state_for_modal.borrow_mut().dispatch(NativeUiEvent::CloseModal);
            apply_state_projection(&window, &state_for_modal.borrow());
        }
    });

    eprintln!("{}", stage0_report_json(&report));
    if child_proof_mode {
        slint::run_event_loop()
    } else {
        window.run()
    }
}

fn start_service_stream_poller(
    window: &AppWindow,
    state: Rc<RefCell<NativeStateKernel>>,
    streams: Rc<RefCell<Vec<mpsc::Receiver<ServiceStreamEvent>>>>,
) {
    let timer = slint::Timer::default();
    let weak = window.as_weak();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            if let Some(window) = weak.upgrade() {
                poll_service_streams(&window, &state, &streams);
            }
        },
    );
    // Keep the timer alive for the lifetime of the process.
    std::mem::forget(timer);
}

fn poll_service_streams(
    window: &AppWindow,
    state: &Rc<RefCell<NativeStateKernel>>,
    streams: &Rc<RefCell<Vec<mpsc::Receiver<ServiceStreamEvent>>>>,
) {
    let mut should_project = false;
    let mut streams = streams.borrow_mut();
    streams.retain_mut(|receiver| {
        loop {
            match receiver.try_recv() {
                Ok(event) => {
                    state
                        .borrow_mut()
                        .dispatch(NativeUiEvent::JobUpserted { job: event.job });
                    should_project = true;
                }
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => return false,
            }
        }
    });
    drop(streams);
    if should_project {
        apply_state_projection(window, &state.borrow());
    }
}

fn apply_report(window: &AppWindow, report: &ingen_native_front::Stage0Report) {
    window.set_proof_status(
        format!(
            "proof={} promotion_ready={}",
            report.proof_hash, report.promotion_ready
        )
        .into(),
    );
    window.set_gpu_status(report.wgpu.summary().into());
    window.set_webview_status(report.webview.summary().into());
}

fn apply_state_projection(window: &AppWindow, state: &NativeStateKernel) {
    let projection = state.projection();
    window.set_active_section(projection.active_section.into());
    window.set_section_title(projection.section_title.into());
    window.set_canvas_title(projection.canvas_title.into());
    window.set_canvas_hint(projection.canvas_hint.into());
    window.set_session_lines(projection.session_lines.into());
    window.set_transcript_lines(projection.transcript_lines.into());
    window.set_plan_cards(projection.plan_cards.into());
    window.set_questionnaire_cards(projection.questionnaire_cards.into());
    window.set_proof_cards(projection.proof_cards.into());
    window.set_agent_surface_status(projection.agent_surface_status.into());
    window.set_hardware_status(projection.hardware_status.into());
    window.set_provider_status(projection.provider_status.into());
    window.set_job_status(projection.job_status.into());
    window.set_proof_badge_status(projection.proof_badge_status.into());
    window.set_brain_status(projection.brain_status.into());
    window.set_monster_status(projection.monster_status.into());
    window.set_panel_status(projection.panel_status.into());
    window.set_native_webexplorer_status(projection.webexplorer_status.into());
    window.set_banger_status(projection.banger_status.into());
    window.set_trading_status(projection.trading_status.into());
    window.set_real_estate_status(projection.real_estate_status.into());
    window.set_modal_visible(projection.modal_visible);
    window.set_modal_title(projection.modal_title.into());
    window.set_modal_body(projection.modal_body.into());
    let current = window.get_proof_status().to_string();
    let base_status = current
        .split(" ui_events=")
        .next()
        .unwrap_or(&current)
        .to_string();
    window.set_proof_status(
        format!(
            "{} ui_events={} ui_hash={}",
            base_status, projection.event_count, projection.projection_hash
        )
        .into(),
    );
}

fn apply_banger_viewport_frame(
    window: &AppWindow,
    state: &mut NativeStateKernel,
    frame: &BangerViewportFrame,
) {
    window.set_banger_frame_image(banger_frame_image(frame));
    window.set_banger_frame_hash(frame.frame_hash.clone().into());
    window.set_banger_bridge_hash(frame.slint_texture_bridge.proof_hash.clone().into());
    state.dispatch(NativeUiEvent::BangerViewportUpdated {
        visible: true,
        frame_hash: Some(frame.frame_hash.clone()),
    });
    state.dispatch(NativeUiEvent::ProofBadgeRaised {
        badge: format!("banger-frame={}", frame.frame_hash),
    });
}

fn apply_atlas_ui_projection(window: &AppWindow, manifest: &AtlasManifest, selected_index: usize) {
    let projection = atlas_ui_projection(manifest, selected_index);
    window.set_atlas_tree_lines(projection.tree_lines.into());
    window.set_atlas_selected_summary(projection.selected_summary.into());
    window.set_atlas_ax_summary(projection.ax_summary.into());
    window.set_atlas_layout_summary(projection.layout_summary.into());
    window.set_atlas_resource_lines(projection.resource_lines.into());
    window.set_atlas_action_candidates(projection.action_candidates.into());
    window.set_atlas_blind_spot_lines(projection.blind_spot_lines.into());
    window.set_atlas_proof_summary(projection.proof_summary.into());
    window.set_atlas_search_summary(
        format!(
            "{} ui_hash={}",
            projection.search_summary, projection.projection_hash
        )
        .into(),
    );
}

fn apply_product_section_projection(
    window: &AppWindow,
    manifest: &ProductSectionsManifest,
    active_section: &str,
) {
    let projection = product_section_projection(manifest, active_section);
    window.set_product_section_title(projection.title.into());
    window.set_product_section_status(projection.status.into());
    window.set_product_section_metrics(projection.metric_lines.into());
    window.set_product_section_cards(projection.card_lines.into());
    window.set_product_section_actions(projection.action_lines.into());
    window.set_product_section_proof(
        format!(
            "{} ui_hash={}",
            projection.proof_summary, projection.projection_hash
        )
        .into(),
    );
}

fn apply_service_snapshot(state: &mut NativeStateKernel, snapshot: &ServiceSnapshot) {
    let sessions = snapshot
        .sessions
        .iter()
        .map(|session| NativeSessionSummary {
            id: session.id.clone(),
            title: session.title.clone(),
        })
        .collect();
    state.dispatch(NativeUiEvent::SessionsReplaced { sessions });
    state.dispatch(NativeUiEvent::HardwareUpdated {
        status: format!(
            "hardware os={} gpu={} proof={}",
            snapshot.hardware.os, snapshot.hardware.gpu, snapshot.hardware.proof_hash
        ),
    });
    state.dispatch(NativeUiEvent::ProviderUpdated {
        provider: snapshot.provider.provider.clone(),
        model: snapshot.provider.model.clone(),
        online: snapshot.provider.online,
    });
    for job in &snapshot.jobs {
        state.dispatch(NativeUiEvent::JobUpserted { job: job.clone() });
    }
    state.dispatch(NativeUiEvent::BrainUpdated {
        status: snapshot.brain_status.clone(),
    });
    state.dispatch(NativeUiEvent::MonsterUpdated {
        status: snapshot.monster_status.clone(),
    });
    state.dispatch(NativeUiEvent::WebExplorerUpdated {
        visible: false,
        focused: false,
        url: Some(snapshot.webexplorer_status.clone()),
    });
    state.dispatch(NativeUiEvent::BangerServiceUpdated {
        status: snapshot.banger_status.clone(),
    });
    state.dispatch(NativeUiEvent::TradingUpdated {
        status: snapshot.trading_status.clone(),
    });
    state.dispatch(NativeUiEvent::RealEstateUpdated {
        status: snapshot.real_estate_status.clone(),
    });
    state.dispatch(NativeUiEvent::ProofBadgeRaised {
        badge: format!("service-proof={}", snapshot.proof_hash),
    });
    state.dispatch(NativeUiEvent::AgentCardUpserted {
        card: NativeAgentCard {
            id: "service-snapshot-proof".to_string(),
            kind: NativeAgentCardKind::Proof,
            title: "Direct service snapshot".to_string(),
            body: "Provider, sessions, jobs and product service status entered through Rust traits."
                .to_string(),
            proof_hash: Some(snapshot.proof_hash.clone()),
        },
    });
}
