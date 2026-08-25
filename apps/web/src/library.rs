use leptos::prelude::*;
use mirabile_app::{AppIntent, AppReadModel, ChartPersistence};

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
                                    on:click=move |_| dispatch.dispatch(AppIntent::OpenChart {
                                        definition_id: chart.definition_id,
                                    })
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
