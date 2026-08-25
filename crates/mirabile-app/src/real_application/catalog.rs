use super::{
    AnalysisProfile, AppResult, AspectSet, AspectSetSummary, BTreeMap, CanonicalResource,
    ChartDefinition, ChartPersistence, ChartRecord, ChartSource, ConfigurationLayer,
    LibraryChartSummary, OpenChartSummary, PointSet, Resolved, ResourceBinding, ResourceEnvelope,
    ResourceId, Revision, Theme, ViewDocument, WheelTemplate, WorkspaceDocument,
    WorkspaceDocumentChart, chart_record_subtitle, conjunction, not_found, push_pin,
    resolve_binding,
};

#[derive(Clone, Default)]
pub(super) struct Catalog {
    pub(super) current: BTreeMap<ResourceId, CanonicalResource>,
    pub(super) history: BTreeMap<(ResourceId, Revision), CanonicalResource>,
}

impl Catalog {
    pub(super) fn insert_current(&mut self, resource: CanonicalResource) {
        self.history
            .insert((resource.id(), resource.revision()), resource.clone());
        self.current.insert(resource.id(), resource);
    }

    pub(super) fn chart_record(&self, id: ResourceId) -> Option<&ResourceEnvelope<ChartRecord>> {
        match self.current.get(&id) {
            Some(CanonicalResource::ChartRecord(value)) => Some(value),
            _ => None,
        }
    }

    pub(super) fn chart_definition(
        &self,
        id: ResourceId,
    ) -> Option<&ResourceEnvelope<ChartDefinition>> {
        match self.current.get(&id) {
            Some(CanonicalResource::ChartDefinition(value)) => Some(value),
            _ => None,
        }
    }

    pub(super) fn chart_record_reference_count(&self, record_id: ResourceId) -> usize {
        self.current
            .values()
            .filter(|resource| {
                matches!(
                    resource,
                    CanonicalResource::ChartDefinition(definition)
                        if matches!(definition.payload.source, ChartSource::Radix { record } if record == record_id)
                )
            })
            .count()
    }

    pub(super) fn aspect_set(&self, id: ResourceId) -> Option<&ResourceEnvelope<AspectSet>> {
        match self.current.get(&id) {
            Some(CanonicalResource::AspectSet(value)) => Some(value),
            _ => None,
        }
    }

    pub(super) fn workspace(&self, id: ResourceId) -> Option<&ResourceEnvelope<WorkspaceDocument>> {
        match self.current.get(&id) {
            Some(CanonicalResource::WorkspaceDocument(value)) => Some(value),
            _ => None,
        }
    }

    pub(super) fn pinned_references(&self) -> Vec<(ResourceId, Revision)> {
        let mut references = Vec::new();
        for resource in self.current.values() {
            let CanonicalResource::WorkspaceDocument(workspace) = resource else {
                continue;
            };
            push_pin(&workspace.payload.profile.displayed_points, &mut references);
            push_pin(&workspace.payload.profile.aspected_points, &mut references);
            push_pin(&workspace.payload.profile.transit_points, &mut references);
            push_pin(&workspace.payload.profile.aspects, &mut references);
            push_pin(&workspace.payload.profile.analysis, &mut references);
            push_pin(&workspace.payload.profile.theme, &mut references);
            push_pin(&workspace.payload.profile.wheel, &mut references);
            for view in &workspace.payload.views {
                push_pin(&view.document, &mut references);
            }
        }
        references.sort_unstable();
        references.dedup();
        references
    }

    pub(super) fn library_charts(&self) -> AppResult<Vec<LibraryChartSummary>> {
        self.current
            .values()
            .filter_map(|resource| match resource {
                CanonicalResource::ChartDefinition(definition) => Some(definition),
                _ => None,
            })
            .map(|definition| {
                let subtitle = match definition.payload.source {
                    ChartSource::Radix { record } => self.chart_record(record).map_or_else(
                        || "Missing source record".into(),
                        |record| chart_record_subtitle(&record.payload),
                    ),
                    ChartSource::Derived { .. } => "Derived chart".into(),
                };
                Ok(LibraryChartSummary {
                    definition_id: definition.id,
                    title: definition.title.clone(),
                    subtitle,
                })
            })
            .collect()
    }

    pub(super) fn open_chart_summary(
        &self,
        chart: &WorkspaceDocumentChart,
    ) -> AppResult<OpenChartSummary> {
        let definition_envelope = self
            .chart_definition(chart.definition)
            .ok_or_else(|| not_found("ChartDefinition", chart.definition))?;
        let subtitle = match definition_envelope.payload.source {
            ChartSource::Radix { record } => self.chart_record(record).map_or_else(
                || "Missing source record".into(),
                |record| chart_record_subtitle(&record.payload),
            ),
            ChartSource::Derived { .. } => "Derived chart".into(),
        };
        Ok(OpenChartSummary {
            instance_id: chart.instance_id,
            title: definition_envelope.title.clone(),
            subtitle,
            persistence: ChartPersistence::Saved {
                definition_id: chart.definition,
            },
        })
    }

    pub(super) fn aspect_set_summaries(&self) -> AppResult<Vec<AspectSetSummary>> {
        self.current
            .values()
            .filter_map(|resource| match resource {
                CanonicalResource::AspectSet(envelope) => Some(envelope),
                _ => None,
            })
            .map(|envelope| {
                Ok(AspectSetSummary {
                    resource_id: envelope.id,
                    title: envelope.title.clone(),
                    revision: envelope.revision,
                    conjunction_orb: conjunction(&envelope.payload)?.orbs.maximum,
                })
            })
            .collect()
    }
}
pub(super) trait BoundPayload: Clone + Sized {
    fn envelope(resource: &CanonicalResource) -> Option<&ResourceEnvelope<Self>>;
}

macro_rules! bound_payload {
    ($payload:ty, $variant:ident) => {
        impl BoundPayload for $payload {
            fn envelope(resource: &CanonicalResource) -> Option<&ResourceEnvelope<Self>> {
                match resource {
                    CanonicalResource::$variant(envelope) => Some(envelope),
                    _ => None,
                }
            }
        }
    };
}

bound_payload!(PointSet, PointSet);
bound_payload!(AspectSet, AspectSet);
bound_payload!(AnalysisProfile, AnalysisProfile);
bound_payload!(WheelTemplate, WheelTemplate);
bound_payload!(Theme, Theme);
bound_payload!(ViewDocument, ViewDocument);

pub(super) fn resolve_typed_binding<T: BoundPayload>(
    binding: &ResourceBinding<T>,
    catalog: &Catalog,
    layer: ConfigurationLayer,
) -> Result<Resolved<T>, mirabile_core::BindingResolutionError> {
    resolve_binding(
        binding,
        |id| catalog.current.get(&id).and_then(T::envelope).cloned(),
        |id, revision| {
            catalog
                .history
                .get(&(id, revision))
                .and_then(T::envelope)
                .cloned()
        },
        layer,
    )
}
