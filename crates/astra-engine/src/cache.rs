use std::collections::BTreeMap;

use crate::{AnalysisKey, CalcKey, ChartAnalysis, ChartSnapshot};

#[derive(Clone, Debug, Default)]
pub struct ComputationCache {
    snapshots: BTreeMap<CalcKey, ChartSnapshot>,
    analyses: BTreeMap<AnalysisKey, ChartAnalysis>,
}

impl ComputationCache {
    pub fn insert_snapshot(&mut self, snapshot: ChartSnapshot) {
        self.snapshots.insert(snapshot.calc_key.clone(), snapshot);
    }

    pub fn insert_analysis(&mut self, analysis: ChartAnalysis) {
        self.analyses
            .insert(analysis.analysis_key.clone(), analysis);
    }

    pub fn snapshot(&self, key: &CalcKey) -> Option<&ChartSnapshot> {
        self.snapshots.get(key)
    }

    pub fn analysis(&self, key: &AnalysisKey) -> Option<&ChartAnalysis> {
        self.analyses.get(key)
    }

    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.analyses.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty() && self.analyses.is_empty()
    }
}
