use leptos::prelude::*;
use mirabile_app::{
    AnalysisProfileMutation, AppIntent, AppReadModel, AspectSetMutation, ChartDefinitionMutation,
    ChartRecordMutation, ControlAddress, ControlId, ControlKind, DraftState, PointSetMutation,
    QueryDefinitionMutation, ResourceDraftKind, ResourceMetadataMutation, ResourceMutation,
    ThemeMutation, ViewDocumentMutation, WheelTemplateMutation, WorkspaceBindingSelection,
    WorkspaceBindingSlot, WorkspaceDocumentMutation,
};

use crate::dispatcher::WorkbenchCoordinator;

#[component]
pub(super) fn Cockpit(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let search = RwSignal::new(String::new());
    let expanded = RwSignal::new(true);
    view! {
        <nav class="cockpit-nav" aria-label="Control cockpit navigation">
            <label>
                <span>"Find a section or resource"</span>
                <input
                    type="search"
                    placeholder="Search cockpit"
                    data-mirabile-control=ControlId::COCKPIT_SEARCH.to_string()
                    data-mirabile-address=ControlAddress::new(ControlId::COCKPIT_SEARCH).to_string()
                    data-mirabile-kind=ControlKind::Text.as_str()
                    data-mirabile-enabled="true"
                    on:input=move |event| search.set(event_target_value(&event))
                />
            </label>
            <button
                type="button"
                class="button secondary"
                data-mirabile-control=ControlId::COCKPIT_EXPAND_ALL.to_string()
                data-mirabile-address=ControlAddress::new(ControlId::COCKPIT_EXPAND_ALL).to_string()
                data-mirabile-kind=ControlKind::Action.as_str()
                data-mirabile-enabled="true"
                on:click=move |_| expanded.set(true)
            >"Expand all"</button>
            <button
                type="button"
                class="button secondary"
                data-mirabile-control=ControlId::COCKPIT_COLLAPSE_ALL.to_string()
                data-mirabile-address=ControlAddress::new(ControlId::COCKPIT_COLLAPSE_ALL).to_string()
                data-mirabile-kind=ControlKind::Action.as_str()
                data-mirabile-enabled="true"
                on:click=move |_| expanded.set(false)
            >"Collapse all"</button>
        </nav>

        <section class="cockpit-sections" aria-label="Comprehensive control cockpit">
            <CockpitSection number=1 title="Application, runtime, capabilities, and notices" search expanded>
                {move || {
                    let snapshot = model.get();
                    view! {
                        <div class="cockpit-grid">
                            <StatusCard label="Application" value=format!("{:?}", snapshot.status) />
                            <StatusCard label="Projection" value=snapshot.version.to_string() />
                            <StatusCard label="Settlement" value=if snapshot.is_settled() { "Settled" } else { "Pending" }.into() />
                            <StatusCard label="Provider" value=snapshot.calculation.as_ref().map_or("Unavailable".into(), |value| value.backend.id.clone()) />
                        </div>
                    }
                }}
            </CockpitSection>

            <CockpitSection number=2 title="Workspace, sessions, views, slots, and library" search expanded>
                {move || {
                    let snapshot = model.get();
                    view! {
                        <div class="cockpit-grid">
                            <StatusCard label="Workspace" value=snapshot.workspace.title />
                            <StatusCard label="Open charts" value=snapshot.workspace.charts.len().to_string() />
                            <StatusCard label="Views" value=snapshot.workspace.views.len().to_string() />
                            <StatusCard label="Library charts" value=snapshot.library.charts.len().to_string() />
                        </div>
                    }
                }}
            </CockpitSection>

            <CockpitSection number=3 title="Chart facts, calculation parameters, and preview" search expanded>
                <p class="cockpit-note">"The atomic ChartRecord + ChartDefinition editor and last-valid preview remain mounted in the workstation below. Shared records retain copy/detach protection."</p>
                {move || model.get().parameters.into_iter().map(|entry| view! {
                    <div class="cockpit-row"><strong>{entry.parameter}</strong><span>{parameter_status(&entry.status)}</span></div>
                }).collect_view()}
            </CockpitSection>

            <CockpitSection number=4 title="Canonical resource inventories and editors" search expanded always_searchable=true>
                <ResourceLaboratory model dispatcher search />
            </CockpitSection>

            <CockpitSection number=5 title="Follow, Pinned, and Inline binding matrix" search expanded>
                <BindingMatrix model dispatcher />
            </CockpitSection>

            <CockpitSection number=6 title="Current semantic output and calculation provenance" search expanded>
                <SemanticOutput model />
            </CockpitSection>

            <CockpitSection number=7 title="Repository heads, history, conflicts, and deletion" search expanded>
                <RepositoryLaboratory model dispatcher />
            </CockpitSection>

            <CockpitSection number=8 title="Diagnostics, trace, macros, and control manifest" search expanded>
                <p class="cockpit-note">"Diagnostics, trace, macro recording/replay, exports, and the native semantic control manifest remain mounted in the workstation below and use the same application coordinator."</p>
            </CockpitSection>
        </section>
    }
}

#[component]
fn BindingMatrix(model: RwSignal<AppReadModel>, dispatcher: WorkbenchCoordinator) -> impl IntoView {
    view! { {move || {
        let snapshot=model.get();
        snapshot.inspector.bindings.into_iter().map(|binding| {
            let kind=binding_resource_kind(binding.slot);
            let candidates=snapshot.resources.inventories.iter().find(|inventory| inventory.kind == kind).map(|inventory| inventory.resources.clone()).unwrap_or_default();
            let (current_mode, current_id, current_revision)=match &binding.source {
                mirabile_app::BindingSourceSummary::Follow { resource_id, revision, .. } => ("follow", Some(*resource_id), Some(*revision)),
                mirabile_app::BindingSourceSummary::Pinned { resource_id, revision, .. } => ("pinned", Some(*resource_id), Some(*revision)),
                mirabile_app::BindingSourceSummary::Inline => ("inline", None, None),
            };
            let fallback=candidates.first().map(|resource| (resource.resource_id, resource.revision));
            let selected_id=current_id.or(fallback.map(|value| value.0));
            let selected_revision=current_revision.or(fallback.map(|value| value.1));
            let enabled=selected_id.is_some();
            let reason=(!enabled).then_some("Create a compatible canonical resource before changing this binding");
            let mode_dispatcher=dispatcher;
            let resource_dispatcher=dispatcher;
            let revision_dispatcher=dispatcher;
            let slot=binding.slot;
            let slot_key=binding_slot_key(slot);
            view! { <article class="binding-row">
                <header><strong>{binding.label}</strong><small>{format!("{:?}", binding.source)}</small></header>
                <label>"Mode"<select prop:value=current_mode disabled=!enabled
                    data-mirabile-control=ControlId::BINDING_MODE.to_string()
                    data-mirabile-address=binding_address(ControlId::BINDING_MODE, &slot_key)
                    data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled=enabled.to_string()
                    data-mirabile-disabled-reason=reason
                    on:change=move |event| if let (Some(resource_id), Some(revision))=(selected_id, selected_revision) && let Some(selection)=binding_selection(&event_target_value(&event), resource_id, revision) { mode_dispatcher.dispatch(AppIntent::SetWorkspaceBinding { slot, selection }); }>
                    <option value="follow">"Follow latest"</option><option value="pinned">"Pinned revision"</option><option value="inline">"Inline copy"</option>
                </select></label>
                <label>"Resource"<select prop:value=selected_id.map(|id| id.to_string()).unwrap_or_default() disabled=!enabled
                    data-mirabile-control=ControlId::BINDING_RESOURCE.to_string()
                    data-mirabile-address=binding_address(ControlId::BINDING_RESOURCE, &slot_key)
                    data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled=enabled.to_string()
                    data-mirabile-disabled-reason=reason
                    on:change=move |event| if let (Ok(resource_id), Some(revision))=(event_target_value(&event).parse(), selected_revision) && let Some(selection)=binding_selection(current_mode, resource_id, revision) { resource_dispatcher.dispatch(AppIntent::SetWorkspaceBinding { slot, selection }); }>
                    {candidates.iter().map(|resource| view! { <option value=resource.resource_id.to_string()>{format!("{} · r{}", resource.title, resource.revision)}</option> }).collect_view()}
                </select></label>
                <label>"Revision"<select prop:value=selected_revision.map(|revision| revision.to_string()).unwrap_or_default() disabled=current_mode != "pinned" || !enabled
                    data-mirabile-control=ControlId::BINDING_REVISION.to_string()
                    data-mirabile-address=binding_address(ControlId::BINDING_REVISION, &slot_key)
                    data-mirabile-kind=ControlKind::Select.as_str()
                    data-mirabile-enabled=(current_mode == "pinned" && enabled).to_string()
                    data-mirabile-disabled-reason=if current_mode == "pinned" && enabled { None } else { Some("Choose Pinned mode to select an immutable revision") }
                    on:change=move |event| if let (Some(resource_id), Some(revision))=(selected_id, parse_revision(&event_target_value(&event))) { revision_dispatcher.dispatch(AppIntent::SetWorkspaceBinding { slot, selection: WorkspaceBindingSelection::Pinned { resource_id, revision } }); }>
                    {selected_revision.into_iter().map(|revision| view! { <option value=revision.to_string()>{revision.to_string()}</option> }).collect_view()}
                </select></label>
            </article> }
        }).collect_view()
    }} }
}

fn binding_resource_kind(slot: WorkspaceBindingSlot) -> mirabile_app::ResourceKind {
    match slot {
        WorkspaceBindingSlot::DisplayedPoints
        | WorkspaceBindingSlot::AspectedPoints
        | WorkspaceBindingSlot::TransitPoints => mirabile_app::ResourceKind::PointSet,
        WorkspaceBindingSlot::Aspects => mirabile_app::ResourceKind::AspectSet,
        WorkspaceBindingSlot::Analysis => mirabile_app::ResourceKind::AnalysisProfile,
        WorkspaceBindingSlot::Theme => mirabile_app::ResourceKind::Theme,
        WorkspaceBindingSlot::Wheel => mirabile_app::ResourceKind::WheelTemplate,
        WorkspaceBindingSlot::ViewDocument { .. } => mirabile_app::ResourceKind::ViewDocument,
    }
}

fn binding_selection(
    mode: &str,
    resource_id: mirabile_app::ResourceId,
    revision: mirabile_app::Revision,
) -> Option<WorkspaceBindingSelection> {
    match mode {
        "follow" => Some(WorkspaceBindingSelection::Follow { resource_id }),
        "pinned" => Some(WorkspaceBindingSelection::Pinned {
            resource_id,
            revision,
        }),
        "inline" => Some(WorkspaceBindingSelection::Inline { resource_id }),
        _ => None,
    }
}

fn binding_slot_key(slot: WorkspaceBindingSlot) -> String {
    match slot {
        WorkspaceBindingSlot::DisplayedPoints => "displayed-points".into(),
        WorkspaceBindingSlot::AspectedPoints => "aspected-points".into(),
        WorkspaceBindingSlot::TransitPoints => "transit-points".into(),
        WorkspaceBindingSlot::Aspects => "aspects".into(),
        WorkspaceBindingSlot::Analysis => "analysis".into(),
        WorkspaceBindingSlot::Theme => "theme".into(),
        WorkspaceBindingSlot::Wheel => "wheel".into(),
        WorkspaceBindingSlot::ViewDocument { view_id } => format!("view-{view_id}"),
    }
}

