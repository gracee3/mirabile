use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Angle, ChartSlotId, InstanceId, PointId, ResourceBinding, ViewInstanceId};
use crate::{
    DomainValidate, DomainValidationError, DomainValidationIssue,
    validation::{finite, nonempty, positive},
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnalysisProfile {
    pub include_applying_state: bool,
    pub include_patterns: bool,
    pub maximum_hits: Option<u32>,
}

impl Default for AnalysisProfile {
    fn default() -> Self {
        Self {
            include_applying_state: true,
            include_patterns: false,
            maximum_hits: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WheelTemplate {
    pub rings: Vec<RingSpec>,
    pub aspect_field: AspectFieldSpec,
    pub houses: HouseDisplaySpec,
    pub zodiac: ZodiacDisplaySpec,
    pub labels: LabelSpec,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RingSpec {
    pub chart_slot: ChartSlotId,
    pub point_role: PointRole,
    pub geometry: RingGeometry,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointRole {
    Primary,
    Transit,
    Progressed,
    Comparison,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RingGeometry {
    pub inner_radius: f64,
    pub outer_radius: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectFieldSpec {
    pub radius: f64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseDisplaySpec {
    pub show_cusps: bool,
    pub show_numbers: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ZodiacDisplaySpec {
    pub show_boundaries: bool,
    pub show_labels: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LabelSpec {
    pub show_degrees: bool,
    pub show_retrograde: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Theme {
    pub background: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub aspect_color: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChartSlot {
    pub id: ChartSlotId,
    pub label: String,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ViewDocument {
    pub chart_slots: Vec<ChartSlot>,
    pub objects: Vec<ViewObject>,
    pub layout: PageLayout,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ViewObject {
    Wheel(WheelObject),
    AspectGrid(GridObject),
    ChartDetails(ChartDetailsObject),
    PointTable(PointTableObject),
    AspectTable(AspectTableObject),
    Text(TextObject),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObjectFrame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WheelObject {
    pub slot: ChartSlotId,
    pub frame: ObjectFrame,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GridObject {
    pub lhs: ChartSlotId,
    pub rhs: Option<ChartSlotId>,
    pub frame: ObjectFrame,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartDetailsObject {
    pub slot: ChartSlotId,
    pub frame: ObjectFrame,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointTableObject {
    pub slot: ChartSlotId,
    pub points: Vec<PointId>,
    pub frame: ObjectFrame,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectTableObject {
    pub slot: ChartSlotId,
    pub frame: ObjectFrame,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TextObject {
    pub text: String,
    pub frame: ObjectFrame,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PageLayout {
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ViewInstance {
    pub id: ViewInstanceId,
    pub document: ResourceBinding<ViewDocument>,
    pub charts: BTreeMap<ChartSlotId, InstanceId>,
    pub overrides: ViewOverrides,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ViewOverrides {
    pub rotation: Option<Angle>,
    pub hidden_points: Vec<PointId>,
}

impl DomainValidate for AnalysisProfile {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        Ok(())
    }
}

impl DomainValidate for WheelTemplate {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        for (index, ring) in self.rings.iter().enumerate() {
            positive(
                ring.geometry.inner_radius,
                &format!("rings[{index}].geometry.inner_radius"),
            )?;
            positive(
                ring.geometry.outer_radius,
                &format!("rings[{index}].geometry.outer_radius"),
            )?;
            if ring.geometry.inner_radius >= ring.geometry.outer_radius {
                return Err(DomainValidationError::new(
                    format!("rings[{index}].geometry"),
                    DomainValidationIssue::InvalidStructure {
                        requirement: "inner radius must be smaller than outer radius".into(),
                    },
                ));
            }
        }
        positive(self.aspect_field.radius, "aspect_field.radius")
    }
}

impl DomainValidate for Theme {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        for (path, value) in [
            ("background", &self.background),
            ("foreground", &self.foreground),
            ("muted", &self.muted),
            ("accent", &self.accent),
            ("aspect_color", &self.aspect_color),
        ] {
            nonempty(value, path)?;
        }
        Ok(())
    }
}

impl DomainValidate for ViewDocument {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        let mut slots = self
            .chart_slots
            .iter()
            .map(|slot| slot.id.clone())
            .collect::<Vec<_>>();
        slots.sort();
        if slots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainValidationError::new(
                "chart_slots.id",
                DomainValidationIssue::Duplicate,
            ));
        }
        for (index, slot) in self.chart_slots.iter().enumerate() {
            nonempty(&slot.label, &format!("chart_slots[{index}].label"))?;
        }
        positive(self.layout.width, "layout.width")?;
        positive(self.layout.height, "layout.height")?;

        for (index, object) in self.objects.iter().enumerate() {
            let prefix = format!("objects[{index}]");
            object
                .frame()
                .domain_validate()
                .map_err(|error| error.prepend(&format!("{prefix}.frame")))?;
            for slot in object.slots() {
                if slots.binary_search(slot).is_err() {
                    return Err(DomainValidationError::new(
                        format!("{prefix}.slot"),
                        DomainValidationIssue::InvalidReference,
                    ));
                }
            }
        }
        Ok(())
    }
}

impl ViewObject {
    fn frame(&self) -> &ObjectFrame {
        match self {
            Self::Wheel(value) => &value.frame,
            Self::AspectGrid(value) => &value.frame,
            Self::ChartDetails(value) => &value.frame,
            Self::PointTable(value) => &value.frame,
            Self::AspectTable(value) => &value.frame,
            Self::Text(value) => &value.frame,
        }
    }

    fn slots(&self) -> Vec<&ChartSlotId> {
        match self {
            Self::Wheel(value) => vec![&value.slot],
            Self::AspectGrid(value) => {
                let mut slots = vec![&value.lhs];
                slots.extend(value.rhs.iter());
                slots
            }
            Self::ChartDetails(value) => vec![&value.slot],
            Self::PointTable(value) => vec![&value.slot],
            Self::AspectTable(value) => vec![&value.slot],
            Self::Text(_) => Vec::new(),
        }
    }
}

impl DomainValidate for ObjectFrame {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        finite(self.x, "x")?;
        finite(self.y, "y")?;
        positive(self.width, "width")?;
        positive(self.height, "height")
    }
}
