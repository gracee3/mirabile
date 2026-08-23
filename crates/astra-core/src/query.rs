use serde::{Deserialize, Serialize};

use crate::{Angle, AspectId, PointId};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QueryDefinition {
    pub expression: QueryExpr,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum QueryExpr {
    Predicate(Predicate),
    And(Vec<QueryExpr>),
    Or(Vec<QueryExpr>),
    Not(Box<QueryExpr>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Predicate {
    InSign {
        point: PointId,
        sign_index: u8,
    },
    Aspect {
        lhs: PointId,
        rhs: PointId,
        aspect: AspectId,
        orb_override: Option<Angle>,
    },
    Longitude {
        point: PointId,
        comparison: NumericComparison,
        value: Angle,
    },
    ChartField {
        field: String,
        comparison: TextComparison,
        value: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericComparison {
    LessThan,
    LessThanOrEqual,
    Equal,
    GreaterThanOrEqual,
    GreaterThan,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextComparison {
    Equal,
    Contains,
    StartsWith,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_ast_preserves_boolean_structure() {
        let point = PointId::new("sun").expect("valid ID");
        let expression = QueryExpr::Not(Box::new(QueryExpr::Or(vec![
            QueryExpr::Predicate(Predicate::InSign {
                point: point.clone(),
                sign_index: 0,
            }),
            QueryExpr::Predicate(Predicate::InSign {
                point,
                sign_index: 6,
            }),
        ])));

        let json = serde_json::to_string(&expression).expect("serialize query");
        let decoded: QueryExpr = serde_json::from_str(&json).expect("deserialize query");
        assert_eq!(decoded, expression);
    }
}
