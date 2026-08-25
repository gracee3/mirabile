use std::str::FromStr;

use leptos::prelude::*;
use mirabile_app::{
    ActionSource, Angle, AppAction, AppIntent, AppReadModel, AspectSetDraftMutation, Availability,
    BindingSourceSummary, ChartPersistence, ControlAddress, ControlId, DraftState, InstanceId,
    ResourceId,
};

use crate::chart_editor::ChartAuthoring;
use crate::dispatcher::{WorkbenchCoordinator, reset_orb_buffer};
use crate::workbench_controls::BufferedNumberField;

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn Inspector(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
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

            <ChartAuthoring model dispatcher />

            <section class="inspector-section" aria-labelledby="slot-title">
                <h3 id="slot-title">"Chart slots"</h3>
                {move || {
                    let snapshot = model.get();
                    snapshot.active_view.map(|active_view| {
                        active_view.slots.into_iter().map(|assignment| {
                            let dispatch = dispatcher;
                            let view_id = active_view.view_id;
                            let slot = assignment.slot.clone();
                            let address = ControlAddress::qualified(
                                ControlId::VIEW_SLOT,
                                [
                                    ("slot", assignment.slot.as_str().to_owned()),
                                    ("view", view_id.to_string()),
                                ],
                            ).expect("view slot address");
                            let origin = address.clone();
                            let current = assignment.chart.map_or_else(String::new, |id| id.to_string());
                            view! {
                                <label class="field-label">
                                    <span>
                                        {assignment.label}
                                        {assignment.required.then_some(" · Required")}
                                    </span>
                                    <select
                                        prop:value=current
                                        data-mirabile-control=ControlId::VIEW_SLOT.to_string()
                                        data-mirabile-slot=assignment.slot.as_str().to_owned()
                                        data-mirabile-view=view_id.to_string()
                                        data-mirabile-address=address.to_string()
                                        on:change=move |event| {
                                            let value = event_target_value(&event);
                                            let chart = if value.is_empty() {
                                                None
                                            } else {
                                                InstanceId::from_str(&value).ok()
                                            };
                                            dispatch.dispatch_from(
                                                AppIntent::AssignChartSlot {
                                                    view_id,
                                                    slot: slot.clone(),
                                                    chart,
                                                },
                                                ActionSource::Human,
                                                Some(origin.clone()),
                                            );
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
                        data-mirabile-control=ControlId::ASPECT_RESOURCE.to_string()
                        data-mirabile-address=ControlAddress::new(ControlId::ASPECT_RESOURCE).to_string()
                        prop:value=move || model.get().inspector.active_aspect_set.map_or_else(String::new, |id| id.to_string())
                        on:change=move |event| {
                            let value = event_target_value(&event);
                            if let Ok(resource_id) = ResourceId::from_str(&value) {
                                if let Some(summary) = model.get().library.aspect_sets.iter().find(|summary| summary.resource_id == resource_id) {
                                    orb_buffer.set(format_orb(summary.conjunction_orb));
                                    orb_error.set(None);
                                }
                                aspect_dispatcher.dispatch_from(
                                    AppIntent::SetWorkspaceAspectSet { resource_id },
                                    ActionSource::Human,
                                    Some(ControlAddress::new(ControlId::ASPECT_RESOURCE)),
                                );
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
                    data-mirabile-control=ControlId::ASPECT_EDIT.to_string()
                    data-mirabile-address=ControlId::ASPECT_EDIT.to_string()
                    disabled=move || !model.get().availability(AppAction::BeginAspectSetEdit).is_enabled()
                    on:click=move |_| {
                        let snapshot = model.get_untracked();
                        if let Some(resource_id) = snapshot.inspector.active_aspect_set {
                            if let Some(summary) = snapshot.library.aspect_sets.iter().find(|summary| summary.resource_id == resource_id) {
                                orb_buffer.set(format_orb(summary.conjunction_orb));
                                orb_error.set(None);
                            }
                            edit_dispatcher.dispatch_from(
                                AppIntent::BeginAspectSetEdit { resource_id },
                                ActionSource::Human,
                                ControlAddress::qualified(
                                    ControlId::ASPECT_EDIT,
                                    [("resource", resource_id.to_string())],
                                ).ok(),
                            );
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
                let aspect_orb_qualifier = aspect_id_for_orb.as_str().to_owned();
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

                        <BufferedNumberField
                            address=ControlAddress::qualified(
                                ControlId::ASPECT_MAXIMUM_ORB,
                                [("aspect", aspect_orb_qualifier.as_str())],
                            ).expect("static Aspect control address").to_string()
                            label="Conjunction maximum orb".to_owned()
                            authoritative=Signal::derive(move || model.get().resource_editor.aspect_set
                                .map_or_else(String::new, |draft| format_orb(draft.conjunction.maximum_orb)))
                            disabled=Signal::derive(move || model.get().resource_editor.aspect_set
                                .is_none_or(|draft| matches!(draft.state, DraftState::Saving { .. })))
                            buffer=orb_buffer
                            error=orb_error
                            parser=Callback::new(|text: String| parse_orb(&text).map(format_orb))
                            on_commit=Callback::new(move |text: String| {
                                if let Ok(maximum) = parse_orb(&text) {
                                    orb_dispatcher.dispatch_from(
                                        AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetOrb {
                                            aspect_id: aspect_id_for_orb.clone(),
                                            maximum,
                                        }),
                                        mirabile_app::ActionSource::Human,
                                        ControlAddress::qualified(
                                            ControlId::ASPECT_MAXIMUM_ORB,
                                            [("aspect", aspect_id_for_orb.as_str())],
                                        ).ok(),
                                    );
                                }
                            })
                            help="Enter a semantic value from 0° through 20°. Enter applies; Escape restores the authoritative value.".to_owned()
                            qualifier_name="aspect".to_owned()
                            qualifier_value=aspect_orb_qualifier
                        />

                        <label class="check-field">
                            <input
                                type="checkbox"
                                data-mirabile-control=ControlId::ASPECT_ENABLED.to_string()
                                data-mirabile-aspect=aspect_id_for_enabled.as_str().to_owned()
                                data-mirabile-address=ControlAddress::qualified(
                                    ControlId::ASPECT_ENABLED,
                                    [("aspect", aspect_id_for_enabled.as_str())],
                                ).expect("aspect enabled address").to_string()
                                prop:checked=draft.conjunction.enabled
                                disabled=matches!(draft.state, DraftState::Saving { .. })
                                on:change=move |event| enabled_dispatcher.dispatch_from(
                                    AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                                            aspect_id: aspect_id_for_enabled.clone(),
                                            enabled: event_target_checked(&event),
                                    }),
                                    ActionSource::Human,
                                    ControlAddress::qualified(
                                        ControlId::ASPECT_ENABLED,
                                        [("aspect", aspect_id_for_enabled.as_str())],
                                    ).ok(),
                                )
                            />
                            <span>"Conjunction enabled"</span>
                        </label>

                        <div class="draft-actions">
                            <button
                                class="button primary"
                                type="button"
                                data-mirabile-control=ControlId::DRAFT_SAVE.to_string()
                                data-mirabile-address=ControlAddress::qualified(
                                    ControlId::DRAFT_SAVE,
                                    [("surface", "editor")],
                                ).expect("editor save address").to_string()
                                disabled=move || !model.get().availability(AppAction::SaveDraft).is_enabled()
                                title=move || availability_title(&model.get().availability(AppAction::SaveDraft))
                                on:click=move |_| save_dispatcher.dispatch_from(
                                    AppIntent::SaveDraft,
                                    ActionSource::Human,
                                    Some(ControlAddress::new(ControlId::DRAFT_SAVE)),
                                )
                            >"Save draft"</button>
                            <button
                                class="button secondary"
                                type="button"
                                data-mirabile-control=ControlId::DRAFT_CANCEL.to_string()
                                data-mirabile-address=ControlAddress::qualified(
                                    ControlId::DRAFT_CANCEL,
                                    [("surface", "editor")],
                                ).expect("editor cancel address").to_string()
                                disabled=move || !model.get().availability(AppAction::CancelDraft).is_enabled()
                                title=move || availability_title(&model.get().availability(AppAction::CancelDraft))
                                on:click=move |_| {
                                    reset_orb_buffer(model, orb_buffer, orb_error);
                                    cancel_dispatcher.dispatch_from(
                                        AppIntent::CancelDraft,
                                        ActionSource::Human,
                                        Some(ControlAddress::new(ControlId::DRAFT_CANCEL)),
                                    );
                                }
                            >"Cancel"</button>
                        </div>
                    </section>
                }
            })}
        </aside>
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

fn availability_title(availability: &Availability) -> String {
    availability
        .disabled_reason()
        .unwrap_or_default()
        .to_owned()
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
}
