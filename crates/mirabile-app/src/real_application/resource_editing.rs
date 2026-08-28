use super::{
    AppError, AppErrorKind, AppResult, CalculationRuntime, CanonicalResource, DraftState,
    PendingWork, RealApplication, RepositoryError, ResourceRepository, info,
};
use crate::{
    AnalysisProfileMutation, AspectSetMutation, ChartDefinitionMutation, ChartRecordMutation,
    DerivedRecipeMutation, PointSetMutation, QueryDefinitionMutation, QueryTreeMutation,
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
        life_events: StableDraftList<DraftLifeEvent>,
    },
    ChartDefinition {
        composite_charts: StableDraftList<mirabile_core::ResourceId>,
    },
    PointSet {
        selectors: StableDraftList<mirabile_core::PointSelector>,
    },
    AspectSet {
        aspects: StableDraftList<mirabile_core::AspectDefinition>,
    },
    WheelTemplate {
        rings: StableDraftList<mirabile_core::RingSpec>,
    },
    ViewDocument {
        chart_slots: StableDraftList<mirabile_core::ChartSlot>,
        objects: StableDraftList<mirabile_core::ViewObject>,
    },
    QueryDefinition {
        tree: DraftQueryNode,
    },
    WorkspaceDocument {
        charts: StableDraftList<mirabile_core::WorkspaceDocumentChart>,
        views: StableDraftList<mirabile_core::ViewInstance>,
    },
}

#[derive(Clone)]
struct DraftQueryNode {
    id: crate::DraftItemId,
    expression: mirabile_core::QueryExpr,
    children: Vec<DraftQueryNode>,
}

#[derive(Clone, Debug, PartialEq)]
struct DraftLifeEvent {
    value: mirabile_core::LifeEvent,
    notes: StableDraftList<mirabile_core::Note>,
}

