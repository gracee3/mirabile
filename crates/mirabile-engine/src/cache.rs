use std::collections::BTreeMap;

use crate::{
    AnalysisKey, CalcKey, CalculationValue, ChartAnalysis, ChartSnapshot, SnapshotContext,
};

#[derive(Clone, Debug, Default)]
pub struct ComputationCache {
    calculations: BTreeMap<CalcKey, CalculationValue>,
    analyses: BTreeMap<AnalysisKey, ChartAnalysis>,
}

impl ComputationCache {
    pub fn insert_snapshot(&mut self, snapshot: ChartSnapshot) {
        self.calculations
            .insert(snapshot.calc_key, snapshot.calculation);
    }

    pub fn insert_calculation(&mut self, key: CalcKey, calculation: CalculationValue) {
        self.calculations.insert(key, calculation);
    }

    pub fn insert_analysis(&mut self, analysis: ChartAnalysis) {
        self.analyses
            .insert(analysis.analysis_key.clone(), analysis);
    }

    pub fn calculation(&self, key: &CalcKey) -> Option<&CalculationValue> {
        self.calculations.get(key)
    }

    pub fn snapshot(&self, key: &CalcKey, context: SnapshotContext) -> Option<ChartSnapshot> {
        self.calculations
            .get(key)
            .cloned()
            .map(|calculation| ChartSnapshot {
                calc_key: key.clone(),
                context,
                calculation,
            })
    }

    pub fn analysis(&self, key: &AnalysisKey) -> Option<&ChartAnalysis> {
        self.analyses.get(key)
    }

    pub fn clear(&mut self) {
        self.calculations.clear();
        self.analyses.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.calculations.is_empty() && self.analyses.is_empty()
    }
}
