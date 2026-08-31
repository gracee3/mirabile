use std::collections::BTreeSet;

use thiserror::Error;

use super::{
    BoundPayload, Catalog, ChartSource, ConfigurationLayer, DerivationSpec, ResourceBinding,
    ResourceId, WorkspaceDocument, WorkspaceSession, resolve_typed_binding,
};

/// Validation that requires the hydrated catalog or application session graph.
///
/// Portable one-object invariants remain `DomainValidate` implementations in
/// `mirabile-core`; this boundary never reaches back into core from a repository.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{path}: {message}")]
pub(super) struct ReferentialValidationError {
    path: String,
    message: String,
}

impl ReferentialValidationError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

pub(super) fn validate_session_references(
    session: &WorkspaceSession,
    catalog: &Catalog,
) -> Result<(), ReferentialValidationError> {
    validate_document_references(&session.document, catalog, false)?;
    let draft_instances = validate_session_chart_identities(session)?;
    validate_session_navigation(session)?;
    validate_draft_assignment_references(session, catalog, &draft_instances)?;
    validate_effective_view_references(session, catalog)
}

fn validate_session_chart_identities(
    session: &WorkspaceSession,
) -> Result<BTreeSet<super::InstanceId>, ReferentialValidationError> {
    let saved_instances = session
        .document
        .chart_instances
        .iter()
        .map(|chart| chart.instance_id)
        .collect::<BTreeSet<_>>();
    let mut draft_instances = BTreeSet::new();
    for (index, draft) in session.draft_charts.iter().enumerate() {
        if saved_instances.contains(&draft.instance_id)
            || !draft_instances.insert(draft.instance_id)
        {
            return Err(ReferentialValidationError::new(
                format!("draft_charts[{index}].instance_id"),
                format!(
                    "chart instance {} is not unique across saved and draft session charts",
                    draft.instance_id
                ),
            ));
        }
    }
    Ok(draft_instances)
}

fn validate_session_navigation(
    session: &WorkspaceSession,
) -> Result<(), ReferentialValidationError> {
    if let Some(active) = session.active_chart
        && !session.contains_chart(active)
    {
        return Err(ReferentialValidationError::new(
            "active_chart",
            format!("chart instance {active} is not open in the session"),
        ));
    }
    for (index, selected) in session.selected_charts.iter().enumerate() {
        if !session.contains_chart(*selected) {
            return Err(ReferentialValidationError::new(
                format!("selected_charts[{index}]"),
                format!("chart instance {selected} is not open in the session"),
            ));
        }
    }
    if let Some(active) = session.active_view
        && !session.document.views.iter().any(|view| view.id == active)
    {
        return Err(ReferentialValidationError::new(
            "active_view",
            format!("view {active} is not present in the working document"),
        ));
    }
    Ok(())
}

