use leptos::prelude::*;
use mirabile_app::{
    ActionSource, AppIntent, AppReadModel, ChartPersistence, ControlAddress, ControlId,
};

use crate::{dispatcher::WorkbenchCoordinator, library::LibraryShelf};

#[component]
#[allow(clippy::too_many_lines)]
pub(super) fn WorkspaceRail(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
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
