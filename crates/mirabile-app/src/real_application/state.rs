use super::{
    AppError, AppErrorKind, AppResult, ApplicationActivityReadModel, ApplicationStatus, BTreeMap,
    BTreeSet, CalculationRequestId, Catalog, ComputationCache, PendingOperationReadModel,
    ProjectionVersion, RealState, VecDeque, WorkspaceDocument, WorkspaceSession,
};

impl Default for RealState {
    fn default() -> Self {
        Self {
            version: ProjectionVersion::INITIAL,
            status: ApplicationStatus::Initializing,
            catalog: Catalog::default(),
            workspace: None,
            session: None,
            views: BTreeMap::new(),
            editor: None,
            chart_editor: None,
            cache: ComputationCache::default(),
            pending: VecDeque::new(),
            inflight: BTreeMap::new(),
            saving_chart_drafts: BTreeSet::new(),
            next_request_id: CalculationRequestId::FIRST,
            waiters: Vec::new(),
            notice: None,
            next_timestamp: 1,
        }
    }
}

impl RealState {
    pub(super) fn workspace(&self) -> Option<&WorkspaceDocument> {
        self.session.as_ref().map(|session| &session.document)
    }

    pub(super) fn session(&self) -> AppResult<&WorkspaceSession> {
        self.session.as_ref().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })
    }

    pub(super) fn advance(&mut self) -> AppResult<()> {
        self.version = self.version.checked_next().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Unavailable,
                "Application projection version overflowed",
            )
        })?;
        for waiter in self.waiters.drain(..) {
            let _ = waiter.send(());
        }
        Ok(())
    }

    pub(super) fn ensure_view_runtimes(&mut self) {
        let view_ids = self
            .workspace()
            .map(|workspace| {
                workspace
                    .views
                    .iter()
                    .map(|view| view.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.views.retain(|id, _| view_ids.contains(id));
        for id in view_ids {
            self.views.entry(id).or_default();
        }
    }

    pub(super) fn has_pending_write(&self) -> bool {
        self.pending.iter().any(|pending| {
            matches!(
                pending,
                super::PendingWork::SaveAspectSet { .. }
                    | super::PendingWork::CreateChart { .. }
                    | super::PendingWork::SaveChartEdit { .. }
                    | super::PendingWork::SaveWorkspace { .. }
            )
        })
    }

    pub(super) fn activity_read_model(&self) -> ApplicationActivityReadModel {
        if matches!(self.status, ApplicationStatus::Initializing) {
            return ApplicationActivityReadModel::pending(vec![
                PendingOperationReadModel::InitializeApplication,
            ]);
        }

        let mut operations = self
            .views
            .iter()
            .filter_map(|(view_id, runtime)| {
                runtime.expected.as_ref().map(|expected| {
                    PendingOperationReadModel::ViewCalculation {
                        view_id: *view_id,
                        request_id: expected.request_id.get(),
                    }
                })
            })
            .collect::<Vec<_>>();
        operations.extend(self.saving_chart_drafts.iter().map(|instance_id| {
            PendingOperationReadModel::ChartCreate {
                instance_id: *instance_id,
            }
        }));
        operations.extend(self.pending.iter().filter_map(|pending| match pending {
            super::PendingWork::SaveAspectSet { next, .. } => {
                Some(PendingOperationReadModel::ResourceSave {
                    resource_id: next.id,
                })
            }
            super::PendingWork::SaveWorkspace { next, .. } => {
                Some(PendingOperationReadModel::WorkspaceSave {
                    resource_id: Some(next.id),
                })
            }
            super::PendingWork::SaveChartEdit { definition_id, .. } => {
                Some(PendingOperationReadModel::ChartSave {
                    definition_id: *definition_id,
                })
            }
            super::PendingWork::CompleteCachedView(_) | super::PendingWork::CreateChart { .. } => {
                None
            }
        }));

        if operations.is_empty() {
            ApplicationActivityReadModel::settled()
        } else {
            ApplicationActivityReadModel::pending(operations)
        }
    }
}
