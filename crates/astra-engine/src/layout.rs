use std::collections::BTreeMap;

use astra_core::{DomainValidate, PointId, PointSelector, PointSet, Theme, WheelTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ChartAnalysis, ChartSnapshot, KeyError, LayoutKey, RenderKey};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointMarker {
    pub point: PointId,
    pub x: f64,
    pub y: f64,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectSegment {
    pub lhs: PointId,
    pub rhs: PointId,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WheelLayout {
    pub key: LayoutKey,
    pub width: f64,
    pub height: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub zodiac_radius: f64,
    pub aspect_radius: f64,
    pub points: Vec<PointMarker>,
    pub aspects: Vec<AspectSegment>,
}

pub fn layout_wheel(
    snapshot: &ChartSnapshot,
    analysis: &ChartAnalysis,
    displayed_points: &PointSet,
    wheel: &WheelTemplate,
) -> Result<WheelLayout, LayoutError> {
    displayed_points.domain_validate()?;
    wheel.domain_validate()?;
    let center_x = 200.0;
    let center_y = 200.0;
    let zodiac_radius = wheel
        .rings
        .iter()
        .map(|ring| ring.geometry.outer_radius)
        .reduce(f64::max)
        .unwrap_or(150.0);
    let aspect_radius = wheel.aspect_field.radius;
    let mut points = Vec::new();
    let mut aspect_anchors = BTreeMap::new();
    let mut resolved_points = Vec::new();

    let mut point_ids = Vec::with_capacity(displayed_points.points.len());
    for selector in &displayed_points.points {
        match selector {
            PointSelector::Point(point) => point_ids.push(point.clone()),
            PointSelector::Category(category) => {
                return Err(LayoutError::UnresolvedPointCategory(category.clone()));
            }
        }
    }
    point_ids.sort();

    for point in &point_ids {
        let Some(state) = snapshot.calculation.point(point) else {
            continue;
        };
        resolved_points.push((point.clone(), state.longitude));
        let radians = (state.longitude.degrees() - 90.0).to_radians();
        let x = center_x + zodiac_radius * radians.cos();
        let y = center_y + zodiac_radius * radians.sin();
        let aspect_x = center_x + aspect_radius * radians.cos();
        let aspect_y = center_y + aspect_radius * radians.sin();
        aspect_anchors.insert(point.clone(), (aspect_x, aspect_y));
        points.push(PointMarker {
            point: point.clone(),
            x,
            y,
            label_x: center_x + (zodiac_radius + 22.0) * radians.cos(),
            label_y: center_y + (zodiac_radius + 22.0) * radians.sin(),
        });
    }

    let mut aspect_pairs = analysis
        .aspects
        .iter()
        .filter_map(|hit| {
            if !aspect_anchors.contains_key(&hit.lhs) || !aspect_anchors.contains_key(&hit.rhs) {
                return None;
            }
            let (lhs, rhs) = if hit.lhs <= hit.rhs {
                (hit.lhs.clone(), hit.rhs.clone())
            } else {
                (hit.rhs.clone(), hit.lhs.clone())
            };
            Some((lhs, rhs))
        })
        .collect::<Vec<_>>();
    aspect_pairs.sort();
    let aspects = aspect_pairs
        .iter()
        .filter_map(|(lhs, rhs)| {
            let (x1, y1) = aspect_anchors.get(lhs)?;
            let (x2, y2) = aspect_anchors.get(rhs)?;
            Some(AspectSegment {
                lhs: lhs.clone(),
                rhs: rhs.clone(),
                x1: *x1,
                y1: *y1,
                x2: *x2,
                y2: *y2,
            })
        })
        .collect();
    let key = LayoutKey::derive(
        &resolved_points,
        &aspect_pairs,
        zodiac_radius,
        aspect_radius,
        "wheel-layout-v1",
    )?;

    Ok(WheelLayout {
        key,
        width: 400.0,
        height: 400.0,
        center_x,
        center_y,
        zodiac_radius,
        aspect_radius,
        points,
        aspects,
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrokeRole {
    Foreground,
    Muted,
    Accent,
    Aspect,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FillRole {
    None,
    Background,
    Foreground,
    Accent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Circle {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub stroke: StrokeRole,
    pub fill: FillRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Line {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke: StrokeRole,
    pub width: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Path {
    pub data: String,
    pub stroke: StrokeRole,
    pub fill: FillRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Glyph {
    pub value: String,
    pub x: f64,
    pub y: f64,
    pub fill: FillRole,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Label {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub fill: FillRole,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Scene {
    pub circles: Vec<Circle>,
    pub lines: Vec<Line>,
    pub paths: Vec<Path>,
    pub glyphs: Vec<Glyph>,
    pub labels: Vec<Label>,
}

impl Scene {
    pub fn from_wheel(layout: &WheelLayout) -> Self {
        let mut scene = Self::default();
        scene.circles.push(Circle {
            cx: layout.center_x,
            cy: layout.center_y,
            radius: layout.zodiac_radius,
            stroke: StrokeRole::Foreground,
            fill: FillRole::Background,
        });
        scene.circles.push(Circle {
            cx: layout.center_x,
            cy: layout.center_y,
            radius: layout.aspect_radius,
            stroke: StrokeRole::Muted,
            fill: FillRole::None,
        });

        for index in 0..12 {
            let radians = (f64::from(index) * 30.0 - 90.0).to_radians();
            scene.lines.push(Line {
                x1: layout.center_x + layout.aspect_radius * radians.cos(),
                y1: layout.center_y + layout.aspect_radius * radians.sin(),
                x2: layout.center_x + layout.zodiac_radius * radians.cos(),
                y2: layout.center_y + layout.zodiac_radius * radians.sin(),
                stroke: StrokeRole::Muted,
                width: 1.0,
            });
        }
        scene
            .lines
            .extend(layout.aspects.iter().map(|segment| Line {
                x1: segment.x1,
                y1: segment.y1,
                x2: segment.x2,
                y2: segment.y2,
                stroke: StrokeRole::Aspect,
                width: 1.4,
            }));
        for marker in &layout.points {
            scene.circles.push(Circle {
                cx: marker.x,
                cy: marker.y,
                radius: 4.0,
                stroke: StrokeRole::Accent,
                fill: FillRole::Accent,
            });
            scene.labels.push(Label {
                text: marker.point.to_string(),
                x: marker.label_x,
                y: marker.label_y,
                fill: FillRole::Foreground,
            });
        }
        scene
    }
}

pub fn render_key(layout: &WheelLayout, theme: &Theme) -> Result<RenderKey, KeyError> {
    RenderKey::derive(&layout.key, theme, "leptos-svg-v1")
}

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error(transparent)]
    InvalidDomain(#[from] astra_core::DomainValidationError),
    #[error("point category {0:?} must be resolved before layout")]
    UnresolvedPointCategory(String),
}