fn binding_address(control: ControlId, slot: &str) -> String {
    ControlAddress::qualified(control, [("slot", slot.to_owned())])
        .expect("binding address")
        .to_string()
}

fn parse_revision(value: &str) -> Option<mirabile_app::Revision> {
    value
        .parse::<u64>()
        .ok()
        .and_then(|value| mirabile_app::Revision::new(value).ok())
}

#[component]
fn CockpitSection(
    number: u8,
    title: &'static str,
    search: RwSignal<String>,
    expanded: RwSignal<bool>,
    #[prop(default = false)] always_searchable: bool,
    children: Children,
) -> impl IntoView {
    let matches = move || {
        let query = search.get().to_lowercase();
        query.is_empty() || title.to_lowercase().contains(&query) || always_searchable
    };
    view! {
        <details class="cockpit-section" open=move || expanded.get() hidden=move || !matches()>
            <summary>{format!("{number}. {title}")}</summary>
            <div class="cockpit-section-body">{children()}</div>
        </details>
    }
}

#[component]
fn StatusCard(label: &'static str, value: String) -> impl IntoView {
    view! { <div class="cockpit-card"><span>{label}</span><strong>{value}</strong></div> }
}

#[component]
fn ResourceLaboratory(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
    search: RwSignal<String>,
) -> impl IntoView {
    view! {
        {move || model.get().resources.inventories.into_iter().filter(|inventory| {
            let query=search.get().to_lowercase();
            query.is_empty() || inventory.label.to_lowercase().contains(&query) || inventory.resources.iter().any(|resource| resource.title.to_lowercase().contains(&query) || resource.tags.iter().any(|tag| tag.to_lowercase().contains(&query)))
        }).map(|inventory| {
            let kind = ResourceDraftKind::try_from(inventory.kind).expect("canonical inventory kind");
            let new_dispatcher = dispatcher;
            let (new_enabled, new_reason) = new_availability(kind);
            view! {
                <article class="resource-laboratory" data-resource-kind=format!("{:?}", kind)>
                    <header>
                        <div><h3>{inventory.label}</h3><small>{format!("{} present", inventory.resources.len())}</small></div>
                        <button
                            type="button"
                            class="button secondary"
                            data-mirabile-control=ControlId::RESOURCE_NEW.to_string()
                            data-mirabile-address=resource_address(ControlId::RESOURCE_NEW, kind, None)
                            data-mirabile-kind=ControlKind::Action.as_str()
                            data-mirabile-enabled=new_enabled.to_string()
                            data-mirabile-disabled-reason=new_reason
                            disabled=!new_enabled
                            on:click=move |_| dispatch_new(new_dispatcher, kind)
                        >"New"</button>
                    </header>
                    <div class="resource-inventory">
                        {inventory.resources.into_iter().map(|resource| {
                            let edit_dispatcher = dispatcher;
                            let (enabled, reason) = edit_availability(kind);
                            view! {
                                <div class="cockpit-row">
                                    <span><strong>{resource.title}</strong><small>{format!("r{} · {} tag(s)", resource.revision, resource.tags.len())}</small></span>
                                    <button
                                        type="button"
                                        class="button secondary"
                                        data-mirabile-control=ControlId::RESOURCE_EDIT.to_string()
                                        data-mirabile-address=resource_address(ControlId::RESOURCE_EDIT, kind, Some(resource.resource_id))
                                        data-mirabile-kind=ControlKind::Action.as_str()
                                        data-mirabile-enabled=enabled.to_string()
                                        data-mirabile-disabled-reason=reason
                                        disabled=!enabled
                                        on:click=move |_| dispatch_edit(edit_dispatcher, kind, resource.resource_id)
                                    >"Edit"</button>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                    {move || model.get().resource_editor.drafts.into_iter().find(|draft| draft.kind == kind).map(|draft| {
                        let resource_id = draft.resource_id;
                        let address_resource = (!matches!(draft.state, DraftState::New)).then_some(resource_id).flatten();
                        let title_dispatcher = dispatcher;
                        let description_dispatcher = dispatcher;
                        let tags_dispatcher = dispatcher;
                        let save_dispatcher = dispatcher;
                        let cancel_dispatcher = dispatcher;
                        let save_enabled = matches!(draft.state, DraftState::New | DraftState::Dirty { .. });
                        view! {
                            <div class="typed-resource-editor">
                                <span class="draft-state">{format!("{:?}", draft.state)}</span>
                                <label>"Title"<input type="text" prop:value=draft.title
                                    data-mirabile-control=ControlId::RESOURCE_TITLE.to_string()
                                    data-mirabile-address=resource_address(ControlId::RESOURCE_TITLE, kind, address_resource)
                                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                                    on:change=move |event| dispatch_metadata(title_dispatcher, kind, ResourceMetadataMutation::SetTitle(event_target_value(&event))) /></label>
                                <label>"Description"<textarea prop:value=draft.description.unwrap_or_default()
                                    data-mirabile-control=ControlId::RESOURCE_DESCRIPTION.to_string()
                                    data-mirabile-address=resource_address(ControlId::RESOURCE_DESCRIPTION, kind, address_resource)
                                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                                    on:change=move |event| { let value=event_target_value(&event); dispatch_metadata(description_dispatcher, kind, ResourceMetadataMutation::SetDescription((!value.trim().is_empty()).then_some(value))); } /></label>
                                <label>"Tags"<input type="text" prop:value=draft.tags.join(", ")
                                    data-mirabile-control=ControlId::RESOURCE_TAGS.to_string()
                                    data-mirabile-address=resource_address(ControlId::RESOURCE_TAGS, kind, address_resource)
                                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                                    on:change=move |event| dispatch_metadata(tags_dispatcher, kind, ResourceMetadataMutation::SetTags(event_target_value(&event).split(',').map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned).collect())) /></label>
                                <PayloadEditor kind value=draft.value.clone() nested=draft.nested.clone() point_options=model.get().authoring.points dispatcher />
                                <p class="persisted-label">{payload_summary(&draft.value)}</p>
                                <div class="draft-actions">
                                    <button type="button" class="button primary" disabled=!save_enabled
                                        data-mirabile-control=ControlId::RESOURCE_SAVE.to_string()
                                        data-mirabile-address=resource_address(ControlId::RESOURCE_SAVE, kind, address_resource)
                                        data-mirabile-kind=ControlKind::Action.as_str()
                                        data-mirabile-enabled=save_enabled.to_string()
                                        data-mirabile-disabled-reason=(!save_enabled).then_some("Draft has no saveable changes")
                                        on:click=move |_| save_dispatcher.dispatch(AppIntent::SaveResourceDraft { kind })>"Save"</button>
                                    <button type="button" class="button secondary"
                                        data-mirabile-control=ControlId::RESOURCE_CANCEL.to_string()
                                        data-mirabile-address=resource_address(ControlId::RESOURCE_CANCEL, kind, address_resource)
                                        data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true"
                                        on:click=move |_| cancel_dispatcher.dispatch(AppIntent::CancelResourceDraft { kind })>"Cancel"</button>
                                </div>
                            </div>
                        }
                    })}
                </article>
            }
        }).collect_view()}
    }
}

#[component]
fn SemanticOutput(model: RwSignal<AppReadModel>) -> impl IntoView {
    view! { {move || {
        let output=model.get().semantic_output;
        if let Some(reason)=output.unavailable_reason {
            return view! { <p class="cockpit-note unavailable-reason">{format!("Unavailable — {reason}")}</p> }.into_any();
        }
        view! { <div class="semantic-tables">
            <h3>"Points"</h3>
            <table><thead><tr><th>"Point"</th><th>"Longitude"</th><th>"Latitude"</th><th>"Speed/day"</th><th>"State"</th></tr></thead>
            <tbody>{output.points.into_iter().map(|point| view! { <tr data-point=point.point_id.to_string()><td>{point.point_id.to_string()}</td><td>{format!("{:.6}°", point.longitude_degrees)}</td><td>{format!("{:.6}°", point.latitude_degrees)}</td><td>{format!("{:.6}°", point.speed_degrees_per_day)}</td><td>{if point.retrograde { "Retrograde" } else if point.derived { "Derived" } else { "Direct" }}</td></tr> }).collect_view()}</tbody></table>
            <h3>"Houses"</h3>
            {if output.houses.is_empty() { view! { <p class="unavailable-reason">"Unavailable — the last-good calculation contains no house cusps"</p> }.into_any() } else { view! { <table><thead><tr><th>"House"</th><th>"Cusp"</th></tr></thead><tbody>{output.houses.into_iter().map(|house| view! { <tr data-house=house.number.to_string()><td>{house.number}</td><td>{format!("{:.6}°", house.cusp_degrees)}</td></tr> }).collect_view()}</tbody></table> }.into_any() }}
            <h3>"Angles"</h3>
            {if output.angles.is_empty() { view! { <p class="unavailable-reason">"Unavailable — the last-good calculation contains no chart angles"</p> }.into_any() } else { view! { <table><thead><tr><th>"Angle"</th><th>"Longitude"</th></tr></thead><tbody>{output.angles.into_iter().map(|angle| { let address=angle.name.to_lowercase(); view! { <tr data-angle=address><td>{angle.name}</td><td>{format!("{:.6}°", angle.longitude_degrees)}</td></tr> } }).collect_view()}</tbody></table> }.into_any() }}
            <h3>"Aspects"</h3>
            {if output.aspects.is_empty() { view! { <p class="unavailable-reason">"No aspect hits in the current bounded analysis."</p> }.into_any() } else { view! { <table><thead><tr><th>"LHS"</th><th>"Aspect"</th><th>"RHS"</th><th>"Separation"</th><th>"Orb"</th><th>"Applying"</th></tr></thead><tbody>{output.aspects.into_iter().map(|aspect| view! { <tr data-aspect=aspect.aspect.to_string()><td>{aspect.lhs.to_string()}</td><td>{aspect.aspect.to_string()}</td><td>{aspect.rhs.to_string()}</td><td>{format!("{:.6}°", aspect.separation_degrees)}</td><td>{format!("{:.6}°", aspect.orb_degrees)}</td><td>{aspect.applying.map_or("Not requested".into(), |value| value.to_string())}</td></tr> }).collect_view()}</tbody></table> }.into_any() }}
            <h3>"Provenance"</h3>
            <table><thead><tr><th>"Responsibility"</th><th>"Implementation"</th><th>"Detail"</th></tr></thead><tbody>{output.provenance.into_iter().map(|entry| view! { <tr><td>{entry.responsibility}</td><td>{entry.implementation}</td><td>{entry.detail}</td></tr> }).collect_view()}</tbody></table>
        </div> }.into_any()
    }} }
}

fn parameter_status(status: &mirabile_app::ParameterStatus) -> String {
    match status {
        mirabile_app::ParameterStatus::Live => "Live".into(),
        mirabile_app::ParameterStatus::Persisted => "Persisted".into(),
        mirabile_app::ParameterStatus::ReadOnly => "Read-only".into(),
        mirabile_app::ParameterStatus::Unavailable { reason } => format!("Unavailable — {reason}"),
    }
}

#[component]
fn DerivedRecipeFields(
    recipe: mirabile_app::DerivationSpec,
    composite_rows: Vec<mirabile_app::StableDraftItemReadModel<mirabile_app::ResourceId>>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    match recipe {
        mirabile_app::DerivationSpec::Harmonic { radix, harmonic } => view! { <div class="recipe-fields">
            <label>"Radix resource ID"<input type="text" prop:value=radix.to_string() data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("radix") data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(radix)=event_target_value(&event).parse() { dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::SetHarmonic { radix, harmonic }); } /></label>
            <label>"Harmonic"<input type="number" min="0.0001" step="0.1" prop:value=harmonic data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("harmonic") data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(harmonic)=event_target_value(&event).parse() { dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::SetHarmonic { radix, harmonic }); } /></label>
        </div> }.into_any(),
        mirabile_app::DerivationSpec::Transit { at, location } => { let date_at=at.clone(); let time_at=at.clone(); let location_for_date=location.clone(); let location_for_time=location.clone(); view! { <div class="recipe-fields">
            <label>"Transit date"<input type="date" prop:value=format_recipe_date(at.civil_datetime.date) data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("date") data-mirabile-kind=ControlKind::Date.as_str() data-mirabile-enabled="true" on:change=move |event| if let Some(date)=parse_recipe_date(&event_target_value(&event)) { let mut at=date_at.clone(); at.civil_datetime.date=date; dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::SetTransit { at, location:location_for_date.clone() }); } /></label>
            <label>"Transit time"<input type="time" step="1" prop:value=format_recipe_time(at.civil_datetime.time) data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("time") data-mirabile-kind=ControlKind::Time.as_str() data-mirabile-enabled="true" on:change=move |event| if let Some(time)=parse_recipe_time(&event_target_value(&event)) { let mut at=time_at.clone(); at.civil_datetime.time=time; dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::SetTransit { at, location:location_for_time.clone() }); } /></label>
            <RecipeLocationFields location mutation_kind="transit" at=Some(at) radix=None dispatcher />
        </div> }.into_any() }
        mirabile_app::DerivationSpec::Relocation { radix, location } => { let location_for_radix=location.clone(); view! { <div class="recipe-fields">
            <label>"Radix resource ID"<input type="text" prop:value=radix.to_string() data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("radix") data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(next)=event_target_value(&event).parse() { dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::SetRelocation { radix:next, location:location_for_radix.clone() }); } /></label>
            <RecipeLocationFields location mutation_kind="relocation" at=None radix=Some(radix) dispatcher />
        </div> }.into_any() },
        mirabile_app::DerivationSpec::Composite { charts, method } => view! { <div class="recipe-fields">
            <label>"Method"<select prop:value=match method { mirabile_app::CompositeMethod::Midpoint=>"midpoint", mirabile_app::CompositeMethod::Davison=>"davison" } data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("method") data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::SetCompositeMethod(if event_target_value(&event)=="davison" { mirabile_app::CompositeMethod::Davison } else { mirabile_app::CompositeMethod::Midpoint }))><option value="midpoint">"Midpoint"</option><option value="davison">"Davison"</option></select></label>
            {composite_rows.into_iter().map(|row| { let item_id=row.item_id; view! { <div class="builder-row"><label>"Chart resource ID"<input type="text" prop:value=row.value.to_string() data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_RECIPE_FIELD, "chartdefinition", "composite-charts", item_id, Some("resource")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::CompositeCharts(mirabile_app::DraftListMutation::Update { item_id, value })); } /></label><button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_MOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_MOVE, "chartdefinition", "composite-charts", item_id, Some("end")) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::CompositeCharts(mirabile_app::DraftListMutation::Move { item_id, before:None }))>"Move to end"</button><button type="button" class="button danger" data-mirabile-control=ControlId::RESOURCE_LIST_REMOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_REMOVE, "chartdefinition", "composite-charts", item_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled=(charts.len()>2).to_string() disabled=charts.len()<=2 on:click=move |_| dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::CompositeCharts(mirabile_app::DraftListMutation::Remove { item_id }))>"Remove"</button></div> } }).collect_view()}
            <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_INSERT.to_string() data-mirabile-address=ControlAddress::qualified(ControlId::RESOURCE_LIST_INSERT, [("kind", "chartdefinition"), ("collection", "composite-charts")]).expect("composite address").to_string() data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatch_recipe(dispatcher, mirabile_app::DerivedRecipeMutation::CompositeCharts(mirabile_app::DraftListMutation::Insert { after:None, value:mirabile_app::ResourceId::new() }))>"Add chart"</button>
        </div> }.into_any(),
    }
}

