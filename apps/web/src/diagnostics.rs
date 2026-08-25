use leptos::prelude::*;
use mirabile_app::{
    AppReadModel, ApplicationStatus, AutomationSnapshotV1, ControlId, ControlKind, ExecutionOutcome,
};

use crate::dispatcher::WorkbenchCoordinator;
use crate::macro_panel::MacroPanel;

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn Diagnostics(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let export_status = RwSignal::new(None::<String>);
    view! {
        <section class="diagnostics-dock" aria-labelledby="diagnostics-title">
            <div class="diagnostics-heading">
                <div>
                    <p class="section-kicker">"AUTHORITATIVE OBSERVATION"</p>
                    <h2 id="diagnostics-title">"Diagnostics"</h2>
                </div>
                <div class="diagnostics-actions">
                    <button
                        type="button"
                        class="button secondary"
                        data-mirabile-control=ControlId::DIAGNOSTICS_EXPORT_SNAPSHOT.to_string()
                        data-mirabile-address=ControlId::DIAGNOSTICS_EXPORT_SNAPSHOT.to_string()
                        data-mirabile-kind=ControlKind::Action.as_str()
                        data-mirabile-enabled=cfg!(target_arch = "wasm32").to_string()
                        data-mirabile-disabled-reason=cfg!(not(target_arch = "wasm32"))
                            .then_some("Exports require a browser build")
                        disabled=cfg!(not(target_arch = "wasm32"))
                        on:click=move |_| {
                            match crate::control_manifest::capture() {
                                Ok(manifest) => {
                                    let snapshot = AutomationSnapshotV1::capture(
                                        &model.get_untracked(),
                                        manifest.controls,
                                        dispatcher.read_model(),
                                        dispatcher.trace(),
                                    );
                                    export_json(
                                        "mirabile-automation-snapshot-v1.json",
                                        &snapshot,
                                        export_status,
                                    );
                                }
                                Err(error) => export_status.set(Some(format!(
                                    "Export failed: {error}"
                                ))),
                            }
                        }
                    >"Export snapshot JSON"</button>
                    <button
                        type="button"
                        class="button secondary"
                        data-mirabile-control=ControlId::DIAGNOSTICS_EXPORT_TRACE.to_string()
                        data-mirabile-address=ControlId::DIAGNOSTICS_EXPORT_TRACE.to_string()
                        data-mirabile-kind=ControlKind::Action.as_str()
                        data-mirabile-enabled=cfg!(target_arch = "wasm32").to_string()
                        data-mirabile-disabled-reason=cfg!(not(target_arch = "wasm32"))
                            .then_some("Exports require a browser build")
                        disabled=cfg!(not(target_arch = "wasm32"))
                        on:click=move |_| export_json(
                            "mirabile-trace.json",
                            &dispatcher.trace(),
                            export_status,
                        )
                    >"Export trace JSON"</button>
                </div>
            </div>

            <div class="diagnostics-grid">
                <article class="diagnostic-panel">
                    <h3>"Application"</h3>
                    {move || {
                        let snapshot = model.get();
                        let status = match snapshot.status {
                            ApplicationStatus::Initializing => "Initializing",
                            ApplicationStatus::Ready => "Ready",
                            ApplicationStatus::Error(_) => "Error",
                        };
                        view! {
                            <DiagnosticValue label="Status" value=status.to_owned() />
                            <DiagnosticValue label="Projection" value=snapshot.version.to_string() />
                            <DiagnosticValue label="Settlement" value=if snapshot.is_settled() { "Settled".into() } else { "Pending".into() } />
                            <DiagnosticValue label="Pending" value=snapshot.activity.pending_operations.len().to_string() />
                        }
                    }}
                </article>

                <article class="diagnostic-panel">
                    <h3>"Workspace"</h3>
                    {move || {
                        let workspace = model.get().workspace;
                        view! {
                            <DiagnosticValue label="Title" value=workspace.title />
                            <DiagnosticValue label="Identity" value=workspace.document_id.map_or_else(|| "Unsaved".into(), |id| id.to_string()) />
                            <DiagnosticValue label="Revision" value=workspace.document_revision.map_or_else(|| "None".into(), |revision| revision.to_string()) />
                            <DiagnosticValue label="Durable dirty" value=workspace.document_dirty.to_string() />
                            <DiagnosticValue label="Temporary display" value=workspace.has_temporary_display_override.to_string() />
                        }
                    }}
                </article>

                <article class="diagnostic-panel">
                    <h3>"Chart and view"</h3>
                    {move || {
                        let snapshot = model.get();
                        let chart = snapshot.inspector.active_chart;
                        let view = snapshot.active_view;
                        view! {
                            <DiagnosticValue label="Chart" value=chart.as_ref().map_or_else(|| "None".into(), |chart| chart.title.clone()) />
                            <DiagnosticValue label="Chart identity" value=chart.map_or_else(|| "None".into(), |chart| chart.instance_id.to_string()) />
                            <DiagnosticValue label="View" value=view.as_ref().map_or_else(|| "None".into(), |view| view.title.clone()) />
                            <DiagnosticValue label="Computation" value=view.as_ref().map_or_else(|| "None".into(), |view| format!("{:?}", view.computation)) />
                            <DiagnosticValue label="Last-good Scene" value=view.is_some_and(|view| view.scene.is_some()).to_string() />
                        }
                    }}
                </article>

                <article class="diagnostic-panel">
                    <h3>"Calculation"</h3>
                    {move || model.get().calculation.map_or_else(
                        || view! { <p class="muted">"Unavailable"</p> }.into_any(),
                        |calculation| view! {
                            <DiagnosticValue label="Backend" value=format!("{} {}", calculation.backend.id, calculation.backend.version) />
                            <DiagnosticValue label="Engine" value=format!("{} {}", calculation.engine.id, calculation.engine.version) />
                            <DiagnosticValue label="Worker protocol" value=calculation.worker_protocol.to_string() />
                            <DiagnosticValue label="Request" value=calculation.active_request_id.map_or_else(|| "None".into(), |id| id.to_string()) />
                            <DiagnosticValue label="CalcKey" value=calculation.calc_key.unwrap_or_else(|| "None".into()) code=true />
                            <DiagnosticValue label="AnalysisKey" value=calculation.analysis_key.unwrap_or_else(|| "None".into()) code=true />
                        }.into_any(),
                    )}
                </article>

                <article class="diagnostic-panel automation-panel">
                    <h3>"Coordinator"</h3>
                    {move || {
                        let coordinator = dispatcher.read_model_tracked();
                        view! {
                            <DiagnosticValue label="Running" value=coordinator.running.to_string() />
                            <DiagnosticValue label="Queued" value=coordinator.queued_actions.to_string() />
                            <DiagnosticValue label="Source" value=coordinator.current_source.map_or_else(|| "None".into(), |source| format!("{source:?}")) />
                            <DiagnosticValue label="Highlight" value=coordinator.highlighted_control.map_or_else(|| "None".into(), |control| control.to_string()) />
                            <DiagnosticValue label="Macro" value=format!("{:?}", coordinator.macro_state) />
                        }
                    }}
                </article>

                <article class="diagnostic-panel trace-panel">
                    <h3>"Recent trace"</h3>
                    {move || {
                        let entries = dispatcher.trace_tracked();
                        entries.into_iter().rev().take(12).map(|entry| {
                            let outcome = match entry.outcome {
                                ExecutionOutcome::Settled => "settled".to_owned(),
                                ExecutionOutcome::Rejected { message, .. } => format!("rejected: {message}"),
                                ExecutionOutcome::Failed { message, .. } => format!("failed: {message}"),
                            };
                            view! {
                                <div class="trace-row">
                                    <span>{entry.sequence}</span>
                                    <code>{entry.semantic_intent}</code>
                                    <span>{format!("{:?}", entry.source)}</span>
                                    <span>{outcome}</span>
                                </div>
                            }
                        }).collect_view()
                    }}
                </article>
            </div>
            <p class="diagnostics-export-status" role="status">
                {move || export_status.get().unwrap_or_default()}
            </p>
            <MacroPanel dispatcher />
        </section>
    }
}

