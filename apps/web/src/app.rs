use std::{rc::Rc, str::FromStr};

#[cfg(test)]
use astra_app::ProjectionVersion;
#[cfg(target_arch = "wasm32")]
use astra_app::RealApplication;
use astra_app::{
    Angle, AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind, AppReadModel,
    Application, ApplicationStatus, AspectSetDraftMutation, Availability, BindingSourceSummary,
    ChartPersistence, DraftState, InstanceId, ResourceId, ViewComputationState,
};
use leptos::{ev, prelude::*};
use wasm_bindgen::JsCast;

#[cfg(not(target_arch = "wasm32"))]
use crate::mock_application::MockApplication;
use crate::{
    commands::{CommandId, command_for_key, metadata},
    render::WheelScene,
};

#[derive(Clone)]
struct AppDispatcher {
    application: Rc<dyn Application>,
    model: RwSignal<AppReadModel>,
}

impl AppDispatcher {
    fn initialize(&self) {
        let application = Rc::clone(&self.application);
        let model = self.model;
        leptos::task::spawn_local(async move {
            match application.initialize().await {
                Ok(updated) => publish_and_settle(application, model, updated).await,
                Err(error) => publish_application_error(model, error),
            }
        });
    }

    fn dispatch(&self, intent: AppIntent) {
        let application = Rc::clone(&self.application);
        let model = self.model;
        leptos::task::spawn_local(async move {
            match application.dispatch(intent).await {
                Ok(updated) => publish_and_settle(application, model, updated).await,
                Err(error) => publish_command_error(model, error),
            }
        });
    }
}

#[derive(Clone, Copy)]
struct Dispatcher {
    stored: StoredValue<AppDispatcher, LocalStorage>,
}

impl Dispatcher {
    fn new(dispatcher: AppDispatcher) -> Self {
        Self {
            stored: StoredValue::new_local(dispatcher),
        }
    }

    fn initialize(self) {
        self.stored.with_value(AppDispatcher::initialize);
    }

    fn dispatch(self, intent: AppIntent) {
        self.stored
            .with_value(|dispatcher| dispatcher.dispatch(intent));
    }
}

async fn publish_and_settle(
    application: Rc<dyn Application>,
    model: RwSignal<AppReadModel>,
    mut incoming: AppReadModel,
) {
    loop {
        let after = incoming.version;
        let pending = has_pending_work(&incoming);
        publish_projection(model, incoming);
        if !pending {
            return;
        }

        match application.wait_for_update(after).await {
            Ok(updated) if updated.version > after => incoming = updated,
            Ok(updated) => {
                publish_command_error(
                    model,
                    AppError::new(
                        AppErrorKind::Unavailable,
                        format!(
                            "Application returned projection {} while waiting after {after}",
                            updated.version
                        ),
                    ),
                );
                return;
            }
            Err(error) => {
                publish_command_error(model, error);
                return;
            }
        }
    }
}

fn publish_projection(model: RwSignal<AppReadModel>, incoming: AppReadModel) {
    model.update(|current| {
        publish_if_newer(current, incoming);
    });
}

/// Publishes only a strictly newer authoritative projection.
///
/// Equal versions are redundant copies; older versions are stale asynchronous completions.
fn publish_if_newer(current: &mut AppReadModel, incoming: AppReadModel) -> bool {
    if incoming.version > current.version {
        *current = incoming;
        true
    } else {
        false
    }
}

fn has_pending_work(model: &AppReadModel) -> bool {
    let view_pending = model.active_view.as_ref().is_some_and(|view| {
        matches!(
            view.computation,
            ViewComputationState::Loading | ViewComputationState::Refreshing
        )
    });
    let save_pending = model
        .resource_editor
        .aspect_set
        .as_ref()
        .is_some_and(|draft| matches!(draft.state, DraftState::Saving { .. }));
    view_pending || save_pending
}

fn publish_application_error(model: RwSignal<AppReadModel>, error: AppError) {
    model.update(|current| {
        current.status = ApplicationStatus::Error(error);
        current.notice = None;
    });
}

fn publish_command_error(model: RwSignal<AppReadModel>, error: AppError) {
    model.update(|current| {
        current.notice = Some(AppNotice {
            kind: if error.kind == AppErrorKind::Conflict {
                AppNoticeKind::Conflict
            } else {
                AppNoticeKind::Warning
            },
            message: error.message,
        });
    });
}

