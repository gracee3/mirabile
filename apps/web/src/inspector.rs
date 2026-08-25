use std::{collections::BTreeSet, str::FromStr};

use leptos::prelude::*;
use mirabile_app::{
    ActionSource, Angle, AppAction, AppIntent, AppReadModel, AspectDraftValue,
    AspectSetDraftMutation, Availability, BindingSourceSummary, ChartPersistence, ControlAddress,
    ControlId, DraftState, InstanceId, ResourceId,
};

use crate::chart_editor::ChartAuthoring;
use crate::dispatcher::{WorkbenchCoordinator, reset_aspect_buffers};
use crate::workbench_controls::{BufferedField, BufferedInputKind, BufferedNumberField};

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn Inspector(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
    invalid_aspect_buffers: RwSignal<BTreeSet<String>>,
) -> impl IntoView {
    let aspect_dispatcher = dispatcher;
    let edit_dispatcher = dispatcher;

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
                            reset_aspect_buffers(invalid_aspect_buffers);
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

                <div class="draft-actions">
                    <button
                        class="button secondary"
                        type="button"
                        data-mirabile-control=ControlId::ASPECT_NEW.to_string()
                        data-mirabile-address=ControlAddress::new(ControlId::ASPECT_NEW).to_string()
                        disabled=move || !model.get().availability(AppAction::BeginNewAspectSet).is_enabled()
                        on:click=move |_| {
                            reset_aspect_buffers(invalid_aspect_buffers);
                            dispatcher.dispatch_from(
                                AppIntent::BeginNewAspectSet,
                                ActionSource::Human,
                                Some(ControlAddress::new(ControlId::ASPECT_NEW)),
                            );
                        }
                    >"New"</button>
                    <button
                        class="button secondary"
                        type="button"
                        data-mirabile-control=ControlId::ASPECT_DUPLICATE.to_string()
                        data-mirabile-address=ControlAddress::new(ControlId::ASPECT_DUPLICATE).to_string()
                        disabled=move || !model.get().availability(AppAction::DuplicateAspectSet).is_enabled()
                        on:click=move |_| {
                            if let Some(resource_id) = model.get_untracked().inspector.active_aspect_set {
                                reset_aspect_buffers(invalid_aspect_buffers);
                                dispatcher.dispatch_from(
                                    AppIntent::DuplicateAspectSet { resource_id },
                                    ActionSource::Human,
                                    ControlAddress::qualified(
                                        ControlId::ASPECT_DUPLICATE,
                                        [("resource", resource_id.to_string())],
                                    ).ok(),
                                );
                            }
                        }
                    >"Duplicate"</button>
                </div>
            </section>

            <AspectSetEditorPanel model dispatcher invalid_aspect_buffers />
        </aside>
    }
}

