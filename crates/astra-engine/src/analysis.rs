use std::collections::BTreeMap;

use astra_core::{
    AnalysisProfile, Angle, AspectId, AspectSet, HouseSystem, PointId, PointSet, PointState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AnalysisKey, ChartSnapshot, KeyError};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AspectHit {
    pub lhs: PointId,
    pub rhs: PointId,
    pub aspect: AspectId,
    pub separation: Angle,
    pub orb: Angle,
    pub applying: Option<bool>,
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
    pub cross_aspects: Vec<AspectHit>,
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
        let selected: Vec<(&PointId, &PointState)> = points
            .direct_points()
            .filter_map(|id| snapshot.points.get_key_value(id))
            .collect();
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