#[component]
fn RecipeLocationFields(
    location: mirabile_app::LocationAssertion,
    mutation_kind: &'static str,
    at: Option<mirabile_app::TemporalAssertion>,
    radix: Option<mirabile_app::ResourceId>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let name_base = location.clone();
    let latitude_base = location.clone();
    let longitude_base = location.clone();
    let name_at = at.clone();
    let latitude_at = at.clone();
    let longitude_at = at;
    view! { <div class="location-fields"><label>"Location name"<input type="text" prop:value=location.display_name data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("location-name") data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| { let mut location=name_base.clone(); location.display_name=event_target_value(&event); dispatch_recipe_location(dispatcher, mutation_kind, name_at.clone(), radix, location); } /></label><label>"Latitude"<input type="number" min="-90" max="90" step="0.0001" prop:value=location.latitude.degrees() data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("latitude") data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(value)=event_target_value(&event).parse() && let Ok(latitude)=mirabile_app::Latitude::from_degrees(value) { let mut location=latitude_base.clone(); location.latitude=latitude; dispatch_recipe_location(dispatcher, mutation_kind, latitude_at.clone(), radix, location); } /></label><label>"Longitude"<input type="number" min="-180" max="180" step="0.0001" prop:value=location.longitude.degrees() data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=recipe_address("longitude") data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(value)=event_target_value(&event).parse() && let Ok(longitude)=mirabile_app::Longitude::from_degrees(value) { let mut location=longitude_base.clone(); location.longitude=longitude; dispatch_recipe_location(dispatcher, mutation_kind, longitude_at.clone(), radix, location); } /></label></div> }
}

fn dispatch_recipe(
    dispatcher: WorkbenchCoordinator,
    mutation: mirabile_app::DerivedRecipeMutation,
) {
    dispatch_payload(
        dispatcher,
        ResourceMutation::ChartDefinition(ChartDefinitionMutation::MutateDerivedRecipe(mutation)),
    );
}
fn dispatch_recipe_location(
    dispatcher: WorkbenchCoordinator,
    kind: &str,
    at: Option<mirabile_app::TemporalAssertion>,
    radix: Option<mirabile_app::ResourceId>,
    location: mirabile_app::LocationAssertion,
) {
    if kind == "transit" {
        if let Some(at) = at {
            dispatch_recipe(
                dispatcher,
                mirabile_app::DerivedRecipeMutation::SetTransit { at, location },
            );
        }
    } else if let Some(radix) = radix {
        dispatch_recipe(
            dispatcher,
            mirabile_app::DerivedRecipeMutation::SetRelocation { radix, location },
        );
    }
}
fn recipe_address(field: &'static str) -> String {
    qualified_resource_address(
        ControlId::RESOURCE_RECIPE_FIELD,
        ResourceDraftKind::ChartDefinition,
        "field",
        field,
    )
}
fn format_recipe_date(value: mirabile_app::CivilDate) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year(),
        value.month(),
        value.day()
    )
}
fn parse_recipe_date(value: &str) -> Option<mirabile_app::CivilDate> {
    let mut parts = value.split('-');
    mirabile_app::CivilDate::new(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    )
    .ok()
}
fn format_recipe_time(value: mirabile_app::CivilTime) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        value.hour(),
        value.minute(),
        value.second()
    )
}
fn parse_recipe_time(value: &str) -> Option<mirabile_app::CivilTime> {
    let mut parts = value.split(':');
    mirabile_app::CivilTime::new(
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next().unwrap_or("0").parse().ok()?,
    )
    .ok()
}

