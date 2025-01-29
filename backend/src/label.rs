use crate::error::ApiError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLabelRequest {
    pub label: String,
}

impl CreateLabelRequest {
    // Validate label name according to common graph database naming rules
    pub fn validate(&self) -> Result<(), ApiError> {
        // Check if empty
        if self.label.is_empty() {
            return Err(ApiError::BadRequest("Label name cannot be empty".into()));
        }

        // Check length
        if self.label.len() > 50 {
            return Err(ApiError::BadRequest(
                "Label name too long (max 50 chars)".into(),
            ));
        }

        // Check if starts with letter
        if !self
            .label
            .chars()
            .next()
            .map_or(false, |c| c.is_ascii_alphabetic())
        {
            return Err(ApiError::BadRequest(
                "Label must start with a letter".into(),
            ));
        }

        // Check if contains only allowed characters (letters, numbers, and underscores)
        if !self
            .label
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(ApiError::BadRequest(
                "Label can only contain letters, numbers, and underscores".into(),
            ));
        }

        Ok(())
    }
}