#[component]
pub fn App() -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    let application: Rc<dyn Application> = Rc::new(RealApplication::browser_default());
    #[cfg(not(target_arch = "wasm32"))]
    let application: Rc<dyn Application> = Rc::new(MockApplication::new());
    let model = RwSignal::new(AppReadModel::initializing());
    let dispatcher = Dispatcher::new(AppDispatcher { application, model });
    let orb_buffer = RwSignal::new(String::new());
    let orb_buffer_resource = RwSignal::new(None::<ResourceId>);
    let orb_error = RwSignal::new(None::<String>);

    dispatcher.initialize();

    Effect::new(move || {
        let draft = model
            .get()
            .resource_editor
            .aspect_set
            .map(|draft| (draft.resource_id, draft.conjunction.maximum_orb));
        match draft {
            Some((resource_id, maximum_orb))
                if orb_buffer_resource.get_untracked() != Some(resource_id) =>
            {
                orb_buffer.set(format_orb(maximum_orb));
                orb_buffer_resource.set(Some(resource_id));
                orb_error.set(None);
            }
            None => orb_buffer_resource.set(None),
            Some(_) => {}
        }
    });

    let shortcut_dispatcher = dispatcher;
    let shortcut_listener = window_event_listener(ev::keydown, move |event| {
        let typing = event_target_is_text_entry(&event);
        let primary_modifier = event.ctrl_key() || event.meta_key();
        if let Some(command) =
            command_for_key(&event.key(), primary_modifier, event.alt_key(), typing)
        {
            event.prevent_default();
            execute_command(command, shortcut_dispatcher, model, orb_buffer, orb_error);
        }
    });
    on_cleanup(move || shortcut_listener.remove());

    view! {
        <div class="app-shell">
            {move || match model.get().status {
                ApplicationStatus::Initializing => view! {
                    <main class="startup-state" aria-labelledby="startup-title">
                        <p class="brand-mark">"ASTRA"</p>
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
                            <p class="brand-mark">"ASTRA"</p>
                            <h1 id="startup-error-title">"The workspace could not open"</h1>
                            <p class="error-message" role="alert">{error.message}</p>
                            <button class="button primary" type="button" on:click=move |_| retry.initialize()>
                                "Retry initialization"
                            </button>
                        </main>
                    }.into_any()
                }
                ApplicationStatus::Ready => view! {
                    <ReadyShell
                        model
                        dispatcher
                        orb_buffer
                        orb_error
                    />
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn ReadyShell(
    model: RwSignal<AppReadModel>,
    dispatcher: Dispatcher,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <header class="command-bar">
            <div class="brand-block">
                <span class="brand-mark">"ASTRA"</span>
                <span class="adapter-badge">{application_label()}</span>
            </div>
            <nav class="view-tabs" aria-label="Available views">
                {move || {
                    let snapshot = model.get();
                    snapshot.workspace.views.into_iter().map(|summary| {
                        let active = snapshot.workspace.active_view == Some(summary.view_id);
                        let dispatch = dispatcher;
                        view! {
                            <button
                                type="button"
                                class="view-tab"
                                class:active=active
                                aria-current=active.then_some("page")
                                on:click=move |_| dispatch.dispatch(AppIntent::SetActiveView {
                                    view_id: summary.view_id,
                                })
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
                orb_buffer
                orb_error
            />
        </header>

        <div class="status-strip" aria-live="polite" aria-atomic="true">
            {move || model.get().notice.map_or_else(
                || "Application ready".to_owned(),
                |notice| notice.message,
            )}
        </div>

        <div class="workstation">
            <WorkspaceRail model dispatcher />
            <ViewHost model />
            <Inspector model dispatcher orb_buffer orb_error />
        </div>
    }
}

#[component]
fn CommandActions(
    model: RwSignal<AppReadModel>,
    dispatcher: Dispatcher,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) -> impl IntoView {
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
                disabled=move || !model.get().availability(AppAction::SaveDraft).is_enabled()
                title=move || command_title(save_meta, &model.get().availability(AppAction::SaveDraft))
                on:click=move |_| execute_command(CommandId::SaveDraft, save, model, orb_buffer, orb_error)
            >
                <span aria-hidden="true">"⌁"</span>
                <span class="command-label">{save_meta.label}</span>
                <kbd>{save_meta.shortcut}</kbd>
            </button>
            <button
                class="icon-command"
                type="button"
                disabled=move || !model.get().availability(AppAction::CancelDraft).is_enabled()
                title=move || command_title(cancel_meta, &model.get().availability(AppAction::CancelDraft))
                on:click=move |_| execute_command(CommandId::CancelDraft, cancel, model, orb_buffer, orb_error)
            >
                <span class="command-label">{cancel_meta.label}</span>
                <kbd>{cancel_meta.shortcut}</kbd>
            </button>
        </div>
    }
}

#[component]
fn WorkspaceRail(model: RwSignal<AppReadModel>, dispatcher: Dispatcher) -> impl IntoView {
    view! {
        <nav
            id="workspace-chart-rail"
            class="chart-rail"
            aria-labelledby="workspace-title"
            tabindex="-1"
        >
            <div class="panel-title-row">
                <div>
                    <p class="section-kicker">"WORKSPACE"</p>
                    <h2 id="workspace-title">"Open charts"</h2>
                </div>
                <span class="count-badge">{move || model.get().workspace.charts.len()}</span>
            </div>
            <p class="rail-legend">
                <span><span class="legend-dot active-dot"></span>"Active"</span>
                <span><span class="legend-dot selected-dot"></span>"Selected"</span>
            </p>
            <ul class="chart-list">
                {move || {
                    let snapshot = model.get();
                    snapshot.workspace.charts.into_iter().map(|chart| {
                        let active = snapshot.workspace.active_chart == Some(chart.instance_id);
                        let selected = snapshot.workspace.selected_charts.contains(&chart.instance_id);
                        let activate = dispatcher;
                        let select = dispatcher;
                        let close = dispatcher;
                        let select_label = format!("Select {}", chart.title);
                        let close_label = format!("Close {}", chart.title);
                        let persistence = match chart.persistence {
                            ChartPersistence::Saved { .. } => "Saved",
                            ChartPersistence::Ephemeral => "Ephemeral",
                        };
                        view! {
                            <li class="chart-row" class:active=active class:selected=selected>
                                <input
                                    class="selection-check"
                                    type="checkbox"
                                    prop:checked=selected
                                    aria-label=select_label
                                    on:change=move |event| select.dispatch(AppIntent::SetChartSelection {
                                        instance_id: chart.instance_id,
                                        selected: event_target_checked(&event),
                                    })
                                />
                                <button
                                    class="chart-activate"
                                    type="button"
                                    aria-current=active.then_some("page")
                                    on:click=move |_| activate.dispatch(AppIntent::ActivateChart {
                                        instance_id: chart.instance_id,
                                    })
                                >
                                    <span class="chart-title">{chart.title}</span>
                                    <span class="chart-subtitle">{chart.subtitle}</span>
                                    <span class="chart-state">
                                        {persistence}
                                        {if active { " · Active" } else { "" }}
                                        {if selected { " · Selected" } else { "" }}
                                    </span>
                                </button>
                                <button
                                    class="close-chart"
                                    type="button"
                                    aria-label=close_label
                                    on:click=move |_| close.dispatch(AppIntent::CloseChart {
                                        instance_id: chart.instance_id,
                                    })
                                >"×"</button>
                            </li>
                        }
                    }).collect_view()
                }}
            </ul>

            <section class="library-shelf" aria-labelledby="library-title">
                <p class="section-kicker">"LIBRARY"</p>
                <h3 id="library-title">"Chart records"</h3>
                <ul>
                    {move || {
                        let snapshot = model.get();
                        snapshot.library.charts.into_iter().map(|chart| {
                            let is_open = snapshot.workspace.charts.iter().any(|open| {
                                matches!(open.persistence, ChartPersistence::Saved { definition_id } if definition_id == chart.definition_id)
                            });
                            let dispatch = dispatcher;
                            let label = if is_open {
                                format!("Activate {}", chart.title)
                            } else {
                                format!("Open {}", chart.title)
                            };
                            view! {
                                <li>
                                    <button
                                        class="library-chart"
                                        type="button"
                                        aria-label=label
                                        on:click=move |_| dispatch.dispatch(AppIntent::OpenChart {
                                            definition_id: chart.definition_id,
                                        })
                                    >
                                        <span>{chart.title}</span>
                                        <small>{if is_open { "Open" } else { "+ Open" }}</small>
                                    </button>
                                </li>
                            }
                        }).collect_view()
                    }}
                </ul>
            </section>
        </nav>
    }
}

#[component]
fn ViewHost(model: RwSignal<AppReadModel>) -> impl IntoView {
    view! {
        <main class="view-host" aria-labelledby="active-view-title">
            {move || model.get().active_view.map_or_else(
                || view! {
                    <section class="empty-view">
                        <h1 id="active-view-title">"No active view"</h1>
                        <p>"Choose a view to begin."</p>
                    </section>
                }.into_any(),
                |active_view| {
                    let state_label = match &active_view.computation {
                        ViewComputationState::Loading => "Loading",
                        ViewComputationState::Fresh => "Fresh",
                        ViewComputationState::Refreshing => "Refreshing",
                        ViewComputationState::Failed(_) => "Refresh failed",
                    };
                    let status_class = match &active_view.computation {
                        ViewComputationState::Fresh => "fresh",
                        ViewComputationState::Loading | ViewComputationState::Refreshing => "pending",
                        ViewComputationState::Failed(_) => "failed",
                    };
                    let scene = active_view.scene.clone();
                    let computation = active_view.computation.clone();
                    view! {
                        <section class="view-stage">
                            <header class="view-heading">
                                <div>
                                    <p class="section-kicker">"ACTIVE VIEW INSTANCE"</p>
                                    <h1 id="active-view-title">{active_view.title.clone()}</h1>
                                </div>
                                <span class=format!("view-status {status_class}")>{state_label}</span>
                            </header>

                            <div class="scene-frame" aria-busy=matches!(computation, ViewComputationState::Loading | ViewComputationState::Refreshing)>
                                {scene.map_or_else(
                                    || view! {
                                        <div class="scene-loading" role="status">
                                            <span class="loading-orbit" aria-hidden="true"></span>
                                            <p>"Calculating the first Scene…"</p>
                                        </div>
                                    }.into_any(),
                                    |scene| view! {
                                        <WheelScene
                                            scene
                                            title=format!("{} chart scene", active_view.title)
                                            description="A deterministic application Scene of chart rings, point labels, and aspect lines."
                                        />
                                    }.into_any(),
                                )}
                                {matches!(computation, ViewComputationState::Refreshing).then(|| view! {
                                    <div class="refresh-overlay" role="status" aria-live="polite">
                                        <span class="pulse-dot" aria-hidden="true"></span>
                                        "Refreshing analysis · last good Scene retained"
                                    </div>
                                })}
                            </div>

                            {match computation {
                                ViewComputationState::Failed(error) => Some(view! {
                                    <div class="view-error" role="alert">
                                        <strong>"View refresh failed"</strong>
                                        <span>{error.message}</span>
                                    </div>
                                }),
                                ViewComputationState::Loading | ViewComputationState::Fresh | ViewComputationState::Refreshing => None,
                            }}
                            <footer class="view-footnote">
                                "Scene is a presentation projection. No astrology is calculated in this component."
                            </footer>
                        </section>
                    }.into_any()
                },
            )}
        </main>
    }
}

#[component]
#[allow(clippy::too_many_lines)]
fn Inspector(
    model: RwSignal<AppReadModel>,
    dispatcher: Dispatcher,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) -> impl IntoView {
    let aspect_dispatcher = dispatcher;
    let edit_dispatcher = dispatcher;
    let orb_dispatcher = dispatcher;
    let enabled_dispatcher = dispatcher;
    let save_dispatcher = dispatcher;
    let cancel_dispatcher = dispatcher;

    view! {
        <aside class="inspector" aria-labelledby="inspector-title">
            <div class="panel-title-row">
                <div>
                    <p class="section-kicker">"CONTEXT"</p>
                    <h2 id="inspector-title">"Inspector"</h2>
                </div>
            </div>

            <section class="inspector-section" aria-labelledby="active-chart-inspector-title">
                <h3 id="active-chart-inspector-title">"Active chart"</h3>
                {move || model.get().inspector.active_chart.map_or_else(
                    || view! { <p class="muted">"No active chart"</p> }.into_any(),
                    |chart| {
                        let state = match chart.persistence {
                            ChartPersistence::Saved { .. } => "Saved library chart",
                            ChartPersistence::Ephemeral => "Ephemeral working chart",
                        };
                        view! {
                            <div class="active-chart-card">
                                <strong>{chart.title}</strong>
                                <span>{chart.subtitle}</span>
                                <small>{state}</small>
                            </div>
                        }.into_any()
                    },
                )}
            </section>

            <section class="inspector-section" aria-labelledby="slot-title">
                <h3 id="slot-title">"Chart slots"</h3>
                {move || {
                    let snapshot = model.get();
                    snapshot.active_view.map(|active_view| {
                        active_view.slots.into_iter().map(|assignment| {
                            let dispatch = dispatcher;
                            let view_id = active_view.view_id;
                            let slot = assignment.slot.clone();
                            let current = assignment.chart.map_or_else(String::new, |id| id.to_string());
                            view! {
                                <label class="field-label">
                                    <span>
                                        {assignment.label}
                                        {assignment.required.then_some(" · Required")}
                                    </span>
                                    <select
                                        prop:value=current
                                        on:change=move |event| {
                                            let value = event_target_value(&event);
                                            let chart = if value.is_empty() {
                                                None
                                            } else {
                                                InstanceId::from_str(&value).ok()
                                            };
                                            dispatch.dispatch(AppIntent::AssignChartSlot {
                                                view_id,
                                                slot: slot.clone(),
                                                chart,
                                            });
                                        }
                                    >
                                        {(!assignment.required).then(|| view! { <option value="">"Unassigned"</option> })}
                                        {snapshot.workspace.charts.iter().map(|chart| view! {
                                            <option value=chart.instance_id.to_string()>{chart.title.clone()}</option>
                                        }).collect_view()}
                                    </select>
                                </label>
                            }
                        }).collect_view()
                    })
                }}
            </section>

            <section class="inspector-section" aria-labelledby="aspect-resource-title">
                <h3 id="aspect-resource-title">"Aspect Set"</h3>
                <label class="field-label" for="aspect-set-picker">
                    <span>"Workspace resource"</span>
                    <select
                        id="aspect-set-picker"
                        prop:value=move || model.get().inspector.active_aspect_set.map_or_else(String::new, |id| id.to_string())
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            if let Ok(resource_id) = ResourceId::from_str(&value) {
                                if let Some(summary) = model.get().library.aspect_sets.iter().find(|summary| summary.resource_id == resource_id) {
                                    orb_buffer.set(format_orb(summary.conjunction_orb));
                                    orb_error.set(None);
                                }
                                aspect_dispatcher.dispatch(AppIntent::SetWorkspaceAspectSet { resource_id });
                            }
                        }
                    >
                        {move || model.get().library.aspect_sets.into_iter().map(|summary| view! {
                            <option value=summary.resource_id.to_string()>
                                {format!("{} · r{}", summary.title, summary.revision)}
                            </option>
                        }).collect_view()}
                    </select>
                </label>

                <div class="binding-summary">
                    {move || model.get().inspector.bindings.into_iter().map(|binding| view! {
                        {
                            let (title, detail) = match binding.source {
                                BindingSourceSummary::Follow { resource_title, revision, .. } => {
                                    (resource_title, format!("Follow · resolved revision {revision}"))
                                }
                                BindingSourceSummary::Pinned { resource_title, revision, .. } => {
                                    (resource_title, format!("Pinned · revision {revision}"))
                                }
                                BindingSourceSummary::Inline => {
                                    ("Inline value".into(), "Embedded in the workspace".into())
                                }
                            };
                            view! {
                                <div>
                                    <span>{binding.label}</span>
                                    <strong>{title}</strong>
                                    <small>{detail}</small>
                                </div>
                            }
                        }
                    }).collect_view()}
                </div>

                <button
                    class="button secondary full-width"
                    type="button"
                    disabled=move || !model.get().availability(AppAction::BeginAspectSetEdit).is_enabled()
                    on:click=move |_| {
                        let snapshot = model.get_untracked();
                        if let Some(resource_id) = snapshot.inspector.active_aspect_set {
                            if let Some(summary) = snapshot.library.aspect_sets.iter().find(|summary| summary.resource_id == resource_id) {
                                orb_buffer.set(format_orb(summary.conjunction_orb));
                                orb_error.set(None);
                            }
                            edit_dispatcher.dispatch(AppIntent::BeginAspectSetEdit { resource_id });
                        }
                    }
                >
                    "Edit selected Aspect Set"
                </button>
            </section>

            {move || model.get().resource_editor.aspect_set.map(|draft| {
                let draft_state = draft_state_label(&draft.state);
                let conflict = match draft.state {
                    DraftState::Conflict { base_revision, remote_revision } => Some((base_revision, remote_revision)),
                    DraftState::Clean { .. } | DraftState::Dirty { .. } | DraftState::Saving { .. } => None,
                };
                let aspect_id_for_orb = draft.conjunction.aspect_id.clone();
                let aspect_id_for_enabled = draft.conjunction.aspect_id;
                view! {
                    <section class="inspector-section draft-editor" aria-labelledby="draft-editor-title">
                        <div class="draft-heading">
                            <div>
                                <p class="section-kicker">"APPLICATION DRAFT"</p>
                                <h3 id="draft-editor-title">{draft.title}</h3>
                            </div>
                            <span class=format!("draft-state {}", draft_state.to_lowercase())>{draft_state}</span>
                        </div>
                        <p class="revision-line">{format!("Base revision {}", draft.state.base_revision())}</p>

                        {conflict.map(|(base, remote)| view! {
                            <div class="conflict-message" role="alert">
                                <strong>"Revision conflict"</strong>
                                <span>{format!("Your draft began at revision {base}; the library is now at revision {remote}.")}</span>
                            </div>
                        })}

                        <label class="field-label" for="conjunction-orb">
                            <span>"Conjunction maximum orb"</span>
                            <div class="input-with-unit">
                                <input
                                    id="conjunction-orb"
                                    type="text"
                                    inputmode="decimal"
                                    prop:value=move || orb_buffer.get()
                                    aria-invalid=move || orb_error.get().is_some()
                                    aria-describedby="orb-help orb-error"
                                    disabled=matches!(draft.state, DraftState::Saving { .. })
                                    on:input=move |event| {
                                        let text = event_target_value(&event);
                                        orb_buffer.set(text.clone());
                                        match parse_orb(&text) {
                                            Ok(maximum) => {
                                                orb_error.set(None);
                                                orb_dispatcher.dispatch(AppIntent::UpdateAspectSetDraft(
                                                    AspectSetDraftMutation::SetOrb {
                                                        aspect_id: aspect_id_for_orb.clone(),
                                                        maximum,
                                                    },
                                                ));
                                            }
                                            Err(message) => orb_error.set(Some(message)),
                                        }
                                    }
                                />
                                <span aria-hidden="true">"°"</span>
                            </div>
                        </label>
                        <p id="orb-help" class="field-help">"Enter a semantic value from 0° through 20°. Temporary text remains UI-local."</p>
                        <p id="orb-error" class="field-error" role="status">{move || orb_error.get().unwrap_or_default()}</p>

                        <label class="check-field">
                            <input
                                type="checkbox"
                                prop:checked=draft.conjunction.enabled
                                disabled=matches!(draft.state, DraftState::Saving { .. })
                                on:change=move |event| enabled_dispatcher.dispatch(
                                    AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                                        aspect_id: aspect_id_for_enabled.clone(),
                                        enabled: event_target_checked(&event),
                                    }),
                                )
                            />
                            <span>"Conjunction enabled"</span>
                        </label>

                        <div class="draft-actions">
                            <button
                                class="button primary"
                                type="button"
                                disabled=move || !model.get().availability(AppAction::SaveDraft).is_enabled()
                                title=move || availability_title(&model.get().availability(AppAction::SaveDraft))
                                on:click=move |_| save_dispatcher.dispatch(AppIntent::SaveDraft)
                            >"Save draft"</button>
                            <button
                                class="button secondary"
                                type="button"
                                disabled=move || !model.get().availability(AppAction::CancelDraft).is_enabled()
                                title=move || availability_title(&model.get().availability(AppAction::CancelDraft))
                                on:click=move |_| {
                                    reset_orb_buffer(model, orb_buffer, orb_error);
                                    cancel_dispatcher.dispatch(AppIntent::CancelDraft);
                                }
                            >"Cancel"</button>
                        </div>
                    </section>
                }
            })}
        </aside>
    }
}

fn execute_command(
    command: CommandId,
    dispatcher: Dispatcher,
    model: RwSignal<AppReadModel>,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) {
    match command {
        CommandId::SaveDraft => {
            if model
                .get_untracked()
                .availability(AppAction::SaveDraft)
                .is_enabled()
            {
                dispatcher.dispatch(AppIntent::SaveDraft);
            }
        }
        CommandId::CancelDraft => {
            if model
                .get_untracked()
                .availability(AppAction::CancelDraft)
                .is_enabled()
            {
                reset_orb_buffer(model, orb_buffer, orb_error);
                dispatcher.dispatch(AppIntent::CancelDraft);
            }
        }
        CommandId::FocusChartRail => focus_chart_rail(),
        CommandId::RefreshView => {
            if model
                .get_untracked()
                .availability(AppAction::RefreshView)
                .is_enabled()
            {
                dispatcher.dispatch(AppIntent::RefreshActiveView);
            }
        }
    }
}

fn reset_orb_buffer(
    model: RwSignal<AppReadModel>,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) {
    let snapshot = model.get_untracked();
    if let Some(resource_id) = snapshot.inspector.active_aspect_set
        && let Some(summary) = snapshot
            .library
            .aspect_sets
            .iter()
            .find(|summary| summary.resource_id == resource_id)
    {
        orb_buffer.set(format_orb(summary.conjunction_orb));
    }
    orb_error.set(None);
}

fn event_target_is_text_entry(event: &ev::KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|element| {
            matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                || element.get_attribute("contenteditable").as_deref() == Some("true")
        })
}

fn focus_chart_rail() {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("workspace-chart-rail"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}

fn format_orb(value: Angle) -> String {
    format!("{:.1}", value.degrees())
}

fn parse_orb(text: &str) -> Result<Angle, String> {
    let value = text
        .parse::<f64>()
        .map_err(|_| "Enter a number between 0 and 20".to_owned())?;
    if !(0.0..=20.0).contains(&value) {
        return Err("Orb must be between 0 and 20 degrees".into());
    }
    Angle::from_degrees(value).map_err(|error| error.to_string())
}

fn draft_state_label(state: &DraftState) -> &'static str {
    match state {
        DraftState::Clean { .. } => "Clean",
        DraftState::Dirty { .. } => "Dirty",
        DraftState::Saving { .. } => "Saving",
        DraftState::Conflict { .. } => "Conflict",
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

fn availability_title(availability: &Availability) -> String {
    availability
        .disabled_reason()
        .unwrap_or_default()
        .to_owned()
}

#[cfg(target_arch = "wasm32")]
const fn application_label() -> &'static str {
    "Real application · deterministic provider"
}

#[cfg(not(target_arch = "wasm32"))]
const fn application_label() -> &'static str {
    "Mock application · frontend test adapter"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orb_parser_accepts_semantic_values_and_rejects_form_buffers() {
        assert_eq!(parse_orb("6.5").expect("valid orb").degrees(), 6.5);
        assert!(parse_orb("").is_err());
        assert!(parse_orb("1.").is_ok());
        assert!(parse_orb("21").is_err());
    }

    #[test]
    fn publish_if_newer_rejects_stale_projection() {
        let mut current = model_at(10);

        assert!(publish_if_newer(&mut current, model_at(12)));
        assert_eq!(current.version, ProjectionVersion::new(12));
        assert!(!publish_if_newer(&mut current, model_at(11)));
        assert_eq!(current.version, ProjectionVersion::new(12));
        assert!(!publish_if_newer(&mut current, model_at(12)));
        assert_eq!(current.version, ProjectionVersion::new(12));
    }

    fn model_at(version: u64) -> AppReadModel {
        let mut model = AppReadModel::initializing();
        model.version = ProjectionVersion::new(version);
        model
    }
}
