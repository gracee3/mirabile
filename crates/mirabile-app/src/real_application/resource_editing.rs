use super::{
    AppError, AppErrorKind, AppResult, CalculationRuntime, CanonicalResource, DraftState,
    PendingWork, RealApplication, RepositoryError, ResourceRepository, info,
};
use crate::{
    AnalysisProfileMutation, AspectSetMutation, ChartDefinitionMutation, ChartRecordMutation,
    DraftItemAddressReadModel, PointSetMutation, QueryDefinitionMutation,
    ResourceDraftConflictReadModel, ResourceDraftKind, ResourceMetadataMutation, ResourceMutation,
    StableDraftList, ThemeMutation, TypedResourceDraftReadModel, ViewDocumentMutation,
    WheelTemplateMutation, WorkspaceDocumentMutation,
};

#[derive(Clone)]
pub(super) struct GenericResourceDraft {
    pub(super) base: Option<CanonicalResource>,
    pub(super) draft: CanonicalResource,
    pub(super) state: DraftState,
    pub(super) conflicts: Vec<ResourceDraftConflictReadModel>,
    nested: NestedDraftState,
}

#[derive(Clone)]
enum NestedDraftState {
    None,
    ChartRecord {
        notes: StableDraftList<mirabile_core::Note>,
        life_events: StableDraftList<mirabile_core::LifeEvent>,
    },
    WheelTemplate {
        rings: StableDraftList<mirabile_core::RingSpec>,
    },
    ViewDocument {
        objects: StableDraftList<mirabile_core::ViewObject>,
    },
    QueryDefinition {
        node_ids: Vec<crate::DraftItemId>,
    },
}

impl GenericResourceDraft {
    fn new(resource: CanonicalResource) -> Self {
        let nested = NestedDraftState::from_resource(&resource);
        let revision = resource.revision();
        Self {
            base: Some(resource.clone()),
            draft: resource,
            state: DraftState::Clean { revision },
            conflicts: Vec::new(),
            nested,
        }
    }

    fn new_unsaved(resource: CanonicalResource) -> Self {
        let nested = NestedDraftState::from_resource(&resource);
        Self {
            base: None,
            draft: resource,
            state: DraftState::New,
            conflicts: Vec::new(),
            nested,
        }
    }

    pub(super) fn read_model(&self) -> TypedResourceDraftReadModel {
        TypedResourceDraftReadModel {
            kind: ResourceDraftKind::try_from(self.draft.kind())
                .expect("canonical resources always have a draft kind"),
            resource_id: Some(self.draft.id()),
            title: self.draft.title().into(),
            description: self.draft.description().map(str::to_owned),
            tags: self.draft.tags().to_vec(),
            state: self.state.clone(),
            conflicts: self.conflicts.clone(),
            nested_items: self.nested.addresses(),
        }
    }