impl DraftLifeEvent {
    fn from_canonical(value: &mirabile_core::LifeEvent) -> Self {
        Self {
            value: value.clone(),
            notes: StableDraftList::from_canonical(&value.notes),
        }
    }
    fn materialize(&self) -> mirabile_core::LifeEvent {
        let mut value = self.value.clone();
        value.notes = self.notes.canonical_values();
        value
    }
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
            nested: self.nested.read_model(),
            value: crate::ResourceDraftValueReadModel::from(&self.draft),
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
                let mutation = match mutation {
                    crate::DraftListMutation::Insert { after, value } => {
                        crate::DraftListMutation::Insert {
                            after,
                            value: DraftLifeEvent::from_canonical(&value),
                        }
                    }
                    crate::DraftListMutation::Update { item_id, value } => {
                        crate::DraftListMutation::Update {
                            item_id,
                            value: DraftLifeEvent::from_canonical(&value),
                        }
                    }
                    crate::DraftListMutation::Remove { item_id } => {
                        crate::DraftListMutation::Remove { item_id }
                    }
                    crate::DraftListMutation::Move { item_id, before } => {
                        crate::DraftListMutation::Move { item_id, before }
                    }
                };
                life_events.apply(mutation).map_err(list_error)?;
                envelope.payload.life_events = life_events
                    .items()
                    .iter()
                    .map(|item| item.value.materialize())
                    .collect();
            }
            ChartRecordMutation::LifeEventNotes {
                life_event_id,
                mutation,
            } => {
                let NestedDraftState::ChartRecord { life_events, .. } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                let event = life_events
                    .items_mut()
                    .iter_mut()
                    .find(|item| item.id == life_event_id)
                    .ok_or_else(|| list_error("Draft life event was not found"))?;
                event.value.notes.apply(mutation).map_err(list_error)?;
                envelope.payload.life_events = life_events
                    .items()
                    .iter()
                    .map(|item| item.value.materialize())
                    .collect();
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
            ChartDefinitionMutation::SetSource(value) => {
                envelope.payload.source = value;
                self.nested = NestedDraftState::from_resource(&self.draft);
            }
            ChartDefinitionMutation::MutateDerivedRecipe(mutation) => {
                let mirabile_core::ChartSource::Derived { recipe } = &mut envelope.payload.source
                else {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Derived recipe mutations require a derived chart source",
                    ));
                };
                match mutation {
                    DerivedRecipeMutation::SetTransit { at, location } => {
                        *recipe = mirabile_core::DerivationSpec::Transit { at, location }
                    }
                    DerivedRecipeMutation::SetHarmonic { radix, harmonic } => {
                        *recipe = mirabile_core::DerivationSpec::Harmonic { radix, harmonic }
                    }
                    DerivedRecipeMutation::SetRelocation { radix, location } => {
                        *recipe = mirabile_core::DerivationSpec::Relocation { radix, location }
                    }
                    DerivedRecipeMutation::SetCompositeMethod(method) => {
                        let mirabile_core::DerivationSpec::Composite {
                            method: current, ..
                        } = recipe
                        else {
                            return Err(AppError::new(
                                AppErrorKind::InvalidIntent,
                                "Composite method requires a composite recipe",
                            ));
                        };
                        *current = method;
                    }
                    DerivedRecipeMutation::CompositeCharts(mutation) => {
                        let NestedDraftState::ChartDefinition { composite_charts } =
                            &mut self.nested
                        else {
                            return Err(kind_mismatch());
                        };
                        composite_charts.apply(mutation).map_err(list_error)?;
                        let mirabile_core::DerivationSpec::Composite { charts, .. } = recipe else {
                            return Err(AppError::new(
                                AppErrorKind::InvalidIntent,
                                "Composite chart rows require a composite recipe",
                            ));
                        };
                        *charts = composite_charts.canonical_values();
                    }
                }
            }
            ChartDefinitionMutation::SetCalculation(value) => envelope.payload.calculation = value,
            ChartDefinitionMutation::Metadata(_) => unreachable!(),
        }
        Ok(())
    }

    fn apply_point_set(&mut self, mutation: PointSetMutation) -> AppResult<()> {
        if let PointSetMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::PointSet(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        let PointSetMutation::Selectors(mutation) = mutation else {
            unreachable!()
        };
        let NestedDraftState::PointSet { selectors } = &mut self.nested else {
            return Err(kind_mismatch());
        };
        selectors.apply(mutation).map_err(list_error)?;
        envelope.payload.points = selectors.canonical_values();
        Ok(())
    }

    fn apply_aspect_set(&mut self, mutation: AspectSetMutation) -> AppResult<()> {
        if let AspectSetMutation::Metadata(mutation) = mutation {
            self.apply_metadata(mutation);
            return Ok(());
        }
        let CanonicalResource::AspectSet(envelope) = &mut self.draft else {
            return Err(kind_mismatch());
        };
        let AspectSetMutation::Aspects(mutation) = mutation else {
            unreachable!()
        };
        let NestedDraftState::AspectSet { aspects } = &mut self.nested else {
            return Err(kind_mismatch());
        };
        aspects.apply(mutation).map_err(list_error)?;
        envelope.payload.aspects = aspects.canonical_values();
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
            ViewDocumentMutation::ChartSlots(mutation) => {
                let NestedDraftState::ViewDocument { chart_slots, .. } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                if let crate::DraftListMutation::Remove { item_id } = &mutation {
                    let slot = chart_slots
                        .items()
                        .iter()
                        .find(|item| item.id == *item_id)
                        .ok_or_else(|| list_error("Draft chart slot was not found"))?;
                    if envelope
                        .payload
                        .objects
                        .iter()
                        .any(|object| object_references_slot(object, &slot.value.id))
                    {
                        return Err(AppError::new(
                            AppErrorKind::Unavailable,
                            "Chart slots cannot be removed while a View Object references them",
                        ));
                    }
                }
                chart_slots.apply(mutation).map_err(list_error)?;
                envelope.payload.chart_slots = chart_slots.canonical_values();
            }
            ViewDocumentMutation::RenameChartSlot { item_id, slot } => {
                let NestedDraftState::ViewDocument {
                    chart_slots,
                    objects,
                } = &mut self.nested
                else {
                    return Err(kind_mismatch());
                };
                let old = chart_slots
                    .items()
                    .iter()
                    .find(|item| item.id == item_id)
                    .ok_or_else(|| list_error("Draft chart slot was not found"))?
                    .value
                    .id
                    .clone();
                chart_slots
                    .apply(crate::DraftListMutation::Update {
                        item_id,
                        value: slot.clone(),
                    })
                    .map_err(list_error)?;
                for item in objects.items().to_vec() {
                    let mut value = item.value;
                    rewrite_object_slot(&mut value, &old, &slot.id);
                    objects
                        .apply(crate::DraftListMutation::Update {
                            item_id: item.id,
                            value,
                        })
                        .map_err(list_error)?;
                }
                envelope.payload.chart_slots = chart_slots.canonical_values();
                envelope.payload.objects = objects.canonical_values();
            }
            ViewDocumentMutation::Objects(mutation) => {
                let NestedDraftState::ViewDocument { objects, .. } = &mut self.nested else {
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
            QueryDefinitionMutation::Tree(mutation) => {
                let NestedDraftState::QueryDefinition { tree } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                tree.apply(mutation)?;
                envelope.payload.expression = tree.materialize();
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
            WorkspaceDocumentMutation::ChartInstances(mutation) => {
                let NestedDraftState::WorkspaceDocument { charts, .. } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                charts.apply(mutation).map_err(list_error)?;
                envelope.payload.chart_instances = charts.canonical_values();
            }
            WorkspaceDocumentMutation::Views(mutation) => {
                let NestedDraftState::WorkspaceDocument { views, .. } = &mut self.nested else {
                    return Err(kind_mismatch());
                };
                views.apply(mutation).map_err(list_error)?;
                envelope.payload.views = views.canonical_values();
            }
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
                life_events: StableDraftList::from_canonical(
                    &envelope
                        .payload
                        .life_events
                        .iter()
                        .map(DraftLifeEvent::from_canonical)
                        .collect::<Vec<_>>(),
                ),
            },
            CanonicalResource::ChartDefinition(envelope) => Self::ChartDefinition {
                composite_charts: StableDraftList::from_canonical(match &envelope.payload.source {
                    mirabile_core::ChartSource::Derived {
                        recipe: mirabile_core::DerivationSpec::Composite { charts, .. },
                    } => charts,
                    _ => &[],
                }),
            },
            CanonicalResource::PointSet(envelope) => Self::PointSet {
                selectors: StableDraftList::from_canonical(&envelope.payload.points),
            },
            CanonicalResource::AspectSet(envelope) => Self::AspectSet {
                aspects: StableDraftList::from_canonical(&envelope.payload.aspects),
            },
            CanonicalResource::WheelTemplate(envelope) => Self::WheelTemplate {
                rings: StableDraftList::from_canonical(&envelope.payload.rings),
            },
            CanonicalResource::ViewDocument(envelope) => Self::ViewDocument {
                chart_slots: StableDraftList::from_canonical(&envelope.payload.chart_slots),
                objects: StableDraftList::from_canonical(&envelope.payload.objects),
            },
            CanonicalResource::QueryDefinition(envelope) => Self::QueryDefinition {
                tree: DraftQueryNode::from_expression(&envelope.payload.expression),
            },
            CanonicalResource::WorkspaceDocument(envelope) => Self::WorkspaceDocument {
                charts: StableDraftList::from_canonical(&envelope.payload.chart_instances),
                views: StableDraftList::from_canonical(&envelope.payload.views),
            },
            CanonicalResource::AnalysisProfile(_) | CanonicalResource::Theme(_) => Self::None,
        }
    }

    fn read_model(&self) -> crate::NestedResourceDraftReadModel {
        fn items<T: Clone>(list: &StableDraftList<T>) -> Vec<crate::StableDraftItemReadModel<T>> {
            list.items()
                .iter()
                .map(|item| crate::StableDraftItemReadModel {
                    item_id: item.id,
                    value: item.value.clone(),
                })
                .collect()
        }
        match self {
            Self::None => crate::NestedResourceDraftReadModel::None,
            Self::ChartRecord { notes, life_events } => {
                crate::NestedResourceDraftReadModel::ChartRecord {
                    notes: items(notes),
                    life_events: life_events
                        .items()
                        .iter()
                        .map(|item| crate::LifeEventDraftReadModel {
                            item_id: item.id,
                            value: item.value.materialize(),
                            notes: items(&item.value.notes),
                        })
                        .collect(),
                }
            }
            Self::ChartDefinition { composite_charts } => {
                crate::NestedResourceDraftReadModel::ChartDefinition {
                    composite_charts: items(composite_charts),
                }
            }
            Self::PointSet { selectors } => {
                crate::NestedResourceDraftReadModel::PointSet(items(selectors))
            }
            Self::AspectSet { aspects } => {
                crate::NestedResourceDraftReadModel::AspectSet(items(aspects))
            }
            Self::WheelTemplate { rings } => {
                crate::NestedResourceDraftReadModel::WheelTemplate(items(rings))
            }
            Self::ViewDocument {
                chart_slots,
                objects,
            } => crate::NestedResourceDraftReadModel::ViewDocument {
                chart_slots: items(chart_slots),
                objects: items(objects),
            },
            Self::QueryDefinition { tree } => {
                crate::NestedResourceDraftReadModel::QueryDefinition(tree.read_model())
            }
            Self::WorkspaceDocument { charts, views } => {
                crate::NestedResourceDraftReadModel::WorkspaceDocument {
                    charts: items(charts),
                    views: items(views),
                }
            }
        }
    }
}

impl DraftQueryNode {
    fn from_expression(expression: &mirabile_core::QueryExpr) -> Self {
        let children = match expression {
            mirabile_core::QueryExpr::And(values) | mirabile_core::QueryExpr::Or(values) => {
                values.iter().map(Self::from_expression).collect()
            }
            mirabile_core::QueryExpr::Not(value) => vec![Self::from_expression(value)],
            mirabile_core::QueryExpr::Predicate(_) => Vec::new(),
        };
        Self {
            id: crate::DraftItemId::new(),
            expression: expression.clone(),
            children,
        }
    }

    fn materialize(&self) -> mirabile_core::QueryExpr {
        match &self.expression {
            mirabile_core::QueryExpr::Predicate(value) => {
                mirabile_core::QueryExpr::Predicate(value.clone())
            }
            mirabile_core::QueryExpr::And(_) => {
                mirabile_core::QueryExpr::And(self.children.iter().map(Self::materialize).collect())
            }
            mirabile_core::QueryExpr::Or(_) => {
                mirabile_core::QueryExpr::Or(self.children.iter().map(Self::materialize).collect())
            }
            mirabile_core::QueryExpr::Not(_) => {
                mirabile_core::QueryExpr::Not(Box::new(self.children[0].materialize()))
            }
        }
    }

    fn read_model(&self) -> crate::QueryNodeDraftReadModel {
        crate::QueryNodeDraftReadModel {
            node_id: self.id,
            expression: self.materialize(),
            children: self.children.iter().map(Self::read_model).collect(),
        }
    }

    fn contains(&self, id: crate::DraftItemId) -> bool {
        self.id == id || self.children.iter().any(|child| child.contains(id))
    }
    fn find_mut(&mut self, id: crate::DraftItemId) -> Option<&mut Self> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
    fn remove_descendant(&mut self, id: crate::DraftItemId) -> Option<Self> {
        if let Some(index) = self.children.iter().position(|child| child.id == id) {
            return Some(self.children.remove(index));
        }
        self.children
            .iter_mut()
            .find_map(|child| child.remove_descendant(id))
    }
    #[allow(clippy::too_many_lines)]
    fn apply(&mut self, mutation: QueryTreeMutation) -> AppResult<()> {
        match mutation {
            QueryTreeMutation::Replace {
                node_id,
                expression,
            } => {
                let node = self
                    .find_mut(node_id)
                    .ok_or_else(|| list_error("Query node was not found"))?;
                *node = Self::from_expression(&expression);
                node.id = node_id;
            }
            QueryTreeMutation::InsertChild {
                parent_id,
                after,
                expression,
            } => {
                let parent = self
                    .find_mut(parent_id)
                    .ok_or_else(|| list_error("Query parent was not found"))?;
                if matches!(parent.expression, mirabile_core::QueryExpr::Predicate(_)) {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Predicate nodes cannot contain children",
                    ));
                }
                if matches!(parent.expression, mirabile_core::QueryExpr::Not(_))
                    && !parent.children.is_empty()
                {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Not nodes contain exactly one child",
                    ));
                }
                let index = after
                    .map_or(Ok(0), |id| {
                        parent
                            .children
                            .iter()
                            .position(|child| child.id == id)
                            .map(|i| i + 1)
                            .ok_or("Query insertion anchor was not found")
                    })
                    .map_err(list_error)?;
                parent
                    .children
                    .insert(index, Self::from_expression(&expression));
            }
            QueryTreeMutation::Remove { node_id } => {
                if self.id == node_id {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "The query root cannot be deleted",
                    ));
                }
                if self.remove_descendant(node_id).is_none() {
                    return Err(list_error("Query node was not found"));
                }
                if !query_groups_valid(self) {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Boolean groups must retain at least one child",
                    ));
                }
            }
            QueryTreeMutation::Move {
                node_id,
                new_parent_id,
                before,
            } => {
                if self.id == node_id {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "The query root cannot be moved",
                    ));
                }
                let moving = self
                    .find_mut(node_id)
                    .ok_or_else(|| list_error("Query node was not found"))?
                    .clone();
                if moving.contains(new_parent_id) {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "A query node cannot move into itself or its descendants",
                    ));
                }
                let backup = self.clone();
                let node = self
                    .remove_descendant(node_id)
                    .ok_or_else(|| list_error("Query node was not found"))?;
                let Some(parent) = self.find_mut(new_parent_id) else {
                    *self = backup;
                    return Err(list_error("Query move parent was not found"));
                };
                if matches!(parent.expression, mirabile_core::QueryExpr::Predicate(_))
                    || (matches!(parent.expression, mirabile_core::QueryExpr::Not(_))
                        && !parent.children.is_empty())
                {
                    *self = backup;
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Query move target cannot accept a child",
                    ));
                }
                let index = before
                    .map_or(Ok(parent.children.len()), |id| {
                        parent
                            .children
                            .iter()
                            .position(|child| child.id == id)
                            .ok_or("Query move target was not found")
                    })
                    .map_err(list_error)?;
                parent.children.insert(index, node);
                if !query_groups_valid(self) {
                    *self = backup;
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Boolean groups must retain at least one child",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn query_groups_valid(node: &DraftQueryNode) -> bool {
    let valid = match node.expression {
        mirabile_core::QueryExpr::And(_)
        | mirabile_core::QueryExpr::Or(_)
        | mirabile_core::QueryExpr::Not(_) => !node.children.is_empty(),
        mirabile_core::QueryExpr::Predicate(_) => true,
    };
    valid && node.children.iter().all(query_groups_valid)
}

