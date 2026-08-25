use std::{cell::Cell, cell::RefCell, collections::VecDeque, rc::Rc};

use leptos::{ev, prelude::*};
#[cfg(test)]
use mirabile_app::ProjectionVersion;
use mirabile_app::{
    ActionSource, AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind,
    AppReadModel, Application, ApplicationActivityReadModel, ApplicationStatus, ControlAddress,
    ControlId, CoordinatorReadModel, ExecutionOutcome, ExecutionTraceEntry, PendingTransition,
    TraceHistory,
};
use wasm_bindgen::JsCast;

use crate::commands::CommandId;

#[derive(Clone)]
struct QueuedAction {
    intent: AppIntent,
    source: ActionSource,
    origin_control: Option<ControlAddress>,
}

#[derive(Clone)]
struct CoordinatorState {
    application: Rc<dyn Application>,
    model: RwSignal<AppReadModel>,
    coordinator: RwSignal<CoordinatorReadModel>,
    queue: Rc<RefCell<VecDeque<QueuedAction>>>,
    running: Rc<Cell<bool>>,
    next_sequence: Rc<Cell<u64>>,
    trace: Rc<RefCell<TraceHistory>>,
}

impl CoordinatorState {
    fn initialize(&self) {
        if self.running.replace(true) {
            return;
        }
        self.coordinator.update(|state| {
            state.running = true;
            state.current_source = Some(ActionSource::System);
        });
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.execute_initialization().await;
            state.drain_queue().await;
        });
    }

    fn enqueue(&self, action: QueuedAction) {
        self.queue.borrow_mut().push_back(action);
        self.coordinator.update(|state| {
            state.queued_actions = self.queue.borrow().len();
        });
        if !self.running.replace(true) {
            let state = self.clone();
            leptos::task::spawn_local(async move {
                state.drain_queue().await;
            });
        }
    }

    async fn execute_initialization(&self) {
        let before = self.model.get_untracked().version;
        let sequence = self.take_sequence();
        match self.application.initialize().await {
            Ok(updated) => {
                let accepted = updated.version;
                let (settled, transitions, outcome) = self.settle(updated).await;
                self.trace.borrow_mut().push(ExecutionTraceEntry {
                    sequence,
                    source: ActionSource::System,
                    origin_control: None,
                    semantic_intent: "application.initialize".into(),
                    accepted_projection: Some(accepted),
                    settled_projection: settled,
                    pending_transitions: transitions,
                    outcome,
                });
            }
            Err(error) => {
                publish_application_error(self.model, error.clone());
                self.trace.borrow_mut().push(ExecutionTraceEntry {
                    sequence,
                    source: ActionSource::System,
                    origin_control: None,
                    semantic_intent: "application.initialize".into(),
                    accepted_projection: None,
                    settled_projection: before,
                    pending_transitions: Vec::new(),
                    outcome: failure_outcome(&error),
                });
            }
        }
    }

    async fn drain_queue(&self) {
        loop {
            let Some(action) = self.queue.borrow_mut().pop_front() else {
                self.running.set(false);
                self.coordinator.update(|state| {
                    state.running = false;
                    state.queued_actions = 0;
                    state.current_source = None;
                    state.highlighted_control = None;
                });
                return;
            };
            self.coordinator.update(|state| {
                state.running = true;
                state.queued_actions = self.queue.borrow().len();
                state.current_source = Some(action.source);
                state.highlighted_control.clone_from(&action.origin_control);
            });
            self.execute_action(action).await;
        }
    }

    async fn execute_action(&self, action: QueuedAction) {
        let sequence = self.take_sequence();
        let semantic_intent = action.intent.semantic_summary();
        let before = self.model.get_untracked().version;
        let (accepted_projection, settled_projection, pending_transitions, outcome) =
            match self.application.dispatch(action.intent).await {
                Ok(updated) => {
                    let accepted = updated.version;
                    let (settled, transitions, outcome) = self.settle(updated).await;
                    (Some(accepted), settled, transitions, outcome)
                }
                Err(error) => {
                    publish_command_error(self.model, error.clone());
                    (
                        None,
                        before,
                        Vec::new(),
                        ExecutionOutcome::Rejected {
                            kind: error_kind(&error),
                            message: error.message,
                        },
                    )
                }
            };
        self.trace.borrow_mut().push(ExecutionTraceEntry {
            sequence,
            source: action.source,
            origin_control: action.origin_control,
            semantic_intent,
            accepted_projection,
            settled_projection,
            pending_transitions,
            outcome,
        });
    }

    async fn settle(
        &self,
        mut incoming: AppReadModel,
    ) -> (
        mirabile_app::ProjectionVersion,
        Vec<PendingTransition>,
        ExecutionOutcome,
    ) {
        let mut transitions = Vec::new();
        loop {
            let after = incoming.version;
            if !incoming.is_settled() {
                transitions.push(PendingTransition {
                    projection: incoming.version,
                    pending_operations: incoming.activity.pending_operations.clone(),
                });
            }
            publish_projection(self.model, incoming);
            if self.model.get_untracked().is_settled() {
                return (after, transitions, ExecutionOutcome::Settled);
            }
            match self.application.wait_for_update(after).await {
                Ok(updated) if updated.version > after => incoming = updated,
                Ok(updated) => {
                    let error = AppError::new(
                        AppErrorKind::Unavailable,
                        format!(
                            "Application returned projection {} while waiting after {after}",
                            updated.version
                        ),
                    );
                    publish_command_error(self.model, error.clone());
                    return (after, transitions, failure_outcome(&error));
                }
                Err(error) => {
                    publish_command_error(self.model, error.clone());
                    return (after, transitions, failure_outcome(&error));
                }
            }
        }
    }

    fn take_sequence(&self) -> u64 {
        let sequence = self.next_sequence.get();
        self.next_sequence.set(sequence.saturating_add(1));
        sequence
    }
}