#[component]
fn PayloadEditor(
    kind: ResourceDraftKind,
    value: mirabile_app::ResourceDraftValueReadModel,
    nested: mirabile_app::NestedResourceDraftReadModel,
    point_options: Vec<mirabile_app::AuthoringOption<mirabile_app::PointId>>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    use mirabile_app::ResourceDraftValueReadModel as Value;

    match value {
        Value::ChartDefinition(definition) => {
            let source_kind=match &definition.source { mirabile_app::ChartSource::Radix { .. } => "radix", mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Transit { .. } } => "transit", mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Harmonic { .. } } => "harmonic", mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Relocation { .. } } => "relocation", mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Composite { .. } } => "composite" };
            let fallback=recipe_reference(&definition).unwrap_or_default();
            view! { <fieldset class="payload-fields"><legend>"Persisted derived recipe"</legend>
                <label>"Recipe type"<select prop:value=source_kind data-mirabile-control=ControlId::RESOURCE_RECIPE_FIELD.to_string() data-mirabile-address=qualified_resource_address(ControlId::RESOURCE_RECIPE_FIELD, kind, "field", "type") data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| { let source=match event_target_value(&event).as_str() { "transit" => mirabile_app::ChartSource::Derived { recipe: default_transit_recipe() }, "relocation" => mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Relocation { radix: fallback, location: default_recipe_location() } }, "composite" => mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Composite { charts: vec![fallback, mirabile_app::ResourceId::new()], method: mirabile_app::CompositeMethod::Midpoint } }, _ => mirabile_app::ChartSource::Derived { recipe: mirabile_app::DerivationSpec::Harmonic { radix: fallback, harmonic: 2.0 } } }; dispatch_payload(dispatcher, ResourceMutation::ChartDefinition(ChartDefinitionMutation::SetSource(source))); }><option value="transit">"Transit"</option><option value="harmonic">"Harmonic"</option><option value="relocation">"Relocation"</option><option value="composite">"Composite"</option></select></label>
                {if let mirabile_app::ChartSource::Derived { recipe }=definition.source { let composite_rows=match nested { mirabile_app::NestedResourceDraftReadModel::ChartDefinition { composite_charts } => composite_charts, _ => Vec::new() }; view! { <DerivedRecipeFields recipe composite_rows dispatcher /> }.into_any() } else { view! { <small>"Radix charts are edited through the atomic chart editor."</small> }.into_any() }}
                <small>"Derived recipes are persisted-only and are not executable."</small>
            </fieldset> }.into_any()
        }
        Value::PointSet(point_set) => {
            let selector_rows=match &nested { mirabile_app::NestedResourceDraftReadModel::PointSet(items) => items.clone(), _ => Vec::new() };
            view! { <fieldset class="payload-fields point-fields"><legend>"Point selectors"</legend>
                {point_options.into_iter().map(|option| {
                    let point_id=option.value.clone();
                    let index=point_set.points.iter().position(|selector| matches!(selector, mirabile_app::PointSelector::Point(point) if point == &point_id));
                    let checked=index.is_some();
                    let item_id=index.and_then(|index| selector_rows.get(index).map(|item| item.item_id));
                    let address=ControlAddress::qualified(ControlId::RESOURCE_POINT, [("kind", "pointset"), ("point", point_id.as_str())]).expect("point resource address").to_string();
                    view! { <label class="checkbox-field"><input type="checkbox" prop:checked=checked disabled=!option.enabled
                        data-mirabile-control=ControlId::RESOURCE_POINT.to_string()
                        data-mirabile-address=address data-mirabile-kind=ControlKind::Checkbox.as_str()
                        data-mirabile-enabled=option.enabled.to_string() data-mirabile-disabled-reason=option.disabled_reason
                        on:change=move |event| { let mutation=if event_target_checked(&event) { mirabile_app::DraftListMutation::Insert { after: None, value: mirabile_app::PointSelector::Point(point_id.clone()) } } else if let Some(item_id)=item_id { mirabile_app::DraftListMutation::Remove { item_id } } else { return; }; dispatch_payload(dispatcher, ResourceMutation::PointSet(PointSetMutation::Selectors(mutation))); } />{option.label}</label> }
                }).collect_view()}
                <PointCategoryBuilder rows=selector_rows dispatcher />
            </fieldset> }.into_any()
        }
        Value::AnalysisProfile(profile) => {
            let applying = profile.clone();
            let patterns = profile.clone();
            let maximum = profile.clone();
            view! {
                <fieldset class="payload-fields"><legend>"Analysis options"</legend>
                    <label class="checkbox-field"><input type="checkbox" prop:checked=profile.include_applying_state
                        data-mirabile-control=ControlId::RESOURCE_ANALYSIS_APPLYING.to_string()
                        data-mirabile-address=resource_address(ControlId::RESOURCE_ANALYSIS_APPLYING, kind, None)
                        data-mirabile-kind=ControlKind::Toggle.as_str() data-mirabile-enabled="true"
                        on:change=move |event| { let mut next=applying.clone(); next.include_applying_state=event_target_checked(&event); dispatch_payload(dispatcher, ResourceMutation::AnalysisProfile(AnalysisProfileMutation::SetProfile(next))); } />"Include applying/separating state"</label>
                    <label class="checkbox-field"><input type="checkbox" prop:checked=profile.include_patterns
                        data-mirabile-control=ControlId::RESOURCE_ANALYSIS_PATTERNS.to_string()
                        data-mirabile-address=resource_address(ControlId::RESOURCE_ANALYSIS_PATTERNS, kind, None)
                        data-mirabile-kind=ControlKind::Toggle.as_str() data-mirabile-enabled="true"
                        on:change=move |event| { let mut next=patterns.clone(); next.include_patterns=event_target_checked(&event); dispatch_payload(dispatcher, ResourceMutation::AnalysisProfile(AnalysisProfileMutation::SetProfile(next))); } />"Include patterns"</label>
                    <label>"Maximum hits (blank means unlimited)"<input type="number" min="1" prop:value=profile.maximum_hits.map(|value| value.to_string()).unwrap_or_default()
                        data-mirabile-control=ControlId::RESOURCE_ANALYSIS_MAXIMUM_HITS.to_string()
                        data-mirabile-address=resource_address(ControlId::RESOURCE_ANALYSIS_MAXIMUM_HITS, kind, None)
                        data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true"
                        on:change=move |event| { let mut next=maximum.clone(); let raw=event_target_value(&event); next.maximum_hits=if raw.trim().is_empty() { None } else { raw.parse().ok() }; dispatch_payload(dispatcher, ResourceMutation::AnalysisProfile(AnalysisProfileMutation::SetProfile(next))); } /></label>
                </fieldset>
            }.into_any()
        }
        Value::Theme(theme) => {
            let colors = [
                ("Background", "background", theme.background.clone()),
                ("Foreground", "foreground", theme.foreground.clone()),
                ("Muted", "muted", theme.muted.clone()),
                ("Accent", "accent", theme.accent.clone()),
                ("Aspect", "aspect", theme.aspect_color.clone()),
            ];
            view! { <fieldset class="payload-fields color-fields"><legend>"Canonical colors"</legend>
                {colors.into_iter().map(|(label, field, current)| {
                    let base=theme.clone();
                    view! { <label>{label}<input type="color" prop:value=current
                        data-mirabile-control=ControlId::RESOURCE_THEME_COLOR.to_string()
                        data-mirabile-address=qualified_resource_address(ControlId::RESOURCE_THEME_COLOR, kind, "field", field)
                        data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                        on:change=move |event| { let mut next=base.clone(); let value=event_target_value(&event); match field { "background" => next.background=value, "foreground" => next.foreground=value, "muted" => next.muted=value, "accent" => next.accent=value, _ => next.aspect_color=value } dispatch_payload(dispatcher, ResourceMutation::Theme(ThemeMutation::SetTheme(next))); } /></label> }
                }).collect_view()}
            </fieldset> }.into_any()
        }
        Value::WheelTemplate(template) => {
            let ring_rows=match &nested { mirabile_app::NestedResourceDraftReadModel::WheelTemplate(items) => items.clone(), _ => Vec::new() };
            let fields = [
                ("Show house cusps", "house-cusps", template.houses.show_cusps),
                ("Show house numbers", "house-numbers", template.houses.show_numbers),
                ("Show zodiac boundaries", "zodiac-boundaries", template.zodiac.show_boundaries),
                ("Show zodiac labels", "zodiac-labels", template.zodiac.show_labels),
                ("Show degrees", "label-degrees", template.labels.show_degrees),
                ("Show retrograde", "label-retrograde", template.labels.show_retrograde),
            ];
            let radius=template.clone();
            view! { <fieldset class="payload-fields"><legend>"Wheel geometry and display"</legend>
                {fields.into_iter().map(|(label, field, checked)| { let base=template.clone(); view! { <label class="checkbox-field"><input type="checkbox" prop:checked=checked
                    data-mirabile-control=ControlId::RESOURCE_WHEEL_FIELD.to_string()
                    data-mirabile-address=qualified_resource_address(ControlId::RESOURCE_WHEEL_FIELD, kind, "field", field)
                    data-mirabile-kind=ControlKind::Toggle.as_str() data-mirabile-enabled="true"
                    on:change=move |event| { let mut next=base.clone(); let value=event_target_checked(&event); match field { "house-cusps" => next.houses.show_cusps=value, "house-numbers" => next.houses.show_numbers=value, "zodiac-boundaries" => next.zodiac.show_boundaries=value, "zodiac-labels" => next.zodiac.show_labels=value, "label-degrees" => next.labels.show_degrees=value, _ => next.labels.show_retrograde=value } dispatch_payload(dispatcher, ResourceMutation::WheelTemplate(WheelTemplateMutation::SetTemplateFields(next))); } />{label}</label> } }).collect_view()}
                <label>"Aspect field radius"<input type="number" min="0.01" step="0.01" prop:value=template.aspect_field.radius
                    data-mirabile-control=ControlId::RESOURCE_WHEEL_FIELD.to_string()
                    data-mirabile-address=qualified_resource_address(ControlId::RESOURCE_WHEEL_FIELD, kind, "field", "aspect-radius")
                    data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true"
                    on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { let mut next=radius.clone(); next.aspect_field.radius=value; dispatch_payload(dispatcher, ResourceMutation::WheelTemplate(WheelTemplateMutation::SetTemplateFields(next))); } /></label>
                <small>{format!("{} stable ring row(s)", template.rings.len())}</small>
                <WheelRingBuilder rows=ring_rows dispatcher />
            </fieldset> }.into_any()
        }
        Value::ViewDocument(document) => {
            let (slot_rows, object_rows)=match &nested { mirabile_app::NestedResourceDraftReadModel::ViewDocument { chart_slots, objects } => (chart_slots.clone(), objects.clone()), _ => (Vec::new(), Vec::new()) };
            let width = document.layout.clone();
            let height = document.layout.clone();
            view! { <fieldset class="payload-fields"><legend>"Page layout"</legend>
                <label>"Width"<input type="number" min="1" step="1" prop:value=document.layout.width
                    data-mirabile-control=ControlId::RESOURCE_VIEW_WIDTH.to_string()
                    data-mirabile-address=resource_address(ControlId::RESOURCE_VIEW_WIDTH, kind, None)
                    data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true"
                    on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { let mut next=width.clone(); next.width=value; dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::SetLayout(next))); } /></label>
                <label>"Height"<input type="number" min="1" step="1" prop:value=document.layout.height
                    data-mirabile-control=ControlId::RESOURCE_VIEW_HEIGHT.to_string()
                    data-mirabile-address=resource_address(ControlId::RESOURCE_VIEW_HEIGHT, kind, None)
                    data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true"
                    on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { let mut next=height.clone(); next.height=value; dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::SetLayout(next))); } /></label>
                <small>{format!("{} slot(s); {} dormant or rendered object(s)", document.chart_slots.len(), document.objects.len())}</small>
                <ViewSlotBuilder rows=slot_rows objects=object_rows dispatcher />
            </fieldset> }.into_any()
        }
        Value::QueryDefinition(query) => {
            let tree=match nested { mirabile_app::NestedResourceDraftReadModel::QueryDefinition(tree) => Some(tree), _ => None };
            view! { <fieldset class="payload-fields"><legend>"Query definition"</legend>
                <label>"Query description"<textarea prop:value=query.description.unwrap_or_default()
                    data-mirabile-control=ControlId::RESOURCE_QUERY_DESCRIPTION.to_string()
                    data-mirabile-address=resource_address(ControlId::RESOURCE_QUERY_DESCRIPTION, kind, None)
                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                    on:change=move |event| { let value=event_target_value(&event); dispatch_payload(dispatcher, ResourceMutation::QueryDefinition(QueryDefinitionMutation::SetDescription((!value.trim().is_empty()).then_some(value)))); } /></label>
                <small>"The typed AST is persisted but execution is deferred."</small>
                {tree.map(|tree| view! { <QueryTreeBuilder tree dispatcher /> })}
            </fieldset> }.into_any()
        }
        _ => view! { <p class="cockpit-note">"Payload fields are controlled by the authoritative composite/session editor or the typed list builder for this resource."</p> }.into_any(),
    }
}

