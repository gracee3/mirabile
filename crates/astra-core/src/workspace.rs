use serde::{Deserialize, Serialize};

use crate::{
    AnalysisProfile, AspectSet, ChartDefinition, InstanceId, PanelId, PointSet, ResourceBinding,
    ResourceId, Theme, ViewInstance, ViewInstanceId, WheelTemplate,
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
