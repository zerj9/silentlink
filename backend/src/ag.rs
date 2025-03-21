use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::decode::Decode;
use sqlx::postgres::{PgRow, PgTypeInfo, PgValueRef, Postgres};
use sqlx::{FromRow, Row};
use tracing::{debug, error};

// Custom type to represent agtype
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgType(pub JsonValue);

// Implement Type for AgType to tell SQLx about the custom type
impl sqlx::Type<Postgres> for AgType {
    fn type_info() -> PgTypeInfo {
        // Use the OID for agtype
        PgTypeInfo::with_name("agtype")
    }
}

impl<'r> FromRow<'r, PgRow> for AgType {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let row: AgType = row.try_get("row")?;
        Ok(row)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Vertex {
    pub label: String,
    pub properties: JsonValue,
}

impl TryFrom<AgType> for Vertex {
    type Error = serde_json::Error;

    fn try_from(ag_type: AgType) -> Result<Self, Self::Error> {
        serde_json::from_value(ag_type.0)
    }
}

impl<'r> Decode<'r, Postgres> for AgType {
    fn decode(
        value: PgValueRef<'r>,
    ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
        // Convert the value to a string
        let value_str: String = value.as_str().unwrap().to_string();

        // Split the string by "::"
        let parts: Vec<&str> = value_str.split("::").collect();

        // Ensure there are at least two parts (content and type)
        if parts.len() >= 2 {
            let content = parts[0].trim(); // First part is the content
            let value_type = parts[parts.len() - 1].trim(); // Last part is the type

            debug!("Raw Content: {:?}", content);
            debug!("Type: {}", value_type);

            // Check if the type is "vertex"
            if value_type == "vertex" {
                // Handle vertex type by parsing the content as a Node
                let content = content.trim_start_matches(char::is_control);
                let vertex: Vertex = serde_json::from_str(content)?;
                Ok(AgType(serde_json::to_value(vertex)?))
            } else {
                // Reject other types
                error!("Unsupported type: {}", value_type);
                Err("Unsupported type: expected 'vertex'".into())
            }
        } else {
            // Handle invalid format (missing type or content)
            error!("Invalid format: expected content::type");
            Err("Invalid format: expected content::type".into())
        }
    }
}
