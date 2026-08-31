use std::{cell::Cell, cell::RefCell, collections::VecDeque, rc::Rc};

use leptos::{ev, prelude::*};
#[cfg(test)]
use mirabile_app::ProjectionVersion;
use mirabile_app::{
    ActionSource, AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind,
    AppReadModel, Application, ApplicationActivityReadModel, ApplicationStatus, ControlAddress,
    ControlId, CoordinatorReadModel, ExecutionOutcome, ExecutionTraceEntry, MacroBindings,
    MacroCoordinatorState, MacroDocumentV1, MacroError, MacroRecorder, PendingTransition,
    TraceHistory,
};
#[cfg(target_arch = "wasm32")]
use mirabile_app::{
    WorkflowActionV1, WorkflowDocumentV1, WorkflowExecutionStatusV1, WorkflowResultV1,
    WorkflowValidationError,
};
use wasm_bindgen::JsCast;

use crate::commands::CommandId;

#[derive(Clone)]
struct QueuedAction {
    intent: AppIntent,
    source: ActionSource,
    origin_control: Option<ControlAddress>,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
enum WorkflowBindingValue {
    Chart(mirabile_app::InstanceId),
    View(mirabile_app::ViewInstanceId),
    Workspace(mirabile_app::ResourceId),
}

#[derive(Clone)]
struct CoordinatorState {
    application: Rc<dyn Application>,
    model: RwSignal<AppReadModel>,
    coordinator: RwSignal<CoordinatorReadModel>,
    queue: Rc<RefCell<VecDeque<QueuedAction>>>,
    running: Rc<Cell<bool>>,
    next_sequence: Rc<Cell<u64>>,
    trace: RwSignal<TraceHistory>,
    recorder: RwSignal<Option<MacroRecorder>>,
    macro_document: RwSignal<Option<MacroDocumentV1>>,
    #[cfg(target_arch = "wasm32")]
    workflow_result: RwSignal<Option<WorkflowResultV1>>,
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
                self.trace.update(|trace| {
                    trace.push(ExecutionTraceEntry {
                        sequence,
                        source: ActionSource::System,
                        origin_control: None,
                        semantic_intent: "application.initialize".into(),
                        accepted_projection: Some(accepted),
                        settled_projection: settled,
                        pending_transitions: transitions,
                        outcome,
                    });
                });
            }
            Err(error) => {
                publish_application_error(self.model, error.clone());
                self.trace.update(|trace| {
                    trace.push(ExecutionTraceEntry {
                        sequence,
                        source: ActionSource::System,
                        origin_control: None,
                        semantic_intent: "application.initialize".into(),
                        accepted_projection: None,
                        settled_projection: before,
                        pending_transitions: Vec::new(),
                        outcome: failure_outcome(&error),
                    });
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

    async fn execute_action(&self, action: QueuedAction) -> ExecutionOutcome {
        let sequence = self.take_sequence();
        let semantic_intent = action.intent.semantic_summary();
        let recorded_intent = action.intent.clone();
        let recorded_origin = action.origin_control.clone();
        let source = action.source;
        let before_model = self.model.get_untracked();
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
        let returned_outcome = outcome.clone();
        self.trace.update(|trace| {
            trace.push(ExecutionTraceEntry {
                sequence,
                source,
                origin_control: action.origin_control,
                semantic_intent,
                accepted_projection,
                settled_projection,
                pending_transitions,
                outcome,
            });
        });
        if accepted_projection.is_some()
            && !matches!(source, ActionSource::Macro | ActionSource::System)
        {
            self.capture_recorded_action(&recorded_intent, recorded_origin, &before_model);
        }
        returned_outcome
    }

    fn capture_recorded_action(
        &self,
        intent: &AppIntent,
        origin: Option<ControlAddress>,
        before_model: &AppReadModel,
    ) {
        let mut failure = None;
        let model = self.model.get_untracked();
        self.recorder.update(|recorder| {
            if let Some(recorder) = recorder
                && let Err(error) = recorder.capture(intent, origin, before_model, &model)
            {
                failure = Some(error.to_string());
            }
        });
        if let Some(message) = failure {
            self.recorder.set(None);
            self.coordinator.update(|state| {
                state.macro_state = MacroCoordinatorState::Failed { step: 0, message };
            });
        }
    }

    fn replay(&self, document: MacroDocumentV1) {
        if let Err(error) = document.validate() {
            self.fail_macro(0, error.to_string(), None);
            return;
        }
        if self.running.get() {
            self.coordinator.update(|state| {
                state.macro_state = MacroCoordinatorState::Failed {
                    step: 0,
                    message: "the coordinator is already executing an action".into(),
                };
            });
            return;
        }
        self.running.set(true);
        self.recorder.set(None);
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.execute_macro(document).await;
        });
    }

    async fn execute_macro(&self, document: MacroDocumentV1) {
        let total = document.steps.len();
        let mut bindings = MacroBindings::default();
        for (index, step) in document.steps.into_iter().enumerate() {
            let step_number = index + 1;
            self.coordinator.update(|state| {
                state.running = true;
                state.current_source = Some(ActionSource::Macro);
                state.highlighted_control.clone_from(&step.origin_control);
                state.macro_state = MacroCoordinatorState::Replaying {
                    step: step_number,
                    total,
                };
            });
            let intent = match step.action.resolve(&self.model.get_untracked(), &bindings) {
                Ok(intent) => intent,
                Err(error) => {
                    self.fail_macro(step_number, error.to_string(), step.origin_control);
                    return;
                }
            };
            let outcome = self
                .execute_action(QueuedAction {
                    intent,
                    source: ActionSource::Macro,
                    origin_control: step.origin_control.clone(),
                })
                .await;
            if !matches!(outcome, ExecutionOutcome::Settled) {
                self.fail_macro(step_number, outcome_message(&outcome), step.origin_control);
                return;
            }
            if let Some(binding) = step.bind {
                let result = match step.action.capture_result(&self.model.get_untracked()) {
                    Ok(result) => result,
                    Err(error) => {
                        self.fail_macro(step_number, error.to_string(), step.origin_control);
                        return;
                    }
                };
                if let Err(error) = bindings.insert(binding, result) {
                    self.fail_macro(step_number, error.to_string(), step.origin_control);
                    return;
                }
            }
        }
        self.coordinator.update(|state| {
            state.macro_state = MacroCoordinatorState::Idle;
        });
        self.drain_queue().await;
    }

    #[cfg(target_arch = "wasm32")]
    fn execute_workflow(&self, document: WorkflowDocumentV1) {
        if self.running.replace(true) {
            self.workflow_result.set(Some(WorkflowResultV1 {
                schema_version: mirabile_app::WORKFLOW_DOCUMENT_VERSION,
                status: WorkflowExecutionStatusV1::Failed,
                failed_step: None,
                errors: vec![workflow_error(
                    None,
                    "workflow",
                    "already_running",
                    "Only one workflow may run at once",
                )],
                final_projection: Some(self.model.get_untracked().version),
                created_chart_ids: Vec::new(),
                created_definition_ids: Vec::new(),
                created_view_ids: Vec::new(),
                created_workspace_ids: Vec::new(),
            }));
            return;
        }
        self.workflow_result.set(Some(WorkflowResultV1::running(
            self.model.get_untracked().version,
        )));
        let state = self.clone();
        leptos::task::spawn_local(async move {
            state.run_workflow(document).await;
            state.running.set(false);
            state.coordinator.update(|value| {
                value.running = false;
                value.current_source = None;
            });
            state.drain_queue().await;
        });
    }

    #[cfg(target_arch = "wasm32")]
    async fn run_workflow(&self, document: WorkflowDocumentV1) {
        let mut bindings = std::collections::BTreeMap::<String, WorkflowBindingValue>::new();
        for step in document.steps {
            let result = self.run_workflow_step(&step.action, &bindings).await;
            match result {
                Ok(binding) => {
                    bindings.insert(step.name.clone(), binding);
                    self.workflow_result.update(|result| {
                        if let Some(result) = result {
                            result.final_projection = Some(self.model.get_untracked().version);
                            match binding {
                                WorkflowBindingValue::Chart(id)
                                    if !result.created_chart_ids.contains(&id) =>
                                {
                                    result.created_chart_ids.push(id)
                                }
                                WorkflowBindingValue::View(id)
                                    if !result.created_view_ids.contains(&id) =>
                                {
                                    result.created_view_ids.push(id)
                                }
                                WorkflowBindingValue::Workspace(id)
                                    if !result.created_workspace_ids.contains(&id) =>
                                {
                                    result.created_workspace_ids.push(id)
                                }
                                _ => {}
                            }
                            if let WorkflowBindingValue::Chart(instance_id) = binding
                                && let Some(chart) = self
                                    .model
                                    .get_untracked()
                                    .workspace
                                    .charts
                                    .iter()
                                    .find(|chart| chart.instance_id == instance_id)
                                && let mirabile_app::ChartPersistence::Saved { definition_id } =
                                    chart.persistence
                                && !result.created_definition_ids.contains(&definition_id)
                            {
                                result.created_definition_ids.push(definition_id);
                            }
                        }
                    });
                }
                Err(message) => {
                    self.workflow_result.update(|result| {
                        if let Some(result) = result {
                            result.status = WorkflowExecutionStatusV1::Failed;
                            result.failed_step = Some(step.name.clone());
                            result.errors.push(workflow_error(
                                Some(&step.name),
                                "action",
                                "execution_failed",
                                message,
                            ));
                            result.final_projection = Some(self.model.get_untracked().version);
                        }
                    });
                    return;
                }
            }
        }
        self.workflow_result.update(|result| {
            if let Some(result) = result {
                result.status = WorkflowExecutionStatusV1::Succeeded;
                result.final_projection = Some(self.model.get_untracked().version);
            }
        });
    }

    #[cfg(target_arch = "wasm32")]
    async fn run_workflow_step(
        &self,
        action: &WorkflowActionV1,
        bindings: &std::collections::BTreeMap<String, WorkflowBindingValue>,
    ) -> Result<WorkflowBindingValue, String> {
        match action {
            WorkflowActionV1::CreateChart { input, save } => {
                self.workflow_intent(AppIntent::BeginNewChart).await?;
                let instance_id = match self
                    .model
                    .get_untracked()
                    .chart_editor
                    .as_ref()
                    .map(|editor| &editor.target)
                {
                    Some(mirabile_app::ChartEditorTarget::New { instance_id }) => *instance_id,
                    _ => {
                        return Err("New chart editor did not expose its stable instance ID".into());
                    }
                };
                for intent in input.intents().map_err(|error| error.message)? {
                    self.workflow_intent(intent).await?;
                }
                if *save {
                    self.workflow_intent(AppIntent::SaveChartEditor).await?;
                }
                Ok(WorkflowBindingValue::Chart(instance_id))
            }
            WorkflowActionV1::EditChart { chart, patch, save } => {
                let chart = resolve_chart(chart, bindings)?;
                self.workflow_intent(AppIntent::BeginSavedChartEdit { instance_id: chart })
                    .await?;
                for intent in chart_patch_intents(patch, &self.model.get_untracked())? {
                    self.workflow_intent(intent).await?;
                }
                if *save {
                    self.workflow_intent(AppIntent::SaveChartEditor).await?;
                }
                Ok(WorkflowBindingValue::Chart(chart))
            }
            WorkflowActionV1::CreateBiwheelView {
                title,
                radix,
                comparison,
            } => {
                let radix = resolve_chart(radix, bindings)?;
                let comparison = resolve_chart(comparison, bindings)?;
                self.workflow_intent(AppIntent::CreateWheelView {
                    title: title.clone(),
                    radix,
                    comparison: Some(comparison),
                })
                .await?;
                self.model
                    .get_untracked()
                    .workspace
                    .active_view
                    .map(WorkflowBindingValue::View)
                    .ok_or_else(|| "Created view was not activated".into())
            }
            WorkflowActionV1::ConfigureViewDisplay { view, patch } => {
                let view = resolve_view(view, bindings)?;
                self.workflow_intent(AppIntent::ApplyViewDisplayPatch {
                    view_id: view,
                    patch: patch.clone(),
                })
                .await?;
                Ok(WorkflowBindingValue::View(view))
            }
            WorkflowActionV1::SaveWorkspace {
                title,
                description,
                tags,
            } => {
                let model = self.model.get_untracked();
                if model.workspace.title != *title {
                    self.workflow_intent(AppIntent::RenameWorkspace {
                        title: title.clone(),
                    })
                    .await?;
                }
                if model.workspace.description != *description {
                    self.workflow_intent(AppIntent::SetWorkspaceDescription {
                        description: description.clone(),
                    })
                    .await?;
                }
                if model.workspace.tags != *tags {
                    self.workflow_intent(AppIntent::SetWorkspaceTags { tags: tags.clone() })
                        .await?;
                }
                self.workflow_intent(AppIntent::SaveWorkspace).await?;
                self.model
                    .get_untracked()
                    .workspace
                    .document_id
                    .map(WorkflowBindingValue::Workspace)
                    .ok_or_else(|| "Workspace save completed without a resource ID".into())
            }
            WorkflowActionV1::OpenWorkspace {
                workspace,
                dirty_policy,
            } => {
                let workspace = resolve_workspace(workspace, bindings)?;
                if self.model.get_untracked().workspace.document_dirty {
                    match dirty_policy {
                        mirabile_app::DirtyPolicyV1::Reject => {
                            return Err(
                                "Workspace has unsaved changes and dirty_policy is reject".into()
                            );
                        }
                        mirabile_app::DirtyPolicyV1::Save => {
                            self.workflow_intent(AppIntent::SaveWorkspace).await?
                        }
                        mirabile_app::DirtyPolicyV1::Discard => {
                            self.workflow_intent(AppIntent::DiscardWorkspaceChanges)
                                .await?
                        }
                    }
                }
                self.workflow_intent(AppIntent::OpenWorkspace {
                    resource_id: workspace,
                })
                .await?;
                Ok(WorkflowBindingValue::Workspace(workspace))
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    async fn workflow_intent(&self, intent: AppIntent) -> Result<(), String> {
        let outcome = self
            .execute_action(QueuedAction {
                intent,
                source: ActionSource::Agent,
                origin_control: None,
            })
            .await;
        match outcome {
            ExecutionOutcome::Settled => Ok(()),
            ExecutionOutcome::Rejected { message, .. }
            | ExecutionOutcome::Failed { message, .. } => Err(message),
        }
    }

    fn fail_macro(&self, step: usize, message: String, origin: Option<ControlAddress>) {
        self.running.set(false);
        self.coordinator.update(|state| {
            state.running = false;
            state.current_source = None;
            state.highlighted_control = origin;
            state.macro_state = MacroCoordinatorState::Failed { step, message };
        });
        if !self.queue.borrow().is_empty() {
            self.running.set(true);
            let state = self.clone();
            leptos::task::spawn_local(async move {
                state.drain_queue().await;
            });
        }
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
                trace: RwSignal::new(TraceHistory::default()),
                recorder: RwSignal::new(None),
                macro_document: RwSignal::new(None),
                #[cfg(target_arch = "wasm32")]
                workflow_result: RwSignal::new(None),
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

    pub(super) fn read_model_tracked(self) -> CoordinatorReadModel {
        self.stored
            .with_value(|coordinator| coordinator.coordinator.get())
    }

    pub(super) fn trace(self) -> Vec<ExecutionTraceEntry> {
        self.stored
            .with_value(|coordinator| coordinator.trace.get_untracked().entries())
    }

    pub(super) fn trace_tracked(self) -> Vec<ExecutionTraceEntry> {
        self.stored
            .with_value(|coordinator| coordinator.trace.get().entries())
    }

    pub(super) fn start_macro_recording(self, name: String) -> Result<(), MacroError> {
        let recorder = MacroRecorder::new(name)?;
        self.stored.with_value(|coordinator| {
            if coordinator.running.get() {
                return Err(MacroError::InvalidValue(
                    "cannot begin recording while the coordinator is running".into(),
                ));
            }
            coordinator.recorder.set(Some(recorder));
            coordinator.coordinator.update(|state| {
                state.macro_state = MacroCoordinatorState::Recording;
            });
            Ok(())
        })
    }

    pub(super) fn stop_macro_recording(self) -> Result<MacroDocumentV1, MacroError> {
        self.stored.with_value(|coordinator| {
            let recorder = coordinator
                .recorder
                .get_untracked()
                .ok_or(MacroError::InvalidValue(
                    "macro recording is not active".into(),
                ))?;
            let document = recorder.finish()?;
            coordinator.recorder.set(None);
            coordinator.macro_document.set(Some(document.clone()));
            coordinator.coordinator.update(|state| {
                state.macro_state = MacroCoordinatorState::Idle;
            });
            Ok(document)
        })
    }

    pub(super) fn import_macro(self, document: MacroDocumentV1) -> Result<(), MacroError> {
        document.validate()?;
        self.stored.with_value(|coordinator| {
            coordinator.macro_document.set(Some(document));
            coordinator.coordinator.update(|state| {
                state.macro_state = MacroCoordinatorState::Idle;
            });
        });
        Ok(())
    }

    pub(super) fn macro_document(self) -> Option<MacroDocumentV1> {
        self.stored
            .with_value(|coordinator| coordinator.macro_document.get_untracked())
    }

    pub(super) fn clear_macro(self) {
        self.stored.with_value(|coordinator| {
            coordinator.recorder.set(None);
            coordinator.macro_document.set(None);
            coordinator.coordinator.update(|state| {
                state.macro_state = MacroCoordinatorState::Idle;
                state.highlighted_control = None;
            });
        });
    }

    pub(super) fn replay_macro(self, document: MacroDocumentV1) {
        self.stored
            .with_value(|coordinator| coordinator.replay(document));
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn execute_workflow(self, document: WorkflowDocumentV1) {
        self.stored
            .with_value(|coordinator| coordinator.execute_workflow(document));
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn workflow_result(self) -> Option<WorkflowResultV1> {
        self.stored
            .with_value(|coordinator| coordinator.workflow_result.get_untracked())
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn reject_workflow(self, errors: Vec<WorkflowValidationError>) {
        self.stored.with_value(|coordinator| {
            coordinator.workflow_result.set(Some(WorkflowResultV1 {
                schema_version: mirabile_app::WORKFLOW_DOCUMENT_VERSION,
                status: WorkflowExecutionStatusV1::Failed,
                failed_step: errors.first().and_then(|error| error.step.clone()),
                errors,
                final_projection: Some(coordinator.model.get_untracked().version),
                created_chart_ids: Vec::new(),
                created_definition_ids: Vec::new(),
                created_view_ids: Vec::new(),
                created_workspace_ids: Vec::new(),
            }));
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn workflow_error(
    step: Option<&str>,
    field: &str,
    code: &str,
    message: impl Into<String>,
) -> WorkflowValidationError {
    WorkflowValidationError {
        step: step.map(str::to_owned),
        field: field.into(),
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(target_arch = "wasm32")]
fn resolve_chart(
    value: &mirabile_app::ChartReferenceV1,
    bindings: &std::collections::BTreeMap<String, WorkflowBindingValue>,
) -> Result<mirabile_app::InstanceId, String> {
    match value {
        mirabile_app::ChartReferenceV1::Id(id) => Ok(*id),
        mirabile_app::ChartReferenceV1::Binding(name) => match bindings.get(name) {
            Some(WorkflowBindingValue::Chart(id)) => Ok(*id),
            _ => Err(format!("Chart binding {name} was not resolved")),
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn resolve_view(
    value: &mirabile_app::ViewReferenceV1,
    bindings: &std::collections::BTreeMap<String, WorkflowBindingValue>,
) -> Result<mirabile_app::ViewInstanceId, String> {
    match value {
        mirabile_app::ViewReferenceV1::Id(id) => Ok(*id),
        mirabile_app::ViewReferenceV1::Binding(name) => match bindings.get(name) {
            Some(WorkflowBindingValue::View(id)) => Ok(*id),
            _ => Err(format!("View binding {name} was not resolved")),
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn resolve_workspace(
    value: &mirabile_app::WorkspaceReferenceV1,
    bindings: &std::collections::BTreeMap<String, WorkflowBindingValue>,
) -> Result<mirabile_app::ResourceId, String> {
    match value {
        mirabile_app::WorkspaceReferenceV1::Id(id) => Ok(*id),
        mirabile_app::WorkspaceReferenceV1::Binding(name) => match bindings.get(name) {
            Some(WorkflowBindingValue::Workspace(id)) => Ok(*id),
            _ => Err(format!("Workspace binding {name} was not resolved")),
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn chart_patch_intents(
    patch: &mirabile_app::ChartInputPatchV1,
    model: &AppReadModel,
) -> Result<Vec<AppIntent>, String> {
    use mirabile_app::{ChartMutation, ChartTimezone};
    let mut intents = Vec::new();
    if let Some(value) = &patch.title {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetTitle(
            value.clone(),
        )));
    }
    if let Some(value) = &patch.event_kind {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetEventKind(
            value.clone(),
        )));
    }
    if let Some(value) = &patch.subject {
        intents.push(AppIntent::ApplyChartMutation(
            ChartMutation::SetSubjectName(value.clone()),
        ));
    }
    if let Some(value) = patch.date {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetCivilDate(
            value,
        )));
    }
    if let Some(value) = patch.time {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetCivilTime(
            value,
        )));
    }
    if let Some(value) = &patch.timezone {
        let timezone = match value {
            mirabile_app::WorkflowTimezoneV1::Utc => ChartTimezone::UniversalTime,
            mirabile_app::WorkflowTimezoneV1::FixedOffset { seconds } => {
                ChartTimezone::FixedOffset(
                    mirabile_app::Offset::from_seconds(*seconds)
                        .map_err(|error| error.to_string())?,
                )
            }
        };
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetTimezone(
            timezone,
        )));
    }
    if let Some(value) = &patch.place_label {
        intents.push(AppIntent::ApplyChartMutation(
            ChartMutation::SetLocationName(value.clone()),
        ));
    }
    if let Some(value) = &patch.country {
        intents.push(AppIntent::ApplyChartMutation(
            ChartMutation::SetCountryRegion(value.clone()),
        ));
    }
    if let Some(value) = patch.latitude {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetLatitude(
            Some(value),
        )));
    }
    if let Some(value) = patch.longitude {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetLongitude(
            Some(value),
        )));
    }
    if let Some(value) = &patch.zodiac {
        intents.push(AppIntent::ApplyChartMutation(ChartMutation::SetZodiac(
            value.clone(),
        )));
    }
    if let Some(value) = patch.houses {
        intents.push(AppIntent::ApplyChartMutation(
            ChartMutation::SetHouseSystem(value),
        ));
    }
    if let Some(value) = patch.coordinates {
        intents.push(AppIntent::ApplyChartMutation(
            ChartMutation::SetCoordinateSystem(value),
        ));
    }
    if let Some(value) = &patch.corrections {
        let mut calculation = model
            .chart_editor
            .as_ref()
            .ok_or_else(|| "Chart editor is unavailable".to_owned())?
            .fields
            .calculation
            .clone();
        calculation.corrections = value.clone();
        intents.push(AppIntent::ApplyChartMutation(
            ChartMutation::SetCalculation(calculation),
        ));
    }
    Ok(intents)
}

fn outcome_message(outcome: &ExecutionOutcome) -> String {
    match outcome {
        ExecutionOutcome::Settled => "settled".into(),
        ExecutionOutcome::Rejected { message, .. } | ExecutionOutcome::Failed { message, .. } => {
            message.clone()
        }
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
    invalid_aspect_buffers: RwSignal<std::collections::BTreeSet<String>>,
) {
    match command {
        CommandId::SaveDraft => {
            if model
                .get_untracked()
                .availability(AppAction::SaveDraft)
                .is_enabled()
                && invalid_aspect_buffers.get_untracked().is_empty()
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
                reset_aspect_buffers(invalid_aspect_buffers);
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

pub(super) fn reset_aspect_buffers(
    invalid_aspect_buffers: RwSignal<std::collections::BTreeSet<String>>,
) {
    invalid_aspect_buffers.set(std::collections::BTreeSet::new());
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
