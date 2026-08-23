use std::{cell::RefCell, collections::BTreeMap, rc::Rc, str::FromStr};

use async_trait::async_trait;
use mirabile_app::{
    ActiveChartInspector, Angle, AppAction, AppError, AppErrorKind, AppIntent, AppNotice,
    AppNoticeKind, AppReadModel, AppResult, Application, ApplicationStatus, AspectDraftValue,
    AspectId, AspectSetDraftMutation, AspectSetDraftReadModel, AspectSetSummary, Availability,
    BindingSourceSummary, ChartPersistence, ChartSlotAssignment, ChartSlotId, Circle,
    CommandCapability, DraftState, FillRole, InspectorReadModel, InstanceId, Label,
    LibraryChartSummary, LibraryReadModel, Line, OpenChartSummary, ProjectionVersion,
    ResourceBindingSummary, ResourceEditorReadModel, ResourceId, Revision, Scene, StrokeRole,
    ViewComputationState, ViewInstanceId, ViewReadModel, ViewSummary, WorkspaceReadModel,
};

const CHART_DEFINITION_IDS: [&str; 5] = [
    "10000000-0000-4000-8000-000000000001",
    "10000000-0000-4000-8000-000000000002",
    "10000000-0000-4000-8000-000000000003",
    "10000000-0000-4000-8000-000000000004",
    "10000000-0000-4000-8000-000000000005",
];
const INSTANCE_IDS: [&str; 6] = [
    "20000000-0000-4000-8000-000000000001",
    "20000000-0000-4000-8000-000000000002",
    "20000000-0000-4000-8000-000000000003",
    "20000000-0000-4000-8000-000000000004",
    "20000000-0000-4000-8000-000000000005",
    "20000000-0000-4000-8000-000000000006",
];
const VIEW_IDS: [&str; 2] = [
    "30000000-0000-4000-8000-000000000001",
    "30000000-0000-4000-8000-000000000002",
];
const ASPECT_SET_IDS: [&str; 3] = [
    "40000000-0000-4000-8000-000000000001",
    "40000000-0000-4000-8000-000000000002",
    "40000000-0000-4000-8000-000000000003",
];

#[derive(Clone)]
pub struct MockApplication {
    state: Rc<RefCell<MockState>>,
}

impl MockApplication {
    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(MockState::fixture(None))),
        }
    }

    #[cfg(test)]
    fn failing_initialization_once(error: AppError) -> Self {
        Self {
            state: Rc::new(RefCell::new(MockState::fixture(Some(error)))),
        }
    }
}

impl Default for MockApplication {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait(?Send)]
impl Application for MockApplication {
    async fn initialize(&self) -> AppResult<AppReadModel> {
        let mut state = self.state.borrow_mut();
        let next_version = state.next_version()?;
        if let Some(error) = state.initialization_failure.take() {
            state.status = ApplicationStatus::Error(error);
            state.notice = None;
            state.version = next_version;
            return Ok(state.read_model());
        }

        state.status = ApplicationStatus::Ready;
        state.notice = Some(AppNotice {
            kind: AppNoticeKind::Info,
            message: "Mock library ready; calculating the first view".into(),
        });
        let active_view = state.active_view;
        let view = state
            .views
            .iter_mut()
            .find(|view| view.view_id == active_view)
            .expect("fixture active view exists");
        view.scene = None;
        view.computation = ViewComputationState::Loading;
        state.pending = Some(PendingWork::InitialView);
        state.version = next_version;
        Ok(state.read_model())
    }

    async fn dispatch(&self, intent: AppIntent) -> AppResult<AppReadModel> {
        let mut state = self.state.borrow_mut();
        if !matches!(state.status, ApplicationStatus::Ready) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The application must be ready before it can accept commands",
            ));
        }
        let next_version = state.next_version()?;
        let prior_notice = state.notice.clone();
        if let Err(error) = state.apply(intent) {
            state.notice = prior_notice;
            return Err(error);
        }
        state.version = next_version;
        Ok(state.read_model())
    }

    async fn snapshot(&self) -> AppResult<AppReadModel> {
        Ok(self.state.borrow().read_model())
    }

    async fn wait_for_update(&self, after: ProjectionVersion) -> AppResult<AppReadModel> {
        let mut state = self.state.borrow_mut();
        if state.version > after {
            return Ok(state.read_model());
        }
        if state.version < after {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                format!(
                    "Cannot wait after future projection {after}; current projection is {}",
                    state.version
                ),
            ));
        }
        if state.pending.is_none() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The deterministic mock has no authoritative update queued",
            ));
        }
        let next_version = state.next_version()?;
        state.complete_pending()?;
        state.version = next_version;
        Ok(state.read_model())
    }
}

struct MockState {
    version: ProjectionVersion,
    status: ApplicationStatus,
    library_charts: Vec<LibraryChartSummary>,
    aspect_sets: Vec<AspectSetFixture>,
    charts: Vec<OpenChartSummary>,
    active_chart: Option<InstanceId>,
    selected_charts: Vec<InstanceId>,
    views: Vec<MockView>,
    active_view: ViewInstanceId,
    active_aspect_set: ResourceId,
    editor: Option<EditorFixture>,
    pending: Option<PendingWork>,
    notice: Option<AppNotice>,
    initialization_failure: Option<AppError>,
    fail_next_manual_refresh: bool,
    conflict_wide_once: bool,
    scene_generation: u32,
}

struct AspectSetFixture {
    summary: AspectSetSummary,
    conjunction_enabled: bool,
}

struct EditorFixture {
    resource_id: ResourceId,
    title: String,
    state: DraftState,
    conjunction_enabled: bool,
    conjunction_orb: Angle,
}

