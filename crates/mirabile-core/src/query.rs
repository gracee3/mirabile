use serde::{Deserialize, Serialize};

use crate::{
    Angle, AspectId, DomainValidate, DomainValidationError, DomainValidationIssue, PointId,
    validation::{in_range, nonempty},
};

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

impl DomainValidate for QueryDefinition {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        self.expression.domain_validate()
    }
}

impl DomainValidate for QueryExpr {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        match self {
            Self::Predicate(predicate) => predicate.domain_validate(),
            Self::And(children) | Self::Or(children) => {
                if children.is_empty() {
                    return Err(DomainValidationError::new(
                        "value",
                        DomainValidationIssue::InvalidStructure {
                            requirement: "boolean groups must contain at least one expression"
                                .into(),
                        },
                    ));
                }
                for (index, child) in children.iter().enumerate() {
                    child
                        .domain_validate()
                        .map_err(|error| error.prepend(&format!("value[{index}]")))?;
                }
                Ok(())
            }
            Self::Not(child) => child.domain_validate(),
        }
    }
}

impl DomainValidate for Predicate {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        match self {
            Self::InSign { sign_index, .. } if *sign_index > 11 => Err(DomainValidationError::new(
                "sign_index",
                DomainValidationIssue::OutOfRange {
                    expected: "between 0 and 11 inclusive".into(),
                },
            )),
            Self::Aspect {
                orb_override: Some(orb),
                ..
            } => in_range(orb.degrees(), 0.0, 180.0, true, "orb_override"),
            Self::Longitude { value, .. } => in_range(value.degrees(), 0.0, 360.0, false, "value"),
            Self::ChartField { field, value, .. } => {
                nonempty(field, "field")?;
                nonempty(value, "value")
            }
            Self::InSign { .. } | Self::Aspect { .. } => Ok(()),
        }
    }
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