fn rewrite_object_slot(
    object: &mut mirabile_core::ViewObject,
    old: &mirabile_core::ChartSlotId,
    new: &mirabile_core::ChartSlotId,
) {
    match object {
        mirabile_core::ViewObject::Wheel(value) if &value.slot == old => value.slot = new.clone(),
        mirabile_core::ViewObject::AspectGrid(value) => {
            if &value.lhs == old {
                value.lhs = new.clone();
            }
            if value.rhs.as_ref() == Some(old) {
                value.rhs = Some(new.clone());
            }
        }
        mirabile_core::ViewObject::ChartDetails(value) if &value.slot == old => {
            value.slot = new.clone();
        }
        mirabile_core::ViewObject::PointTable(value) if &value.slot == old => {
            value.slot = new.clone();
        }
        mirabile_core::ViewObject::AspectTable(value) if &value.slot == old => {
            value.slot = new.clone();
        }
        _ => {}
    }
}

fn object_references_slot(
    object: &mirabile_core::ViewObject,
    slot: &mirabile_core::ChartSlotId,
) -> bool {
    match object {
        mirabile_core::ViewObject::Wheel(value) => &value.slot == slot,
        mirabile_core::ViewObject::AspectGrid(value) => {
            &value.lhs == slot || value.rhs.as_ref() == Some(slot)
        }
        mirabile_core::ViewObject::ChartDetails(value) => &value.slot == slot,
        mirabile_core::ViewObject::PointTable(value) => &value.slot == slot,
        mirabile_core::ViewObject::AspectTable(value) => &value.slot == slot,
        mirabile_core::ViewObject::Text(_) => false,
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
        if kind == ResourceDraftKind::ChartRecord {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Chart records and definitions are edited together through the atomic chart editor",
            ));
        }
        if kind == ResourceDraftKind::ChartDefinition
            && matches!(&resource, CanonicalResource::ChartDefinition(envelope) if matches!(envelope.payload.source, mirabile_core::ChartSource::Radix { .. }))
        {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Radix definitions are edited atomically with their ChartRecord; derived definitions use the persisted recipe editor",
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
        if let Some(draft) = state.resource_drafts.get(&kind) {
            validate_derived_references(&draft.draft, &state.catalog.current)?;
        }
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

fn validate_derived_references(
    resource: &CanonicalResource,
    catalog: &std::collections::BTreeMap<crate::ResourceId, CanonicalResource>,
) -> AppResult<()> {
    let CanonicalResource::ChartDefinition(envelope) = resource else {
        return Ok(());
    };
    let mirabile_core::ChartSource::Derived { recipe } = &envelope.payload.source else {
        return Ok(());
    };
    let references: Vec<_> = match recipe {
        mirabile_core::DerivationSpec::Transit { .. } => Vec::new(),
        mirabile_core::DerivationSpec::Harmonic { radix, .. }
        | mirabile_core::DerivationSpec::Relocation { radix, .. } => vec![*radix],
        mirabile_core::DerivationSpec::Composite { charts, .. } => charts.clone(),
    };
    for reference in references {
        if reference == envelope.id
            || !matches!(
                catalog.get(&reference),
                Some(CanonicalResource::ChartDefinition(_))
            )
        {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                format!(
                    "Derived recipe references missing or incompatible ChartDefinition {reference}"
                ),
            ));
        }
    }
    Ok(())
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
        ResourceDraftKind::ChartDefinition => {
            CanonicalResource::ChartDefinition(mirabile_core::ResourceEnvelope::new(
                "Untitled Derived Chart",
                mirabile_core::ChartDefinition {
                    source: mirabile_core::ChartSource::Derived {
                        recipe: mirabile_core::DerivationSpec::Harmonic {
                            radix: crate::ResourceId::new(),
                            harmonic: 2.0,
                        },
                    },
                    calculation: mirabile_core::CalculationSpec::default(),
                },
                timestamp,
            ))
        }
        ResourceDraftKind::ChartRecord
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