struct MockView {
    view_id: ViewInstanceId,
    title: String,
    scene: Option<Scene>,
    computation: ViewComputationState,
    slots: Vec<ChartSlotAssignment>,
}

enum PendingWork {
    InitialView,
    Refresh {
        outcome: AppResult<Scene>,
    },
    Save {
        resource_id: ResourceId,
        conflict: bool,
    },
}

impl MockState {
    #[allow(clippy::too_many_lines)]
    fn fixture(initialization_failure: Option<AppError>) -> Self {
        let chart_definitions = CHART_DEFINITION_IDS.map(parse_resource);
        let instances = INSTANCE_IDS.map(parse_instance);
        let views = VIEW_IDS.map(parse_view);
        let aspect_sets = ASPECT_SET_IDS.map(parse_resource);
        let radix = ChartSlotId::new("radix").expect("fixture slot is valid");
        let outer = ChartSlotId::new("outer").expect("fixture slot is valid");

        let library_charts = vec![
            library_chart(
                chart_definitions[0],
                "Mara Ellison",
                "Natal · Portland, Maine",
            ),
            library_chart(
                chart_definitions[1],
                "Harbor Launch",
                "Event · Baltimore, Maryland",
            ),
            library_chart(
                chart_definitions[2],
                "August Ingress",
                "Event · Washington, D.C.",
            ),
            library_chart(
                chart_definitions[3],
                "Ada Lovelace",
                "Natal · London, England",
            ),
            library_chart(
                chart_definitions[4],
                "Lunar Eclipse",
                "Event · Reykjavík, Iceland",
            ),
        ];
        let charts = vec![
            open_saved(&library_charts[0], instances[0]),
            open_saved(&library_charts[1], instances[1]),
            OpenChartSummary {
                instance_id: instances[2],
                title: "Rectification study".into(),
                subtitle: "Unsaved working chart · 14:32?".into(),
                persistence: ChartPersistence::Ephemeral,
            },
            open_saved(&library_charts[2], instances[3]),
        ];
        let standard_orb = angle(8.0);
        let aspect_set_fixtures = vec![
            aspect_fixture(aspect_sets[0], "Standard", 1, standard_orb),
            aspect_fixture(aspect_sets[1], "Tight", 3, angle(4.0)),
            aspect_fixture(aspect_sets[2], "Wide", 2, angle(10.0)),
        ];
        let mock_views = vec![
            MockView {
                view_id: views[0],
                title: "Single wheel".into(),
                scene: Some(mock_scene(0, standard_orb)),
                computation: ViewComputationState::Fresh,
                slots: vec![ChartSlotAssignment {
                    slot: radix.clone(),
                    label: "Radix".into(),
                    required: true,
                    chart: Some(instances[1]),
                }],
            },
            MockView {
                view_id: views[1],
                title: "Two-chart comparison".into(),
                scene: None,
                computation: ViewComputationState::Loading,
                slots: vec![
                    ChartSlotAssignment {
                        slot: radix,
                        label: "Radix".into(),
                        required: true,
                        chart: Some(instances[1]),
                    },
                    ChartSlotAssignment {
                        slot: outer,
                        label: "Outer wheel".into(),
                        required: false,
                        chart: Some(instances[0]),
                    },
                ],
            },
        ];

        Self {
            version: ProjectionVersion::INITIAL,
            status: ApplicationStatus::Initializing,
            library_charts,
            aspect_sets: aspect_set_fixtures,
            charts,
            active_chart: Some(instances[1]),
            selected_charts: vec![instances[0], instances[2]],
            views: mock_views,
            active_view: views[1],
            active_aspect_set: aspect_sets[0],
            editor: None,
            pending: None,
            notice: None,
            initialization_failure,
            fail_next_manual_refresh: true,
            conflict_wide_once: true,
            scene_generation: 1,
        }
    }

    fn read_model(&self) -> AppReadModel {
        if !matches!(self.status, ApplicationStatus::Ready) {
            let mut model = AppReadModel::initializing();
            model.version = self.version;
            model.status = self.status.clone();
            model.notice.clone_from(&self.notice);
            return model;
        }

        let active_chart = self.active_chart.and_then(|active_id| {
            self.charts
                .iter()
                .find(|chart| chart.instance_id == active_id)
                .map(|chart| ActiveChartInspector {
                    instance_id: chart.instance_id,
                    title: chart.title.clone(),
                    subtitle: chart.subtitle.clone(),
                    persistence: chart.persistence.clone(),
                })
        });
        let aspect_set = self
            .aspect_sets
            .iter()
            .find(|aspect_set| aspect_set.summary.resource_id == self.active_aspect_set)
            .expect("fixture aspect set exists");
        let active_view = self
            .views
            .iter()
            .find(|view| view.view_id == self.active_view)
            .expect("fixture active view exists");

        AppReadModel {
            version: self.version,
            status: self.status.clone(),
            library: LibraryReadModel {
                charts: self.library_charts.clone(),
                aspect_sets: self
                    .aspect_sets
                    .iter()
                    .map(|fixture| fixture.summary.clone())
                    .collect(),
            },
            workspace: WorkspaceReadModel {
                charts: self.charts.clone(),
                active_chart: self.active_chart,
                selected_charts: self.selected_charts.clone(),
                views: self
                    .views
                    .iter()
                    .map(|view| ViewSummary {
                        view_id: view.view_id,
                        title: view.title.clone(),
                    })
                    .collect(),
                active_view: Some(self.active_view),
            },
            active_view: Some(ViewReadModel {
                view_id: active_view.view_id,
                title: active_view.title.clone(),
                scene: active_view.scene.clone(),
                computation: active_view.computation.clone(),
                slots: active_view.slots.clone(),
            }),
            inspector: InspectorReadModel {
                active_chart,
                bindings: vec![ResourceBindingSummary {
                    label: "Aspect set".into(),
                    source: BindingSourceSummary::Follow {
                        resource_id: aspect_set.summary.resource_id,
                        resource_title: aspect_set.summary.title.clone(),
                        revision: aspect_set.summary.revision,
                    },
                }],
                active_aspect_set: Some(self.active_aspect_set),
            },
            resource_editor: ResourceEditorReadModel {
                aspect_set: self.editor.as_ref().map(|editor| AspectSetDraftReadModel {
                    resource_id: editor.resource_id,
                    title: editor.title.clone(),
                    state: editor.state.clone(),
                    conjunction: AspectDraftValue {
                        aspect_id: conjunction_id(),
                        label: "Conjunction".into(),
                        enabled: editor.conjunction_enabled,
                        maximum_orb: editor.conjunction_orb,
                    },
                }),
            },
            capabilities: self.capabilities(),
            notice: self.notice.clone(),
        }
    }

