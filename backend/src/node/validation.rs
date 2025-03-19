use super::AttributeDefinition;
use crate::error::ApiError;
use sqlx::PgPool;
use tracing::error;

pub async fn validate_node_properties(
    pool: &PgPool,
    graph_id: &str,
    type_name: &str,
    properties: &serde_json::Value,
) -> Result<(), ApiError> {
    // Get all attributes for this node type
    let attributes = sqlx::query_as::<_, AttributeDefinition>(
        "SELECT attribute_name, data_type, required FROM node_type_attributes 
         WHERE graph_id = $1 AND type_name = $2",
    )
    .bind(graph_id)
    .bind(type_name)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch node type attributes: {}", e);
        ApiError::InternalServerError
    })?;

    if let serde_json::Value::Object(prop_map) = properties {
        // Check for required attributes
        for attr in &attributes {
            if attr.required && !prop_map.contains_key(&attr.name) {
                return Err(ApiError::BadRequest(format!(
                    "Required attribute '{}' is missing",
                    attr.name
                )));
            }

            // You could also add type validation here
            if let Some(value) = prop_map.get(&attr.name) {
                match attr.data_type.as_ref() {
                    "string" => {
                        if !value.is_string() {
                            return Err(ApiError::BadRequest(format!(
                                "Attribute '{}' must be a string",
                                attr.name
                            )));
                        }
                    }
                    "number" => {
                        if !value.is_number() {
                            return Err(ApiError::BadRequest(format!(
                                "Attribute '{}' must be a number",
                                attr.name
                            )));
                        }
                    }
                    "boolean" => {
                        if !value.is_boolean() {
                            return Err(ApiError::BadRequest(format!(
                                "Attribute '{}' must be a boolean",
                                attr.name
                            )));
                        }
                    }
                    "date" => {
                        // Basic validation for date strings
                        if !value.is_string() {
                            return Err(ApiError::BadRequest(format!(
                                "Attribute '{}' must be a date string",
                                attr.name
                            )));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