#[component]
fn PointCategoryBuilder(
    rows: Vec<mirabile_app::StableDraftItemReadModel<mirabile_app::PointSelector>>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let category = RwSignal::new(String::new());
    let last = rows.last().map(|row| row.item_id);
    view! { <div class="nested-builder"><h4>"Category selectors"</h4>
        {rows.into_iter().filter_map(|row| match row.value { mirabile_app::PointSelector::Category(value) => Some((row.item_id, value)), mirabile_app::PointSelector::Point(_) => None }).map(|(item_id, value)| {
            let update=dispatcher; let remove=dispatcher; let move_dispatcher=dispatcher;
            view! { <div class="builder-row">
                <input type="text" prop:value=value data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string()
                    data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "pointset", "selectors", item_id, Some("category"))
                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                    on:change=move |event| update.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::PointSet(PointSetMutation::Selectors(mirabile_app::DraftListMutation::Update { item_id, value: mirabile_app::PointSelector::Category(event_target_value(&event)) }))))) />
                <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_MOVE.to_string()
                    data-mirabile-address=list_address(ControlId::RESOURCE_LIST_MOVE, "pointset", "selectors", item_id, Some("end")) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true"
                    on:click=move |_| move_dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::PointSet(PointSetMutation::Selectors(mirabile_app::DraftListMutation::Move { item_id, before: None })))))>"Move to end"</button>
                <button type="button" class="button danger" data-mirabile-control=ControlId::RESOURCE_LIST_REMOVE.to_string()
                    data-mirabile-address=list_address(ControlId::RESOURCE_LIST_REMOVE, "pointset", "selectors", item_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true"
                    on:click=move |_| remove.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::PointSet(PointSetMutation::Selectors(mirabile_app::DraftListMutation::Remove { item_id })))))>"Remove"</button>
            </div> }
        }).collect_view()}
        <div class="builder-row"><input type="text" placeholder="Category" on:input=move |event| category.set(event_target_value(&event)) />
            <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_INSERT.to_string()
                data-mirabile-address=ControlAddress::qualified(ControlId::RESOURCE_LIST_INSERT, [("kind", "pointset"), ("collection", "selectors")]).expect("builder address").to_string()
                data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled=move || (!category.get().trim().is_empty()).to_string()
                data-mirabile-disabled-reason=move || category.get().trim().is_empty().then_some("Enter a category name") disabled=move || category.get().trim().is_empty()
                on:click=move |_| {
                    let value=category.get();
                    dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(
                        ResourceMutation::PointSet(PointSetMutation::Selectors(
                            mirabile_app::DraftListMutation::Insert { after: last, value: mirabile_app::PointSelector::Category(value) }
                        ))
                    )));
                }>"Add category"</button></div>
    </div> }
}

#[component]
fn WheelRingBuilder(
    rows: Vec<mirabile_app::StableDraftItemReadModel<mirabile_app::RingSpec>>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    view! { <div class="nested-builder"><h4>"Stable ring rows"</h4>
        {rows.into_iter().map(|row| { let item_id=row.item_id; let slot_base=row.value.clone(); let role_base=row.value.clone(); let inner_base=row.value.clone(); let outer_base=row.value.clone(); let remove=dispatcher; let mover=dispatcher;
            view! { <div class="builder-row ring-row">
                <label>"Slot"<input type="text" prop:value=row.value.chart_slot.to_string() data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "wheeltemplate", "rings", item_id, Some("slot")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                    on:change=move |event| if let Ok(value)=mirabile_app::ChartSlotId::new(event_target_value(&event)) { let mut next=slot_base.clone(); next.chart_slot=value; dispatch_payload(dispatcher, ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label>
                <label>"Role"<select prop:value=format!("{:?}", row.value.point_role).to_lowercase() data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "wheeltemplate", "rings", item_id, Some("role")) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true"
                    on:change=move |event| { let mut next=role_base.clone(); next.point_role=match event_target_value(&event).as_str() { "transit" => mirabile_app::PointRole::Transit, "progressed" => mirabile_app::PointRole::Progressed, "comparison" => mirabile_app::PointRole::Comparison, _ => mirabile_app::PointRole::Primary }; dispatch_payload(dispatcher, ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Update { item_id, value: next }))); }><option value="primary">"Primary"</option><option value="transit">"Transit"</option><option value="progressed">"Progressed"</option><option value="comparison">"Comparison"</option></select></label>
                <label>"Inner"<input type="number" step="0.01" prop:value=row.value.geometry.inner_radius data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "wheeltemplate", "rings", item_id, Some("inner-radius")) data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { let mut next=inner_base.clone(); next.geometry.inner_radius=value; dispatch_payload(dispatcher, ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label>
                <label>"Outer"<input type="number" step="0.01" prop:value=row.value.geometry.outer_radius data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "wheeltemplate", "rings", item_id, Some("outer-radius")) data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { let mut next=outer_base.clone(); next.geometry.outer_radius=value; dispatch_payload(dispatcher, ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label>
                <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_MOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_MOVE, "wheeltemplate", "rings", item_id, Some("end")) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| mover.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Move { item_id, before: None })))))>"Move to end"</button>
                <button type="button" class="button danger" data-mirabile-control=ControlId::RESOURCE_LIST_REMOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_REMOVE, "wheeltemplate", "rings", item_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| remove.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Remove { item_id })))))>"Remove"</button>
            </div> }
        }).collect_view()}
        <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_INSERT.to_string() data-mirabile-address=ControlAddress::qualified(ControlId::RESOURCE_LIST_INSERT, [("kind", "wheeltemplate"), ("collection", "rings")]).expect("ring address").to_string() data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true"
            on:click=move |_| dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::WheelTemplate(WheelTemplateMutation::Rings(mirabile_app::DraftListMutation::Insert { after: None, value: mirabile_app::RingSpec { chart_slot: mirabile_app::ChartSlotId::new("primary").expect("slot"), point_role: mirabile_app::PointRole::Primary, geometry: mirabile_app::RingGeometry { inner_radius: 0.7, outer_radius: 0.9 } } })))))>"Add ring"</button>
    </div> }
}

#[component]
fn ViewSlotBuilder(
    rows: Vec<mirabile_app::StableDraftItemReadModel<mirabile_app::ChartSlot>>,
    objects: Vec<mirabile_app::StableDraftItemReadModel<mirabile_app::ViewObject>>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let first_slot = rows.first().map_or_else(
        || mirabile_app::ChartSlotId::new("primary").expect("slot"),
        |row| row.value.id.clone(),
    );
    view! { <div class="nested-builder"><h4>"Chart slots and View Objects"</h4>
        {rows.into_iter().map(|row| { let item_id=row.item_id; let id_base=row.value.clone(); let label_base=row.value.clone(); let required_base=row.value.clone(); let remove=dispatcher;
            view! { <div class="builder-row">
                <label>"ID"<input type="text" prop:value=row.value.id.to_string() data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "chart-slots", item_id, Some("id")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(id)=mirabile_app::ChartSlotId::new(event_target_value(&event)) { let mut slot=id_base.clone(); slot.id=id; dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::RenameChartSlot { item_id, slot })); } /></label>
                <label>"Label"<input type="text" prop:value=row.value.label data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "chart-slots", item_id, Some("label")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| { let mut value=label_base.clone(); value.label=event_target_value(&event); dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::ChartSlots(mirabile_app::DraftListMutation::Update { item_id, value }))); } /></label>
                <label class="checkbox-field"><input type="checkbox" prop:checked=row.value.required data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "chart-slots", item_id, Some("required")) data-mirabile-kind=ControlKind::Checkbox.as_str() data-mirabile-enabled="true" on:change=move |event| { let mut value=required_base.clone(); value.required=event_target_checked(&event); dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::ChartSlots(mirabile_app::DraftListMutation::Update { item_id, value }))); } />"Required"</label>
                <button type="button" class="button danger" data-mirabile-control=ControlId::RESOURCE_LIST_REMOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_REMOVE, "viewdocument", "chart-slots", item_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| remove.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::ViewDocument(ViewDocumentMutation::ChartSlots(mirabile_app::DraftListMutation::Remove { item_id })))))>"Remove slot"</button>
            </div> }
        }).collect_view()}
        <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_INSERT.to_string() data-mirabile-address=ControlAddress::qualified(ControlId::RESOURCE_LIST_INSERT, [("kind", "viewdocument"), ("collection", "chart-slots")]).expect("slot address").to_string() data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::ViewDocument(ViewDocumentMutation::ChartSlots(mirabile_app::DraftListMutation::Insert { after: None, value: mirabile_app::ChartSlot { id: mirabile_app::ChartSlotId::new("new-slot").expect("slot"), label: "New slot".into(), required: false } })))))>"Add slot"</button>
        {objects.into_iter().map(|row| view! { <ViewObjectEditor row dispatcher /> }).collect_view()}
        <label>"Add View Object"<select data-mirabile-control=ControlId::RESOURCE_LIST_INSERT.to_string() data-mirabile-address=ControlAddress::qualified(ControlId::RESOURCE_LIST_INSERT, [("kind", "viewdocument"), ("collection", "objects")]).expect("object address").to_string() data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| {
            let frame=mirabile_app::ObjectFrame { x: 0.0, y: 0.0, width: 400.0, height: 400.0 };
            let value=match event_target_value(&event).as_str() {
                "aspect-grid" => mirabile_app::ViewObject::AspectGrid(mirabile_app::GridObject { lhs: first_slot.clone(), rhs: None, frame }),
                "chart-details" => mirabile_app::ViewObject::ChartDetails(mirabile_app::ChartDetailsObject { slot: first_slot.clone(), frame }),
                "point-table" => mirabile_app::ViewObject::PointTable(mirabile_app::PointTableObject { slot: first_slot.clone(), points: Vec::new(), frame }),
                "aspect-table" => mirabile_app::ViewObject::AspectTable(mirabile_app::AspectTableObject { slot: first_slot.clone(), frame }),
                "text" => mirabile_app::ViewObject::Text(mirabile_app::TextObject { text: "Text".into(), frame }),
                _ => mirabile_app::ViewObject::Wheel(mirabile_app::WheelObject { slot: first_slot.clone(), frame }),
            };
            dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Insert { after: None, value })))));
        }><option value="wheel">"Wheel"</option><option value="aspect-grid">"Aspect grid"</option><option value="chart-details">"Chart details"</option><option value="point-table">"Point table"</option><option value="aspect-table">"Aspect table"</option><option value="text">"Text"</option></select></label>
    </div> }
}

#[component]
fn ViewObjectEditor(
    row: mirabile_app::StableDraftItemReadModel<mirabile_app::ViewObject>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let item_id = row.item_id;
    let frame = view_object_frame(&row.value).clone();
    let variant_base = row.value.clone();
    view! { <div class="builder-row object-row"><strong>{view_object_kind(&row.value)}</strong>
        <label>"Variant"<select prop:value=view_object_kind_key(&row.value) data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some("variant")) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| {
            let next=view_object_with_kind(&event_target_value(&event), &variant_base);
            dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: next })));
        }><option value="wheel">"Wheel"</option><option value="aspect-grid">"Aspect grid"</option><option value="chart-details">"Chart details"</option><option value="point-table">"Point table"</option><option value="aspect-table">"Aspect table"</option><option value="text">"Text"</option></select></label>
        {[("x", frame.x), ("y", frame.y), ("width", frame.width), ("height", frame.height)].into_iter().map(|(field, current)| { let base=row.value.clone(); view! { <label>{field}<input type="number" step="1" prop:value=current data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some(field)) data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(value)=event_target_value(&event).parse() { let mut object=base.clone(); let frame=view_object_frame_mut(&mut object); match field { "x" => frame.x=value, "y" => frame.y=value, "width" => frame.width=value, _ => frame.height=value } dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: object }))); } /></label> } }).collect_view()}
        <ViewObjectFields value=row.value.clone() item_id dispatcher />
        <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_LIST_MOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_MOVE, "viewdocument", "objects", item_id, Some("end")) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Move { item_id, before: None })))>"Move to end"</button>
        <button type="button" class="button danger" data-mirabile-control=ControlId::RESOURCE_LIST_REMOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_REMOVE, "viewdocument", "objects", item_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Remove { item_id })))))>"Remove object"</button>
    </div> }
}