#[cfg(test)]
mod nested_tests {
    use super::*;

    fn timestamp() -> mirabile_core::Timestamp {
        mirabile_core::Timestamp::from_unix_millis(1)
    }
    fn predicate() -> mirabile_core::QueryExpr {
        mirabile_core::QueryExpr::Predicate(mirabile_core::Predicate::InSign {
            point: mirabile_core::PointId::new("sun").expect("point"),
            sign_index: 0,
        })
    }

    #[test]
    fn query_tree_preserves_ids_and_rejects_root_empty_group_and_cycles() {
        let mut draft = GenericResourceDraft::new_unsaved(
            new_resource(ResourceDraftKind::QueryDefinition, timestamp()).expect("query"),
        );
        let root = match draft.read_model().nested {
            crate::NestedResourceDraftReadModel::QueryDefinition(tree) => tree.node_id,
            _ => panic!("query tree"),
        };
        draft
            .apply(ResourceMutation::QueryDefinition(
                QueryDefinitionMutation::Tree(QueryTreeMutation::Replace {
                    node_id: root,
                    expression: mirabile_core::QueryExpr::And(vec![
                        predicate(),
                        mirabile_core::QueryExpr::And(vec![predicate()]),
                    ]),
                }),
            ))
            .expect("replace root");
        let crate::NestedResourceDraftReadModel::QueryDefinition(tree) = draft.read_model().nested
        else {
            panic!("query tree")
        };
        assert_eq!(tree.node_id, root);
        let first = tree.children[0].node_id;
        let group = tree.children[1].node_id;
        let grandchild = tree.children[1].children[0].node_id;
        assert!(
            draft
                .apply(ResourceMutation::QueryDefinition(
                    QueryDefinitionMutation::Tree(QueryTreeMutation::Move {
                        node_id: group,
                        new_parent_id: grandchild,
                        before: None
                    })
                ))
                .is_err()
        );
        draft
            .apply(ResourceMutation::QueryDefinition(
                QueryDefinitionMutation::Tree(QueryTreeMutation::Remove { node_id: first }),
            ))
            .expect("one child remains");
        assert!(
            draft
                .apply(ResourceMutation::QueryDefinition(
                    QueryDefinitionMutation::Tree(QueryTreeMutation::Remove { node_id: group })
                ))
                .is_err()
        );
        assert!(
            draft
                .apply(ResourceMutation::QueryDefinition(
                    QueryDefinitionMutation::Tree(QueryTreeMutation::Remove { node_id: root })
                ))
                .is_err()
        );
    }

