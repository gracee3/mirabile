use leptos::prelude::*;
use mirabile_app::{AppIntent, AppReadModel, ChartPersistence};

use crate::{dispatcher::WorkbenchCoordinator, library::LibraryShelf};

#[component]
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

            <LibraryShelf model dispatcher />
        </nav>
    }
}