#[component]
fn ViewObjectFields(
    value: mirabile_app::ViewObject,
    item_id: mirabile_app::DraftItemId,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let slot_value = value.clone();
    let slot = match &value {
        mirabile_app::ViewObject::Wheel(v) => Some(v.slot.clone()),
        mirabile_app::ViewObject::ChartDetails(v) => Some(v.slot.clone()),
        mirabile_app::ViewObject::PointTable(v) => Some(v.slot.clone()),
        mirabile_app::ViewObject::AspectTable(v) => Some(v.slot.clone()),
        _ => None,
    };
    view! { <div class="object-fields">
        {slot.map(|slot| view! { <label>"Chart slot"<input type="text" prop:value=slot.to_string() data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some("slot")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(slot)=mirabile_app::ChartSlotId::new(event_target_value(&event)) { let mut next=slot_value.clone(); set_view_object_slot(&mut next, slot); dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label> })}
        {if let mirabile_app::ViewObject::AspectGrid(grid)=value.clone() { let lhs_base=value.clone(); let rhs_base=value.clone(); view! { <><label>"LHS slot"<input type="text" prop:value=grid.lhs.to_string() data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some("lhs")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(slot)=mirabile_app::ChartSlotId::new(event_target_value(&event)) { let mut next=lhs_base.clone(); if let mirabile_app::ViewObject::AspectGrid(v)=&mut next { v.lhs=slot; } dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label><label>"RHS slot (blank for same chart)"<input type="text" prop:value=grid.rhs.map(|v| v.to_string()).unwrap_or_default() data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some("rhs")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| { let raw=event_target_value(&event); let rhs=if raw.trim().is_empty() { Some(None) } else { mirabile_app::ChartSlotId::new(raw).ok().map(Some) }; if let Some(rhs)=rhs { let mut next=rhs_base.clone(); if let mirabile_app::ViewObject::AspectGrid(v)=&mut next { v.rhs=rhs; } dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } } /></label></> }.into_any() } else if let mirabile_app::ViewObject::PointTable(table)=value.clone() { let base=value.clone(); let points_text=table.points.iter().map(ToString::to_string).collect::<Vec<_>>().join(","); view! { <label>"Points (comma separated IDs)"<input type="text" prop:value=points_text data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some("points")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| { let points=event_target_value(&event).split(',').filter_map(|raw| mirabile_app::PointId::new(raw.trim()).ok()).collect(); let mut next=base.clone(); if let mirabile_app::ViewObject::PointTable(v)=&mut next { v.points=points; } dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label> }.into_any() } else if let mirabile_app::ViewObject::Text(text)=value.clone() { let base=value.clone(); view! { <label>"Text"<textarea prop:value=text.text data-mirabile-control=ControlId::RESOURCE_LIST_FIELD.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_LIST_FIELD, "viewdocument", "objects", item_id, Some("text")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| { let mut next=base.clone(); if let mirabile_app::ViewObject::Text(v)=&mut next { v.text=event_target_value(&event); } dispatch_payload(dispatcher, ResourceMutation::ViewDocument(ViewDocumentMutation::Objects(mirabile_app::DraftListMutation::Update { item_id, value: next }))); } /></label> }.into_any() } else { view! { <small>{format!("Slots: {}", view_object_slots(&value))}</small> }.into_any() }}
    </div> }
}

fn view_object_kind_key(value: &mirabile_app::ViewObject) -> &'static str {
    match value {
        mirabile_app::ViewObject::Wheel(_) => "wheel",
        mirabile_app::ViewObject::AspectGrid(_) => "aspect-grid",
        mirabile_app::ViewObject::ChartDetails(_) => "chart-details",
        mirabile_app::ViewObject::PointTable(_) => "point-table",
        mirabile_app::ViewObject::AspectTable(_) => "aspect-table",
        mirabile_app::ViewObject::Text(_) => "text",
    }
}
fn view_object_with_kind(kind: &str, old: &mirabile_app::ViewObject) -> mirabile_app::ViewObject {
    let frame = view_object_frame(old).clone();
    let slot = match old {
        mirabile_app::ViewObject::Wheel(v) => v.slot.clone(),
        mirabile_app::ViewObject::AspectGrid(v) => v.lhs.clone(),
        mirabile_app::ViewObject::ChartDetails(v) => v.slot.clone(),
        mirabile_app::ViewObject::PointTable(v) => v.slot.clone(),
        mirabile_app::ViewObject::AspectTable(v) => v.slot.clone(),
        mirabile_app::ViewObject::Text(_) => {
            mirabile_app::ChartSlotId::new("primary").expect("slot")
        }
    };
    match kind {
        "aspect-grid" => mirabile_app::ViewObject::AspectGrid(mirabile_app::GridObject {
            lhs: slot,
            rhs: None,
            frame,
        }),
        "chart-details" => {
            mirabile_app::ViewObject::ChartDetails(mirabile_app::ChartDetailsObject { slot, frame })
        }
        "point-table" => mirabile_app::ViewObject::PointTable(mirabile_app::PointTableObject {
            slot,
            points: Vec::new(),
            frame,
        }),
        "aspect-table" => {
            mirabile_app::ViewObject::AspectTable(mirabile_app::AspectTableObject { slot, frame })
        }
        "text" => mirabile_app::ViewObject::Text(mirabile_app::TextObject {
            text: String::new(),
            frame,
        }),
        _ => mirabile_app::ViewObject::Wheel(mirabile_app::WheelObject { slot, frame }),
    }
}
fn set_view_object_slot(value: &mut mirabile_app::ViewObject, slot: mirabile_app::ChartSlotId) {
    match value {
        mirabile_app::ViewObject::Wheel(v) => v.slot = slot,
        mirabile_app::ViewObject::ChartDetails(v) => v.slot = slot,
        mirabile_app::ViewObject::PointTable(v) => v.slot = slot,
        mirabile_app::ViewObject::AspectTable(v) => v.slot = slot,
        _ => {}
    }
}

fn view_object_kind(value: &mirabile_app::ViewObject) -> &'static str {
    match value {
        mirabile_app::ViewObject::Wheel(_) => "Wheel",
        mirabile_app::ViewObject::AspectGrid(_) => "Aspect grid",
        mirabile_app::ViewObject::ChartDetails(_) => "Chart details",
        mirabile_app::ViewObject::PointTable(_) => "Point table",
        mirabile_app::ViewObject::AspectTable(_) => "Aspect table",
        mirabile_app::ViewObject::Text(_) => "Text",
    }
}
fn view_object_frame(value: &mirabile_app::ViewObject) -> &mirabile_app::ObjectFrame {
    match value {
        mirabile_app::ViewObject::Wheel(value) => &value.frame,
        mirabile_app::ViewObject::AspectGrid(value) => &value.frame,
        mirabile_app::ViewObject::ChartDetails(value) => &value.frame,
        mirabile_app::ViewObject::PointTable(value) => &value.frame,
        mirabile_app::ViewObject::AspectTable(value) => &value.frame,
        mirabile_app::ViewObject::Text(value) => &value.frame,
    }
}
fn view_object_frame_mut(value: &mut mirabile_app::ViewObject) -> &mut mirabile_app::ObjectFrame {
    match value {
        mirabile_app::ViewObject::Wheel(value) => &mut value.frame,
        mirabile_app::ViewObject::AspectGrid(value) => &mut value.frame,
        mirabile_app::ViewObject::ChartDetails(value) => &mut value.frame,
        mirabile_app::ViewObject::PointTable(value) => &mut value.frame,
        mirabile_app::ViewObject::AspectTable(value) => &mut value.frame,
        mirabile_app::ViewObject::Text(value) => &mut value.frame,
    }
}
fn view_object_slots(value: &mirabile_app::ViewObject) -> String {
    match value {
        mirabile_app::ViewObject::Wheel(value) => value.slot.to_string(),
        mirabile_app::ViewObject::AspectGrid(value) => format!(
            "{} / {}",
            value.lhs,
            value
                .rhs
                .as_ref()
                .map_or_else(|| "none".into(), ToString::to_string)
        ),
        mirabile_app::ViewObject::ChartDetails(value) => value.slot.to_string(),
        mirabile_app::ViewObject::PointTable(value) => {
            format!("{}; {} point(s)", value.slot, value.points.len())
        }
        mirabile_app::ViewObject::AspectTable(value) => value.slot.to_string(),
        mirabile_app::ViewObject::Text(_) => "none".into(),
    }
}

#[component]
fn QueryTreeBuilder(
    tree: mirabile_app::QueryNodeDraftReadModel,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let root_id = tree.node_id;
    let mut nodes = Vec::new();
    flatten_query_nodes(tree, 0, &mut nodes);
    let groups = nodes
        .iter()
        .filter(|(node, _)| {
            matches!(
                node.expression,
                mirabile_app::QueryExpr::And(_) | mirabile_app::QueryExpr::Or(_)
            )
        })
        .map(|(node, depth)| {
            (
                node.node_id,
                format!(
                    "{}{}",
                    "  ".repeat(*depth),
                    query_expression_kind(&node.expression)
                ),
            )
        })
        .collect::<Vec<_>>();
    view! { <div class="query-tree">{nodes.into_iter().map(|(node, depth)| { let root=node.node_id == root_id; view! { <QueryNodeRow node root depth groups=groups.clone() dispatcher /> } }).collect_view()}</div> }
}

#[component]
fn QueryNodeRow(
    node: mirabile_app::QueryNodeDraftReadModel,
    root: bool,
    depth: usize,
    groups: Vec<(mirabile_app::DraftItemId, String)>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let node_id = node.node_id;
    let kind = match &node.expression {
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::InSign { .. }) => "in-sign",
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect { .. }) => "aspect",
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Longitude { .. }) => {
            "longitude"
        }
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::ChartField { .. }) => {
            "chart-field"
        }
        mirabile_app::QueryExpr::And(_) => "and",
        mirabile_app::QueryExpr::Or(_) => "or",
        mirabile_app::QueryExpr::Not(_) => "not",
    };
    view! { <div class="query-node" style=format!("margin-left: {}rem", depth) data-query-node=node_id.to_string()>
        <select prop:value=kind data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("type")) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| { let point=mirabile_app::PointId::new("sun").expect("point"); let expression=match event_target_value(&event).as_str() { "and" => mirabile_app::QueryExpr::And(vec![default_query_predicate()]), "or" => mirabile_app::QueryExpr::Or(vec![default_query_predicate()]), "not" => mirabile_app::QueryExpr::Not(Box::new(default_query_predicate())), "aspect" => mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect { lhs: point.clone(), rhs: mirabile_app::PointId::new("moon").expect("point"), aspect: mirabile_app::AspectId::new("conjunction").expect("aspect"), orb_override: None }), "longitude" => mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Longitude { point, comparison: mirabile_app::NumericComparison::Equal, value: mirabile_app::Angle::from_degrees(0.0).expect("angle") }), "chart-field" => mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::ChartField { field: "title".into(), comparison: mirabile_app::TextComparison::Contains, value: "chart".into() }), _ => default_query_predicate() }; dispatch_payload(dispatcher, ResourceMutation::QueryDefinition(QueryDefinitionMutation::Tree(mirabile_app::QueryTreeMutation::Replace { node_id, expression }))); }>
            <option value="in-sign">"In sign"</option><option value="aspect">"Aspect"</option><option value="longitude">"Longitude"</option><option value="chart-field">"Chart field"</option><option value="and">"And"</option><option value="or">"Or"</option><option value="not">"Not"</option>
        </select>
        <QueryPredicateFields expression=node.expression.clone() node_id dispatcher />
        {matches!(node.expression, mirabile_app::QueryExpr::And(_) | mirabile_app::QueryExpr::Or(_)).then(|| view! { <button type="button" class="button secondary" data-mirabile-control=ControlId::RESOURCE_QUERY_INSERT.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_INSERT, "querydefinition", "tree", node_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatch_payload(dispatcher, ResourceMutation::QueryDefinition(QueryDefinitionMutation::Tree(mirabile_app::QueryTreeMutation::InsertChild { parent_id: node_id, after: None, expression: default_query_predicate() })))>"Add child"</button> })}
        {(!root).then(|| { let excluded=query_node_ids(&node); let move_groups=groups.clone(); view! { <label>"Move to group"<select data-mirabile-control=ControlId::RESOURCE_QUERY_MOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_MOVE, "querydefinition", "tree", node_id, None) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| { let selected=event_target_value(&event); if let Some((new_parent_id, _))=move_groups.iter().find(|(id, _)| id.to_string() == selected) { dispatch_payload(dispatcher, ResourceMutation::QueryDefinition(QueryDefinitionMutation::Tree(mirabile_app::QueryTreeMutation::Move { node_id, new_parent_id: *new_parent_id, before: None }))); } }><option value="">"Choose group"</option>{groups.into_iter().filter(|(id, _)| !excluded.contains(id)).map(|(id, label)| view! { <option value=id.to_string()>{label}</option> }).collect_view()}</select></label> } })}
        {(!root).then(|| view! { <button type="button" class="button danger" data-mirabile-control=ControlId::RESOURCE_QUERY_REMOVE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_REMOVE, "querydefinition", "tree", node_id, None) data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true" on:click=move |_| dispatch_payload(dispatcher, ResourceMutation::QueryDefinition(QueryDefinitionMutation::Tree(mirabile_app::QueryTreeMutation::Remove { node_id })))>"Remove node"</button> })}
    </div> }
}

