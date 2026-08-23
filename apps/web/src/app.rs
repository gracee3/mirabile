use astra_core::{
    Angle, AspectSet, CanonicalResource, Command, EditorState, ResourceEnvelope, Timestamp,
};
use astra_engine::{
    AspectAnalyzer, CalculationEngine, DeterministicEphemeris, Scene, layout_wheel, render_key,
};
use leptos::ev;
use leptos::prelude::*;

use crate::{demo, render::WheelScene};

#[component]
#[allow(clippy::too_many_lines)]
pub fn App() -> impl IntoView {
    let record = RwSignal::new(demo::record());
    let definition = RwSignal::new(demo::definition(record.get_untracked().id));
    let points = RwSignal::new(demo::points());
    let analysis_profile = RwSignal::new(demo::analysis_profile());
    let wheel = RwSignal::new(demo::wheel());
    let canonical_aspects = RwSignal::new(demo::aspect_resource());
    let editor = RwSignal::new(EditorState::clean(
        canonical_aspects.get_untracked().revision,
    ));
    let theme = RwSignal::new(demo::dark_theme());
    let dark_mode = RwSignal::new(true);
    let storage_status = RwSignal::new(String::from("Opening local library…"));

    initialize_local_resource(canonical_aspects, editor, storage_status);

    let effective_aspects = Memo::new(move |_| {
        let canonical = canonical_aspects.get();
        editor.get().effective(&canonical.payload).clone()
    });
    let snapshot = Memo::new(move |_| {
        CalculationEngine::new(
            DeterministicEphemeris,
            "astra-engine-v1",
            "deterministic-no-tzdb",
        )
        .calculate(&definition.get(), &record.get())
        .map_err(|error| error.to_string())
    });
    let analysis = Memo::new(move |_| {
        snapshot.get().and_then(|snapshot| {
            AspectAnalyzer::analyze(
                &snapshot,
                &points.get(),
                &effective_aspects.get(),
                &analysis_profile.get(),
            )
            .map_err(|error| error.to_string())
        })
    });
    let layout = Memo::new(move |_| {
        snapshot.get().and_then(|snapshot| {
            analysis.get().and_then(|analysis| {
                layout_wheel(&snapshot, &analysis, &points.get(), &wheel.get(), None)
                    .map_err(|error| error.to_string())
            })
        })
    });
    let scene = Memo::new(move |_| {
        layout
            .get()
            .map(|layout| Scene::from_wheel(&layout))
            .map_err(|error| error.clone())
    });
    let current_render_key = Memo::new(move |_| {
        layout
            .get()
            .and_then(|layout| render_key(&layout, &theme.get()).map_err(|error| error.to_string()))
    });
    let error_message = Memo::new(move |_| {
        snapshot
            .get()
            .err()
            .or_else(|| analysis.get().err())
            .or_else(|| layout.get().err())
    });

    let edit_orb = move |event: ev::Event| {
        let Ok(value) = event_target_value(&event).parse::<f64>() else {
            return;
        };
        let Ok(orb) = Angle::from_degrees(value) else {
            return;
        };
        if editor.get_untracked().is_saving() {
            return;
        }
        let canonical = canonical_aspects.get_untracked();
        editor.update(|state| {
            *state = state.clone().edit(&canonical.payload, |draft| {
                if let Some(conjunction) = draft.aspects.first_mut() {
                    conjunction.orbs.maximum = orb;
                }
            });
        });
        storage_status.set("Previewing an uncommitted draft".into());
    };

    let save = move |_| {
        let base = canonical_aspects.get_untracked();
        let prior_editor = editor.get_untracked();
        if !prior_editor.is_dirty() || prior_editor.is_saving() {
            return;
        }
        let draft = prior_editor.effective(&base.payload).clone();
        let timestamp = Timestamp::from_unix_millis(base.modified_at.unix_millis() + 1);
        let Ok(next) = base.next_with_payload(draft, timestamp) else {
            storage_status.set("Revision overflow; draft was not saved".into());
            return;
        };
        let command = Command::SaveResourceDraft {
            expected_revision: base.revision,
            resource: CanonicalResource::AspectSet(next.clone()),
        };
        editor.set(prior_editor.clone().begin_save());
        storage_status.set("Saving locally…".into());
        save_local_revision(
            base,
            next,
            command,
            prior_editor,
            canonical_aspects,
            editor,
            storage_status,
        );
    };

    let cancel = move |_| {
        let revision = canonical_aspects.get_untracked().revision;
        editor.update(|state| *state = state.clone().cancel(revision));
        storage_status.set("Draft canceled; canonical revision restored".into());
    };

    let toggle_theme = move |_| {
        let next_dark = !dark_mode.get_untracked();
        dark_mode.set(next_dark);
        theme.set(if next_dark {
            demo::dark_theme()
        } else {
            demo::light_theme()
        });
    };

    view! {
        <main class="app-shell">
            <header class="masthead">
                <div>
                    <p class="eyebrow">"ASTRA / ARCHITECTURE FOUNDATION"</p>
                    <h1>"One chart, explicit dependencies"</h1>
                    <p class="lede">
                        "A deterministic test provider proves the flow from canonical assertions to an astrology-free SVG scene."
                    </p>
                </div>
                <button class="secondary" type="button" on:click=toggle_theme>
                    {move || if dark_mode.get() { "Use light theme" } else { "Use dark theme" }}
                </button>
            </header>

            <section class="workspace" aria-label="Chart workspace">
                <article class="chart-panel">
                    <div class="panel-heading">
                        <div>
                            <p class="eyebrow">"DERIVED VIEW"</p>
                            <h2>"Radix preview"</h2>
                        </div>
                        <span class="badge">"fake ephemeris"</span>
                    </div>
                    <WheelScene scene=scene theme=theme />
                    {move || {
                        error_message
                            .get()
                            .map(|message| view! { <p class="error" role="alert">{message}</p> })
                    }}
                </article>

                <aside class="editor-panel" aria-labelledby="aspect-editor-title">
                    <p class="eyebrow">"EPHEMERAL EDITOR STATE"</p>
                    <h2 id="aspect-editor-title">"Conjunction orb"</h2>
                    <p>
                        "The draft feeds analysis immediately. The canonical resource changes only after a successful local revision write."
                    </p>

                    <label for="conjunction-orb">
                        "Maximum orb"
                        <output for="conjunction-orb">
                            {move || format!("{:.1}°", conjunction_orb(&effective_aspects.get()))}
                        </output>
                    </label>
                    <input
                        id="conjunction-orb"
                        type="range"
                        min="0"
                        max="12"
                        step="0.5"
                        prop:value=move || format!("{:.1}", conjunction_orb(&effective_aspects.get()))
                        disabled=move || editor.get().is_saving()
                        on:input=edit_orb
                    />

                    <div class="actions">
                        <button
                            class="primary"
                            type="button"
                            disabled=move || !editor.get().is_dirty() || editor.get().is_saving()
                            on:click=save
                        >
                            "Save revision"
                        </button>
                        <button
                            class="secondary"
                            type="button"
                            disabled=move || !editor.get().is_dirty() || editor.get().is_saving()
                            on:click=cancel
                        >
                            "Cancel"
                        </button>
                    </div>

                    <dl class="facts">
                        <div>
                            <dt>"Canonical revision"</dt>
                            <dd>{move || canonical_aspects.get().revision.to_string()}</dd>
                        </div>
                        <div>
                            <dt>"Editor"</dt>
                            <dd>{move || editor_label(&editor.get())}</dd>
                        </div>
                        <div>
                            <dt>"Aspect hits"</dt>
                            <dd>{move || analysis.get().map_or_else(|_| "—".into(), |value| value.aspects.len().to_string())}</dd>
                        </div>
                    </dl>
                    <p class="status" aria-live="polite">{move || storage_status.get()}</p>
                </aside>
            </section>

            <section class="keys" aria-labelledby="dependency-keys-title">
                <div class="panel-heading">
                    <div>
                        <p class="eyebrow">"CONTENT-ADDRESSED COMPUTATION"</p>
                        <h2 id="dependency-keys-title">"Dependency keys"</h2>
                    </div>
                </div>
                <div class="key-grid">
                    <KeyCard
                        label="CalcKey"
                        detail="civil time + location + calculation + provider"
                        value=Signal::derive(move || key_text(snapshot.get().map(|value| value.calc_key)))
                    />
                    <KeyCard
                        label="AnalysisKey"
                        detail="CalcKey + points + aspects + analysis profile"
                        value=Signal::derive(move || key_text(analysis.get().map(|value| value.analysis_key)))
                    />
                    <KeyCard
                        label="RenderKey"
                        detail="LayoutKey + theme + renderer version"
                        value=Signal::derive(move || key_text(current_render_key.get()))
                    />
                </div>
                <p class="invalidation-note">
                    "Move the orb slider: CalcKey stays fixed while AnalysisKey changes. Toggle the theme: both CalcKey and AnalysisKey stay fixed while RenderKey changes."
                </p>
            </section>
        </main>
    }
}

