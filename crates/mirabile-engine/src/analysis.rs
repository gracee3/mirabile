use std::collections::BTreeMap;

use mirabile_core::{
    AnalysisProfile, Angle, AspectClass, AspectId, AspectSet, HouseSystem, PointId, PointSet,
    PointState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AnalysisKey, ChartSnapshot, KeyError};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectHit {
    pub lhs: PointId,
    pub rhs: PointId,
    pub aspect: AspectId,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_aspect_classification")]
    pub classification: AspectClass,
    pub separation: Angle,
    pub orb: Angle,
    pub applying: Option<bool>,
}

const fn default_aspect_classification() -> AspectClass {
    AspectClass::Custom
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct AspectPattern {
    pub name: String,
    pub points: Vec<PointId>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MidpointIndex {
    pub entries: BTreeMap<String, Angle>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct HouseAnalysis {
    pub system: Option<HouseSystem>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartAnalysis {
    pub snapshot_key: crate::CalcKey,
    pub analysis_key: AnalysisKey,
    pub aspects: Vec<AspectHit>,
    pub patterns: Vec<AspectPattern>,
    pub midpoints: MidpointIndex,
    pub houses: HouseAnalysis,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RelationshipAnalysis {
    pub lhs: crate::CalcKey,
    pub rhs: crate::CalcKey,
    pub cross_aspects: Vec<RelationshipAspectHit>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OwnedPointRef {
    pub slot: mirabile_core::ChartSlotId,
    pub point: PointId,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RelationshipAspectHit {
    pub lhs: OwnedPointRef,
    pub rhs: OwnedPointRef,
    pub aspect: AspectId,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_aspect_classification")]
    pub classification: AspectClass,
    pub separation: Angle,
    pub orb: Angle,
    pub applying: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AspectAnalyzer;

impl AspectAnalyzer {
    pub fn analyze(
        snapshot: &ChartSnapshot,
        points: &PointSet,
        aspects: &AspectSet,
        profile: &AnalysisProfile,
    ) -> Result<ChartAnalysis, AnalysisError> {
        let calc_keys = [snapshot.calc_key.clone()];
        let analysis_key = AnalysisKey::derive(&calc_keys, points, aspects, profile)?;
        let mut selected: Vec<(&PointId, &PointState)> = points
            .direct_points()
            .filter_map(|id| snapshot.calculation.point_entry(id))
            .collect();
        selected.sort_by_key(|(point, _)| *point);
        let mut hits = Vec::new();

        for (index, (lhs_id, lhs)) in selected.iter().enumerate() {
            for (rhs_id, rhs) in selected.iter().skip(index + 1) {
                let separation = lhs.longitude.separation(rhs.longitude);
                for definition in aspects.aspects.iter().filter(|aspect| aspect.enabled) {
                    let orb_degrees = (separation.degrees() - definition.angle.degrees()).abs();
                    let applying = applying(lhs, rhs, definition.angle.degrees());
                    let multiplier = if applying {
                        definition.orbs.applying_multiplier
                    } else {
                        1.0
                    };
                    if orb_degrees <= definition.orbs.maximum.degrees() * multiplier {
                        hits.push(AspectHit {
                            lhs: (*lhs_id).clone(),
                            rhs: (*rhs_id).clone(),
                            aspect: definition.id.clone(),
                            name: definition.name.clone(),
                            classification: definition.classification,
                            separation,
                            orb: Angle::from_degrees(orb_degrees)
                                .map_err(|_| AnalysisError::NonFiniteAngle)?,
                            applying: profile.include_applying_state.then_some(applying),
                        });
                    }
                }
            }
        }

        hits.sort_by(|lhs, rhs| {
            lhs.orb
                .degrees()
                .total_cmp(&rhs.orb.degrees())
                .then_with(|| lhs.lhs.cmp(&rhs.lhs))
                .then_with(|| lhs.rhs.cmp(&rhs.rhs))
                .then_with(|| lhs.aspect.cmp(&rhs.aspect))
        });
        if let Some(maximum) = profile.maximum_hits {
            hits.truncate(maximum as usize);
        }

        Ok(ChartAnalysis {
            snapshot_key: snapshot.calc_key.clone(),
            analysis_key,
            aspects: hits,
            patterns: Vec::new(),
            midpoints: midpoint_index(&selected)?,
            houses: HouseAnalysis::default(),
        })
    }

    pub fn analyze_relationship(
        lhs_slot: &mirabile_core::ChartSlotId,
        lhs: &ChartSnapshot,
        rhs_slot: &mirabile_core::ChartSlotId,
        rhs: &ChartSnapshot,
        points: &PointSet,
        aspects: &AspectSet,
        profile: &AnalysisProfile,
    ) -> Result<RelationshipAnalysis, AnalysisError> {
        let mut lhs_points: Vec<_> = points
            .direct_points()
            .filter_map(|id| lhs.calculation.point_entry(id))
            .collect();
        let mut rhs_points: Vec<_> = points
            .direct_points()
            .filter_map(|id| rhs.calculation.point_entry(id))
            .collect();
        lhs_points.sort_by_key(|(id, _)| *id);
        rhs_points.sort_by_key(|(id, _)| *id);
        let mut hits = Vec::new();
        for (lhs_id, lhs_state) in lhs_points {
            for (rhs_id, rhs_state) in &rhs_points {
                let separation = lhs_state.longitude.separation(rhs_state.longitude);
                for definition in aspects.aspects.iter().filter(|aspect| aspect.enabled) {
                    let orb_degrees = (separation.degrees() - definition.angle.degrees()).abs();
                    let is_applying = applying(lhs_state, rhs_state, definition.angle.degrees());
                    let multiplier = if is_applying {
                        definition.orbs.applying_multiplier
                    } else {
                        1.0
                    };
                    if orb_degrees <= definition.orbs.maximum.degrees() * multiplier {
                        hits.push(RelationshipAspectHit {
                            lhs: OwnedPointRef {
                                slot: lhs_slot.clone(),
                                point: lhs_id.clone(),
                            },
                            rhs: OwnedPointRef {
                                slot: rhs_slot.clone(),
                                point: (*rhs_id).clone(),
                            },
                            aspect: definition.id.clone(),
                            name: definition.name.clone(),
                            classification: definition.classification,
                            separation,
                            orb: Angle::from_degrees(orb_degrees)
                                .map_err(|_| AnalysisError::NonFiniteAngle)?,
                            applying: profile.include_applying_state.then_some(is_applying),
                        });
                    }
                }
            }
        }
        hits.sort_by(|lhs, rhs| {
            lhs.orb
                .degrees()
                .total_cmp(&rhs.orb.degrees())
                .then_with(|| lhs.lhs.slot.cmp(&rhs.lhs.slot))
                .then_with(|| lhs.lhs.point.cmp(&rhs.lhs.point))
                .then_with(|| lhs.rhs.slot.cmp(&rhs.rhs.slot))
                .then_with(|| lhs.rhs.point.cmp(&rhs.rhs.point))
                .then_with(|| lhs.aspect.cmp(&rhs.aspect))
        });
        if let Some(maximum) = profile.maximum_hits {
            hits.truncate(maximum as usize);
        }
        Ok(RelationshipAnalysis {
            lhs: lhs.calc_key.clone(),
            rhs: rhs.calc_key.clone(),
            cross_aspects: hits,
        })
    }
}

fn applying(lhs: &PointState, rhs: &PointState, target: f64) -> bool {
    let current = (lhs.longitude.separation(rhs.longitude).degrees() - target).abs();
    let lhs_next = Angle::normalized(
        lhs.longitude.degrees() + lhs.speed_longitude.as_degrees_per_day() / 100.0,
    )
    .expect("finite point state");
    let rhs_next = Angle::normalized(
        rhs.longitude.degrees() + rhs.speed_longitude.as_degrees_per_day() / 100.0,
    )
    .expect("finite point state");
    let next = (lhs_next.separation(rhs_next).degrees() - target).abs();
    next < current
}

fn midpoint_index(selected: &[(&PointId, &PointState)]) -> Result<MidpointIndex, AnalysisError> {
    let mut entries = BTreeMap::new();
    for (index, (lhs_id, lhs)) in selected.iter().enumerate() {
        for (rhs_id, rhs) in selected.iter().skip(index + 1) {
            let mut delta = (rhs.longitude.degrees() - lhs.longitude.degrees()).rem_euclid(360.0);
            if delta > 180.0 {
                delta -= 360.0;
            }
            let midpoint = lhs.longitude.degrees() + delta / 2.0;
            entries.insert(
                format!("{lhs_id}/{rhs_id}"),
                Angle::normalized(midpoint).map_err(|_| AnalysisError::NonFiniteAngle)?,
            );
        }
    }
    Ok(MidpointIndex { entries })
}

#[derive(Debug, Error)]
pub enum AnalysisError {
    #[error(transparent)]
    Key(#[from] KeyError),
    #[error("analysis produced a non-finite angle")]
    NonFiniteAngle,
}