fn query_expression_kind(expression: &mirabile_app::QueryExpr) -> &'static str {
    match expression {
        mirabile_app::QueryExpr::Predicate(_) => "Predicate",
        mirabile_app::QueryExpr::And(_) => "And",
        mirabile_app::QueryExpr::Or(_) => "Or",
        mirabile_app::QueryExpr::Not(_) => "Not",
    }
}

fn query_node_ids(node: &mirabile_app::QueryNodeDraftReadModel) -> Vec<mirabile_app::DraftItemId> {
    let mut ids = vec![node.node_id];
    for child in &node.children {
        ids.extend(query_node_ids(child));
    }
    ids
}

#[component]
fn QueryPredicateFields(
    expression: mirabile_app::QueryExpr,
    node_id: mirabile_app::DraftItemId,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let replace = move |expression| {
        dispatch_payload(
            dispatcher,
            ResourceMutation::QueryDefinition(QueryDefinitionMutation::Tree(
                mirabile_app::QueryTreeMutation::Replace {
                    node_id,
                    expression,
                },
            )),
        );
    };
    match expression {
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::InSign {
            point,
            sign_index,
        }) => {
            let point_base = point.clone();
            view! { <div class="query-fields"><label>"Point"<input type="text" prop:value=point.to_string() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("point")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(point)=mirabile_app::PointId::new(event_target_value(&event)) { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::InSign { point, sign_index })); } /></label><label>"Sign"<select prop:value=sign_index.to_string() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("sign")) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(sign_index)=event_target_value(&event).parse() { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::InSign { point: point_base.clone(), sign_index })); }>{(0_u8..12).map(|index| view! { <option value=index.to_string()>{index+1}</option> }).collect_view()}</select></label></div> }.into_any()
        }
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect {
            lhs,
            rhs,
            aspect,
            orb_override,
        }) => {
            let lhs_for_rhs = lhs.clone();
            let rhs_for_lhs = rhs.clone();
            let aspect_for_lhs = aspect.clone();
            let aspect_for_rhs = aspect.clone();
            let lhs_for_aspect = lhs.clone();
            let rhs_for_aspect = rhs.clone();
            let lhs_for_orb = lhs.clone();
            let rhs_for_orb = rhs.clone();
            let aspect_for_orb = aspect.clone();
            let orb_lhs = orb_override;
            let orb_rhs = orb_override;
            let orb_aspect = orb_override;
            view! { <div class="query-fields"><label>"LHS point"<input type="text" prop:value=lhs.to_string() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("lhs")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(lhs)=mirabile_app::PointId::new(event_target_value(&event)) { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect { lhs, rhs:rhs_for_lhs.clone(), aspect:aspect_for_lhs.clone(), orb_override:orb_lhs })); } /></label><label>"RHS point"<input type="text" prop:value=rhs.to_string() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("rhs")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(rhs)=mirabile_app::PointId::new(event_target_value(&event)) { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect { lhs:lhs_for_rhs.clone(), rhs, aspect:aspect_for_rhs.clone(), orb_override:orb_rhs })); } /></label><label>"Aspect ID"<input type="text" prop:value=aspect.to_string() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("aspect")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(aspect)=mirabile_app::AspectId::new(event_target_value(&event)) { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect { lhs:lhs_for_aspect.clone(), rhs:rhs_for_aspect.clone(), aspect, orb_override:orb_aspect })); } /></label><label>"Orb override (blank for default)"<input type="number" min="0" max="180" step="0.1" prop:value=orb_override.map(|v| v.degrees().to_string()).unwrap_or_default() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("orb")) data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| { let raw=event_target_value(&event); let orb_override=if raw.trim().is_empty() { Some(None) } else { raw.parse().ok().and_then(|degrees| mirabile_app::Angle::from_degrees(degrees).ok()).map(Some) }; if let Some(orb_override)=orb_override { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Aspect { lhs:lhs_for_orb.clone(), rhs:rhs_for_orb.clone(), aspect:aspect_for_orb.clone(), orb_override })); } } /></label></div> }.into_any()
        }
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Longitude {
            point,
            comparison,
            value,
        }) => {
            let point_for_value = point.clone();
            let point_for_comparison = point.clone();
            view! { <div class="query-fields"><label>"Point"<input type="text" prop:value=point.to_string() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("point")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(point)=mirabile_app::PointId::new(event_target_value(&event)) { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Longitude { point, comparison, value })); } /></label><label>"Comparison"<select prop:value=numeric_comparison_key(comparison) data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("comparison")) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Longitude { point:point_for_comparison.clone(), comparison:parse_numeric_comparison(&event_target_value(&event)), value }))><option value="lt">"Less than"</option><option value="le">"Less than or equal"</option><option value="eq">"Equal"</option><option value="ge">"Greater than or equal"</option><option value="gt">"Greater than"</option></select></label><label>"Longitude"<input type="number" min="0" max="359.999" step="0.1" prop:value=value.degrees() data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("value")) data-mirabile-kind=ControlKind::Number.as_str() data-mirabile-enabled="true" on:change=move |event| if let Ok(degrees)=event_target_value(&event).parse() && let Ok(value)=mirabile_app::Angle::from_degrees(degrees) { replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::Longitude { point:point_for_value.clone(), comparison, value })); } /></label></div> }.into_any()
        }
        mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::ChartField {
            field,
            comparison,
            value,
        }) => {
            let field_for_value = field.clone();
            let field_for_comparison = field.clone();
            let value_for_field = value.clone();
            let value_for_comparison = value.clone();
            view! { <div class="query-fields"><label>"Field"<input type="text" prop:value=field data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("field")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::ChartField { field:event_target_value(&event), comparison, value:value_for_field.clone() })) /></label><label>"Comparison"<select prop:value=text_comparison_key(comparison) data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("comparison")) data-mirabile-kind=ControlKind::Select.as_str() data-mirabile-enabled="true" on:change=move |event| replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::ChartField { field:field_for_comparison.clone(), comparison:parse_text_comparison(&event_target_value(&event)), value:value_for_comparison.clone() }))><option value="eq">"Equal"</option><option value="contains">"Contains"</option><option value="starts-with">"Starts with"</option></select></label><label>"Value"<input type="text" prop:value=value data-mirabile-control=ControlId::RESOURCE_QUERY_NODE.to_string() data-mirabile-address=list_address(ControlId::RESOURCE_QUERY_NODE, "querydefinition", "tree", node_id, Some("value")) data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true" on:change=move |event| replace(mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::ChartField { field:field_for_value.clone(), comparison, value:event_target_value(&event) })) /></label></div> }.into_any()
        }
        _ => view! { <span></span> }.into_any(),
    }
}

fn numeric_comparison_key(value: mirabile_app::NumericComparison) -> &'static str {
    match value {
        mirabile_app::NumericComparison::LessThan => "lt",
        mirabile_app::NumericComparison::LessThanOrEqual => "le",
        mirabile_app::NumericComparison::Equal => "eq",
        mirabile_app::NumericComparison::GreaterThanOrEqual => "ge",
        mirabile_app::NumericComparison::GreaterThan => "gt",
    }
}
fn parse_numeric_comparison(value: &str) -> mirabile_app::NumericComparison {
    match value {
        "lt" => mirabile_app::NumericComparison::LessThan,
        "le" => mirabile_app::NumericComparison::LessThanOrEqual,
        "ge" => mirabile_app::NumericComparison::GreaterThanOrEqual,
        "gt" => mirabile_app::NumericComparison::GreaterThan,
        _ => mirabile_app::NumericComparison::Equal,
    }
}
fn text_comparison_key(value: mirabile_app::TextComparison) -> &'static str {
    match value {
        mirabile_app::TextComparison::Equal => "eq",
        mirabile_app::TextComparison::Contains => "contains",
        mirabile_app::TextComparison::StartsWith => "starts-with",
    }
}
fn parse_text_comparison(value: &str) -> mirabile_app::TextComparison {
    match value {
        "contains" => mirabile_app::TextComparison::Contains,
        "starts-with" => mirabile_app::TextComparison::StartsWith,
        _ => mirabile_app::TextComparison::Equal,
    }
}

fn flatten_query_nodes(
    node: mirabile_app::QueryNodeDraftReadModel,
    depth: usize,
    output: &mut Vec<(mirabile_app::QueryNodeDraftReadModel, usize)>,
) {
    let children = node.children.clone();
    output.push((node, depth));
    for child in children {
        flatten_query_nodes(child, depth + 1, output);
    }
}