#[component]
fn DiagnosticValue(
    label: &'static str,
    value: String,
    #[prop(default = false)] code: bool,
) -> impl IntoView {
    view! {
        <div class="diagnostic-value">
            <span>{label}</span>
            {if code {
                view! { <code title=value.clone()>{value.clone()}</code> }.into_any()
            } else {
                view! { <strong>{value}</strong> }.into_any()
            }}
        </div>
    }
}

pub(super) fn export_json<T: serde::Serialize>(
    filename: &str,
    value: &T,
    status: RwSignal<Option<String>>,
) {
    match serde_json::to_string_pretty(value)
        .map_err(|error| error.to_string())
        .and_then(|json| download_json(filename, &json))
    {
        Ok(()) => status.set(Some(format!("Exported {filename}"))),
        Err(error) => status.set(Some(format!("Export failed: {error}"))),
    }
}

#[cfg(target_arch = "wasm32")]
fn download_json(filename: &str, json: &str) -> Result<(), String> {
    use wasm_bindgen::{JsCast as _, JsValue};
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(json));
    let options = BlobPropertyBag::new();
    options.set_type("application/json");
    let blob = Blob::new_with_str_sequence_and_options(&parts, &options)
        .map_err(|_| "could not create JSON download".to_owned())?;
    let url = Url::create_object_url_with_blob(&blob)
        .map_err(|_| "could not create download URL".to_owned())?;
    let anchor = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| "browser document is unavailable".to_owned())?
        .create_element("a")
        .map_err(|_| "could not create download control".to_owned())?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "download control was not an anchor".to_owned())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    Url::revoke_object_url(&url).map_err(|_| "could not release download URL".to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
fn download_json(_filename: &str, _json: &str) -> Result<(), String> {
    Err("JSON download requires a browser build".into())
}
