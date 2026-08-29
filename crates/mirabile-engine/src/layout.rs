use std::collections::BTreeMap;

use mirabile_core::{
    Angle, AspectClass, DomainValidate, PointId, PointSelector, PointSet, Theme, WheelTemplate,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AspectHit, ChartAnalysis, ChartSnapshot, KeyError, LayoutKey, RenderKey};

const REGULAR_SIZE: f64 = 520.0;
const CANVAS_MARGIN: f64 = 12.0;
const ZODIAC_BAND_WIDTH: f64 = 30.0;
const ZODIAC_GAP: f64 = 8.0;
const LABEL_OFFSET: f64 = 58.0;
const LABEL_LANE_SPACING: f64 = 16.0;
const LABEL_LANES: usize = 3;
const LABEL_LANE_COUNT: f64 = 2.0;
const LABEL_DISPLACEMENT_THRESHOLD: f64 = 1.5;
const LABEL_ANGULAR_GAP: f64 = 1.5;
const LABEL_CHARACTER_WIDTH: f64 = 6.2;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct WheelLayoutBounds {
    pub width: f64,
    pub height: f64,
}

impl Default for WheelLayoutBounds {
    fn default() -> Self {
        Self {
            width: REGULAR_SIZE,
            height: REGULAR_SIZE,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LineGeometry {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PointMarker {
    pub point: PointId,
    pub x: f64,
    pub y: f64,
    pub label_x: f64,
    pub label_y: f64,
    #[serde(default)]
    pub longitude_degrees: f64,
    #[serde(default)]
    pub latitude_degrees: f64,
    #[serde(default)]
    pub screen_angle_degrees: f64,
    #[serde(default)]
    pub label_angle_degrees: f64,
    #[serde(default)]
    pub label_radius: f64,
    #[serde(default)]
    pub label_lane: usize,
    #[serde(default)]
    pub label_width: f64,
    #[serde(default)]
    pub glyph: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub glyph_fallback: bool,
    #[serde(default)]
    pub formatted_position: String,
    #[serde(default)]
    pub display_label: String,
    #[serde(default)]
    pub retrograde: bool,
    #[serde(default)]
    pub show_retrograde: bool,
    #[serde(default)]
    pub leader: Option<LineGeometry>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectVisualStyle {
    Conjunction,
    Opposition,
    Square,
    Trine,
    Sextile,
    Quincunx,
    #[default]
    Neutral,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectSegment {
    pub lhs: PointId,
    pub rhs: PointId,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    #[serde(default)]
    pub aspect_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_aspect_classification")]
    pub classification: AspectClass,
    #[serde(default)]
    pub separation_degrees: f64,
    #[serde(default)]
    pub orb_degrees: f64,
    #[serde(default)]
    pub applying: Option<bool>,
    #[serde(default = "default_draw_chord")]
    pub draw_chord: bool,
    #[serde(default)]
    pub style: AspectVisualStyle,
}

const fn default_aspect_classification() -> AspectClass {
    AspectClass::Custom
}

const fn default_draw_chord() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ZodiacDivision {
    pub index: usize,
    pub id: String,
    pub name: String,
    pub glyph: String,
    pub longitude_degrees: f64,
    pub screen_angle_degrees: f64,
    pub line: LineGeometry,
    pub label_x: f64,
    pub label_y: f64,
    pub show_boundary: bool,
    pub show_label: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HouseMarker {
    pub number: usize,
    pub cusp_longitude_degrees: f64,
    pub screen_angle_degrees: f64,
    pub line: LineGeometry,
    pub number_x: f64,
    pub number_y: f64,
    pub show_cusp: bool,
    pub show_number: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartAngleMarker {
    pub id: String,
    pub name: String,
    pub abbreviation: String,
    pub longitude_degrees: f64,
    pub screen_angle_degrees: f64,
    pub derived_opposite: bool,
    pub line: LineGeometry,
    pub label_x: f64,
    pub label_y: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FormattedLongitude {
    pub sign_id: String,
    pub sign_name: String,
    pub sign_glyph: String,
    pub degree: u8,
    pub minute: u8,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WheelLayout {
    pub key: LayoutKey,
    pub width: f64,
    pub height: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub rotation_degrees: f64,
    pub zodiac_radius: f64,
    pub zodiac_outer_radius: f64,
    pub aspect_radius: f64,
    pub zodiac: Vec<ZodiacDivision>,
    pub houses: Vec<HouseMarker>,
    pub angles: Vec<ChartAngleMarker>,
    pub points: Vec<PointMarker>,
    pub aspects: Vec<AspectSegment>,
}

#[derive(Serialize)]
struct LayoutMaterial<'a> {
    bounds: WheelLayoutBounds,
    rotation_degrees: f64,
    zodiac_radius: f64,
    zodiac_outer_radius: f64,
    aspect_radius: f64,
    zodiac: &'a [ZodiacDivision],
    houses: &'a [HouseMarker],
    angles: &'a [ChartAngleMarker],
    points: &'a [PointMarker],
    aspects: &'a [AspectSegment],
}

pub fn layout_wheel(
    snapshot: &ChartSnapshot,
    analysis: &ChartAnalysis,
    displayed_points: &PointSet,
    wheel: &WheelTemplate,
) -> Result<WheelLayout, LayoutError> {
    layout_wheel_in_bounds(
        snapshot,
        analysis,
        displayed_points,
        wheel,
        None,
        WheelLayoutBounds::default(),
    )
}

pub fn layout_wheel_with_rotation(
    snapshot: &ChartSnapshot,
    analysis: &ChartAnalysis,
    displayed_points: &PointSet,
    wheel: &WheelTemplate,
    rotation: Option<Angle>,
) -> Result<WheelLayout, LayoutError> {
    layout_wheel_in_bounds(
        snapshot,
        analysis,
        displayed_points,
        wheel,
        rotation,
        WheelLayoutBounds::default(),
    )
}

#[allow(clippy::too_many_lines)]
pub fn layout_wheel_in_bounds(
    snapshot: &ChartSnapshot,
    analysis: &ChartAnalysis,
    displayed_points: &PointSet,
    wheel: &WheelTemplate,
    rotation_override: Option<Angle>,
    bounds: WheelLayoutBounds,
) -> Result<WheelLayout, LayoutError> {
    displayed_points.domain_validate()?;
    wheel.domain_validate()?;
    validate_bounds(bounds)?;

    let center_x = bounds.width / 2.0;
    let center_y = bounds.height / 2.0;
    let raw_zodiac_radius = wheel
        .rings
        .iter()
        .map(|ring| ring.geometry.outer_radius)
        .reduce(f64::max)
        .unwrap_or(150.0);
    let raw_point_radius = wheel
        .rings
        .iter()
        .map(|ring| f64::midpoint(ring.geometry.inner_radius, ring.geometry.outer_radius))
        .reduce(f64::max)
        .unwrap_or(raw_zodiac_radius - 12.0);
    let raw_maximum_radius =
        raw_zodiac_radius + LABEL_OFFSET + LABEL_LANE_SPACING * LABEL_LANE_COUNT;
    let available_radius = bounds.width.min(bounds.height) / 2.0 - CANVAS_MARGIN;
    let scale = (available_radius / raw_maximum_radius).min(1.0);
    let zodiac_radius = raw_zodiac_radius * scale;
    let point_radius = raw_point_radius * scale;
    let zodiac_inner_radius = (raw_zodiac_radius + ZODIAC_GAP) * scale;
    let zodiac_outer_radius = (raw_zodiac_radius + ZODIAC_GAP + ZODIAC_BAND_WIDTH) * scale;
    let aspect_radius = wheel.aspect_field.radius * scale;
    let label_base_radius = (raw_zodiac_radius + LABEL_OFFSET) * scale;
    let label_lane_spacing = LABEL_LANE_SPACING * scale;
    let rotation_degrees = rotation_override.map_or_else(
        || {
            snapshot
                .calculation
                .angles
                .ascendant
                .map_or(0.0, |ascendant| 270.0 - ascendant.degrees())
        },
        Angle::degrees,
    );
    let rotation_degrees = normalize_degrees(rotation_degrees);

    let zodiac = zodiac_divisions(
        center_x,
        center_y,
        zodiac_inner_radius,
        zodiac_outer_radius,
        rotation_degrees,
        wheel,
    );
    let houses = house_markers(
        snapshot,
        center_x,
        center_y,
        aspect_radius,
        zodiac_radius,
        rotation_degrees,
        wheel,
    );
    let angles = chart_angle_markers(
        snapshot,
        center_x,
        center_y,
        aspect_radius * 0.72,
        zodiac_outer_radius,
        rotation_degrees,
    );
    let points = point_markers(
        snapshot,
        displayed_points,
        center_x,
        center_y,
        point_radius,
        label_base_radius,
        label_lane_spacing,
        rotation_degrees,
        wheel,
    )?;
    let point_anchors = points
        .iter()
        .map(|point| (point.point.clone(), (point.x, point.y)))
        .collect::<BTreeMap<_, _>>();
    let aspects = aspect_segments(analysis, &point_anchors, center_x, center_y, aspect_radius);

    let key = LayoutKey::derive(
        &LayoutMaterial {
            bounds,
            rotation_degrees,
            zodiac_radius,
            zodiac_outer_radius,
            aspect_radius,
            zodiac: &zodiac,
            houses: &houses,
            angles: &angles,
            points: &points,
            aspects: &aspects,
        },
        "professional-wheel-layout-v2",
    )?;

    Ok(WheelLayout {
        key,
        width: bounds.width,
        height: bounds.height,
        center_x,
        center_y,
        rotation_degrees,
        zodiac_radius,
        zodiac_outer_radius,
        aspect_radius,
        zodiac,
        houses,
        angles,
        points,
        aspects,
    })
}

fn validate_bounds(bounds: WheelLayoutBounds) -> Result<(), LayoutError> {
    if bounds.width.is_finite()
        && bounds.height.is_finite()
        && bounds.width >= 320.0
        && bounds.height >= 320.0
    {
        Ok(())
    } else {
        Err(LayoutError::InvalidBounds)
    }
}

fn zodiac_divisions(
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
    outer_radius: f64,
    rotation: f64,
    wheel: &WheelTemplate,
) -> Vec<ZodiacDivision> {
    zodiac_metadata()
        .iter()
        .enumerate()
        .map(|(index, (id, name, glyph))| {
            let longitude = f64::from(u32::try_from(index).expect("zodiac index")) * 30.0;
            let boundary_angle = screen_angle(longitude, rotation);
            let label_angle = screen_angle(longitude + 15.0, rotation);
            let (x1, y1) = polar(center_x, center_y, inner_radius, boundary_angle);
            let (x2, y2) = polar(center_x, center_y, outer_radius, boundary_angle);
            let (label_x, label_y) = polar(
                center_x,
                center_y,
                f64::midpoint(inner_radius, outer_radius),
                label_angle,
            );
            ZodiacDivision {
                index,
                id: (*id).into(),
                name: (*name).into(),
                glyph: (*glyph).into(),
                longitude_degrees: longitude,
                screen_angle_degrees: boundary_angle,
                line: LineGeometry { x1, y1, x2, y2 },
                label_x,
                label_y,
                show_boundary: wheel.zodiac.show_boundaries,
                show_label: wheel.zodiac.show_labels,
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn house_markers(
    snapshot: &ChartSnapshot,
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
    outer_radius: f64,
    rotation: f64,
    wheel: &WheelTemplate,
) -> Vec<HouseMarker> {
    let Some(houses) = &snapshot.calculation.houses else {
        return Vec::new();
    };
    houses
        .cusps
        .iter()
        .enumerate()
        .map(|(index, cusp)| {
            let next = houses
                .cusps
                .get(index + 1)
                .unwrap_or_else(|| &houses.cusps[0]);
            let cusp_longitude = normalize_degrees(cusp.degrees());
            let directed_arc = (next.degrees() - cusp.degrees()).rem_euclid(360.0);
            let midpoint = normalize_degrees(cusp.degrees() + directed_arc / 2.0);
            let cusp_angle = screen_angle(cusp_longitude, rotation);
            let number_angle = screen_angle(midpoint, rotation);
            let (x1, y1) = polar(center_x, center_y, inner_radius, cusp_angle);
            let (x2, y2) = polar(center_x, center_y, outer_radius, cusp_angle);
            let (number_x, number_y) = polar(
                center_x,
                center_y,
                f64::midpoint(inner_radius, outer_radius),
                number_angle,
            );
            HouseMarker {
                number: index + 1,
                cusp_longitude_degrees: cusp_longitude,
                screen_angle_degrees: cusp_angle,
                line: LineGeometry { x1, y1, x2, y2 },
                number_x,
                number_y,
                show_cusp: wheel.houses.show_cusps,
                show_number: wheel.houses.show_numbers,
            }
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn chart_angle_markers(
    snapshot: &ChartSnapshot,
    center_x: f64,
    center_y: f64,
    inner_radius: f64,
    outer_radius: f64,
    rotation: f64,
) -> Vec<ChartAngleMarker> {
    let mut angles = Vec::new();
    if let Some(ascendant) = snapshot.calculation.angles.ascendant {
        angles.push(("asc", "Ascendant", "ASC", ascendant.degrees(), false));
        angles.push((
            "dsc",
            "Descendant",
            "DSC",
            ascendant.degrees() + 180.0,
            true,
        ));
    }
    if let Some(midheaven) = snapshot.calculation.angles.midheaven {
        angles.push(("mc", "Midheaven", "MC", midheaven.degrees(), false));
        angles.push(("ic", "Imum Coeli", "IC", midheaven.degrees() + 180.0, true));
    }
    angles
        .into_iter()
        .map(|(id, name, abbreviation, longitude, derived_opposite)| {
            let longitude_degrees = normalize_degrees(longitude);
            let screen_angle_degrees = screen_angle(longitude_degrees, rotation);
            let (x1, y1) = polar(center_x, center_y, inner_radius, screen_angle_degrees);
            let (x2, y2) = polar(center_x, center_y, outer_radius, screen_angle_degrees);
            let (label_x, label_y) = polar(
                center_x,
                center_y,
                outer_radius - 12.0,
                screen_angle_degrees,
            );
            ChartAngleMarker {
                id: id.into(),
                name: name.into(),
                abbreviation: abbreviation.into(),
                longitude_degrees,
                screen_angle_degrees,
                derived_opposite,
                line: LineGeometry { x1, y1, x2, y2 },
                label_x,
                label_y,
            }
        })
        .collect()
}

struct PendingPoint<'a> {
    point: PointId,
    state: &'a mirabile_core::PointState,
    longitude: f64,
    label: String,
    glyph: String,
    name: String,
    fallback: bool,
    formatted_position: String,
    label_width: f64,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn point_markers(
    snapshot: &ChartSnapshot,
    displayed_points: &PointSet,
    center_x: f64,
    center_y: f64,
    point_radius: f64,
    label_base_radius: f64,
    lane_spacing: f64,
    rotation: f64,
    wheel: &WheelTemplate,
) -> Result<Vec<PointMarker>, LayoutError> {
    let mut points = Vec::new();
    for selector in &displayed_points.points {
        let point = match selector {
            PointSelector::Point(point) => point,
            PointSelector::Category(category) => {
                return Err(LayoutError::UnresolvedPointCategory(category.clone()));
            }
        };
        let Some(state) = snapshot.calculation.point(point) else {
            continue;
        };
        let longitude = normalize_degrees(state.longitude.degrees());
        let (glyph, name, fallback) = point_metadata(point);
        let formatted = format_longitude(longitude);
        let display_label = if wheel.labels.show_degrees {
            format!("{glyph} {name} · {}", formatted.text)
        } else {
            format!("{glyph} {name}")
        };
        let label_width = estimated_text_width(&display_label)
            + if state.retrograde && wheel.labels.show_retrograde {
                LABEL_CHARACTER_WIDTH * 2.0
            } else {
                0.0
            };
        points.push(PendingPoint {
            point: point.clone(),
            state,
            longitude,
            label: display_label,
            glyph,
            name,
            fallback,
            formatted_position: formatted.text,
            label_width,
        });
    }
    points.sort_by(|lhs, rhs| {
        lhs.longitude
            .total_cmp(&rhs.longitude)
            .then_with(|| lhs.point.cmp(&rhs.point))
    });
    if points.is_empty() {
        return Ok(Vec::new());
    }

    let break_after = largest_gap_index(&points);
    let start = (break_after + 1) % points.len();
    let sequence = points.iter().cycle().skip(start).take(points.len());
    let mut previous_target = points[start].longitude;
    let mut last_by_lane = [None::<(f64, f64)>; LABEL_LANES];
    let mut output = Vec::with_capacity(points.len());

    for (index, pending) in sequence.enumerate() {
        let mut target = pending.longitude;
        if index > 0 {
            while target < previous_target {
                target += 360.0;
            }
        }
        previous_target = target;
        let mut best = None::<(f64, usize)>;
        for (lane, previous) in last_by_lane.iter().enumerate() {
            let lane_number = u32::try_from(lane).expect("lane index");
            let radius = label_base_radius + lane_spacing * f64::from(lane_number);
            let shift = previous.map_or(0.0, |(previous_angle, previous_width)| {
                let separation =
                    minimum_label_separation(previous_width, pending.label_width, radius);
                (previous_angle + separation - target).max(0.0)
            });
            if best.is_none_or(|current| (shift, lane) < current) {
                best = Some((shift, lane));
            }
        }
        let (shift, lane) = best.expect("at least one label lane");
        let placed_longitude = target + shift;
        last_by_lane[lane] = Some((placed_longitude, pending.label_width));
        let marker_angle = screen_angle(pending.longitude, rotation);
        let label_angle = screen_angle(placed_longitude, rotation);
        let lane_number = u32::try_from(lane).expect("lane index");
        let label_radius = label_base_radius + lane_spacing * f64::from(lane_number);
        let (x, y) = polar(center_x, center_y, point_radius, marker_angle);
        let (label_x, label_y) = polar(center_x, center_y, label_radius, label_angle);
        let displaced = angular_distance(pending.longitude, normalize_degrees(placed_longitude));
        let leader = (lane > 0 || displaced > LABEL_DISPLACEMENT_THRESHOLD).then(|| {
            let (x2, y2) = polar(
                center_x,
                center_y,
                (label_radius - 7.0).max(point_radius),
                label_angle,
            );
            LineGeometry {
                x1: x,
                y1: y,
                x2,
                y2,
            }
        });
        output.push(PointMarker {
            point: pending.point.clone(),
            x,
            y,
            label_x,
            label_y,
            longitude_degrees: pending.longitude,
            latitude_degrees: pending.state.latitude.degrees(),
            screen_angle_degrees: marker_angle,
            label_angle_degrees: label_angle,
            label_radius,
            label_lane: lane,
            label_width: pending.label_width,
            glyph: pending.glyph.clone(),
            name: pending.name.clone(),
            glyph_fallback: pending.fallback,
            formatted_position: pending.formatted_position.clone(),
            display_label: pending.label.clone(),
            retrograde: pending.state.retrograde,
            show_retrograde: wheel.labels.show_retrograde,
            leader,
        });
    }
    reconcile_wrap_seam(&mut output, center_x, center_y);
    Ok(output)
}

fn largest_gap_index(points: &[PendingPoint<'_>]) -> usize {
    let mut largest = (f64::NEG_INFINITY, 0_usize);
    for index in 0..points.len() {
        let current = points[index].longitude;
        let next = points
            .get(index + 1)
            .map_or(points[0].longitude + 360.0, |point| point.longitude);
        let gap = next - current;
        if gap > largest.0 {
            largest = (gap, index);
        }
    }
    largest.1
}

fn reconcile_wrap_seam(points: &mut [PointMarker], center_x: f64, center_y: f64) {
    for lane in 0..LABEL_LANES {
        let indices = points
            .iter()
            .enumerate()
            .filter_map(|(index, point)| (point.label_lane == lane).then_some(index))
            .collect::<Vec<_>>();
        let (Some(&first_index), Some(&last_index)) = (indices.first(), indices.last()) else {
            continue;
        };
        if first_index == last_index {
            continue;
        }
        let first = &points[first_index];
        let last = &points[last_index];
        let wrap_gap = (first.label_angle_degrees - last.label_angle_degrees).rem_euclid(360.0);
        let required = minimum_label_separation(
            last.label_width,
            first.label_width,
            first.label_radius.min(last.label_radius),
        );
        if wrap_gap + f64::EPSILON >= required {
            continue;
        }
        let adjustment = (required - wrap_gap) / 2.0;
        for (index, direction) in [(first_index, 1.0), (last_index, -1.0)] {
            let point = &mut points[index];
            point.label_angle_degrees =
                normalize_degrees(point.label_angle_degrees + direction * adjustment);
            (point.label_x, point.label_y) = polar(
                center_x,
                center_y,
                point.label_radius,
                point.label_angle_degrees,
            );
            if let Some(leader) = &mut point.leader {
                (leader.x2, leader.y2) = polar(
                    center_x,
                    center_y,
                    (point.label_radius - 7.0).max(0.0),
                    point.label_angle_degrees,
                );
            }
        }
    }
    for point in points {
        let displaced = angular_distance(point.longitude_degrees, point.label_angle_degrees);
        if point.leader.is_none()
            && (point.label_lane > 0 || displaced > LABEL_DISPLACEMENT_THRESHOLD)
        {
            let (x2, y2) = polar(
                center_x,
                center_y,
                (point.label_radius - 7.0).max(0.0),
                point.label_angle_degrees,
            );
            point.leader = Some(LineGeometry {
                x1: point.x,
                y1: point.y,
                x2,
                y2,
            });
        }
    }
}

fn minimum_label_separation(lhs_width: f64, rhs_width: f64, radius: f64) -> f64 {
    let half_span = ((lhs_width + rhs_width) / 4.0).min(radius * 0.95);
    (2.0 * (half_span / radius).asin().to_degrees()).max(4.0) + LABEL_ANGULAR_GAP
}

fn aspect_segments(
    analysis: &ChartAnalysis,
    point_anchors: &BTreeMap<PointId, (f64, f64)>,
    center_x: f64,
    center_y: f64,
    aspect_radius: f64,
) -> Vec<AspectSegment> {
    let mut hits = analysis.aspects.iter().collect::<Vec<_>>();
    hits.sort_by(|lhs, rhs| {
        lhs.lhs
            .cmp(&rhs.lhs)
            .then_with(|| lhs.rhs.cmp(&rhs.rhs))
            .then_with(|| lhs.aspect.cmp(&rhs.aspect))
            .then_with(|| lhs.orb.degrees().total_cmp(&rhs.orb.degrees()))
    });
    hits.into_iter()
        .map(|hit| {
            let lhs = point_anchors
                .get(&hit.lhs)
                .map(|(x, y)| project_to_radius(*x, *y, center_x, center_y, aspect_radius));
            let rhs = point_anchors
                .get(&hit.rhs)
                .map(|(x, y)| project_to_radius(*x, *y, center_x, center_y, aspect_radius));
            let ((x1, y1), (x2, y2)) = lhs.zip(rhs).unwrap_or(((0.0, 0.0), (0.0, 0.0)));
            let style = aspect_style(hit);
            AspectSegment {
                lhs: hit.lhs.clone(),
                rhs: hit.rhs.clone(),
                x1,
                y1,
                x2,
                y2,
                aspect_id: hit.aspect.to_string(),
                name: hit.name.clone(),
                classification: hit.classification,
                separation_degrees: hit.separation.degrees(),
                orb_degrees: hit.orb.degrees(),
                applying: hit.applying,
                draw_chord: !matches!(style, AspectVisualStyle::Conjunction)
                    && lhs.is_some()
                    && rhs.is_some(),
                style,
            }
        })
        .collect()
}

fn aspect_style(hit: &AspectHit) -> AspectVisualStyle {
    match hit.aspect.as_str() {
        "conjunction" => AspectVisualStyle::Conjunction,
        "opposition" => AspectVisualStyle::Opposition,
        "square" => AspectVisualStyle::Square,
        "trine" => AspectVisualStyle::Trine,
        "sextile" => AspectVisualStyle::Sextile,
        "quincunx" | "inconjunct" => AspectVisualStyle::Quincunx,
        _ => AspectVisualStyle::Neutral,
    }
}

fn project_to_radius(x: f64, y: f64, center_x: f64, center_y: f64, radius: f64) -> (f64, f64) {
    let angle = (y - center_y).atan2(x - center_x);
    (
        center_x + radius * angle.cos(),
        center_y + radius * angle.sin(),
    )
}

pub fn format_longitude(longitude: f64) -> FormattedLongitude {
    let normalized = if longitude.is_finite() {
        normalize_degrees(longitude)
    } else {
        0.0
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let total_minutes = (normalized * 60.0).round() as u32;
    let wrapped_minutes = total_minutes % (360 * 60);
    let sign_index = usize::try_from(wrapped_minutes / (30 * 60)).unwrap_or(0);
    let within_sign = wrapped_minutes % (30 * 60);
    let degree = u8::try_from(within_sign / 60).unwrap_or(0);
    let minute = u8::try_from(within_sign % 60).unwrap_or(0);
    let (id, name, glyph) = zodiac_metadata()[sign_index];
    FormattedLongitude {
        sign_id: id.into(),
        sign_name: name.into(),
        sign_glyph: glyph.into(),
        degree,
        minute,
        text: format!("{degree:02}°{minute:02}′ {glyph} {name}"),
    }
}

fn point_metadata(point: &PointId) -> (String, String, bool) {
    let (glyph, name) = match point.as_str() {
        "sun" => ("☉", "Sun"),
        "moon" => ("☽", "Moon"),
        "mercury" => ("☿", "Mercury"),
        "venus" => ("♀", "Venus"),
        "mars" => ("♂", "Mars"),
        "jupiter" => ("♃", "Jupiter"),
        value => return (value.into(), readable_identifier(value), true),
    };
    (glyph.into(), name.into(), false)
}

fn readable_identifier(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut characters = segment.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn zodiac_metadata() -> &'static [(&'static str, &'static str, &'static str); 12] {
    &[
        ("aries", "Aries", "♈"),
        ("taurus", "Taurus", "♉"),
        ("gemini", "Gemini", "♊"),
        ("cancer", "Cancer", "♋"),
        ("leo", "Leo", "♌"),
        ("virgo", "Virgo", "♍"),
        ("libra", "Libra", "♎"),
        ("scorpio", "Scorpio", "♏"),
        ("sagittarius", "Sagittarius", "♐"),
        ("capricorn", "Capricorn", "♑"),
        ("aquarius", "Aquarius", "♒"),
        ("pisces", "Pisces", "♓"),
    ]
}

fn estimated_text_width(value: &str) -> f64 {
    f64::from(u32::try_from(value.chars().count()).unwrap_or(u32::MAX)) * LABEL_CHARACTER_WIDTH
}

fn polar(center_x: f64, center_y: f64, radius: f64, screen_angle_degrees: f64) -> (f64, f64) {
    let radians = screen_angle_degrees.to_radians();
    (
        center_x + radius * radians.cos(),
        center_y + radius * radians.sin(),
    )
}

fn screen_angle(longitude: f64, rotation: f64) -> f64 {
    normalize_degrees(longitude + rotation - 90.0)
}

fn normalize_degrees(value: f64) -> f64 {
    value.rem_euclid(360.0)
}

fn angular_distance(lhs: f64, rhs: f64) -> f64 {
    let difference = (lhs - rhs).rem_euclid(360.0);
    difference.min(360.0 - difference)
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
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub circles: Vec<Circle>,
    #[serde(default)]
    pub lines: Vec<Line>,
    #[serde(default)]
    pub paths: Vec<Path>,
    #[serde(default)]
    pub glyphs: Vec<Glyph>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub zodiac: Vec<ZodiacDivision>,
    #[serde(default)]
    pub houses: Vec<HouseMarker>,
    #[serde(default)]
    pub angles: Vec<ChartAngleMarker>,
    #[serde(default)]
    pub points: Vec<PointMarker>,
    #[serde(default)]
    pub aspects: Vec<AspectSegment>,
}

impl Scene {
    pub fn from_wheel(layout: &WheelLayout) -> Self {
        let mut scene = Self {
            width: layout.width,
            height: layout.height,
            zodiac: layout.zodiac.clone(),
            houses: layout.houses.clone(),
            angles: layout.angles.clone(),
            points: layout.points.clone(),
            aspects: layout.aspects.clone(),
            ..Self::default()
        };
        scene.circles.extend([
            Circle {
                cx: layout.center_x,
                cy: layout.center_y,
                radius: layout.zodiac_radius,
                stroke: StrokeRole::Foreground,
                fill: FillRole::Background,
            },
            Circle {
                cx: layout.center_x,
                cy: layout.center_y,
                radius: layout.zodiac_outer_radius,
                stroke: StrokeRole::Foreground,
                fill: FillRole::None,
            },
            Circle {
                cx: layout.center_x,
                cy: layout.center_y,
                radius: layout.aspect_radius,
                stroke: StrokeRole::Muted,
                fill: FillRole::None,
            },
        ]);
        scene
    }
}

pub fn render_key(layout: &WheelLayout, theme: &Theme) -> Result<RenderKey, KeyError> {
    RenderKey::derive(&layout.key, theme, "leptos-semantic-svg-v2")
}

#[derive(Debug, Error)]
pub enum LayoutError {
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error(transparent)]
    InvalidDomain(#[from] mirabile_core::DomainValidationError),
    #[error("point category {0:?} must be resolved before layout")]
    UnresolvedPointCategory(String),
    #[error("wheel layout bounds must be finite and at least 320 by 320")]
    InvalidBounds,
}
