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
                                    data-mirabile-address=resource_address(ControlId::RESOURCE_TITLE, kind, resource_id)
                                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                                    on:change=move |event| dispatch_metadata(title_dispatcher, kind, ResourceMetadataMutation::SetTitle(event_target_value(&event))) /></label>
                                <label>"Description"<textarea prop:value=draft.description.unwrap_or_default()
                                    data-mirabile-control=ControlId::RESOURCE_DESCRIPTION.to_string()
                                    data-mirabile-address=resource_address(ControlId::RESOURCE_DESCRIPTION, kind, resource_id)
                                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                                    on:change=move |event| { let value=event_target_value(&event); dispatch_metadata(description_dispatcher, kind, ResourceMetadataMutation::SetDescription((!value.trim().is_empty()).then_some(value))); } /></label>
                                <label>"Tags"<input type="text" prop:value=draft.tags.join(", ")
                                    data-mirabile-control=ControlId::RESOURCE_TAGS.to_string()
                                    data-mirabile-address=resource_address(ControlId::RESOURCE_TAGS, kind, resource_id)
                                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                                    on:change=move |event| dispatch_metadata(tags_dispatcher, kind, ResourceMetadataMutation::SetTags(event_target_value(&event).split(',').map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned).collect())) /></label>
                                <PayloadEditor kind value=draft.value.clone() dispatcher />
                                <p class="persisted-label">{payload_summary(&draft.value)}</p>
                                <div class="draft-actions">
                                    <button type="button" class="button primary" disabled=!save_enabled
                                        data-mirabile-control=ControlId::RESOURCE_SAVE.to_string()
                                        data-mirabile-address=resource_address(ControlId::RESOURCE_SAVE, kind, resource_id)
                                        data-mirabile-kind=ControlKind::Action.as_str()
                                        data-mirabile-enabled=save_enabled.to_string()
                                        data-mirabile-disabled-reason=(!save_enabled).then_some("Draft has no saveable changes")
                                        on:click=move |_| save_dispatcher.dispatch(AppIntent::SaveResourceDraft { kind })>"Save"</button>
                                    <button type="button" class="button secondary"
                                        data-mirabile-control=ControlId::RESOURCE_CANCEL.to_string()
                                        data-mirabile-address=resource_address(ControlId::RESOURCE_CANCEL, kind, resource_id)
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
fn PayloadEditor(
    kind: ResourceDraftKind,
    value: mirabile_app::ResourceDraftValueReadModel,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    use mirabile_app::ResourceDraftValueReadModel as Value;

    match value {
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
        Value::ViewDocument(document) => {
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
            </fieldset> }.into_any()
        }
        Value::QueryDefinition(query) => {
            view! { <fieldset class="payload-fields"><legend>"Query definition"</legend>
                <label>"Query description"<textarea prop:value=query.description.unwrap_or_default()
                    data-mirabile-control=ControlId::RESOURCE_QUERY_DESCRIPTION.to_string()
                    data-mirabile-address=resource_address(ControlId::RESOURCE_QUERY_DESCRIPTION, kind, None)
                    data-mirabile-kind=ControlKind::Text.as_str() data-mirabile-enabled="true"
                    on:change=move |event| { let value=event_target_value(&event); dispatch_payload(dispatcher, ResourceMutation::QueryDefinition(QueryDefinitionMutation::SetDescription((!value.trim().is_empty()).then_some(value)))); } /></label>
                <small>"The typed AST is persisted but execution is deferred."</small>
            </fieldset> }.into_any()
        }
        _ => view! { <p class="cockpit-note">"Payload fields are controlled by the authoritative composite/session editor or the typed list builder for this resource."</p> }.into_any(),
    }
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
        ResourceDraftKind::ChartDefinition => dispatcher.dispatch(AppIntent::BeginNewChart),
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
        ResourceDraftKind::ChartDefinition => (
            false,
            Some("Open the saved chart in the workspace before editing its atomic pair"),
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
