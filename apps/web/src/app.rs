use std::{collections::BTreeSet, rc::Rc};

use leptos::{ev, prelude::*};
#[cfg(target_arch = "wasm32")]
use mirabile_app::RealApplication;
use mirabile_app::{
    ActionSource, AppAction, AppIntent, AppReadModel, Application, ApplicationStatus, Availability,
    ControlAddress, ControlId, ControlKind,
};

#[cfg(not(target_arch = "wasm32"))]
use crate::mock_application::MockApplication;
use crate::{
    cockpit::Cockpit,
    commands::{CommandId, command_for_key, metadata},
    diagnostics::Diagnostics,
    dispatcher::{WorkbenchCoordinator, event_target_is_text_entry, execute_command},
    inspector::Inspector,
    view_host::ViewHost,
    workbench_controls::{InvalidBufferRegistry, invalid_buffer_registry, resource_save_pending},
    workspace_rail::WorkspaceRail,
};

#[component]
pub fn App() -> impl IntoView {
    #[cfg(all(target_arch = "wasm32", feature = "workbench-automation"))]
    let automation_configuration = crate::automation_bridge::configuration_from_window();
    #[cfg(all(target_arch = "wasm32", feature = "workbench-automation"))]
    let application: Rc<dyn Application> = automation_configuration.as_ref().map_or_else(
        || Rc::new(RealApplication::browser_default()) as Rc<dyn Application>,
        |configuration| Rc::new(RealApplication::indexed_db(&configuration.database_name)),
    );
    #[cfg(all(target_arch = "wasm32", not(feature = "workbench-automation")))]
    let application: Rc<dyn Application> = Rc::new(RealApplication::browser_default());
    #[cfg(not(target_arch = "wasm32"))]
    let application: Rc<dyn Application> = Rc::new(MockApplication::new());
    let model = RwSignal::new(AppReadModel::initializing());
    let dispatcher = WorkbenchCoordinator::new(application, model);
    #[cfg(all(target_arch = "wasm32", feature = "workbench-automation"))]
    if let Some(configuration) = automation_configuration {
        crate::automation_bridge::install(model, dispatcher, &configuration.database_name);
    }
    let invalid_aspect_buffers = RwSignal::new(BTreeSet::<String>::new());
    let invalid_buffers = RwSignal::new(BTreeSet::<String>::new());
    provide_context(InvalidBufferRegistry::new(invalid_buffers));

    dispatcher.initialize();

    let shortcut_dispatcher = dispatcher;
    let shortcut_listener = window_event_listener(ev::keydown, move |event| {
        let typing = event_target_is_text_entry(&event);
        let primary_modifier = event.ctrl_key() || event.meta_key();
        if let Some(command) =
            command_for_key(&event.key(), primary_modifier, event.alt_key(), typing)
        {
            event.prevent_default();
            execute_command(command, shortcut_dispatcher, model, invalid_aspect_buffers);
        }
    });
    on_cleanup(move || shortcut_listener.remove());

    view! {
        <div class="app-shell">
            {move || match model.get().status {
                ApplicationStatus::Initializing => view! {
                    <main class="startup-state" aria-labelledby="startup-title">
                        <p class="brand-mark">"MIRABILE"</p>
                        <h1 id="startup-title">"Opening your workspace"</h1>
                        <p class="muted" role="status" aria-live="polite">
                            "Initializing the application contract…"
                        </p>
                    </main>
                }.into_any(),
                ApplicationStatus::Error(error) => {
                    let retry = dispatcher;
                    view! {
                        <main class="startup-state error-state" aria-labelledby="startup-error-title">
                            <p class="brand-mark">"MIRABILE"</p>
                            <h1 id="startup-error-title">"The workspace could not open"</h1>
                            <p class="error-message" role="alert">{error.message}</p>
                            <button
                                class="button primary"
                                type="button"
                                data-mirabile-control=ControlId::APPLICATION_RETRY.to_string()
                                data-mirabile-address=ControlAddress::new(ControlId::APPLICATION_RETRY).to_string()
                                data-mirabile-kind=ControlKind::Action.as_str()
                                data-mirabile-enabled="true"
                                on:click=move |_| retry.initialize()
                            >
                                "Retry initialization"
                            </button>
                        </main>
                    }.into_any()
                }
                ApplicationStatus::Ready => view! {
                    <ReadyShell
                        model
                        dispatcher
                        invalid_aspect_buffers
                    />
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn ReadyShell(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
    invalid_aspect_buffers: RwSignal<BTreeSet<String>>,
) -> impl IntoView {
    view! {
        <header class="command-bar">
            <div class="brand-block">
                <span class="brand-mark">"MIRABILE"</span>
                <span class="adapter-badge">{application_label()}</span>
            </div>
            <nav class="view-tabs" aria-label="Available views">
                {move || {
                    let snapshot = model.get();
                    snapshot.workspace.views.into_iter().map(|summary| {
                        let active = snapshot.workspace.active_view == Some(summary.view_id);
                        let dispatch = dispatcher;
                        let address = ControlAddress::qualified(
                            ControlId::VIEW_ACTIVATE,
                            [("view", summary.view_id.to_string())],
                        ).expect("view address");
                        let origin = address.clone();
                        view! {
                            <button
                                type="button"
                                class="view-tab"
                                class:active=active
                                aria-current=active.then_some("page")
                                data-mirabile-control=ControlId::VIEW_ACTIVATE.to_string()
                                data-mirabile-view=summary.view_id.to_string()
                                data-mirabile-address=address.to_string()
                                data-mirabile-kind=ControlKind::Action.as_str()
                                data-mirabile-enabled="true"
                                on:click=move |_| dispatch.dispatch_from(
                                    AppIntent::SetActiveView { view_id: summary.view_id },
                                    ActionSource::Human,
                                    Some(origin.clone()),
                                )
                            >
                                {summary.title}
                            </button>
                        }
                    }).collect_view()
                }}
            </nav>
            <CommandActions
                model
                dispatcher
                invalid_aspect_buffers
            />
        </header>

        <div
            class="status-strip"
            aria-live="polite"
            aria-atomic="true"
            data-coordinator-running=move || dispatcher.read_model().running.to_string()
            data-trace-count=move || {
                model.track();
                dispatcher.trace().len().to_string()
            }
        >
            {move || model.get().notice.map_or_else(
                || "Application ready".to_owned(),
                |notice| notice.message,
            )}
        </div>

        <Cockpit model dispatcher />

        <div class="workstation" aria-label="Live workspace, preview, and diagnostics controls">
            <WorkspaceRail model dispatcher />
            <div class="center-workbench">
                <ViewHost model />
                <Diagnostics model dispatcher />
            </div>
            <Inspector model dispatcher invalid_aspect_buffers />
        </div>
    }
}

#[component]
fn CommandActions(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
    invalid_aspect_buffers: RwSignal<BTreeSet<String>>,
) -> impl IntoView {
    let invalid_buffers = invalid_buffer_registry();
    let refresh = dispatcher;
    let save = dispatcher;
    let cancel = dispatcher;
    let refresh_meta = metadata(CommandId::RefreshView);
    let save_meta = metadata(CommandId::SaveDraft);
    let cancel_meta = metadata(CommandId::CancelDraft);

    view! {
        <div class="command-actions" aria-label="Application commands">
            <button
                class="icon-command"
                type="button"
                data-mirabile-control=ControlId::APPLICATION_REFRESH.to_string()
                data-mirabile-address=ControlAddress::new(ControlId::APPLICATION_REFRESH).to_string()
                data-mirabile-kind=ControlKind::Action.as_str()
                data-mirabile-enabled=move || model.get().availability(AppAction::RefreshView).is_enabled().to_string()
                data-mirabile-disabled-reason=move || model.get().availability(AppAction::RefreshView).disabled_reason().map(str::to_owned)
                disabled=move || !model.get().availability(AppAction::RefreshView).is_enabled()
                title=move || command_title(refresh_meta, &model.get().availability(AppAction::RefreshView))
                on:click=move |_| refresh.dispatch(AppIntent::RefreshActiveView)
            >
                <span aria-hidden="true">"↻"</span>
                <span class="command-label">{refresh_meta.label}</span>
            </button>
            <button
                class="icon-command primary"
                type="button"
                data-mirabile-control=ControlId::DRAFT_SAVE.to_string()
                data-mirabile-address=ControlAddress::qualified(
                    ControlId::DRAFT_SAVE,
                    [("surface", "toolbar")],
                ).expect("toolbar save address").to_string()
                data-mirabile-kind=ControlKind::Action.as_str()
                data-mirabile-enabled=move || (model.get().availability(AppAction::SaveDraft).is_enabled()
                    && invalid_aspect_buffers.get().is_empty()
                    && !invalid_buffers.has_prefix("aspect.")).to_string()
                data-mirabile-disabled-reason=move || {
                    if invalid_aspect_buffers.get().is_empty() && !invalid_buffers.has_prefix("aspect.") {
                        model.get().availability(AppAction::SaveDraft).disabled_reason().map(str::to_owned)
                    } else {
                        Some("Correct invalid local values before saving".to_owned())
                    }
                }
                data-mirabile-pending=move || resource_save_pending(&model.get()).to_string()
                disabled=move || !model.get().availability(AppAction::SaveDraft).is_enabled()
                    || !invalid_aspect_buffers.get().is_empty()
                    || invalid_buffers.has_prefix("aspect.")
                title=move || command_title(save_meta, &model.get().availability(AppAction::SaveDraft))
                on:click=move |_| execute_command(CommandId::SaveDraft, save, model, invalid_aspect_buffers)
            >
                <span aria-hidden="true">"⌁"</span>
                <span class="command-label">{save_meta.label}</span>
                <kbd>{save_meta.shortcut}</kbd>
            </button>
            <button
                class="icon-command"
                type="button"
                data-mirabile-control=ControlId::DRAFT_CANCEL.to_string()
                data-mirabile-address=ControlAddress::qualified(
                    ControlId::DRAFT_CANCEL,
                    [("surface", "toolbar")],
                ).expect("toolbar cancel address").to_string()
                data-mirabile-kind=ControlKind::Action.as_str()
                data-mirabile-enabled=move || model.get().availability(AppAction::CancelDraft).is_enabled().to_string()
                data-mirabile-disabled-reason=move || model.get().availability(AppAction::CancelDraft).disabled_reason().map(str::to_owned)
                disabled=move || !model.get().availability(AppAction::CancelDraft).is_enabled()
                title=move || command_title(cancel_meta, &model.get().availability(AppAction::CancelDraft))
                on:click=move |_| execute_command(CommandId::CancelDraft, cancel, model, invalid_aspect_buffers)
            >
                <span class="command-label">{cancel_meta.label}</span>
                <kbd>{cancel_meta.shortcut}</kbd>
            </button>
        </div>
    }
}

fn command_title(
    command: &crate::commands::CommandMetadata,
    availability: &Availability,
) -> String {
    availability.disabled_reason().map_or_else(
        || format!("{} ({})", command.label, command.shortcut),
        |reason| format!("{}: {reason}", command.label),
    )
}

#[cfg(target_arch = "wasm32")]
const fn application_label() -> &'static str {
    "Real application · local calculation worker"
}

#[cfg(not(target_arch = "wasm32"))]
const fn application_label() -> &'static str {
    "Mock application · frontend test adapter"
}
