use super::{
    AppError, AppErrorKind, AppResult, ApplicationStatus, BTreeMap, BTreeSet, CalculationRequestId,
    Catalog, ComputationCache, ProjectionVersion, RealState, VecDeque, WorkspaceDocument,
    WorkspaceSession,
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
}
