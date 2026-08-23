use astra_core::{ChartDefinition, ChartRecord, ResourceEnvelope};
use serde::{Deserialize, Serialize};

use crate::ChartSnapshot;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalcRequest {
    pub request_id: String,
    pub definition: ResourceEnvelope<ChartDefinition>,
    pub record: ResourceEnvelope<ChartRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CalcResult {
    Completed {
        request_id: String,
        snapshot: Box<ChartSnapshot>,
    },
    Failed {
        request_id: String,
        message: String,
    },
}
