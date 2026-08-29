use mirabile_core::{CalculationSpec, ChartRecord};

/// User-facing unsaved chart aggregate.
///
/// A draft owns factual assertions and calculation semantics without claiming a canonical
/// resource identity. Saving creates the distinct `ChartRecord` and `ChartDefinition` resources.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartDraft {
    /// Display title for the saved `ChartDefinition`.
    pub title: String,
    pub definition_description: Option<String>,
    pub definition_tags: Vec<String>,
    pub record_title: String,
    pub record_description: Option<String>,
    pub record_tags: Vec<String>,
    pub record: ChartRecord,
    pub calculation: CalculationSpec,
}