#[component]
fn KeyCard(label: &'static str, detail: &'static str, value: Signal<String>) -> impl IntoView {
    view! {
        <article class="key-card">
            <h3>{label}</h3>
            <code>{move || value.get()}</code>
            <p>{detail}</p>
        </article>
    }
}

fn conjunction_orb(aspects: &AspectSet) -> f64 {
    aspects
        .aspects
        .first()
        .map_or(0.0, |aspect| aspect.orbs.maximum.degrees())
}

fn editor_label(editor: &EditorState<AspectSet>) -> &'static str {
    match editor {
        EditorState::Clean { .. } => "Clean",
        EditorState::Dirty { .. } => "Dirty preview",
        EditorState::Saving { .. } => "Saving",
        EditorState::Conflict { .. } => "Conflict",
    }
}

fn key_text<T: std::fmt::Display>(value: Result<T, String>) -> String {
    value.map_or_else(
        |_| "unavailable".into(),
        |key| {
            let text = key.to_string();
            text.chars().take(16).collect()
        },
    )
}

fn initialize_local_resource(
    canonical: RwSignal<ResourceEnvelope<AspectSet>>,
    editor: RwSignal<EditorState<AspectSet>>,
    status: RwSignal<String>,
) {
    #[cfg(target_arch = "wasm32")]
    {
        let seed = canonical.get_untracked();
        leptos::task::spawn_local(async move {
            match crate::persistence::load_or_seed(seed).await {
                Ok(resource) => {
                    let revision = resource.revision;
                    canonical.set(resource);
                    editor.set(EditorState::clean(revision));
                    status.set(format!("Local library ready at revision {revision}"));
                }
                Err(error) => status.set(format!("Local persistence unavailable: {error}")),
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (canonical, editor);
        status.set("IndexedDB is available in the WASM build".into());
    }
}

#[allow(clippy::needless_pass_by_value)]
fn save_local_revision(
    base: ResourceEnvelope<AspectSet>,
    next: ResourceEnvelope<AspectSet>,
    command: Command,
    prior_editor: EditorState<AspectSet>,
    canonical: RwSignal<ResourceEnvelope<AspectSet>>,
    editor: RwSignal<EditorState<AspectSet>>,
    status: RwSignal<String>,
) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        match crate::persistence::save_command(base, command).await {
            Ok(()) => {
                let revision = next.revision;
                canonical.set(next);
                editor.set(prior_editor.saved(revision));
                status.set(format!("Saved locally as revision {revision}"));
            }
            Err(error) => {
                if let Some(remote_revision) = crate::persistence::conflict_revision(&error) {
                    editor.set(prior_editor.conflict(remote_revision));
                } else {
                    editor.set(prior_editor);
                }
                status.set(format!("Save failed: {error}"));
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (base, command);
        let revision = next.revision;
        canonical.set(next);
        editor.set(prior_editor.saved(revision));
        status.set(format!(
            "Committed in native preview as revision {revision}"
        ));
    }
}