    fn next_version(&self) -> AppResult<ProjectionVersion> {
        self.version.checked_next().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Unavailable,
                "Application projection version overflowed",
            )
        })
    }

    fn capabilities(&self) -> Vec<CommandCapability> {
        let (save, cancel) = match self.editor.as_ref().map(|editor| &editor.state) {
            None => (
                disabled("Begin an Aspect Set edit before saving"),
                disabled("There is no draft to cancel"),
            ),
            Some(DraftState::Clean { .. }) => (
                disabled("The draft has no changes"),
                disabled("The draft has no changes"),
            ),
            Some(DraftState::Dirty { .. }) => (Availability::Enabled, Availability::Enabled),
            Some(DraftState::Saving { .. }) => (
                disabled("The draft is currently saving"),
                disabled("Wait for the save to finish"),
            ),
            Some(DraftState::Conflict { .. }) => (
                disabled("Resolve or cancel the revision conflict before saving"),
                Availability::Enabled,
            ),
        };
        let refresh = self
            .views
            .iter()
            .find(|view| view.view_id == self.active_view)
            .map_or_else(
                || disabled("No active view"),
                |view| match view.computation {
                    ViewComputationState::Loading | ViewComputationState::Refreshing => {
                        disabled("The active view is already computing")
                    }
                    ViewComputationState::Fresh | ViewComputationState::Failed(_) => {
                        Availability::Enabled
                    }
                },
            );

        vec![
            capability(AppAction::BeginAspectSetEdit, Availability::Enabled),
            capability(AppAction::SaveDraft, save),
            capability(AppAction::CancelDraft, cancel),
            capability(AppAction::RefreshView, refresh),
        ]
    }

    #[allow(clippy::too_many_lines)]
    fn apply(&mut self, intent: AppIntent) -> AppResult<()> {
        self.notice = None;
        match intent {
            AppIntent::OpenChart { definition_id } => self.open_chart(definition_id),
            AppIntent::CloseChart { instance_id } => self.close_chart(instance_id),
            AppIntent::ActivateChart { instance_id } => {
                self.ensure_open(instance_id)?;
                self.active_chart = Some(instance_id);
                self.notice = Some(info("Active chart changed; selection was preserved"));
                Ok(())
            }
            AppIntent::SetChartSelection {
                instance_id,
                selected,
            } => {
                self.ensure_open(instance_id)?;
                if selected && !self.selected_charts.contains(&instance_id) {
                    self.selected_charts.push(instance_id);
                } else if !selected {
                    self.selected_charts.retain(|id| *id != instance_id);
                }
                self.notice = Some(info("Chart selection changed independently of activation"));
                Ok(())
            }
            AppIntent::SetActiveView { view_id } => {
                if !self.views.iter().any(|view| view.view_id == view_id) {
                    return Err(not_found("view"));
                }
                self.active_view = view_id;
                self.notice = Some(info("Active view changed"));
                Ok(())
            }
            AppIntent::AssignChartSlot {
                view_id,
                slot,
                chart,
            } => {
                if let Some(chart) = chart {
                    self.ensure_open(chart)?;
                }
                let view = self
                    .views
                    .iter_mut()
                    .find(|view| view.view_id == view_id)
                    .ok_or_else(|| not_found("view"))?;
                let assignment = view
                    .slots
                    .iter_mut()
                    .find(|assignment| assignment.slot == slot)
                    .ok_or_else(|| not_found("chart slot"))?;
                if assignment.required && chart.is_none() {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "A required chart slot cannot be cleared",
                    ));
                }
                assignment.chart = chart;
                self.notice = Some(info("Chart slot assignment updated"));
                Ok(())
            }
            AppIntent::SetWorkspaceAspectSet { resource_id } => {
                self.aspect_fixture(resource_id)?;
                self.active_aspect_set = resource_id;
                self.editor = None;
                self.queue_refresh(Ok(()));
                self.notice = Some(info("Aspect Set changed; refreshing analysis"));
                Ok(())
            }
            AppIntent::BeginAspectSetEdit { resource_id } => {
                let aspect_set = self.aspect_fixture(resource_id)?;
                self.editor = Some(EditorFixture {
                    resource_id,
                    title: aspect_set.summary.title.clone(),
                    state: DraftState::Clean {
                        revision: aspect_set.summary.revision,
                    },
                    conjunction_enabled: aspect_set.conjunction_enabled,
                    conjunction_orb: aspect_set.summary.conjunction_orb,
                });
                self.notice = Some(info("Aspect Set draft opened"));
                Ok(())
            }
            AppIntent::UpdateAspectSetDraft(mutation) => self.update_draft(mutation),
            AppIntent::SaveDraft => self.begin_save(),
            AppIntent::CancelDraft => self.cancel_draft(),
            AppIntent::RefreshActiveView => {
                let outcome = if self.fail_next_manual_refresh {
                    self.fail_next_manual_refresh = false;
                    Err(AppError::new(
                        AppErrorKind::ViewComputation,
                        "Mock worker rejected this refresh; the last good Scene is still shown",
                    ))
                } else {
                    Ok(())
                };
                self.queue_refresh(outcome);
                self.notice = Some(info("View refresh requested"));
                Ok(())
            }
        }
    }

    fn complete_pending(&mut self) -> AppResult<()> {
        let Some(pending) = self.pending.take() else {
            return Ok(());
        };
        match pending {
            PendingWork::InitialView => {
                let orb = self.effective_orb();
                let scene = mock_scene(self.scene_generation, orb);
                let view = self.active_view_mut()?;
                view.scene = Some(scene);
                view.computation = ViewComputationState::Fresh;
                self.notice = Some(success("Initial mock Scene ready"));
            }
            PendingWork::Refresh { outcome } => {
                let view = self.active_view_mut()?;
                match outcome {
                    Ok(scene) => {
                        view.scene = Some(scene);
                        view.computation = ViewComputationState::Fresh;
                        self.notice = Some(success("View refresh complete"));
                    }
                    Err(error) => {
                        view.computation = ViewComputationState::Failed(error.clone());
                        self.notice = Some(AppNotice {
                            kind: AppNoticeKind::Warning,
                            message: error.message,
                        });
                    }
                }
            }
            PendingWork::Save {
                resource_id,
                conflict,
            } => {
                if conflict {
                    self.complete_conflict(resource_id)?;
                } else {
                    self.complete_save(resource_id)?;
                }
            }
        }
        Ok(())
    }

    fn open_chart(&mut self, definition_id: ResourceId) -> AppResult<()> {
        if let Some(existing) = self.charts.iter().find(|chart| {
            matches!(chart.persistence, ChartPersistence::Saved { definition_id: id } if id == definition_id)
        }) {
            self.active_chart = Some(existing.instance_id);
            self.notice = Some(info("The chart was already open; it is now active"));
            return Ok(());
        }
        let library = self
            .library_charts
            .iter()
            .find(|chart| chart.definition_id == definition_id)
            .ok_or_else(|| not_found("library chart"))?;
        let instance_id = match definition_id {
            id if id == parse_resource(CHART_DEFINITION_IDS[3]) => parse_instance(INSTANCE_IDS[4]),
            id if id == parse_resource(CHART_DEFINITION_IDS[4]) => parse_instance(INSTANCE_IDS[5]),
            _ => {
                return Err(AppError::new(
                    AppErrorKind::Unavailable,
                    "This mock fixture chart has no unopened instance",
                ));
            }
        };
        self.charts.push(open_saved(library, instance_id));
        self.active_chart = Some(instance_id);
        self.notice = Some(success(
            "Chart opened and activated; selection was preserved",
        ));
        Ok(())
    }

    fn close_chart(&mut self, instance_id: InstanceId) -> AppResult<()> {
        let index = self
            .charts
            .iter()
            .position(|chart| chart.instance_id == instance_id)
            .ok_or_else(|| not_found("open chart"))?;
        let was_active = self.active_chart == Some(instance_id);
        self.charts.remove(index);
        self.selected_charts.retain(|id| *id != instance_id);
        if was_active {
            self.active_chart = self
                .charts
                .get(index)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|prior| self.charts.get(prior))
                })
                .map(|chart| chart.instance_id);
        }
        let required_slot_replacement = self.active_chart;
        for view in &mut self.views {
            for assignment in &mut view.slots {
                if assignment.chart == Some(instance_id) {
                    assignment.chart = if assignment.required {
                        required_slot_replacement
                    } else {
                        None
                    };
                }
            }
        }
        self.notice = Some(info(
            "Chart closed; a neighboring chart was activated without changing other selections",
        ));
        Ok(())
    }

    fn update_draft(&mut self, mutation: AspectSetDraftMutation) -> AppResult<()> {
        let editor = self.editor.as_mut().ok_or_else(|| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                "Begin an Aspect Set edit before updating the draft",
            )
        })?;
        if matches!(editor.state, DraftState::Saving { .. }) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The draft cannot change while it is saving",
            ));
        }
        let base_revision = editor.state.base_revision();
        match mutation {
            AspectSetDraftMutation::SetOrb { aspect_id, maximum } => {
                if aspect_id != conjunction_id() {
                    return Err(not_found("draft aspect"));
                }
                editor.conjunction_orb = maximum;
            }
            AspectSetDraftMutation::SetEnabled { aspect_id, enabled } => {
                if aspect_id != conjunction_id() {
                    return Err(not_found("draft aspect"));
                }
                editor.conjunction_enabled = enabled;
            }
        }
        if !matches!(editor.state, DraftState::Conflict { .. }) {
            editor.state = DraftState::Dirty { base_revision };
        }
        self.queue_refresh(Ok(()));
        self.notice = Some(info(
            "Typed draft mutation accepted; analysis is refreshing with the prior Scene retained",
        ));
        Ok(())
    }

    fn begin_save(&mut self) -> AppResult<()> {
        let editor = self.editor.as_mut().ok_or_else(|| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                "There is no open draft to save",
            )
        })?;
        let DraftState::Dirty { base_revision } = editor.state else {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "Only a dirty draft can be saved",
            ));
        };
        editor.state = DraftState::Saving { base_revision };
        let resource_id = editor.resource_id;
        let conflict = resource_id == parse_resource(ASPECT_SET_IDS[2]) && self.conflict_wide_once;
        if conflict {
            self.conflict_wide_once = false;
        }
        self.pending = Some(PendingWork::Save {
            resource_id,
            conflict,
        });
        self.notice = Some(info("Saving Aspect Set draft"));
        Ok(())
    }

    fn cancel_draft(&mut self) -> AppResult<()> {
        let resource_id = self
            .editor
            .as_ref()
            .ok_or_else(|| {
                AppError::new(AppErrorKind::InvalidIntent, "There is no draft to cancel")
            })?
            .resource_id;
        let aspect_set = self.aspect_fixture(resource_id)?;
        let revision = aspect_set.summary.revision;
        let conjunction_orb = aspect_set.summary.conjunction_orb;
        let conjunction_enabled = aspect_set.conjunction_enabled;
        let editor = self.editor.as_mut().expect("editor was checked");
        editor.state = DraftState::Clean { revision };
        editor.conjunction_orb = conjunction_orb;
        editor.conjunction_enabled = conjunction_enabled;
        self.queue_refresh(Ok(()));
        self.notice = Some(info("Draft canceled; canonical values restored"));
        Ok(())
    }

    fn complete_save(&mut self, resource_id: ResourceId) -> AppResult<()> {
        let editor = self
            .editor
            .as_ref()
            .filter(|editor| editor.resource_id == resource_id)
            .ok_or_else(|| not_found("saving draft"))?;
        let conjunction_orb = editor.conjunction_orb;
        let conjunction_enabled = editor.conjunction_enabled;
        let next_revision = self
            .aspect_fixture(resource_id)?
            .summary
            .revision
            .next()
            .map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))?;
        let aspect_set = self.aspect_fixture_mut(resource_id)?;
        aspect_set.summary.revision = next_revision;
        aspect_set.summary.conjunction_orb = conjunction_orb;
        aspect_set.conjunction_enabled = conjunction_enabled;
        let editor = self.editor.as_mut().expect("editor was checked");
        editor.state = DraftState::Clean {
            revision: next_revision,
        };
        self.notice = Some(success(format!(
            "Aspect Set saved as revision {next_revision}"
        )));
        Ok(())
    }

    fn complete_conflict(&mut self, resource_id: ResourceId) -> AppResult<()> {
        let remote_revision = self
            .aspect_fixture(resource_id)?
            .summary
            .revision
            .next()
            .map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))?;
        self.aspect_fixture_mut(resource_id)?.summary.revision = remote_revision;
        let editor = self
            .editor
            .as_mut()
            .filter(|editor| editor.resource_id == resource_id)
            .ok_or_else(|| not_found("saving draft"))?;
        let base_revision = editor.state.base_revision();
        editor.state = DraftState::Conflict {
            base_revision,
            remote_revision,
        };
        self.notice = Some(AppNotice {
            kind: AppNoticeKind::Conflict,
            message: format!(
                "Save conflict: the library advanced to revision {remote_revision}; the local draft is retained"
            ),
        });
        Ok(())
    }

    fn queue_refresh(&mut self, outcome: AppResult<()>) {
        self.scene_generation = self.scene_generation.saturating_add(1);
        let scene = mock_scene(self.scene_generation, self.effective_orb());
        if let Ok(view) = self.active_view_mut() {
            view.computation = ViewComputationState::Refreshing;
        }
        self.pending = Some(PendingWork::Refresh {
            outcome: outcome.map(|()| scene),
        });
    }

    fn effective_orb(&self) -> Angle {
        self.editor.as_ref().map_or_else(
            || {
                self.aspect_sets
                    .iter()
                    .find(|aspect_set| aspect_set.summary.resource_id == self.active_aspect_set)
                    .expect("active aspect fixture exists")
                    .summary
                    .conjunction_orb
            },
            |editor| editor.conjunction_orb,
        )
    }

    fn active_view_mut(&mut self) -> AppResult<&mut MockView> {
        self.views
            .iter_mut()
            .find(|view| view.view_id == self.active_view)
            .ok_or_else(|| not_found("active view"))
    }

    fn ensure_open(&self, instance_id: InstanceId) -> AppResult<()> {
        if self
            .charts
            .iter()
            .any(|chart| chart.instance_id == instance_id)
        {
            Ok(())
        } else {
            Err(not_found("open chart"))
        }
    }

    fn aspect_fixture(&self, resource_id: ResourceId) -> AppResult<&AspectSetFixture> {
        self.aspect_sets
            .iter()
            .find(|aspect_set| aspect_set.summary.resource_id == resource_id)
            .ok_or_else(|| not_found("Aspect Set"))
    }

    fn aspect_fixture_mut(&mut self, resource_id: ResourceId) -> AppResult<&mut AspectSetFixture> {
        self.aspect_sets
            .iter_mut()
            .find(|aspect_set| aspect_set.summary.resource_id == resource_id)
            .ok_or_else(|| not_found("Aspect Set"))
    }
}