#[component]
#[allow(clippy::too_many_lines)]
fn AspectSetEditorPanel(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
    invalid_aspect_buffers: RwSignal<BTreeSet<String>>,
) -> impl IntoView {
    let save_dispatcher = dispatcher;
    let cancel_dispatcher = dispatcher;
    view! {
        {move || model.get().resource_editor.aspect_set.map(|draft| {
                let draft_state = draft_state_label(&draft.state);
                let conflict = match draft.state {
                    DraftState::Conflict { base_revision, remote_revision } => Some((base_revision, remote_revision)),
                    DraftState::New | DraftState::Creating | DraftState::Clean { .. }
                    | DraftState::Dirty { .. } | DraftState::Saving { .. } => None,
                };
                let title = draft.title.clone();
                let title_buffer = RwSignal::new(title.clone());
                let title_error = RwSignal::new(None::<String>);
                track_invalid_buffer("title".into(), title_error, invalid_aspect_buffers);
                let title_dispatcher = dispatcher;
                let title_pending = matches!(draft.state, DraftState::Saving { .. } | DraftState::Creating);
                view! {
                    <section class="inspector-section draft-editor" aria-labelledby="draft-editor-title">
                        <div class="draft-heading">
                            <div>
                                <p class="section-kicker">"APPLICATION DRAFT"</p>
                                <h3 id="draft-editor-title">{draft.title}</h3>
                            </div>
                            <span class=format!("draft-state {}", draft_state.to_lowercase())>{draft_state}</span>
                        </div>
                        <p class="revision-line">{draft.state.base_revision().map_or_else(
                            || "New canonical resource".into(),
                            |revision| format!("Base revision {revision}"),
                        )}</p>

                        {conflict.map(|(base, remote)| view! {
                            <div class="conflict-message" role="alert">
                                <strong>"Revision conflict"</strong>
                                <span>{format!("Your draft began at revision {base}; the library is now at revision {remote}.")}</span>
                            </div>
                        })}

                        <BufferedField
                            address=ControlAddress::new(ControlId::ASPECT_TITLE).to_string()
                            label="Title".to_owned()
                            kind=BufferedInputKind::Text
                            authoritative=Signal::derive(move || model.get().resource_editor.aspect_set
                                .map_or_else(String::new, |draft| draft.title))
                            disabled=Signal::derive(move || title_pending)
                            buffer=title_buffer
                            error=title_error
                            parser=Callback::new(|text: String| {
                                let title = text.trim();
                                (!title.is_empty()).then(|| title.to_owned())
                                    .ok_or_else(|| "Title must not be empty".into())
                            })
                            on_commit=Callback::new(move |title: String| title_dispatcher.dispatch_from(
                                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetTitle(title)),
                                ActionSource::Human,
                                Some(ControlAddress::new(ControlId::ASPECT_TITLE)),
                            ))
                            help="Enter applies; Escape restores the authoritative title.".to_owned()
                        />

                        {draft.aspects.into_iter().map(|aspect| view! {
                            <AspectEditorRow
                                model
                                dispatcher
                                aspect
                                invalid_aspect_buffers
                            />
                        }).collect_view()}

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
                                    || !invalid_aspect_buffers.get().is_empty()
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
                                    reset_aspect_buffers(invalid_aspect_buffers);
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
    }
}

#[component]
fn AspectEditorRow(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
    aspect: AspectDraftValue,
    invalid_aspect_buffers: RwSignal<BTreeSet<String>>,
) -> impl IntoView {
    let aspect_id = aspect.aspect_id;
    let qualifier = aspect_id.as_str().to_owned();
    let buffer = RwSignal::new(format_orb(aspect.maximum_orb));
    let error = RwSignal::new(None::<String>);
    track_invalid_buffer(qualifier.clone(), error, invalid_aspect_buffers);
    let orb_id = aspect_id.clone();
    let enabled_id = aspect_id.clone();
    let orb_dispatcher = dispatcher;
    let enabled_dispatcher = dispatcher;
    let orb_qualifier = qualifier.clone();
    let enabled_qualifier = qualifier.clone();
    let label = aspect.label;
    let pending = Signal::derive(move || {
        model.get().resource_editor.aspect_set.is_none_or(|draft| {
            matches!(
                draft.state,
                DraftState::Saving { .. } | DraftState::Creating
            )
        })
    });
    let authoritative_id = aspect_id.clone();
    view! {
        <div class="aspect-editor-row">
            <BufferedNumberField
                address=ControlAddress::qualified(
                    ControlId::ASPECT_MAXIMUM_ORB,
                    [("aspect", orb_qualifier.as_str())],
                ).expect("Aspect control address").to_string()
                label=format!("{label} maximum orb")
                authoritative=Signal::derive(move || model.get().resource_editor.aspect_set
                    .and_then(|draft| draft.aspects.into_iter()
                        .find(|row| row.aspect_id == authoritative_id))
                    .map_or_else(String::new, |row| format_orb(row.maximum_orb)))
                disabled=pending
                buffer
                error
                parser=Callback::new(|text: String| parse_orb(&text).map(format_orb))
                on_commit=Callback::new(move |text: String| {
                    if let Ok(maximum) = parse_orb(&text) {
                        orb_dispatcher.dispatch_from(
                            AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetOrb {
                                aspect_id: orb_id.clone(),
                                maximum,
                            }),
                            ActionSource::Human,
                            ControlAddress::qualified(
                                ControlId::ASPECT_MAXIMUM_ORB,
                                [("aspect", orb_id.as_str())],
                            ).ok(),
                        );
                    }
                })
                help="Enter a value from 0 through 20 degrees. Enter applies; Escape restores the authoritative value.".to_owned()
                qualifier_name="aspect".to_owned()
                qualifier_value=orb_qualifier
            />
            <label class="check-field">
                <input
                    type="checkbox"
                    data-mirabile-control=ControlId::ASPECT_ENABLED.to_string()
                    data-mirabile-aspect=enabled_qualifier.clone()
                    data-mirabile-address=ControlAddress::qualified(
                        ControlId::ASPECT_ENABLED,
                        [("aspect", enabled_qualifier.as_str())],
                    ).expect("aspect enabled address").to_string()
                    prop:checked=aspect.enabled
                    disabled=move || pending.get()
                    on:change=move |event| enabled_dispatcher.dispatch_from(
                        AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                            aspect_id: enabled_id.clone(),
                            enabled: event_target_checked(&event),
                        }),
                        ActionSource::Human,
                        ControlAddress::qualified(
                            ControlId::ASPECT_ENABLED,
                            [("aspect", enabled_id.as_str())],
                        ).ok(),
                    )
                />
                <span>{format!("{label} enabled")}</span>
            </label>
        </div>
    }
}

fn track_invalid_buffer(
    key: String,
    error: RwSignal<Option<String>>,
    invalid_aspect_buffers: RwSignal<BTreeSet<String>>,
) {
    let effect_key = key.clone();
    Effect::new(move || {
        let is_invalid = error.get().is_some();
        let was_invalid = invalid_aspect_buffers.get_untracked().contains(&effect_key);
        if is_invalid != was_invalid {
            invalid_aspect_buffers.update(|invalid| {
                if is_invalid {
                    invalid.insert(effect_key.clone());
                } else {
                    invalid.remove(&effect_key);
                }
            });
        }
    });
    on_cleanup(move || {
        invalid_aspect_buffers.update(|invalid| {
            invalid.remove(&key);
        });
    });
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
        DraftState::New => "New",
        DraftState::Creating => "Creating",
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
