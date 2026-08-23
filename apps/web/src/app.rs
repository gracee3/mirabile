use astra_core::{
    Angle, AspectSet, CanonicalResource, Command, EditorState, ResourceEnvelope, Timestamp,
};
use astra_engine::{
    AspectAnalyzer, CalculationEngine, DeterministicEphemeris, Scene, layout_wheel, render_key,
};
use leptos::ev;
use leptos::prelude::*;

use crate::{demo, persistence::BrowserRepository, render::WheelScene};

#[derive(Clone, Debug)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
enum BrowserLibraryState {
    Loading,
    Ready {
        repository: BrowserRepository,
        canonical: ResourceEnvelope<AspectSet>,
        editor: EditorState<AspectSet>,
    },
    Error {
        message: String,
    },
}

type LibrarySignal = RwSignal<BrowserLibraryState, LocalStorage>;

impl BrowserLibraryState {
    fn effective_aspects(&self) -> Option<AspectSet> {
        match self {
            Self::Ready {
                canonical, editor, ..
            } => Some(editor.effective(&canonical.payload).clone()),
            Self::Loading | Self::Error { .. } => None,
        }
    }

    fn edit_orb(&mut self, orb: Angle) -> bool {
        let Self::Ready {
            canonical, editor, ..
        } = self
        else {
            return false;
        };
        if editor.is_saving() {
            return false;
        }
        *editor = editor.clone().edit(&canonical.payload, |draft| {
            if let Some(conjunction) = draft.aspects.first_mut() {
                conjunction.orbs.maximum = orb;
            }
        });
        true
    }

