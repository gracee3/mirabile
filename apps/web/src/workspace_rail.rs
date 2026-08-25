use leptos::prelude::*;
use mirabile_app::{
    ActionSource, AppAction, AppIntent, AppReadModel, ChartPersistence, ControlAddress, ControlId,
    WorkspaceSwitchAction,
};

use crate::{
    dispatcher::WorkbenchCoordinator,
    library::LibraryShelf,
    workbench_controls::{ActionControl, BufferedTextField},
};

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn WorkspaceRail(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    let title_buffer = RwSignal::new(String::new());
    let title_error = RwSignal::new(None::<String>);
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
                    <h2 id="workspace-title">{move || model.get().workspace.title}</h2>
                </div>
                <span class="count-badge">{move || model.get().workspace.charts.len()}</span>
            </div>
            <div class="workspace-management">
                <BufferedTextField
                    address=ControlAddress::new(ControlId::WORKSPACE_TITLE).to_string()
                    label="Workspace title".into()
                    authoritative=Signal::derive(move || model.get().workspace.title)
                    disabled=Signal::derive(move || model.get().workspace.switch_decision.is_some())
                    buffer=title_buffer
                    error=title_error
                    parser=Callback::new(|value: String| {
                        let trimmed = value.trim();
                        (!trimmed.is_empty())
                            .then(|| trimmed.to_owned())
                            .ok_or_else(|| "Workspace title is required".to_owned())
                    })
                    on_commit=Callback::new(move |title: String| dispatcher.dispatch_from(
                        AppIntent::RenameWorkspace { title },
                        ActionSource::Human,
                        Some(ControlAddress::new(ControlId::WORKSPACE_TITLE)),
                    ))
                />
                <div class="draft-actions">
                    <ActionControl
                        address=ControlAddress::new(ControlId::WORKSPACE_NEW).to_string()
                        label="New workspace".into()
                        disabled=Signal::derive(move || model.get().workspace.switch_decision.is_some())
                        on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                            AppIntent::NewWorkspace,
                            ActionSource::Human,
                            Some(ControlAddress::new(ControlId::WORKSPACE_NEW)),
                        ))
                    />
                    <ActionControl
                        address=ControlAddress::new(ControlId::WORKSPACE_SAVE).to_string()
                        label="Save workspace".into()
                        disabled=Signal::derive(move || !model.get().availability(AppAction::SaveWorkspace).is_enabled())
                        on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                            AppIntent::SaveWorkspace,
                            ActionSource::Human,
                            Some(ControlAddress::new(ControlId::WORKSPACE_SAVE)),
                        ))
                    />
                    <ActionControl
                        address=ControlAddress::new(ControlId::WORKSPACE_DISCARD).to_string()
                        label="Discard workspace changes".into()
                        disabled=Signal::derive(move || model.get().workspace.switch_decision.is_some())
                        on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                            AppIntent::DiscardWorkspaceChanges,
                            ActionSource::Human,
                            Some(ControlAddress::new(ControlId::WORKSPACE_DISCARD)),
                        ))
                    />
                    <ActionControl
                        address=ControlAddress::new(ControlId::WORKSPACE_LOAD_DEMO).to_string()
                        label="Load demo bundle".into()
                        disabled=Signal::derive(move || !model.get().is_settled())
                        on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                            AppIntent::LoadDemoBundle,
                            ActionSource::Human,
                            Some(ControlAddress::new(ControlId::WORKSPACE_LOAD_DEMO)),
                        ))
                    />
                </div>
                <div class="workspace-library" aria-label="Saved workspaces">
                    {move || model.get().library.workspaces.into_iter().map(|workspace| {
                        let address = ControlAddress::qualified(
                            ControlId::WORKSPACE_OPEN,
                            [("resource", workspace.resource_id.to_string())],
                        ).expect("workspace open address");
                        let origin = address.clone();
                        view! {
                            <ActionControl
                                address=address.to_string()
                                label=format!("Open {} · revision {}", workspace.title, workspace.revision)
                                disabled=Signal::derive(move || model.get().workspace.switch_decision.is_some())
                                on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                    AppIntent::OpenWorkspace { resource_id: workspace.resource_id },
                                    ActionSource::Human,
                                    Some(origin.clone()),
                                ))
                            />
                        }
                    }).collect_view()}
                </div>
                {move || model.get().workspace.switch_decision.map(|decision| {
                    let reasons = decision.reasons;
                    let save_enabled = decision.save_and_switch_enabled;
                    view! {
                        <div class="notice warning workspace-switch-decision" role="alert">
                            <strong>"Switch workspace?"</strong>
                            <ul>{reasons.into_iter().map(|reason| view! { <li>{reason}</li> }).collect_view()}</ul>
                            <div class="draft-actions">
                                <ActionControl
                                    address=ControlAddress::new(ControlId::WORKSPACE_SWITCH_SAVE).to_string()
                                    label="Save and switch".into()
                                    disabled=Signal::derive(move || !save_enabled)
                                    on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                        AppIntent::ResolveWorkspaceSwitch { action: WorkspaceSwitchAction::SaveAndSwitch },
                                        ActionSource::Human,
                                        Some(ControlAddress::new(ControlId::WORKSPACE_SWITCH_SAVE)),
                                    ))
                                />
                                <ActionControl
                                    address=ControlAddress::new(ControlId::WORKSPACE_SWITCH_DISCARD).to_string()
                                    label="Discard and switch".into()
                                    disabled=Signal::derive(|| false)
                                    on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                        AppIntent::ResolveWorkspaceSwitch { action: WorkspaceSwitchAction::DiscardAndSwitch },
                                        ActionSource::Human,
                                        Some(ControlAddress::new(ControlId::WORKSPACE_SWITCH_DISCARD)),
                                    ))
                                />
                                <ActionControl
                                    address=ControlAddress::new(ControlId::WORKSPACE_SWITCH_STAY).to_string()
                                    label="Stay".into()
                                    disabled=Signal::derive(|| false)
                                    on_activate=Callback::new(move |()| dispatcher.dispatch_from(
                                        AppIntent::ResolveWorkspaceSwitch { action: WorkspaceSwitchAction::Stay },
                                        ActionSource::Human,
                                        Some(ControlAddress::new(ControlId::WORKSPACE_SWITCH_STAY)),
                                    ))
                                />
                            </div>
                        </div>
                    }
                })}
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
                        let activate_address = ControlAddress::qualified(
                            ControlId::CHART_ACTIVATE,
                            [("instance", chart.instance_id.to_string())],
                        ).expect("chart activate address");
                        let select_address = ControlAddress::qualified(
                            ControlId::CHART_SELECT,
                            [("instance", chart.instance_id.to_string())],
                        ).expect("chart select address");
                        let close_address = ControlAddress::qualified(
                            ControlId::CHART_CLOSE,
                            [("instance", chart.instance_id.to_string())],
                        ).expect("chart close address");
                        let activate_origin = activate_address.clone();
                        let select_origin = select_address.clone();
                        let close_origin = close_address.clone();
                        view! {
                            <li class="chart-row" class:active=active class:selected=selected>
                                <input
                                    class="selection-check"
                                    type="checkbox"
                                    prop:checked=selected
                                    aria-label=select_label
                                    data-mirabile-control=ControlId::CHART_SELECT.to_string()
                                    data-mirabile-instance=chart.instance_id.to_string()
                                    data-mirabile-address=select_address.to_string()
                                    on:change=move |event| select.dispatch_from(
                                        AppIntent::SetChartSelection {
                                            instance_id: chart.instance_id,
                                            selected: event_target_checked(&event),
                                        },
                                        ActionSource::Human,
                                        Some(select_origin.clone()),
                                    )
                                />
                                <button
                                    class="chart-activate"
                                    type="button"
                                    aria-current=active.then_some("page")
                                    data-mirabile-control=ControlId::CHART_ACTIVATE.to_string()
                                    data-mirabile-instance=chart.instance_id.to_string()
                                    data-mirabile-address=activate_address.to_string()
                                    on:click=move |_| activate.dispatch_from(
                                        AppIntent::ActivateChart { instance_id: chart.instance_id },
                                        ActionSource::Human,
                                        Some(activate_origin.clone()),
                                    )
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
                                    data-mirabile-control=ControlId::CHART_CLOSE.to_string()
                                    data-mirabile-instance=chart.instance_id.to_string()
                                    data-mirabile-address=close_address.to_string()
                                    on:click=move |_| close.dispatch_from(
                                        AppIntent::CloseChart { instance_id: chart.instance_id },
                                        ActionSource::Human,
                                        Some(close_origin.clone()),
                                    )
                                >"×"</button>
                            </li>
                        }
                    }).collect_view()
                }}
            </ul>

            <LibraryShelf model dispatcher />
        </nav>
    }
}