fn default_query_predicate() -> mirabile_app::QueryExpr {
    mirabile_app::QueryExpr::Predicate(mirabile_app::Predicate::InSign {
        point: mirabile_app::PointId::new("sun").expect("point"),
        sign_index: 0,
    })
}

fn recipe_reference(
    definition: &mirabile_app::ChartDefinition,
) -> Option<mirabile_app::ResourceId> {
    match &definition.source {
        mirabile_app::ChartSource::Radix { record } => Some(*record),
        mirabile_app::ChartSource::Derived {
            recipe:
                mirabile_app::DerivationSpec::Harmonic { radix, .. }
                | mirabile_app::DerivationSpec::Relocation { radix, .. },
        } => Some(*radix),
        mirabile_app::ChartSource::Derived {
            recipe: mirabile_app::DerivationSpec::Composite { charts, .. },
        } => charts.first().copied(),
        mirabile_app::ChartSource::Derived {
            recipe: mirabile_app::DerivationSpec::Transit { .. },
        } => None,
    }
}

fn default_recipe_location() -> mirabile_app::LocationAssertion {
    mirabile_app::LocationAssertion {
        display_name: "Location".into(),
        country_region: None,
        latitude: mirabile_app::Latitude::from_degrees(0.0).expect("latitude"),
        longitude: mirabile_app::Longitude::from_degrees(0.0).expect("longitude"),
        atlas_provenance: None,
    }
}

fn default_transit_recipe() -> mirabile_app::DerivationSpec {
    mirabile_app::DerivationSpec::Transit {
        at: mirabile_app::TemporalAssertion {
            civil_datetime: mirabile_app::CivilDateTime {
                date: mirabile_app::CivilDate::new(2000, 1, 1).expect("date"),
                time: mirabile_app::CivilTime::new(12, 0, 0).expect("time"),
            },
            calendar: mirabile_app::CalendarSpec::ProlepticGregorian,
            zone: mirabile_app::TimeZoneAssertion::UniversalTime,
            disambiguation: None,
        },
        location: default_recipe_location(),
    }
}

fn list_address(
    control: ControlId,
    kind: &str,
    collection: &str,
    item_id: mirabile_app::DraftItemId,
    field: Option<&str>,
) -> String {
    let mut qualifiers = vec![
        ("kind", kind.to_owned()),
        ("collection", collection.to_owned()),
        ("draft-item", item_id.to_string()),
    ];
    if let Some(field) = field {
        qualifiers.push(("field", field.to_owned()));
    }
    ControlAddress::qualified(control, qualifiers)
        .expect("list address")
        .to_string()
}

fn dispatch_payload(dispatcher: WorkbenchCoordinator, mutation: ResourceMutation) {
    dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(mutation)));
}

#[component]
fn RepositoryLaboratory(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    view! {
        {move || model.get().repository.heads.into_iter().map(|head| {
            let select = dispatcher;
            view! { <div class="cockpit-row"><span>{format!("{:?} · r{} · {:?}", head.kind, head.revision, head.state)}</span>
                <button type="button" class="button secondary"
                    data-mirabile-control=ControlId::REPOSITORY_SELECT.to_string()
                    data-mirabile-address=ControlAddress::qualified(ControlId::REPOSITORY_SELECT, [("resource", head.resource_id.to_string())]).expect("repository address").to_string()
                    data-mirabile-kind=ControlKind::Action.as_str() data-mirabile-enabled="true"
                    on:click=move |_| select.dispatch(AppIntent::SelectRepositoryResource { resource_id: head.resource_id })>"History"</button></div> }
        }).collect_view()}
        {move || model.get().repository.selected_history.into_iter().map(|revision| view! {
            <div class="cockpit-row revision-line"><span>{format!("r{} · {:?}", revision.revision, revision.state)}</span></div>
        }).collect_view()}
        {move || {
            let deletion=model.get().repository.deletion;
            let first_dispatcher=dispatcher;
            let second_dispatcher=dispatcher;
            let (resource_id, revision, enabled, reason, confirmed)=deletion.map_or(
                (None, None, false, Some("Select a present resource before deletion".to_owned()), false),
                |deletion| (Some(deletion.resource_id), Some(deletion.expected_revision), deletion.enabled, deletion.disabled_reason, deletion.first_confirmation_complete),
            );
            let confirm_enabled=enabled && confirmed;
            view! { <div class="delete-actions">
                <button type="button" class="button secondary" disabled=!enabled
                    data-mirabile-control=ControlId::REPOSITORY_DELETE.to_string()
                    data-mirabile-address=ControlAddress::new(ControlId::REPOSITORY_DELETE).to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=enabled.to_string() data-mirabile-disabled-reason=reason.clone()
                    on:click=move |_| if let (Some(resource_id), Some(expected_revision))=(resource_id, revision) { first_dispatcher.dispatch(AppIntent::BeginDeleteResource { resource_id, expected_revision }); }>"Delete resource — confirmation 1"</button>
                <button type="button" class="button danger" disabled=!confirm_enabled
                    data-mirabile-control=ControlId::REPOSITORY_CONFIRM_DELETE.to_string()
                    data-mirabile-address=ControlAddress::new(ControlId::REPOSITORY_CONFIRM_DELETE).to_string()
                    data-mirabile-kind=ControlKind::Action.as_str()
                    data-mirabile-enabled=confirm_enabled.to_string()
                    data-mirabile-disabled-reason=if confirm_enabled { None } else if !enabled { reason } else { Some("First deletion confirmation is required".to_owned()) }
                    on:click=move |_| if let (Some(resource_id), Some(expected_revision))=(resource_id, revision) { second_dispatcher.dispatch(AppIntent::ConfirmDeleteResource { resource_id, expected_revision }); }>"Confirm deletion — confirmation 2"</button>
            </div> }
        }}
    }
}

fn dispatch_new(dispatcher: WorkbenchCoordinator, kind: ResourceDraftKind) {
    match kind {
        ResourceDraftKind::ChartDefinition => {
            dispatcher.dispatch(AppIntent::BeginResourceCreate { kind });
        }
        ResourceDraftKind::AspectSet => dispatcher.dispatch(AppIntent::BeginNewAspectSet),
        ResourceDraftKind::WorkspaceDocument => dispatcher.dispatch(AppIntent::NewWorkspace),
        _ => dispatcher.dispatch(AppIntent::BeginResourceCreate { kind }),
    }
}

fn dispatch_edit(
    dispatcher: WorkbenchCoordinator,
    kind: ResourceDraftKind,
    resource_id: mirabile_app::ResourceId,
) {
    match kind {
        ResourceDraftKind::AspectSet => {
            dispatcher.dispatch(AppIntent::BeginAspectSetEdit { resource_id });
        }
        ResourceDraftKind::WorkspaceDocument => {
            dispatcher.dispatch(AppIntent::OpenWorkspace { resource_id });
        }
        _ => dispatcher.dispatch(AppIntent::BeginResourceEdit { resource_id }),
    }
}

fn dispatch_metadata(
    dispatcher: WorkbenchCoordinator,
    kind: ResourceDraftKind,
    mutation: ResourceMetadataMutation,
) {
    let mutation = match kind {
        ResourceDraftKind::ChartRecord => {
            ResourceMutation::ChartRecord(ChartRecordMutation::Metadata(mutation))
        }
        ResourceDraftKind::ChartDefinition => {
            ResourceMutation::ChartDefinition(ChartDefinitionMutation::Metadata(mutation))
        }
        ResourceDraftKind::PointSet => {
            ResourceMutation::PointSet(PointSetMutation::Metadata(mutation))
        }
        ResourceDraftKind::AspectSet => {
            ResourceMutation::AspectSet(AspectSetMutation::Metadata(mutation))
        }
        ResourceDraftKind::AnalysisProfile => {
            ResourceMutation::AnalysisProfile(AnalysisProfileMutation::Metadata(mutation))
        }
        ResourceDraftKind::WheelTemplate => {
            ResourceMutation::WheelTemplate(WheelTemplateMutation::Metadata(mutation))
        }
        ResourceDraftKind::ViewDocument => {
            ResourceMutation::ViewDocument(ViewDocumentMutation::Metadata(mutation))
        }
        ResourceDraftKind::Theme => ResourceMutation::Theme(ThemeMutation::Metadata(mutation)),
        ResourceDraftKind::QueryDefinition => {
            ResourceMutation::QueryDefinition(QueryDefinitionMutation::Metadata(mutation))
        }
        ResourceDraftKind::WorkspaceDocument => {
            ResourceMutation::WorkspaceDocument(WorkspaceDocumentMutation::Metadata(mutation))
        }
    };
    dispatcher.dispatch(AppIntent::ApplyResourceMutation(Box::new(mutation)));
}

fn new_availability(kind: ResourceDraftKind) -> (bool, Option<&'static str>) {
    match kind {
        ResourceDraftKind::ChartRecord => (
            false,
            Some("ChartRecord creation is atomic with ChartDefinition; use New Chart"),
        ),
        _ => (true, None),
    }
}

fn edit_availability(kind: ResourceDraftKind) -> (bool, Option<&'static str>) {
    match kind {
        ResourceDraftKind::ChartRecord => (
            false,
            Some("ChartRecord facts are edited through an open composite chart"),
        ),
        _ => (true, None),
    }
}

fn resource_address(
    control: ControlId,
    kind: ResourceDraftKind,
    resource: Option<mirabile_app::ResourceId>,
) -> String {
    let mut qualifiers = vec![("kind", format!("{kind:?}").to_lowercase())];
    if let Some(resource) = resource {
        qualifiers.push(("resource", resource.to_string()));
    }
    ControlAddress::qualified(control, qualifiers)
        .expect("resource address")
        .to_string()
}

fn qualified_resource_address(
    control: ControlId,
    kind: ResourceDraftKind,
    qualifier: &'static str,
    value: &'static str,
) -> String {
    ControlAddress::qualified(
        control,
        [
            ("kind", format!("{kind:?}").to_lowercase()),
            (qualifier, value.to_owned()),
        ],
    )
    .expect("qualified resource address")
    .to_string()
}

fn payload_summary(value: &mirabile_app::ResourceDraftValueReadModel) -> String {
    use mirabile_app::ResourceDraftValueReadModel as Value;
    match value {
        Value::PointSet(value) => format!("{} selector(s) · Persisted", value.points.len()),
        Value::AnalysisProfile(_) => "Analysis switches and limit · Persisted".into(),
        Value::WheelTemplate(value) => {
            format!("{} ring(s) and geometry · Persisted", value.rings.len())
        }
        Value::ViewDocument(value) => format!(
            "{} view object(s), including dormant objects · Persisted",
            value.objects.len()
        ),
        Value::Theme(_) => "Five canonical colors · Persisted".into(),
        Value::QueryDefinition(_) => "Typed Query AST · Persisted; execution deferred".into(),
        Value::ChartRecord(_) => "Chart facts · Live through composite editor".into(),
        Value::ChartDefinition(_) => {
            "Calculation parameters · Live through composite editor".into()
        }
        Value::AspectSet(value) => format!("{} aspect row(s) · Live", value.aspects.len()),
        Value::WorkspaceDocument(_) => "Workspace composition · Live through session".into(),
    }
}
