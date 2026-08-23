use thiserror::Error;

use super::{
    BoundPayload, Catalog, ChartSource, ConfigurationLayer, DerivationSpec, ResourceBinding,
    ResourceId, WorkspaceSession, resolve_typed_binding,
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
    validate_profile_bindings(session, catalog)?;

    for (index, chart) in session.document.chart_instances.iter().enumerate() {
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

    for (view_index, view) in session.document.views.iter().enumerate() {
        let document = resolve_typed_binding(&view.document, catalog, ConfigurationLayer::View)
            .map_err(|error| {
                ReferentialValidationError::new(
                    format!("views[{view_index}].document"),
                    error.to_string(),
                )
            })?;
        for (slot, chart) in &view.charts {
            if !document
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
            for required in document
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
    session: &WorkspaceSession,
    catalog: &Catalog,
) -> Result<(), ReferentialValidationError> {
    let profile = &session.document.profile;
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
