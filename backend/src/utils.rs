use serde_json::Value as JsonValue;
use serde_json::Value;
use std::collections::HashMap;

use rand::{distr::Alphanumeric, rng, Rng};
use validator::ValidationError;

pub fn create_id(length: u64) -> String {
    let code: String = (0..length)
        .map(|_| rng().sample(Alphanumeric) as char)
        .collect();
    code.to_uppercase()
}

pub fn generate_props_clause(properties: &HashMap<String, Value>) -> String {
    let prop_strings: Vec<String> = properties
        .iter()
        .map(|(key, value)| {
            let value_str = match value {
                Value::String(s) => format!("'{}'", s.replace("'", "\\'")),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => format!("'{}'", value.to_string().replace("'", "\\'")),
            };
            format!("{}: {}", key, value_str)
        })
        .collect();

    format!("{{{}}}", prop_strings.join(", "))
}

pub fn validate_label(label: &str) -> Result<(), ValidationError> {
    if !label
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_alphabetic())
    {
        return Err(ValidationError::new("label_must_start_with_letter"));
    }
    if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(ValidationError::new("invalid_label_characters"));
    }
    Ok(())
}

pub fn validate_properties(props: &HashMap<String, JsonValue>) -> Result<(), ValidationError> {
    // Check for maximum number of properties
    if props.len() > 100 {
        return Err(ValidationError::new("too_many_properties"));
    }

    // Validate property keys
    for key in props.keys() {
        if key.len() > 50 {
            return Err(ValidationError::new("property_key_too_long"));
        }
        if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ValidationError::new("invalid_property_key_characters"));
        }
    }

    // Validate property values
    for value in props.values() {
        match value {
            JsonValue::String(s) if s.len() > 1000 => {
                return Err(ValidationError::new("string_value_too_long"));
            }
            JsonValue::Array(arr) => {
                if arr.len() > 100 {
                    return Err(ValidationError::new("array_too_large"));
                }
                // Validate array elements
                for elem in arr {
                    match elem {
                        JsonValue::String(s) if s.len() > 1000 => {
                            return Err(ValidationError::new("array_string_too_long"));
                        }
                        JsonValue::Array(_) => {
                            return Err(ValidationError::new("nested_arrays_not_allowed"));
                        }
                        JsonValue::Object(_) => {
                            return Err(ValidationError::new("objects_in_arrays_not_allowed"));
                        }
                        JsonValue::Null => {
                            return Err(ValidationError::new("null_values_not_allowed"));
                        }
                        JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::String(_) => {}
                    }
                }
            }
            JsonValue::Object(_) => {
                return Err(ValidationError::new("nested_objects_not_allowed"));
            }
            JsonValue::Null => {
                return Err(ValidationError::new("null_values_not_allowed"));
            }
            JsonValue::Number(_) | JsonValue::Bool(_) | JsonValue::String(_) => {}
        }
    }

    // Add numeric validation
    for value in props.values() {
        if let JsonValue::Number(n) = value {
            if let Some(n) = n.as_f64() {
                if !n.is_finite() || n.abs() > 1e308 {
                    return Err(ValidationError::new("numeric_value_out_of_bounds"));
                }
            }
        }
    }

    Ok(())
}