#[derive(Clone, Copy)]
pub(super) struct WorkbenchCoordinator {
    stored: StoredValue<CoordinatorState, LocalStorage>,
}

impl WorkbenchCoordinator {
    pub(super) fn new(application: Rc<dyn Application>, model: RwSignal<AppReadModel>) -> Self {
        Self {
            stored: StoredValue::new_local(CoordinatorState {
                application,
                model,
                coordinator: RwSignal::new(CoordinatorReadModel::default()),
                queue: Rc::new(RefCell::new(VecDeque::new())),
                running: Rc::new(Cell::new(false)),
                next_sequence: Rc::new(Cell::new(1)),
                trace: Rc::new(RefCell::new(TraceHistory::default())),
            }),
        }
    }

    pub(super) fn initialize(self) {
        self.stored.with_value(CoordinatorState::initialize);
    }

    pub(super) fn dispatch(self, intent: AppIntent) {
        self.dispatch_from(intent, ActionSource::Human, None);
    }

    pub(super) fn dispatch_from(
        self,
        intent: AppIntent,
        source: ActionSource,
        origin_control: Option<ControlAddress>,
    ) {
        self.stored.with_value(|coordinator| {
            coordinator.enqueue(QueuedAction {
                intent,
                source,
                origin_control,
            });
        });
    }

    pub(super) fn read_model(self) -> CoordinatorReadModel {
        self.stored
            .with_value(|coordinator| coordinator.coordinator.get_untracked())
    }

    pub(super) fn trace(self) -> Vec<ExecutionTraceEntry> {
        self.stored
            .with_value(|coordinator| coordinator.trace.borrow().entries())
    }
}

fn error_kind(error: &AppError) -> String {
    format!("{:?}", error.kind).to_ascii_lowercase()
}

fn failure_outcome(error: &AppError) -> ExecutionOutcome {
    ExecutionOutcome::Failed {
        kind: error_kind(error),
        message: error.message.clone(),
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

fn publish_application_error(model: RwSignal<AppReadModel>, error: AppError) {
    model.update(|current| {
        current.status = ApplicationStatus::Error(error);
        current.activity = ApplicationActivityReadModel::settled();
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
    dispatcher: WorkbenchCoordinator,
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
                && orb_error.get_untracked().is_none()
            {
                dispatcher.dispatch_from(
                    AppIntent::SaveDraft,
                    ActionSource::Human,
                    Some(ControlAddress::new(ControlId::DRAFT_SAVE)),
                );
            }
        }
        CommandId::CancelDraft => {
            if model
                .get_untracked()
                .availability(AppAction::CancelDraft)
                .is_enabled()
            {
                reset_orb_buffer(model, orb_buffer, orb_error);
                dispatcher.dispatch_from(
                    AppIntent::CancelDraft,
                    ActionSource::Human,
                    Some(ControlAddress::new(ControlId::DRAFT_CANCEL)),
                );
            }
        }
        CommandId::FocusChartRail => focus_chart_rail(),
        CommandId::RefreshView => {
            if model
                .get_untracked()
                .availability(AppAction::RefreshView)
                .is_enabled()
            {
                dispatcher.dispatch_from(
                    AppIntent::RefreshActiveView,
                    ActionSource::Human,
                    Some(ControlAddress::new(ControlId::APPLICATION_REFRESH)),
                );
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