fn validate_draft_assignment_references(
    session: &WorkspaceSession,
    catalog: &Catalog,
    draft_instances: &BTreeSet<super::InstanceId>,
) -> Result<(), ReferentialValidationError> {
    for (view_id, assignments) in &session.draft_chart_assignments {
        let (view_index, view) = session
            .document
            .views
            .iter()
            .enumerate()
            .find(|(_, view)| view.id == *view_id)
            .ok_or_else(|| {
                ReferentialValidationError::new(
                    format!("draft_chart_assignments.{view_id}"),
                    "view is not present in the working document",
                )
            })?;
        let resolved_view =
            resolve_typed_binding(&view.document, catalog, ConfigurationLayer::View).map_err(
                |error| {
                    ReferentialValidationError::new(
                        format!("views[{view_index}].document"),
                        error.to_string(),
                    )
                },
            )?;
        for (slot, chart) in assignments {
            if !resolved_view
                .value
                .chart_slots
                .iter()
                .any(|candidate| candidate.id == *slot)
            {
                return Err(ReferentialValidationError::new(
                    format!("draft_chart_assignments.{view_id}.{slot}"),
                    "slot is not declared by the resolved ViewDocument",
                ));
            }
            if !draft_instances.contains(chart) {
                return Err(ReferentialValidationError::new(
                    format!("draft_chart_assignments.{view_id}.{slot}"),
                    format!("chart instance {chart} is not an open draft"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_effective_view_references(
    session: &WorkspaceSession,
    catalog: &Catalog,
) -> Result<(), ReferentialValidationError> {
    for (view_index, view) in session.document.views.iter().enumerate() {
        let resolved_view =
            resolve_typed_binding(&view.document, catalog, ConfigurationLayer::View).map_err(
                |error| {
                    ReferentialValidationError::new(
                        format!("views[{view_index}].document"),
                        error.to_string(),
                    )
                },
            )?;
        let effective = session.effective_chart_assignments(view.id);
        for (slot, chart) in &effective {
            if !resolved_view
                .value
                .chart_slots
                .iter()
                .any(|candidate| candidate.id == *slot)
            {
                return Err(ReferentialValidationError::new(
                    format!("views[{view_index}].charts.{slot}"),
                    "slot is not declared by the resolved ViewDocument",
                ));
            }
            if !session.contains_chart(*chart) {
                return Err(ReferentialValidationError::new(
                    format!("views[{view_index}].charts.{slot}"),
                    format!("chart instance {chart} is not open in the session"),
                ));
            }
        }
        if !session.document.chart_instances.is_empty() || !session.draft_charts.is_empty() {
            for required in resolved_view
                .value
                .chart_slots
                .iter()
                .filter(|slot| slot.required)
            {
                if !effective.contains_key(&required.id) {
                    return Err(ReferentialValidationError::new(
                        format!("views[{view_index}].charts.{}", required.id),
                        "required slot is not assigned",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Validates the exact payload that may cross the canonical persistence boundary.
///
/// Session overlays and drafts are intentionally unavailable here, so every chart assignment must
/// resolve to a saved chart instance in this `WorkspaceDocument`.
pub(super) fn validate_durable_document_references(
    document: &WorkspaceDocument,
    catalog: &Catalog,
) -> Result<(), ReferentialValidationError> {
    validate_document_references(document, catalog, true)
}

fn validate_document_references(
    document: &WorkspaceDocument,
    catalog: &Catalog,
    require_complete_slots: bool,
) -> Result<(), ReferentialValidationError> {
    validate_profile_bindings(document, catalog)?;
    let saved_instances = document
        .chart_instances
        .iter()
        .map(|chart| chart.instance_id)
        .collect::<BTreeSet<_>>();

    for (index, chart) in document.chart_instances.iter().enumerate() {
        let definition = catalog.chart_definition(chart.definition).ok_or_else(|| {
            ReferentialValidationError::new(
                format!("chart_instances[{index}].definition"),
                format!("ChartDefinition {} is missing", chart.definition),
            )
        })?;
        validate_chart_source(
            &definition.payload.source,
            catalog,
            &format!("chart_instances[{index}].definition.source"),
        )?;
    }

    for (view_index, view) in document.views.iter().enumerate() {
        if let Some(binding) = &view.points {
            require_binding(binding, catalog, &format!("views[{view_index}].points"))?;
        }
        if let Some(binding) = &view.aspects {
            require_binding(binding, catalog, &format!("views[{view_index}].aspects"))?;
        }
        if let Some(binding) = &view.analysis {
            require_binding(binding, catalog, &format!("views[{view_index}].analysis"))?;
        }
        if let Some(binding) = &view.wheel {
            require_binding(binding, catalog, &format!("views[{view_index}].wheel"))?;
        }
        if let Some(binding) = &view.theme {
            require_binding(binding, catalog, &format!("views[{view_index}].theme"))?;
        }
        let resolved_view =
            resolve_typed_binding(&view.document, catalog, ConfigurationLayer::View).map_err(
                |error| {
                    ReferentialValidationError::new(
                        format!("views[{view_index}].document"),
                        error.to_string(),
                    )
                },
            )?;
        for (slot, chart) in &view.charts {
            if !resolved_view
                .value
                .chart_slots
                .iter()
                .any(|candidate| candidate.id == *slot)
            {
                return Err(ReferentialValidationError::new(
                    format!("views[{view_index}].charts.{slot}"),
                    "slot is not declared by the resolved ViewDocument",
                ));
            }
            if !saved_instances.contains(chart) {
                return Err(ReferentialValidationError::new(
                    format!("views[{view_index}].charts.{slot}"),
                    format!(
                        "chart instance {chart} is not a saved chart in this WorkspaceDocument"
                    ),
                ));
            }
        }
        if require_complete_slots && !document.chart_instances.is_empty() {
            for required in resolved_view
                .value
                .chart_slots
                .iter()
                .filter(|slot| slot.required)
            {
                if !view.charts.contains_key(&required.id) {
                    return Err(ReferentialValidationError::new(
                        format!("views[{view_index}].charts.{}", required.id),
                        "required slot is not assigned",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_profile_bindings(
    document: &WorkspaceDocument,
    catalog: &Catalog,
) -> Result<(), ReferentialValidationError> {
    let profile = &document.profile;
    require_binding(
        &profile.displayed_points,
        catalog,
        "profile.displayed_points",
    )?;
    require_binding(&profile.aspected_points, catalog, "profile.aspected_points")?;
    require_binding(&profile.transit_points, catalog, "profile.transit_points")?;
    require_binding(&profile.aspects, catalog, "profile.aspects")?;
    require_binding(&profile.analysis, catalog, "profile.analysis")?;
    require_binding(&profile.theme, catalog, "profile.theme")?;
    require_binding(&profile.wheel, catalog, "profile.wheel")
}

fn require_binding<T: BoundPayload>(
    binding: &ResourceBinding<T>,
    catalog: &Catalog,
    path: &str,
) -> Result<(), ReferentialValidationError> {
    resolve_typed_binding(binding, catalog, ConfigurationLayer::Workspace)
        .map(|_| ())
        .map_err(|error| ReferentialValidationError::new(path, error.to_string()))
}

fn validate_chart_source(
    source: &ChartSource,
    catalog: &Catalog,
    path: &str,
) -> Result<(), ReferentialValidationError> {
    match source {
        ChartSource::Radix { record } => require_record(*record, catalog, path),
        ChartSource::Derived { recipe } => match recipe {
            DerivationSpec::Transit { .. } => Ok(()),
            DerivationSpec::Harmonic { radix, .. } | DerivationSpec::Relocation { radix, .. } => {
                require_definition(*radix, catalog, path)
            }
            DerivationSpec::Composite { charts, .. } => charts
                .iter()
                .try_for_each(|id| require_definition(*id, catalog, path)),
        },
    }
}

fn require_record(
    id: ResourceId,
    catalog: &Catalog,
    path: &str,
) -> Result<(), ReferentialValidationError> {
    catalog.chart_record(id).map(|_| ()).ok_or_else(|| {
        ReferentialValidationError::new(path, format!("ChartRecord {id} is missing"))
    })
}

fn require_definition(
    id: ResourceId,
    catalog: &Catalog,
    path: &str,
) -> Result<(), ReferentialValidationError> {
    catalog.chart_definition(id).map(|_| ()).ok_or_else(|| {
        ReferentialValidationError::new(path, format!("ChartDefinition {id} is missing"))
    })
}