    fn cancel(&mut self) -> bool {
        let Self::Ready {
            canonical, editor, ..
        } = self
        else {
            return false;
        };
        *editor = editor.clone().cancel(canonical.revision);
        true
    }

    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready { .. })
    }

    fn is_saving(&self) -> bool {
        matches!(self, Self::Ready { editor, .. } if editor.is_saving())
    }

    fn can_save(&self) -> bool {
        matches!(self, Self::Ready { editor, .. } if editor.is_dirty() && !editor.is_saving())
    }

    fn can_cancel(&self) -> bool {
        self.can_save()
    }

    fn revision_text(&self) -> String {
        match self {
            Self::Ready { canonical, .. } => canonical.revision.to_string(),
            Self::Loading | Self::Error { .. } => "—".into(),
        }
    }

    fn editor_text(&self) -> &'static str {
        match self {
            Self::Loading => "Loading",
            Self::Error { .. } => "Unavailable",
            Self::Ready { editor, .. } => editor_label(editor),
        }
    }
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn App() -> impl IntoView {
    let record = RwSignal::new(demo::record());
    let definition = RwSignal::new(demo::definition(record.get_untracked().id));
    let points = RwSignal::new(demo::points());
    let analysis_profile = RwSignal::new(demo::analysis_profile());
    let wheel = RwSignal::new(demo::wheel());
    let library = RwSignal::new_local(BrowserLibraryState::Loading);
    let theme = RwSignal::new(demo::dark_theme());
    let dark_mode = RwSignal::new(true);
    let storage_status = RwSignal::new(String::from("Opening local library…"));
    let operation_epoch = RwSignal::new(0_u64);

    start_local_resource(library, storage_status, operation_epoch);

    let effective_aspects = Memo::new(move |_| {
        library
            .get()
            .effective_aspects()
            .unwrap_or_else(|| demo::aspect_resource().payload)
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
                layout_wheel(&snapshot, &analysis, &points.get(), &wheel.get())
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
        let mut accepted = false;
        library.update(|state| accepted = state.edit_orb(orb));
        if accepted {
            storage_status.set("Previewing an uncommitted draft".into());
        }
    };

    let save = move |_| {
        let BrowserLibraryState::Ready {
            repository,
            canonical: base,
            editor: prior_editor,
        } = library.get_untracked()
        else {
            return;
        };
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
        let token = next_epoch(operation_epoch);
        library.update(|state| {
            if let BrowserLibraryState::Ready { editor, .. } = state {
                *editor = prior_editor.clone().begin_save();
            }
        });
        storage_status.set("Saving locally…".into());
        save_local_revision(
            repository,
            base,
            next,
            command,
            prior_editor,
            token,
            operation_epoch,
            library,
            storage_status,
        );
    };

    let cancel = move |_| {
        let mut accepted = false;
        library.update(|state| accepted = state.cancel());
        if accepted {
            storage_status.set("Draft canceled; canonical revision restored".into());
        }
    };

    let retry = move |_| start_local_resource(library, storage_status, operation_epoch);

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

                    {move || match library.get() {
                        BrowserLibraryState::Loading => view! {
                            <p class="status" data-editor-state="loading">"Opening IndexedDB before enabling edits…"</p>
                        }.into_any(),
                        BrowserLibraryState::Error { message } => view! {
                            <div data-editor-state="error">
                                <p class="error" role="alert">{message}</p>
                                <button class="secondary" type="button" on:click=retry>"Retry"</button>
                            </div>
                        }.into_any(),
                        BrowserLibraryState::Ready { .. } => view! {
                            <p class="status" data-editor-state="ready">"Local repository ready"</p>
                        }.into_any(),
                    }}

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
                        disabled=move || !library.get().is_ready() || library.get().is_saving()
                        on:input=edit_orb
                    />

                    <div class="actions">
                        <button
                            class="primary"
                            type="button"
                            disabled=move || !library.get().can_save()
                            on:click=save
                        >
                            "Save revision"
                        </button>
                        <button
                            class="secondary"
                            type="button"
                            disabled=move || !library.get().can_cancel()
                            on:click=cancel
                        >
                            "Cancel"
                        </button>
                    </div>

                    <dl class="facts">
                        <div>
                            <dt>"Canonical revision"</dt>
                            <dd>{move || library.get().revision_text()}</dd>
                        </div>
                        <div>
                            <dt>"Editor"</dt>
                            <dd>{move || library.get().editor_text()}</dd>
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
                        detail="civil time + numeric location + calculation + provider"
                        value=Signal::derive(move || key_text(snapshot.get().map(|value| value.calc_key)))
                    />
                    <KeyCard
                        label="AnalysisKey"
                        detail="CalcKey + resolved points + numeric aspect rules"
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

fn next_epoch(epoch: RwSignal<u64>) -> u64 {
    let next = epoch.get_untracked().wrapping_add(1);
    epoch.set(next);
    next
}

fn start_local_resource(library: LibrarySignal, status: RwSignal<String>, epoch: RwSignal<u64>) {
    let token = next_epoch(epoch);
    library.set(BrowserLibraryState::Loading);
    status.set("Opening local library…".into());
    #[cfg(target_arch = "wasm32")]
    {
        let seed = demo::aspect_resource();
        leptos::task::spawn_local(async move {
            let result = crate::persistence::open_and_load(seed).await;
            if epoch.get_untracked() != token {
                return;
            }
            match result {
                Ok((repository, canonical)) => {
                    let revision = canonical.revision;
                    library.set(BrowserLibraryState::Ready {
                        repository,
                        canonical,
                        editor: EditorState::clean(revision),
                    });
                    status.set(format!("Local library ready at revision {revision}"));
                }
                Err(error) => {
                    let message = format!("Local persistence unavailable: {error}");
                    library.set(BrowserLibraryState::Error {
                        message: message.clone(),
                    });
                    status.set(message);
                }
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = token;
        library.set(BrowserLibraryState::Error {
            message: "IndexedDB is available only in the WASM build".into(),
        });
        status.set("IndexedDB is available only in the WASM build".into());
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn save_local_revision(
    repository: BrowserRepository,
    base: ResourceEnvelope<AspectSet>,
    next: ResourceEnvelope<AspectSet>,
    command: Command,
    prior_editor: EditorState<AspectSet>,
    token: u64,
    epoch: RwSignal<u64>,
    library: LibrarySignal,
    status: RwSignal<String>,
) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        let result = crate::persistence::save_command(&repository, command).await;
        if epoch.get_untracked() != token {
            return;
        }
        match result {
            Ok(()) => {
                let revision = next.revision;
                let mut applied = false;
                library.update(|state| {
                    if let BrowserLibraryState::Ready {
                        canonical, editor, ..
                    } = state
                        && canonical.id == base.id
                        && canonical.revision == base.revision
                        && editor.is_saving()
                        && editor.base_revision() == base.revision
                    {
                        *canonical = next.clone();
                        *editor = prior_editor.clone().saved(revision);
                        applied = true;
                    }
                });
                if applied {
                    status.set(format!("Saved locally as revision {revision}"));
                }
            }
            Err(error) => {
                let remote_revision = crate::persistence::conflict_revision(&error);
                let mut applied = false;
                library.update(|state| {
                    if let BrowserLibraryState::Ready {
                        canonical, editor, ..
                    } = state
                        && canonical.id == base.id
                        && canonical.revision == base.revision
                        && editor.is_saving()
                        && editor.base_revision() == base.revision
                    {
                        *editor = remote_revision.map_or_else(
                            || prior_editor.clone(),
                            |revision| prior_editor.clone().conflict(revision),
                        );
                        applied = true;
                    }
                });
                if applied {
                    status.set(format!("Save failed: {error}"));
                }
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (
            repository,
            base,
            next,
            command,
            prior_editor,
            token,
            epoch,
            library,
            status,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loading_and_error_states_reject_edits() {
        let orb = Angle::from_degrees(6.0).expect("valid angle");
        let mut loading = BrowserLibraryState::Loading;
        let mut error = BrowserLibraryState::Error {
            message: "failed".into(),
        };

        assert!(!loading.edit_orb(orb));
        assert!(!error.edit_orb(orb));
        assert!(!loading.can_save());
        assert!(!error.can_save());
    }
}
