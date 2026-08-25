use leptos::prelude::*;
use mirabile_app::{AppReadModel, ViewComputationState};

use crate::render::WheelScene;

#[component]
pub(super) fn ViewHost(model: RwSignal<AppReadModel>) -> impl IntoView {
    view! {
        <main class="view-host" aria-labelledby="active-view-title">
            {move || model.get().active_view.map_or_else(
                || view! {
                    <section class="empty-view">
                        <h1 id="active-view-title">"No active view"</h1>
                        <p>"Choose a view to begin."</p>
                    </section>
                }.into_any(),
                |active_view| {
                    let state_label = match &active_view.computation {
                        ViewComputationState::Loading => "Loading",
                        ViewComputationState::Fresh => "Fresh",
                        ViewComputationState::Refreshing => "Refreshing",
                        ViewComputationState::Failed(_) => "Refresh failed",
                    };
                    let status_class = match &active_view.computation {
                        ViewComputationState::Fresh => "fresh",
                        ViewComputationState::Loading | ViewComputationState::Refreshing => "pending",
                        ViewComputationState::Failed(_) => "failed",
                    };
                    let scene = active_view.scene.clone();
                    let computation = active_view.computation.clone();
                    view! {
                        <section class="view-stage">
                            <header class="view-heading">
                                <div>
                                    <p class="section-kicker">"ACTIVE VIEW INSTANCE"</p>
                                    <h1 id="active-view-title">{active_view.title.clone()}</h1>
                                </div>
                                <span class=format!("view-status {status_class}")>{state_label}</span>
                            </header>

                            <div class="scene-frame" aria-busy=matches!(computation, ViewComputationState::Loading | ViewComputationState::Refreshing)>
                                {scene.map_or_else(
                                    || view! {
                                        <div class="scene-loading" role="status">
                                            <span class="loading-orbit" aria-hidden="true"></span>
                                            <p>"Calculating the first Scene…"</p>
                                        </div>
                                    }.into_any(),
                                    |scene| view! {
                                        <WheelScene
                                            scene
                                            title=format!("{} chart scene", active_view.title)
                                            description="An application Scene of chart rings, point labels, and aspect lines."
                                        />
                                    }.into_any(),
                                )}
                                {matches!(computation, ViewComputationState::Refreshing).then(|| view! {
                                    <div class="refresh-overlay" role="status" aria-live="polite">
                                        <span class="pulse-dot" aria-hidden="true"></span>
                                        "Refreshing analysis · last good Scene retained"
                                    </div>
                                })}
                            </div>

                            {match computation {
                                ViewComputationState::Failed(error) => Some(view! {
                                    <div class="view-error" role="alert">
                                        <strong>"View refresh failed"</strong>
                                        <span>{error.message}</span>
                                    </div>
                                }),
                                ViewComputationState::Loading | ViewComputationState::Fresh | ViewComputationState::Refreshing => None,
                            }}
                            <footer class="view-footnote">
                                "Scene is a presentation projection. No astrology is calculated in this component."
                            </footer>
                        </section>
                    }.into_any()
                },
            )}
        </main>
    }
}
