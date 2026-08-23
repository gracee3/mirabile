use mirabile_core::{CalculationSpec, ChartRecord};

/// User-facing unsaved chart aggregate.
///
/// A draft owns factual assertions and calculation semantics without claiming a canonical
/// resource identity. Saving creates the distinct `ChartRecord` and `ChartDefinition` resources.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartDraft {
    pub title: String,
    pub record: ChartRecord,
    pub calculation: CalculationSpec,
}
