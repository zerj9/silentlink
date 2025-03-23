use crate::utils::{validate_label, validate_properties};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use validator::{Validate, ValidationError};

#[derive(Debug, Serialize, Deserialize)]
struct Edge {
    id: Option<i64>,
    label: String,
    from_id: i64,
    to_id: i64,
    properties: HashMap<String, JsonValue>,
}

#[derive(Debug, Validate, Deserialize)]
pub struct CreateEdgeRequest {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom = "validate_edge_type")]
    label: String,

    #[validate(range(min = 0))]
    from_id: i64,

    #[validate(range(min = 0))]
    to_id: i64,

    #[validate(custom = "validate_properties")]
    properties: HashMap<String, JsonValue>,
}

// TODO: Add additional validation functions specific to edges
// TODO: Check if edge label has been created, and if not, return an error
fn validate_edge_type(label: &str) -> Result<(), ValidationError> {
    // Add any edge-specific label validation rules
    if label.len() > 50 {
        return Err(ValidationError::new("edge_label_too_long"));
    }

    // Call the existing label validation
    validate_label(label)?;

    // Add edge-specific checks
    if label.contains("->") || label.contains("<-") {
        return Err(ValidationError::new("edge_label_contains_arrow"));
    }

    Ok(())
}
