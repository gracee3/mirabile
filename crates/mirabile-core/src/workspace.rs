use serde::{Deserialize, Serialize};

use crate::{
    AnalysisProfile, AspectSet, ChartDefinition, DomainValidate, DomainValidationError,
    DomainValidationIssue, InstanceId, PanelId, PointSet, ResourceBinding, ResourceId, Theme,
    ViewInstance, ViewInstanceId, WheelTemplate,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Workspace {
    pub chart_instances: Vec<WorkspaceChart>,
    pub active_chart: Option<InstanceId>,
    pub selected_charts: Vec<InstanceId>,
    pub views: Vec<ViewInstance>,
    pub active_view: Option<ViewInstanceId>,
    pub profile: WorkspaceProfile,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum WorkspaceChart {
    Saved {
        instance_id: InstanceId,
        definition: ResourceId,
    },
    Ephemeral {
        instance_id: InstanceId,
        definition: Box<ChartDefinition>,
    },
}

impl WorkspaceChart {
    pub const fn instance_id(&self) -> InstanceId {
        match self {
            Self::Saved { instance_id, .. } | Self::Ephemeral { instance_id, .. } => *instance_id,
        }
    }

    pub const fn is_saved(&self) -> bool {
        matches!(self, Self::Saved { .. })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceProfile {
    pub displayed_points: ResourceBinding<PointSet>,
    pub aspected_points: ResourceBinding<PointSet>,
    pub transit_points: ResourceBinding<PointSet>,
    pub aspects: ResourceBinding<AspectSet>,
    pub analysis: ResourceBinding<AnalysisProfile>,
    pub theme: ResourceBinding<Theme>,
    pub wheel: ResourceBinding<WheelTemplate>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct WorkspaceUiState {
    pub sidebar_width: f32,
    pub inspector_open: bool,
    pub focused_panel: Option<PanelId>,
    pub scroll_positions: Vec<(PanelId, f64)>,
}

impl DomainValidate for Workspace {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        let mut instance_ids = self
            .chart_instances
            .iter()
            .map(WorkspaceChart::instance_id)
            .collect::<Vec<_>>();
        instance_ids.sort_unstable();
        if instance_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "chart_instances.instance_id",
                DomainValidationIssue::Duplicate,
            ));
        }
        for (index, chart) in self.chart_instances.iter().enumerate() {
            if let WorkspaceChart::Ephemeral { definition, .. } = chart {
                definition.domain_validate().map_err(|error| {
                    error.prepend(&format!("chart_instances[{index}].definition"))
                })?;
            }
        }
        if self
            .active_chart
            .is_some_and(|id| instance_ids.binary_search(&id).is_err())
        {
            return Err(DomainValidationError::new(
                "active_chart",
                DomainValidationIssue::InvalidReference,
            ));
        }
        let mut selected = self.selected_charts.clone();
        selected.sort_unstable();
        if selected.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "selected_charts",
                DomainValidationIssue::Duplicate,
            ));
        }
        if selected
            .iter()
            .any(|id| instance_ids.binary_search(id).is_err())
        {
            return Err(DomainValidationError::new(
                "selected_charts",
                DomainValidationIssue::InvalidReference,
            ));
        }

        let mut view_ids = self.views.iter().map(|view| view.id).collect::<Vec<_>>();
        view_ids.sort_unstable();
        if view_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "views.id",
                DomainValidationIssue::Duplicate,
            ));
        }
        if self
            .active_view
            .is_some_and(|id| view_ids.binary_search(&id).is_err())
        {
            return Err(DomainValidationError::new(
                "active_view",
                DomainValidationIssue::InvalidReference,
            ));
        }
        for (index, view) in self.views.iter().enumerate() {
            if view
                .charts
                .values()
                .any(|id| instance_ids.binary_search(id).is_err())
            {
                return Err(DomainValidationError::new(
                    format!("views[{index}].charts"),
                    DomainValidationIssue::InvalidReference,
                ));
            }
            if let ResourceBinding::Inline { value } = &view.document {
                value
                    .domain_validate()
                    .map_err(|error| error.prepend(&format!("views[{index}].document")))?;
                if view.charts.keys().any(|slot| {
                    !value
                        .chart_slots
                        .iter()
                        .any(|candidate| candidate.id == *slot)
                }) {
                    return Err(DomainValidationError::new(
                        format!("views[{index}].charts"),
                        DomainValidationIssue::InvalidReference,
                    ));
                }
            }
        }
        self.profile.domain_validate()
    }
}

impl DomainValidate for WorkspaceProfile {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        validate_binding(&self.displayed_points, "displayed_points")?;
        validate_binding(&self.aspected_points, "aspected_points")?;
        validate_binding(&self.transit_points, "transit_points")?;
        validate_binding(&self.aspects, "aspects")?;
        validate_binding(&self.analysis, "analysis")?;
        validate_binding(&self.theme, "theme")?;
        validate_binding(&self.wheel, "wheel")
    }
}

fn validate_binding<T: DomainValidate>(
    binding: &ResourceBinding<T>,
    path: &str,
) -> Result<(), DomainValidationError> {
    if let ResourceBinding::Inline { value } = binding {
        value
            .domain_validate()
            .map_err(|error| error.prepend(path))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnalysisProfile, AspectFieldSpec, CalculationSpec, ChartSource, HouseDisplaySpec,
        LabelSpec, PointSet, RingSpec, ZodiacDisplaySpec,
    };

    fn profile() -> WorkspaceProfile {
        let empty_points = || ResourceBinding::Inline {
            value: PointSet { points: Vec::new() },
        };
        WorkspaceProfile {
            displayed_points: empty_points(),
            aspected_points: empty_points(),
            transit_points: empty_points(),
            aspects: ResourceBinding::Inline {
                value: AspectSet {
                    aspects: Vec::new(),
                },
            },
            analysis: ResourceBinding::Inline {
                value: AnalysisProfile::default(),
            },
            theme: ResourceBinding::Inline {
                value: Theme {
                    background: "white".into(),
                    foreground: "black".into(),
                    muted: "gray".into(),
                    accent: "blue".into(),
                    aspect_color: "red".into(),
                },
            },
            wheel: ResourceBinding::Inline {
                value: WheelTemplate {
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
            },
        }
    }

    #[test]
    fn ephemeral_chart_does_not_require_library_resource() {
        let chart = WorkspaceChart::Ephemeral {
            instance_id: InstanceId::new(),
            definition: Box::new(ChartDefinition {
                source: ChartSource::Radix {
                    record: ResourceId::new(),
                },
                calculation: CalculationSpec::default(),
            }),
        };
        let workspace = Workspace {
            active_chart: Some(chart.instance_id()),
            selected_charts: vec![chart.instance_id()],
            chart_instances: vec![chart],
            views: Vec::new(),
            active_view: None,
            profile: profile(),
        };

        assert!(!workspace.chart_instances[0].is_saved());
    }
}