fn library_chart(definition_id: ResourceId, title: &str, subtitle: &str) -> LibraryChartSummary {
    LibraryChartSummary {
        definition_id,
        title: title.into(),
        subtitle: subtitle.into(),
    }
}

fn open_saved(chart: &LibraryChartSummary, instance_id: InstanceId) -> OpenChartSummary {
    OpenChartSummary {
        instance_id,
        title: chart.title.clone(),
        subtitle: chart.subtitle.clone(),
        persistence: ChartPersistence::Saved {
            definition_id: chart.definition_id,
        },
    }
}

fn aspect_fixture(
    resource_id: ResourceId,
    title: &str,
    revision: u64,
    conjunction_orb: Angle,
) -> AspectSetFixture {
    AspectSetFixture {
        summary: AspectSetSummary {
            resource_id,
            title: title.into(),
            revision: Revision::new(revision).expect("fixture revision is valid"),
            conjunction_orb,
        },
        conjunction_enabled: true,
    }
}

fn capability(action: AppAction, availability: Availability) -> CommandCapability {
    CommandCapability {
        action,
        availability,
    }
}

fn disabled(reason: &str) -> Availability {
    Availability::Disabled {
        reason: Some(reason.into()),
    }
}

fn info(message: impl Into<String>) -> AppNotice {
    AppNotice {
        kind: AppNoticeKind::Info,
        message: message.into(),
    }
}