    fn apply(&mut self, mutation: ResourceMutation) -> AppResult<()> {
        let kind = ResourceDraftKind::try_from(self.draft.kind())
            .expect("canonical resources always have a draft kind");
        if mutation.kind() != kind {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "Resource mutation kind does not match the active typed draft",
            ));
        }
        match mutation {
            ResourceMutation::ChartRecord(mutation) => self.apply_chart_record(mutation)?,
            ResourceMutation::ChartDefinition(mutation) => {
                self.apply_chart_definition(mutation)?;
            }
            ResourceMutation::PointSet(mutation) => self.apply_point_set(mutation)?,
            ResourceMutation::AspectSet(mutation) => self.apply_aspect_set(mutation)?,
            ResourceMutation::AnalysisProfile(mutation) => {
                self.apply_analysis_profile(mutation)?;
            }
            ResourceMutation::WheelTemplate(mutation) => self.apply_wheel_template(mutation)?,
            ResourceMutation::ViewDocument(mutation) => self.apply_view_document(mutation)?,
            ResourceMutation::Theme(mutation) => self.apply_theme(mutation)?,
            ResourceMutation::QueryDefinition(mutation) => {
                self.apply_query_definition(mutation)?;
            }
            ResourceMutation::WorkspaceDocument(mutation) => {
                self.apply_workspace_document(mutation)?;
            }
        }
        self.draft.validate().map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("Typed resource mutation produced an invalid draft: {error}"),
            )
        })?;
        if let Some(base) = &self.base {
            self.state = DraftState::Dirty {
                base_revision: base.revision(),
            };
        }
        self.conflicts.clear();
        Ok(())
    }

    fn apply_metadata(&mut self, mutation: ResourceMetadataMutation) {
        match mutation {
            ResourceMetadataMutation::SetTitle(value) => self.draft.set_title(value),
            ResourceMetadataMutation::SetDescription(value) => self.draft.set_description(value),
            ResourceMetadataMutation::SetTags(value) => self.draft.set_tags(value),
        }
    }

    fn apply_chart_record(&mut self, mutation: ChartRecordMutation) -> AppResult<()> {
        if let ChartRecordMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::ChartRecord(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        match mutation {
            ChartRecordMutation::SetEventKind(value) => envelope.payload.event_kind = value,
            ChartRecordMutation::SetSubject(value) => envelope.payload.subject = value,
            ChartRecordMutation::SetTime(value) => envelope.payload.time = value,
            ChartRecordMutation::SetLocation(value) => envelope.payload.location = value,
            ChartRecordMutation::SetSource(value) => envelope.payload.source = value,
            ChartRecordMutation::Notes(mutation) => {
                let NestedDraftState::ChartRecord { notes, .. } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                notes.apply(mutation).map_err(list_error)?;
                envelope.payload.notes = notes.canonical_values();
            }
            ChartRecordMutation::LifeEvents(mutation) => {
                let NestedDraftState::ChartRecord { life_events, .. } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                life_events.apply(mutation).map_err(list_error)?;
                envelope.payload.life_events = life_events.canonical_values();
            }
            ChartRecordMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }

    fn apply_chart_definition(&mut self, mutation: ChartDefinitionMutation) -> AppResult<()> {
        if let ChartDefinitionMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::ChartDefinition(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        match mutation {
            ChartDefinitionMutation::SetSource(value) => envelope.payload.source = value,
            ChartDefinitionMutation::SetCalculation(value) => envelope.payload.calculation = value,
            ChartDefinitionMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }

    fn apply_point_set(&mut self, mutation: PointSetMutation) -> AppResult<()> {
        if let PointSetMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
        } else if let (CanonicalResource::PointSet(envelope), PointSetMutation::SetPoints(value)) =
            (&mut self.draft, mutation)
        {
            envelope.payload.points = value;
        } else {
            return Err(kind_mismatch());
        }
        Ok(())
    }

    fn apply_aspect_set(&mut self, mutation: AspectSetMutation) -> AppResult<()> {
        if let AspectSetMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
        } else if let (
            CanonicalResource::AspectSet(envelope),
            AspectSetMutation::SetAspects(value),
        ) = (&mut self.draft, mutation)
        {
            envelope.payload.aspects = value;
        } else {
            return Err(kind_mismatch());
        }
        Ok(())
    }

    fn apply_analysis_profile(&mut self, mutation: AnalysisProfileMutation) -> AppResult<()> {
        if let AnalysisProfileMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
        } else if let (
            CanonicalResource::AnalysisProfile(envelope),
            AnalysisProfileMutation::SetProfile(value),
        ) = (&mut self.draft, mutation)
        {
            envelope.payload = value;
        } else {
            return Err(kind_mismatch());
        }
        Ok(())
    }

    fn apply_wheel_template(&mut self, mutation: WheelTemplateMutation) -> AppResult<()> {
        if let WheelTemplateMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::WheelTemplate(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        match mutation {
            WheelTemplateMutation::Rings(mutation) => {
                let NestedDraftState::WheelTemplate { rings } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                rings.apply(mutation).map_err(list_error)?;
                envelope.payload.rings = rings.canonical_values();
            }
            WheelTemplateMutation::SetTemplateFields(value) => {
                envelope.payload = value;
                self.nested = NestedDraftState::from_resource(&self.draft);
            }
            WheelTemplateMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }

    fn apply_view_document(&mut self, mutation: ViewDocumentMutation) -> AppResult<()> {
        if let ViewDocumentMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::ViewDocument(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        match mutation {
            ViewDocumentMutation::SetChartSlots(value) => envelope.payload.chart_slots = value,
            ViewDocumentMutation::Objects(mutation) => {
                let NestedDraftState::ViewDocument { objects } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                objects.apply(mutation).map_err(list_error)?;
                envelope.payload.objects = objects.canonical_values();
            }
            ViewDocumentMutation::SetLayout(value) => envelope.payload.layout = value,
            ViewDocumentMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }

    fn apply_theme(&mut self, mutation: ThemeMutation) -> AppResult<()> {
        if let ThemeMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
        } else if let (CanonicalResource::Theme(envelope), ThemeMutation::SetTheme(value)) =
            (&mut self.draft, mutation)
        {
            envelope.payload = value;
        } else {
            return Err(kind_mismatch());
        }
        Ok(())
    }

    fn apply_query_definition(&mut self, mutation: QueryDefinitionMutation) -> AppResult<()> {
        if let QueryDefinitionMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::QueryDefinition(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        match mutation {
            QueryDefinitionMutation::SetDescription(value) => envelope.payload.description = value,
            QueryDefinitionMutation::SetExpression(value) => {
                envelope.payload.expression = value;
                self.nested = NestedDraftState::from_resource(&self.draft);
            }
            QueryDefinitionMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }

    fn apply_workspace_document(&mut self, mutation: WorkspaceDocumentMutation) -> AppResult<()> {
        if let WorkspaceDocumentMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::WorkspaceDocument(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        match mutation {
            WorkspaceDocumentMutation::SetChartInstances(value) => {
                envelope.payload.chart_instances = value;
            }
            WorkspaceDocumentMutation::SetViews(value) => envelope.payload.views = value,
            WorkspaceDocumentMutation::SetProfile(value) => envelope.payload.profile = *value,
            WorkspaceDocumentMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }
}

impl NestedDraftState {
    fn from_resource(resource: &CanonicalResource) -> Self {
        match resource {
            CanonicalResource::ChartRecord(envelope) => Self::ChartRecord {
                notes: StableDraftList::from_canonical(&envelope.payload.notes),
                life_events: StableDraftList::from_canonical(&envelope.payload.life_events),
            },
            CanonicalResource::WheelTemplate(envelope) => Self::WheelTemplate {
                rings: StableDraftList::from_canonical(&envelope.payload.rings),
            },
            CanonicalResource::ViewDocument(envelope) => Self::ViewDocument {
                objects: StableDraftList::from_canonical(&envelope.payload.objects),
            },
            CanonicalResource::QueryDefinition(envelope) => Self::QueryDefinition {
                node_ids: (0..query_node_count(&envelope.payload.expression))
                    .map(|_| crate::DraftItemId::new())
                    .collect(),
            },
            CanonicalResource::ChartDefinition(_)
            | CanonicalResource::PointSet(_)
            | CanonicalResource::AspectSet(_)
            | CanonicalResource::AnalysisProfile(_)
            | CanonicalResource::Theme(_)
            | CanonicalResource::WorkspaceDocument(_) => Self::None,
        }
    }

    fn addresses(&self) -> Vec<DraftItemAddressReadModel> {
        let addresses = |collection: &str, ids: Vec<crate::DraftItemId>| {
            ids.into_iter()
                .map(|item_id| DraftItemAddressReadModel {
                    collection: collection.into(),
                    item_id,
                })
                .collect::<Vec<_>>()
        };
        match self {
            Self::None => Vec::new(),
            Self::ChartRecord { notes, life_events } => {
                let mut result =
                    addresses("notes", notes.items().iter().map(|item| item.id).collect());
                result.extend(addresses(
                    "life_events",
                    life_events.items().iter().map(|item| item.id).collect(),
                ));
                result
            }
            Self::WheelTemplate { rings } => {
                addresses("rings", rings.items().iter().map(|item| item.id).collect())
            }
            Self::ViewDocument { objects } => addresses(
                "objects",
                objects.items().iter().map(|item| item.id).collect(),
            ),
            Self::QueryDefinition { node_ids } => addresses("query_nodes", node_ids.clone()),
        }
    }
}

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn begin_resource_create(&self, kind: ResourceDraftKind) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if let Some(existing) = state.resource_drafts.get(&kind)
            && !matches!(existing.state, DraftState::Clean { .. })
        {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Save or cancel the current draft before creating another resource of this type",
            ));
        }
        let timestamp = mirabile_core::Timestamp::from_unix_millis(state.next_timestamp);
        let resource = new_resource(kind, timestamp)?;
        state
            .resource_drafts
            .insert(kind, GenericResourceDraft::new_unsaved(resource));
        state.next_timestamp = state.next_timestamp.saturating_add(1);
        state.notice = Some(info("New typed resource draft opened"));
        state.advance()
    }

    pub(super) fn begin_resource_edit(&self, resource_id: crate::ResourceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let resource = state
            .catalog
            .current
            .get(&resource_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("Resource {resource_id} was not found"),
                )
            })?;
        let kind = ResourceDraftKind::try_from(resource.kind()).expect("canonical draft kind");
        if matches!(
            kind,
            ResourceDraftKind::ChartRecord | ResourceDraftKind::ChartDefinition
        ) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Chart records and definitions are edited together through the atomic chart editor",
            ));
        }
        if kind == ResourceDraftKind::AspectSet {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Aspect Sets use the existing typed Aspect Set editor",
            ));
        }
        if kind == ResourceDraftKind::WorkspaceDocument {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Workspaces are edited through the authoritative workspace session",
            ));
        }
        if let Some(existing) = state.resource_drafts.get(&kind) {
            if existing.draft.id() == resource_id {
                return Ok(());
            }
            if !matches!(existing.state, DraftState::Clean { .. }) {
                return Err(AppError::new(
                    AppErrorKind::Unavailable,
                    "Save or cancel the dirty draft before selecting another resource of this type",
                ));
            }
        }
        state
            .resource_drafts
            .insert(kind, GenericResourceDraft::new(resource));
        state.notice = Some(info("Typed resource draft opened"));
        state.advance()
    }

    pub(super) fn apply_resource_mutation(&self, mutation: ResourceMutation) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let draft = state
            .resource_drafts
            .get_mut(&mutation.kind())
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Begin editing this resource type before applying a mutation",
                )
            })?;
        if matches!(draft.state, DraftState::Saving { .. }) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The typed resource draft cannot change while saving",
            ));
        }
        let backup = draft.clone();
        if let Err(error) = draft.apply(mutation) {
            *draft = backup;
            return Err(error);
        }
        state.notice = Some(info("Typed resource mutation accepted"));
        state.advance()
    }

    pub(super) fn begin_save_resource_draft(&self, kind: ResourceDraftKind) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let timestamp = mirabile_core::Timestamp::from_unix_millis(state.next_timestamp);
        let draft = state.resource_drafts.get_mut(&kind).ok_or_else(|| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                "There is no typed resource draft to save",
            )
        })?;
        let expected_revision = match draft.state {
            DraftState::New => None,
            DraftState::Dirty { base_revision } => Some(base_revision),
            _ => {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "The typed resource draft has no saveable changes",
                ));
            }
        };
        let next = if expected_revision.is_some() {
            draft.draft.next_revision(timestamp).map_err(|error| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    format!("Draft is invalid: {error}"),
                )
            })?
        } else {
            draft.draft.validate().map_err(|error| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    format!("Draft is invalid: {error}"),
                )
            })?;
            draft.draft.clone()
        };
        draft.state = expected_revision.map_or(DraftState::Creating, |base_revision| {
            DraftState::Saving { base_revision }
        });
        state.pending.push_back(PendingWork::SaveTypedResource {
            kind,
            expected_revision,
            next: Box::new(next),
        });
        state.next_timestamp = state.next_timestamp.saturating_add(1);
        state.notice = Some(info("Typed resource save accepted"));
        state.advance()
    }

    pub(super) fn cancel_resource_draft(&self, kind: ResourceDraftKind) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state.resource_drafts.remove(&kind).is_none() {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "There is no typed resource draft to cancel",
            ));
        }
        state.notice = Some(info(
            "Typed resource draft canceled without a repository write",
        ));
        state.advance()
    }

    pub(super) async fn complete_typed_resource_save(
        &self,
        kind: ResourceDraftKind,
        expected_revision: Option<crate::Revision>,
        next: CanonicalResource,
    ) -> AppResult<()> {
        let result = if let Some(expected_revision) = expected_revision {
            self.repository.save(expected_revision, next.clone()).await
        } else {
            self.repository.create(next.clone()).await
        };
        let mut state = self.state.borrow_mut();
        match result {
            Ok(()) => {
                state.catalog.insert_current(next.clone());
                if let Some(draft) = state.resource_drafts.get_mut(&kind) {
                    draft.base = Some(next.clone());
                    draft.draft = next.clone();
                    draft.state = DraftState::Clean {
                        revision: next.revision(),
                    };
                    draft.conflicts.clear();
                }
                state.notice = Some(super::success("Typed resource revision saved"));
            }
            Err(RepositoryError::Conflict { actual, .. }) => {
                let expected_revision = expected_revision.expect("only existing saves conflict");
                if let Some(draft) = state.resource_drafts.get_mut(&kind) {
                    draft.state = DraftState::Conflict {
                        base_revision: expected_revision,
                        remote_revision: actual,
                    };
                    draft.conflicts = vec![ResourceDraftConflictReadModel {
                        resource_id: next.id(),
                        expected_revision,
                        actual_revision: actual,
                    }];
                }
                state.notice = Some(super::AppNotice {
                    kind: super::AppNoticeKind::Warning,
                    message: "Typed resource save conflicted; the local draft was retained".into(),
                });
            }
            Err(error) => {
                if let Some(draft) = state.resource_drafts.get_mut(&kind) {
                    draft.state = expected_revision.map_or(DraftState::New, |base_revision| {
                        DraftState::Dirty { base_revision }
                    });
                }
                state.notice = Some(super::AppNotice {
                    kind: super::AppNoticeKind::Warning,
                    message: format!(
                        "Typed resource save failed; the local draft was retained: {error}"
                    ),
                });
            }
        }
        state.advance()
    }
}