    #[test]
    fn slot_removal_is_blocked_and_slot_rename_rewrites_object_references() {
        let mut draft = GenericResourceDraft::new_unsaved(
            new_resource(ResourceDraftKind::ViewDocument, timestamp()).expect("view"),
        );
        let slot = mirabile_core::ChartSlot {
            id: mirabile_core::ChartSlotId::new("primary").expect("slot"),
            label: "Primary".into(),
            required: true,
        };
        draft
            .apply(ResourceMutation::ViewDocument(
                ViewDocumentMutation::ChartSlots(crate::DraftListMutation::Insert {
                    after: None,
                    value: slot.clone(),
                }),
            ))
            .expect("slot");
        let slot_item = match draft.read_model().nested {
            crate::NestedResourceDraftReadModel::ViewDocument { chart_slots, .. } => {
                chart_slots[0].item_id
            }
            _ => panic!("view"),
        };
        draft
            .apply(ResourceMutation::ViewDocument(
                ViewDocumentMutation::Objects(crate::DraftListMutation::Insert {
                    after: None,
                    value: mirabile_core::ViewObject::Wheel(mirabile_core::WheelObject {
                        slot: slot.id.clone(),
                        frame: mirabile_core::ObjectFrame {
                            x: 0.0,
                            y: 0.0,
                            width: 10.0,
                            height: 10.0,
                        },
                    }),
                }),
            ))
            .expect("object");
        assert!(
            draft
                .apply(ResourceMutation::ViewDocument(
                    ViewDocumentMutation::ChartSlots(crate::DraftListMutation::Remove {
                        item_id: slot_item
                    })
                ))
                .is_err()
        );
        let renamed = mirabile_core::ChartSlot {
            id: mirabile_core::ChartSlotId::new("renamed").expect("slot"),
            ..slot
        };
        draft
            .apply(ResourceMutation::ViewDocument(
                ViewDocumentMutation::RenameChartSlot {
                    item_id: slot_item,
                    slot: renamed.clone(),
                },
            ))
            .expect("rename");
        let crate::ResourceDraftValueReadModel::ViewDocument(value) = draft.read_model().value
        else {
            panic!("view")
        };
        assert!(
            matches!(&value.objects[0], mirabile_core::ViewObject::Wheel(object) if object.slot == renamed.id)
        );
    }

    #[test]
    fn derived_recipe_references_are_checked_without_claiming_execution() {
        let referenced = mirabile_core::CanonicalResource::ChartDefinition(
            mirabile_core::ResourceEnvelope::new(
                "Radix",
                mirabile_core::ChartDefinition {
                    source: mirabile_core::ChartSource::Radix {
                        record: crate::ResourceId::new(),
                    },
                    calculation: mirabile_core::CalculationSpec::default(),
                },
                timestamp(),
            ),
        );
        let derived = mirabile_core::CanonicalResource::ChartDefinition(
            mirabile_core::ResourceEnvelope::new(
                "Harmonic",
                mirabile_core::ChartDefinition {
                    source: mirabile_core::ChartSource::Derived {
                        recipe: mirabile_core::DerivationSpec::Harmonic {
                            radix: referenced.id(),
                            harmonic: 2.0,
                        },
                    },
                    calculation: mirabile_core::CalculationSpec::default(),
                },
                timestamp(),
            ),
        );
        assert!(validate_derived_references(&derived, &std::collections::BTreeMap::new()).is_err());
        let mut catalog = std::collections::BTreeMap::new();
        catalog.insert(referenced.id(), referenced);
        validate_derived_references(&derived, &catalog).expect("valid reference");
    }
}