fn success(message: impl Into<String>) -> AppNotice {
    AppNotice {
        kind: AppNoticeKind::Success,
        message: message.into(),
    }
}

fn not_found(noun: &str) -> AppError {
    AppError::new(AppErrorKind::NotFound, format!("Mock {noun} was not found"))
}

fn parse_resource(value: &str) -> ResourceId {
    ResourceId::from_str(value).expect("fixture resource ID is valid")
}

fn parse_instance(value: &str) -> InstanceId {
    InstanceId::from_str(value).expect("fixture instance ID is valid")
}

fn parse_view(value: &str) -> ViewInstanceId {
    ViewInstanceId::from_str(value).expect("fixture view ID is valid")
}

fn conjunction_id() -> AspectId {
    AspectId::new("conjunction").expect("fixture aspect ID is valid")
}

fn angle(degrees: f64) -> Angle {
    Angle::from_degrees(degrees).expect("fixture angle is valid")
}

fn mock_scene(generation: u32, orb: Angle) -> Scene {
    let center = 200.0;
    let outer = 168.0;
    let inner = 112.0;
    let mut scene = Scene::default();
    scene.circles.extend([
        Circle {
            cx: center,
            cy: center,
            radius: outer,
            stroke: StrokeRole::Foreground,
            fill: FillRole::Background,
        },
        Circle {
            cx: center,
            cy: center,
            radius: inner,
            stroke: StrokeRole::Muted,
            fill: FillRole::None,
        },
        Circle {
            cx: center,
            cy: center,
            radius: 76.0,
            stroke: StrokeRole::Muted,
            fill: FillRole::None,
        },
    ]);
    for index in 0..12 {
        let radians = (f64::from(index) * 30.0 - 90.0).to_radians();
        scene.lines.push(Line {
            x1: center + inner * radians.cos(),
            y1: center + inner * radians.sin(),
            x2: center + outer * radians.cos(),
            y2: center + outer * radians.sin(),
            stroke: StrokeRole::Muted,
            width: 1.0,
        });
    }

    let anchors = [
        ("Sun", 18.0),
        ("Moon", 74.0),
        ("Mercury", 127.0),
        ("Venus", 192.0),
        ("Mars", 244.0),
        ("Jupiter", 309.0),
    ];
    let mut positions = BTreeMap::new();
    for (label, degrees) in anchors {
        let adjusted = degrees + f64::from(generation % 4);
        let radians = (adjusted - 90.0).to_radians();
        let x = center + 142.0 * radians.cos();
        let y = center + 142.0 * radians.sin();
        positions.insert(label, (x, y));
        scene.circles.push(Circle {
            cx: x,
            cy: y,
            radius: 4.5,
            stroke: StrokeRole::Accent,
            fill: FillRole::Accent,
        });
        scene.labels.push(Label {
            text: label.into(),
            x: center + 184.0 * radians.cos(),
            y: center + 184.0 * radians.sin(),
            fill: FillRole::Foreground,
        });
    }

    let aspect_pairs = [
        ("Sun", "Moon"),
        ("Moon", "Venus"),
        ("Mercury", "Mars"),
        ("Venus", "Jupiter"),
        ("Sun", "Jupiter"),
        ("Moon", "Mars"),
    ];
    let aspect_count = match orb.degrees() {
        value if value < 6.0 => 2,
        value if value < 8.0 => 3,
        value if value < 10.0 => 4,
        value if value < 12.0 => 5,
        _ => aspect_pairs.len(),
    };
    for (lhs, rhs) in aspect_pairs.into_iter().take(aspect_count) {
        let (x1, y1) = positions[lhs];
        let (x2, y2) = positions[rhs];
        scene.lines.push(Line {
            x1,
            y1,
            x2,
            y2,
            stroke: StrokeRole::Aspect,
            width: 1.4,
        });
    }
    scene
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;

    fn ready(application: &MockApplication) -> AppReadModel {
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        assert_eq!(loading.status, ApplicationStatus::Ready);
        assert!(matches!(
            loading.active_view.as_ref().map(|view| &view.computation),
            Some(ViewComputationState::Loading)
        ));
        block_on(application.wait_for_update(loading.version)).expect("first view settles")
    }

    #[test]
    fn initialization_transitions_to_ready_or_error() {
        let application = MockApplication::new();
        let initial = block_on(application.snapshot()).expect("snapshot succeeds");
        assert_eq!(initial.status, ApplicationStatus::Initializing);
        let ready = ready(&application);
        assert_eq!(ready.status, ApplicationStatus::Ready);

        let expected = AppError::new(AppErrorKind::Initialization, "fixture open failed");
        let failing = MockApplication::failing_initialization_once(expected.clone());
        let error = block_on(failing.initialize()).expect("failure is represented in the model");
        assert_eq!(error.status, ApplicationStatus::Error(expected));
        let retry = block_on(failing.initialize()).expect("retry succeeds");
        assert_eq!(retry.status, ApplicationStatus::Ready);
    }

    #[test]
    fn projection_versions_are_monotonic_across_initialize_dispatch_and_completion() {
        let application = MockApplication::new();
        let initial = block_on(application.snapshot()).expect("initial snapshot succeeds");
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        let ready =
            block_on(application.wait_for_update(loading.version)).expect("initial view settles");
        let refreshing = block_on(application.dispatch(AppIntent::SetWorkspaceAspectSet {
            resource_id: parse_resource(ASPECT_SET_IDS[1]),
        }))
        .expect("refresh is accepted");
        let fresh =
            block_on(application.wait_for_update(refreshing.version)).expect("refresh completes");

        assert!(initial.version < loading.version);
        assert!(loading.version < ready.version);
        assert!(ready.version < refreshing.version);
        assert!(refreshing.version < fresh.version);
    }

    #[test]
    fn snapshot_is_immediate_and_preserves_projection_version() {
        let application = MockApplication::new();
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        let snapshot = block_on(application.snapshot()).expect("snapshot succeeds");

        assert_eq!(snapshot.version, loading.version);
        assert_eq!(snapshot, loading);
        assert!(matches!(
            snapshot.active_view.map(|view| view.computation),
            Some(ViewComputationState::Loading)
        ));
    }

    #[test]
    fn workspace_transitions_keep_activation_and_selection_independent() {
        let application = MockApplication::new();
        let initial = ready(&application);
        let first = initial.workspace.charts[0].instance_id;
        let active = initial.workspace.active_chart.expect("active chart");
        let selected = initial.workspace.selected_charts.clone();
        assert!(!selected.contains(&active));

        let added = block_on(application.dispatch(AppIntent::SetChartSelection {
            instance_id: active,
            selected: true,
        }))
        .expect("selection succeeds");
        assert_eq!(added.workspace.active_chart, Some(active));
        assert!(added.workspace.selected_charts.contains(&active));
        let removed = block_on(application.dispatch(AppIntent::SetChartSelection {
            instance_id: active,
            selected: false,
        }))
        .expect("deselection succeeds");
        assert_eq!(removed.workspace.active_chart, Some(active));
        assert_eq!(removed.workspace.selected_charts, selected);

        let activated =
            block_on(application.dispatch(AppIntent::ActivateChart { instance_id: first }))
                .expect("activation succeeds");
        assert_eq!(activated.workspace.active_chart, Some(first));
        assert_eq!(activated.workspace.selected_charts, selected);

        let selected_active = block_on(application.dispatch(AppIntent::SetChartSelection {
            instance_id: first,
            selected: false,
        }))
        .expect("selection succeeds");
        assert_eq!(selected_active.workspace.active_chart, Some(first));
        assert!(!selected_active.workspace.selected_charts.contains(&first));

        let closed = block_on(application.dispatch(AppIntent::CloseChart { instance_id: first }))
            .expect("close succeeds");
        assert_eq!(closed.workspace.charts.len(), 3);
        assert_eq!(
            closed.workspace.active_chart,
            Some(closed.workspace.charts[0].instance_id)
        );
        assert!(!closed.workspace.selected_charts.contains(&first));

        let opened = block_on(application.dispatch(AppIntent::OpenChart {
            definition_id: parse_resource(CHART_DEFINITION_IDS[3]),
        }))
        .expect("open succeeds");
        let opened_id = opened
            .workspace
            .active_chart
            .expect("opened chart is active");
        assert_eq!(opened.workspace.charts.len(), 4);
        assert!(!opened.workspace.selected_charts.contains(&opened_id));
    }

    #[test]
    fn closing_active_chart_repairs_required_and_optional_slots() {
        let application = MockApplication::new();
        let initial = ready(&application);
        let active = initial.workspace.active_chart.expect("active chart");
        let closed = block_on(application.dispatch(AppIntent::CloseChart {
            instance_id: active,
        }))
        .expect("close succeeds");
        let replacement = closed
            .workspace
            .active_chart
            .expect("neighbor becomes active");
        let view = closed.active_view.expect("active view");
        let radix = view
            .slots
            .iter()
            .find(|assignment| assignment.required)
            .expect("required slot");
        assert_eq!(radix.chart, Some(replacement));
        assert!(
            view.slots
                .iter()
                .filter(|assignment| !assignment.required)
                .all(|assignment| assignment.chart != Some(active))
        );
    }

    #[test]
    fn slot_assignment_is_authoritative_application_state() {
        let application = MockApplication::new();
        let initial = ready(&application);
        let view = initial.active_view.expect("active view");
        let outer = view.slots[1].slot.clone();
        let replacement = initial.workspace.charts[3].instance_id;

        let updated = block_on(application.dispatch(AppIntent::AssignChartSlot {
            view_id: view.view_id,
            slot: outer.clone(),
            chart: Some(replacement),
        }))
        .expect("slot assignment succeeds");
        let assigned = updated
            .active_view
            .expect("active view")
            .slots
            .into_iter()
            .find(|assignment| assignment.slot == outer)
            .expect("outer slot");
        assert_eq!(assigned.chart, Some(replacement));
    }

    #[test]
    fn wait_for_update_settles_refresh_with_last_good_scene() {
        let application = MockApplication::new();
        let initial = ready(&application);
        let original = initial
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone())
            .expect("initial scene");

        let refreshing = block_on(application.dispatch(AppIntent::SetWorkspaceAspectSet {
            resource_id: parse_resource(ASPECT_SET_IDS[1]),
        }))
        .expect("aspect selection succeeds");
        let refreshing_version = refreshing.version;
        let refreshing_view = refreshing.active_view.expect("active view");
        assert_eq!(refreshing_view.scene, Some(original.clone()));
        assert_eq!(
            refreshing_view.computation,
            ViewComputationState::Refreshing
        );
        let fresh =
            block_on(application.wait_for_update(refreshing_version)).expect("refresh settles");
        assert!(fresh.version > refreshing_version);
        let fresh_view = fresh.active_view.expect("active view");
        let new_scene = fresh_view.scene.expect("new scene");
        assert_ne!(new_scene, original);
        assert_eq!(fresh_view.computation, ViewComputationState::Fresh);

        let failing = block_on(application.dispatch(AppIntent::RefreshActiveView))
            .expect("manual refresh accepted");
        let failing_version = failing.version;
        assert_eq!(
            failing
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(new_scene.clone())
        );
        assert!(matches!(
            failing.active_view.map(|view| view.computation),
            Some(ViewComputationState::Refreshing)
        ));
        let failed =
            block_on(application.wait_for_update(failing_version)).expect("failed refresh settles");
        assert!(failed.version > failing_version);
        let failed_view = failed.active_view.expect("active view");
        assert_eq!(failed_view.scene, Some(new_scene));
        assert!(matches!(
            failed_view.computation,
            ViewComputationState::Failed(AppError {
                kind: AppErrorKind::ViewComputation,
                ..
            })
        ));
    }

    #[test]
    fn wait_for_update_completes_saving_as_clean() {
        let application = MockApplication::new();
        let initial = ready(&application);
        let standard = initial.library.aspect_sets[0].clone();
        let begin = block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard.resource_id,
        }))
        .expect("begin edit succeeds");
        assert!(matches!(
            begin.resource_editor.aspect_set.map(|draft| draft.state),
            Some(DraftState::Clean { .. })
        ));

        let changed_orb = angle(6.5);
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: conjunction_id(),
                maximum: changed_orb,
            },
        )))
        .expect("typed mutation succeeds");
        let dirty_version = dirty.version;
        let draft = dirty.resource_editor.aspect_set.expect("draft");
        assert_eq!(draft.conjunction.maximum_orb, changed_orb);
        assert!(matches!(draft.state, DraftState::Dirty { .. }));
        block_on(application.wait_for_update(dirty_version)).expect("preview refresh settles");

        let canceled =
            block_on(application.dispatch(AppIntent::CancelDraft)).expect("cancel succeeds");
        let canceled_version = canceled.version;
        let canceled_draft = canceled
            .resource_editor
            .aspect_set
            .expect("draft remains projected");
        assert_eq!(
            canceled_draft.conjunction.maximum_orb,
            standard.conjunction_orb
        );
        assert!(matches!(canceled_draft.state, DraftState::Clean { .. }));
        block_on(application.wait_for_update(canceled_version)).expect("cancel refresh settles");

        let refreshing = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: conjunction_id(),
                maximum: changed_orb,
            },
        )))
        .expect("second mutation succeeds");
        let preview = block_on(application.wait_for_update(refreshing.version))
            .expect("preview refresh settles");
        let saving = block_on(application.dispatch(AppIntent::SaveDraft)).expect("save begins");
        let saving_version = saving.version;
        assert!(preview.version < saving_version);
        assert!(matches!(
            saving
                .resource_editor
                .aspect_set
                .as_ref()
                .map(|draft| &draft.state),
            Some(DraftState::Saving { .. })
        ));
        assert!(!saving.availability(AppAction::SaveDraft).is_enabled());
        let saved = block_on(application.wait_for_update(saving_version)).expect("save settles");
        assert!(saved.version > saving_version);
        let saved_summary = saved
            .library
            .aspect_sets
            .iter()
            .find(|summary| summary.resource_id == standard.resource_id)
            .expect("saved summary");
        assert_eq!(saved_summary.revision.get(), standard.revision.get() + 1);
        assert_eq!(saved_summary.conjunction_orb, changed_orb);
        assert!(matches!(
            saved.resource_editor.aspect_set.map(|draft| draft.state),
            Some(DraftState::Clean { .. })
        ));
    }

    #[test]
    fn wait_for_update_completes_saving_as_conflict() {
        let application = MockApplication::new();
        let initial = ready(&application);
        let wide = initial.library.aspect_sets[2].clone();
        let selecting = block_on(application.dispatch(AppIntent::SetWorkspaceAspectSet {
            resource_id: wide.resource_id,
        }))
        .expect("aspect selection succeeds");
        block_on(application.wait_for_update(selecting.version))
            .expect("selection refresh settles");
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: wide.resource_id,
        }))
        .expect("begin edit succeeds");
        let refreshing = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: conjunction_id(),
                maximum: angle(11.0),
            },
        )))
        .expect("typed mutation succeeds");
        let preview = block_on(application.wait_for_update(refreshing.version))
            .expect("preview refresh settles");
        let saving = block_on(application.dispatch(AppIntent::SaveDraft)).expect("save begins");
        assert!(preview.version < saving.version);
        assert!(matches!(
            saving
                .resource_editor
                .aspect_set
                .as_ref()
                .map(|draft| &draft.state),
            Some(DraftState::Saving { .. })
        ));
        let conflict =
            block_on(application.wait_for_update(saving.version)).expect("save conflict settles");
        assert!(conflict.version > saving.version);
        assert!(matches!(
            conflict
                .resource_editor
                .aspect_set
                .as_ref()
                .map(|draft| &draft.state),
            Some(DraftState::Conflict { .. })
        ));
        assert_eq!(
            conflict.notice.as_ref().map(|notice| notice.kind),
            Some(AppNoticeKind::Conflict)
        );
        assert_eq!(
            conflict
                .availability(AppAction::SaveDraft)
                .disabled_reason(),
            Some("Resolve or cancel the revision conflict before saving")
        );
    }
}
