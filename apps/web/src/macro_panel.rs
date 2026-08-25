use leptos::prelude::*;
use mirabile_app::{ControlId, ControlKind, MacroCoordinatorState, MacroDocumentV1};

use crate::{diagnostics::export_json, dispatcher::WorkbenchCoordinator};

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn MacroPanel(dispatcher: WorkbenchCoordinator) -> impl IntoView {
    let name = RwSignal::new("Recorded macro".to_owned());
    let json = RwSignal::new(
        dispatcher
            .macro_document()
            .and_then(|document| serde_json::to_string_pretty(&document).ok())
            .unwrap_or_default(),
    );
    let status = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let highlighted = dispatcher.read_model_tracked().highlighted_control;
        apply_highlight(highlighted.as_ref().map(ToString::to_string).as_deref());
    });

    let current_state = move || dispatcher.read_model_tracked().macro_state;
    let json_error = move || {
        let value = json.get();
        (!value.trim().is_empty())
            .then(|| MacroDocumentV1::from_json(&value).err())
            .flatten()
    };

    view! {
        <section class="macro-panel" aria-labelledby="macro-title">
            <div class="macro-heading">
                <div>
                    <p class="section-kicker">"SEMANTIC AUTOMATION"</p>
                    <h3 id="macro-title">"Macros"</h3>
                </div>
                <span class="macro-state">{move || macro_state_label(&current_state())}</span>
            </div>
            <div class="macro-toolbar">
                <label>
                    <span>"Name"</span>
                    <input
                        type="text"
                        data-mirabile-native="value"
                        data-mirabile-control=ControlId::MACRO_NAME.to_string()
                        data-mirabile-address=ControlId::MACRO_NAME.to_string()
                        data-mirabile-label="Macro name"
                        data-mirabile-kind=ControlKind::Text.as_str()
                        data-mirabile-enabled="true"
                        prop:value=move || name.get()
                        on:input=move |event| name.set(event_target_value(&event))
                    />
                </label>
                <button
                    type="button"
                    class="button secondary"
                    data-mirabile-control=ControlId::MACRO_START.to_string()
                    data-mirabile-address=ControlId::MACRO_START.to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=move || (!matches!(current_state(), MacroCoordinatorState::Recording | MacroCoordinatorState::Replaying { .. })
                        && !name.get().trim().is_empty()).to_string()
                    data-mirabile-disabled-reason=move || match current_state() {
                        MacroCoordinatorState::Recording => Some("Macro recording is already active".to_owned()),
                        MacroCoordinatorState::Replaying { .. } => Some("Wait for macro replay to finish".to_owned()),
                        MacroCoordinatorState::Idle | MacroCoordinatorState::Failed { .. } if name.get().trim().is_empty() => {
                            Some("Enter a macro name before recording".to_owned())
                        }
                        MacroCoordinatorState::Idle | MacroCoordinatorState::Failed { .. } => None,
                    }
                    disabled=move || matches!(current_state(), MacroCoordinatorState::Recording | MacroCoordinatorState::Replaying { .. }) || name.get().trim().is_empty()
                    on:click=move |_| match dispatcher.start_macro_recording(name.get_untracked()) {
                        Ok(()) => status.set(Some("Recording accepted semantic actions".into())),
                        Err(error) => status.set(Some(error.to_string())),
                    }
                >"Record"</button>
                <button
                    type="button"
                    class="button secondary"
                    data-mirabile-control=ControlId::MACRO_STOP.to_string()
                    data-mirabile-address=ControlId::MACRO_STOP.to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=move || matches!(current_state(), MacroCoordinatorState::Recording).to_string()
                    data-mirabile-disabled-reason=move || match current_state() {
                        MacroCoordinatorState::Recording => None,
                        MacroCoordinatorState::Replaying { .. } => Some("Wait for macro replay to finish".to_owned()),
                        MacroCoordinatorState::Idle | MacroCoordinatorState::Failed { .. } => {
                            Some("Start macro recording before stopping".to_owned())
                        }
                    }
                    disabled=move || !matches!(current_state(), MacroCoordinatorState::Recording)
                    on:click=move |_| match dispatcher.stop_macro_recording() {
                        Ok(document) => match serde_json::to_string_pretty(&document) {
                            Ok(value) => {
                                let count = document.steps.len();
                                json.set(value);
                                status.set(Some(format!("Recorded {count} semantic steps")));
                            }
                            Err(error) => status.set(Some(error.to_string())),
                        },
                        Err(error) => status.set(Some(error.to_string())),
                    }
                >"Stop"</button>
                <button
                    type="button"
                    class="button secondary"
                    data-mirabile-control=ControlId::MACRO_IMPORT.to_string()
                    data-mirabile-address=ControlId::MACRO_IMPORT.to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=move || (!json.get().trim().is_empty() && json_error().is_none()).to_string()
                    data-mirabile-disabled-reason=move || if json.get().trim().is_empty() {
                        Some("Enter a macro document before importing".to_owned())
                    } else {
                        json_error().map(|_| "Correct the invalid macro document before importing".to_owned())
                    }
                    disabled=move || json.get().trim().is_empty() || json_error().is_some()
                    on:click=move |_| match MacroDocumentV1::from_json(&json.get_untracked())
                        .and_then(|document| dispatcher.import_macro(document))
                    {
                        Ok(()) => status.set(Some("Macro schema accepted".into())),
                        Err(error) => status.set(Some(error.to_string())),
                    }
                >"Import"</button>
                <button
                    type="button"
                    class="button primary"
                    data-mirabile-control=ControlId::MACRO_REPLAY.to_string()
                    data-mirabile-address=ControlId::MACRO_REPLAY.to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=move || (!matches!(current_state(), MacroCoordinatorState::Recording | MacroCoordinatorState::Replaying { .. })
                        && !json.get().trim().is_empty() && json_error().is_none()).to_string()
                    data-mirabile-disabled-reason=move || match current_state() {
                        MacroCoordinatorState::Recording => Some("Stop macro recording before replaying".to_owned()),
                        MacroCoordinatorState::Replaying { .. } => Some("Macro replay is already active".to_owned()),
                        MacroCoordinatorState::Idle | MacroCoordinatorState::Failed { .. } if json.get().trim().is_empty() => {
                            Some("Enter a macro document before replaying".to_owned())
                        }
                        MacroCoordinatorState::Idle | MacroCoordinatorState::Failed { .. } => json_error()
                            .map(|_| "Correct the invalid macro document before replaying".to_owned()),
                    }
                    disabled=move || matches!(current_state(), MacroCoordinatorState::Recording | MacroCoordinatorState::Replaying { .. }) || json.get().trim().is_empty() || json_error().is_some()
                    on:click=move |_| match MacroDocumentV1::from_json(&json.get_untracked()) {
                        Ok(document) => {
                            let _ = dispatcher.import_macro(document.clone());
                            dispatcher.replay_macro(document);
                            status.set(Some("Macro replay started".into()));
                        }
                        Err(error) => status.set(Some(error.to_string())),
                    }
                >"Replay"</button>
                <button
                    type="button"
                    class="button secondary"
                    data-mirabile-control=ControlId::MACRO_EXPORT.to_string()
                    data-mirabile-address=ControlId::MACRO_EXPORT.to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=move || (!json.get().trim().is_empty() && json_error().is_none()).to_string()
                    data-mirabile-disabled-reason=move || if json.get().trim().is_empty() {
                        Some("Enter a macro document before exporting".to_owned())
                    } else {
                        json_error().map(|_| "Correct the invalid macro document before exporting".to_owned())
                    }
                    disabled=move || json.get().trim().is_empty() || json_error().is_some()
                    on:click=move |_| match MacroDocumentV1::from_json(&json.get_untracked()) {
                        Ok(document) => export_json("mirabile-macro-v1.json", &document, status),
                        Err(error) => status.set(Some(error.to_string())),
                    }
                >"Export"</button>
                <button
                    type="button"
                    class="button secondary"
                    data-mirabile-control=ControlId::MACRO_CLEAR.to_string()
                    data-mirabile-address=ControlId::MACRO_CLEAR.to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled="true"
                    on:click=move |_| {
                        dispatcher.clear_macro();
                        json.set(String::new());
                        status.set(Some("Macro cleared".into()));
                    }
                >"Clear"</button>
            </div>
            <label class="macro-json-field">
                <span>"Versioned macro JSON"</span>
                <textarea
                    rows="8"
                    spellcheck="false"
                    data-mirabile-native="value"
                    data-mirabile-control=ControlId::MACRO_JSON.to_string()
                    data-mirabile-address=ControlId::MACRO_JSON.to_string()
                    data-mirabile-label="Versioned macro JSON"
                    data-mirabile-kind=ControlKind::Text.as_str()
                    data-mirabile-enabled="true"
                    data-mirabile-invalid=move || json_error().is_some().to_string()
                    aria-invalid=move || json_error().is_some().then_some("true")
                    prop:value=move || json.get()
                    on:input=move |event| json.set(event_target_value(&event))
                ></textarea>
            </label>
            <div class="macro-feedback" role="status">
                <span>{move || status.get().unwrap_or_default()}</span>
                {move || match current_state() {
                    MacroCoordinatorState::Failed { step, message } => {
                        view! { <strong>{format!("Step {step} failed: {message}")}</strong> }.into_any()
                    }
                    _ => ().into_any(),
                }}
                {move || json_error().map(|error| view! {
                    <strong>{error.to_string()}</strong>
                })}
            </div>
        </section>
    }
}

fn macro_state_label(state: &MacroCoordinatorState) -> String {
    match state {
        MacroCoordinatorState::Idle => "Idle".into(),
        MacroCoordinatorState::Recording => "Recording".into(),
        MacroCoordinatorState::Replaying { step, total } => format!("Replaying {step}/{total}"),
        MacroCoordinatorState::Failed { step, .. } => format!("Failed at step {step}"),
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_highlight(address: Option<&str>) {
    use wasm_bindgen::JsCast as _;

    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Ok(nodes) = document.query_selector_all("[data-mirabile-address]") else {
        return;
    };
    for index in 0..nodes.length() {
        if let Some(element) = nodes
            .item(index)
            .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
        {
            let active = address.is_some_and(|address| {
                element.get_attribute("data-mirabile-address").as_deref() == Some(address)
            });
            let _ = element
                .class_list()
                .toggle_with_force("macro-highlight", active);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_highlight(_address: Option<&str>) {}
