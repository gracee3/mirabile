use std::rc::Rc;

use leptos::{ev, prelude::*};
#[cfg(test)]
use mirabile_app::ProjectionVersion;
use mirabile_app::{
    AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind, AppReadModel,
    Application, ApplicationStatus, DraftState, ViewComputationState,
};
use wasm_bindgen::JsCast;

use crate::commands::CommandId;

#[derive(Clone)]
struct AppDispatcher {
    application: Rc<dyn Application>,
    model: RwSignal<AppReadModel>,
}

impl AppDispatcher {
    fn initialize(&self) {
        let application = Rc::clone(&self.application);
        let model = self.model;
        leptos::task::spawn_local(async move {
            match application.initialize().await {
                Ok(updated) => publish_and_settle(application, model, updated).await,
                Err(error) => publish_application_error(model, error),
            }
        });
    }

    fn dispatch(&self, intent: AppIntent) {
        let application = Rc::clone(&self.application);
        let model = self.model;
        leptos::task::spawn_local(async move {
            match application.dispatch(intent).await {
                Ok(updated) => publish_and_settle(application, model, updated).await,
                Err(error) => publish_command_error(model, error),
            }
        });
    }
}

#[derive(Clone, Copy)]
pub(super) struct Dispatcher {
    stored: StoredValue<AppDispatcher, LocalStorage>,
}

impl Dispatcher {
    pub(super) fn new(application: Rc<dyn Application>, model: RwSignal<AppReadModel>) -> Self {
        Self {
            stored: StoredValue::new_local(AppDispatcher { application, model }),
        }
    }

    pub(super) fn initialize(self) {
        self.stored.with_value(AppDispatcher::initialize);
    }

    pub(super) fn dispatch(self, intent: AppIntent) {
        self.stored
            .with_value(|dispatcher| dispatcher.dispatch(intent));
    }
}

async fn publish_and_settle(
    application: Rc<dyn Application>,
    model: RwSignal<AppReadModel>,
    mut incoming: AppReadModel,
) {
    loop {
        let after = incoming.version;
        let pending = has_pending_work(&incoming);
        publish_projection(model, incoming);
        if !pending {
            return;
        }

        match application.wait_for_update(after).await {
            Ok(updated) if updated.version > after => incoming = updated,
            Ok(updated) => {
                publish_command_error(
                    model,
                    AppError::new(
                        AppErrorKind::Unavailable,
                        format!(
                            "Application returned projection {} while waiting after {after}",
                            updated.version
                        ),
                    ),
                );
                return;
            }
            Err(error) => {
                publish_command_error(model, error);
                return;
            }
        }
    }
}

fn publish_projection(model: RwSignal<AppReadModel>, incoming: AppReadModel) {
    model.update(|current| {
        publish_if_newer(current, incoming);
    });
}

/// Publishes only a strictly newer authoritative projection.
///
/// Equal versions are redundant copies; older versions are stale asynchronous completions.
fn publish_if_newer(current: &mut AppReadModel, incoming: AppReadModel) -> bool {
    if incoming.version > current.version {
        *current = incoming;
        true
    } else {
        false
    }
}

fn has_pending_work(model: &AppReadModel) -> bool {
    let view_pending = model.active_view.as_ref().is_some_and(|view| {
        matches!(
            view.computation,
            ViewComputationState::Loading | ViewComputationState::Refreshing
        )
    });
    let save_pending = model
        .resource_editor
        .aspect_set
        .as_ref()
        .is_some_and(|draft| matches!(draft.state, DraftState::Saving { .. }));
    view_pending || save_pending
}

fn publish_application_error(model: RwSignal<AppReadModel>, error: AppError) {
    model.update(|current| {
        current.status = ApplicationStatus::Error(error);
        current.notice = None;
    });
}

fn publish_command_error(model: RwSignal<AppReadModel>, error: AppError) {
    model.update(|current| {
        current.notice = Some(AppNotice {
            kind: if error.kind == AppErrorKind::Conflict {
                AppNoticeKind::Conflict
            } else {
                AppNoticeKind::Warning
            },
            message: error.message,
        });
    });
}

pub(super) fn execute_command(
    command: CommandId,
    dispatcher: Dispatcher,
    model: RwSignal<AppReadModel>,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) {
    match command {
        CommandId::SaveDraft => {
            if model
                .get_untracked()
                .availability(AppAction::SaveDraft)
                .is_enabled()
            {
                dispatcher.dispatch(AppIntent::SaveDraft);
            }
        }
        CommandId::CancelDraft => {
            if model
                .get_untracked()
                .availability(AppAction::CancelDraft)
                .is_enabled()
            {
                reset_orb_buffer(model, orb_buffer, orb_error);
                dispatcher.dispatch(AppIntent::CancelDraft);
            }
        }
        CommandId::FocusChartRail => focus_chart_rail(),
        CommandId::RefreshView => {
            if model
                .get_untracked()
                .availability(AppAction::RefreshView)
                .is_enabled()
            {
                dispatcher.dispatch(AppIntent::RefreshActiveView);
            }
        }
    }
}

pub(super) fn reset_orb_buffer(
    model: RwSignal<AppReadModel>,
    orb_buffer: RwSignal<String>,
    orb_error: RwSignal<Option<String>>,
) {
    let snapshot = model.get_untracked();
    if let Some(resource_id) = snapshot.inspector.active_aspect_set
        && let Some(summary) = snapshot
            .library
            .aspect_sets
            .iter()
            .find(|summary| summary.resource_id == resource_id)
    {
        orb_buffer.set(format!("{:.1}", summary.conjunction_orb.degrees()));
    }
    orb_error.set(None);
}

pub(super) fn event_target_is_text_entry(event: &ev::KeyboardEvent) -> bool {
    event
        .target()
        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
        .is_some_and(|element| {
            matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                || element.get_attribute("contenteditable").as_deref() == Some("true")
        })
}

fn focus_chart_rail() {
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("workspace-chart-rail"))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_if_newer_rejects_stale_projection() {
        let mut current = model_at(10);

        assert!(publish_if_newer(&mut current, model_at(12)));
        assert_eq!(current.version, ProjectionVersion::new(12));
        assert!(!publish_if_newer(&mut current, model_at(11)));
        assert_eq!(current.version, ProjectionVersion::new(12));
        assert!(!publish_if_newer(&mut current, model_at(12)));
        assert_eq!(current.version, ProjectionVersion::new(12));
    }

    fn model_at(version: u64) -> AppReadModel {
        let mut model = AppReadModel::initializing();
        model.version = ProjectionVersion::new(version);
        model
    }
}
