pub async fn validate_vertex_properties(
    pool: &PgPool,
    graph_id: &str,
    type_name: &str,
    properties: &serde_json::Value,
) -> Result<(), ApiError> {
    // Get all attributes for this node type
    let attributes = sqlx::query!(
        "SELECT attribute_name, data_type, required FROM node_type_attributes 
         WHERE graph_id = $1 AND type_name = $2",
        graph_id,
        type_name
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch node type attributes: {}", e);
        ApiError::InternalServerError
    })?;

    if let serde_json::Value::Object(prop_map) = properties {
        // Check for required attributes
        for attr in &attributes {
            if attr.required && !prop_map.contains_key(&attr.attribute_name) {
                return Err(ApiError::BadRequest(format!(
                    "Required attribute '{}' is missing",
                    attr.attribute_name
                )));
            }

            // You could also add type validation here
            if let Some(value) = prop_map.get(&attr.attribute_name) {
                match attr.data_type.as_str() {
                    "String" => {
                        if !value.is_string() {
                            return Err(ApiError::BadRequest(format!(
                                "Attribute '{}' must be a string",
                                attr.attribute_name
                            )));
                        }
                    }
                    "Number" => {
                        if !value.is_number() {
                            return Err(ApiError::BadRequest(format!(
                                "Attribute '{}' must be a number",
                                attr.attribute_name
                            )));
                        }
                    }
                    // Add other type validations as needed
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