fn query_node_count(expression: &mirabile_core::QueryExpr) -> usize {
    match expression {
        mirabile_core::QueryExpr::Predicate(_) => 1,
        mirabile_core::QueryExpr::And(children) | mirabile_core::QueryExpr::Or(children) => {
            1 + children.iter().map(query_node_count).sum::<usize>()
        }
        mirabile_core::QueryExpr::Not(child) => 1 + query_node_count(child),
    }
}

fn kind_mismatch() -> AppError {
    AppError::new(
        AppErrorKind::InvalidIntent,
        "Typed resource mutation did not match its canonical payload",
    )
}

fn list_error(message: &'static str) -> AppError {
    AppError::new(AppErrorKind::InvalidIntent, message)
}

#[allow(clippy::too_many_lines)]
fn new_resource(
    kind: ResourceDraftKind,
    timestamp: mirabile_core::Timestamp,
) -> AppResult<CanonicalResource> {
    use mirabile_core::{
        AnalysisProfile, AspectFieldSpec, HouseDisplaySpec, LabelSpec, PageLayout, Predicate,
        QueryDefinition, QueryExpr, ResourceEnvelope, RingSpec, Theme, ViewDocument, WheelTemplate,
        ZodiacDisplaySpec,
    };

    let resource = match kind {
        ResourceDraftKind::PointSet => CanonicalResource::PointSet(ResourceEnvelope::new(
            "Untitled Point Set",
            mirabile_core::PointSet { points: Vec::new() },
            timestamp,
        )),
        ResourceDraftKind::AnalysisProfile => {
            CanonicalResource::AnalysisProfile(ResourceEnvelope::new(
                "Untitled Analysis Profile",
                AnalysisProfile::default(),
                timestamp,
            ))
        }
        ResourceDraftKind::WheelTemplate => {
            CanonicalResource::WheelTemplate(ResourceEnvelope::new(
                "Untitled Wheel Template",
                WheelTemplate {
                    rings: Vec::<RingSpec>::new(),
                    aspect_field: AspectFieldSpec { radius: 1.0 },
                    houses: HouseDisplaySpec {
                        show_cusps: true,
                        show_numbers: true,
                    },
                    zodiac: ZodiacDisplaySpec {
                        show_boundaries: true,
                        show_labels: true,
                    },
                    labels: LabelSpec {
                        show_degrees: true,
                        show_retrograde: true,
                    },
                },
                timestamp,
            ))
        }
        ResourceDraftKind::ViewDocument => CanonicalResource::ViewDocument(ResourceEnvelope::new(
            "Untitled View Document",
            ViewDocument {
                chart_slots: Vec::new(),
                objects: Vec::new(),
                layout: PageLayout {
                    width: 800.0,
                    height: 800.0,
                },
            },
            timestamp,
        )),
        ResourceDraftKind::Theme => CanonicalResource::Theme(ResourceEnvelope::new(
            "Untitled Theme",
            Theme {
                background: "#08131f".into(),
                foreground: "#e9f3ff".into(),
                muted: "#7189a3".into(),
                accent: "#66d9ef".into(),
                aspect_color: "#ff8f70".into(),
            },
            timestamp,
        )),
        ResourceDraftKind::QueryDefinition => {
            CanonicalResource::QueryDefinition(ResourceEnvelope::new(
                "Untitled Query",
                QueryDefinition {
                    expression: QueryExpr::Predicate(Predicate::ChartField {
                        field: "title".into(),
                        comparison: mirabile_core::TextComparison::Contains,
                        value: "chart".into(),
                    }),
                    description: None,
                },
                timestamp,
            ))
        }
        ResourceDraftKind::ChartRecord
        | ResourceDraftKind::ChartDefinition
        | ResourceDraftKind::AspectSet
        | ResourceDraftKind::WorkspaceDocument => {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "This resource type is created through its composite application workflow",
            ));
        }
    };
    Ok(resource)
}
