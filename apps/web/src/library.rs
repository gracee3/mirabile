use leptos::prelude::*;
use mirabile_app::{
    ActionSource, AppIntent, AppReadModel, ChartPersistence, ControlAddress, ControlId, ControlKind,
};

use crate::dispatcher::WorkbenchCoordinator;

#[component]
pub(super) fn LibraryShelf(
    model: RwSignal<AppReadModel>,
    dispatcher: WorkbenchCoordinator,
) -> impl IntoView {
    view! {
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
                        let address = ControlAddress::qualified(
                            ControlId::CHART_OPEN,
                            [("definition", chart.definition_id.to_string())],
                        ).expect("chart library address");
                        let origin = address.clone();
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
                                    data-mirabile-control=ControlId::CHART_OPEN.to_string()
                                    data-mirabile-definition=chart.definition_id.to_string()
                                    data-mirabile-address=address.to_string()
                                    data-mirabile-kind=ControlKind::Action.as_str()
                                    data-mirabile-enabled="true"
                                    on:click=move |_| dispatch.dispatch_from(
                                        AppIntent::OpenChart { definition_id: chart.definition_id },
                                        ActionSource::Human,
                                        Some(origin.clone()),
                                    )
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
    }
}
