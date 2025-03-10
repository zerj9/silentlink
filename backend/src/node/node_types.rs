use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use strum_macros::{AsRefStr, Display, EnumString};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeType {
    pub type_name: String,
    pub display_name: String,
    pub description: String,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Implement FromRow for NodeType
impl<'r> FromRow<'r, PgRow> for NodeType {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            type_name: row.try_get("type_name")?,
            display_name: row.try_get("display_name")?,
            description: row.try_get("description")?,
            created_by: row.try_get("created_by")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct AttributeDefinition {
    pub name: String,
    pub data_type: AttributeDataType,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AttributeDataType {
    String,
    Number,
    Boolean,
    Date,
    // Add other types as needed
}

// Implement FromRow for AttributeDefinition
impl<'r> FromRow<'r, PgRow> for AttributeDefinition {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let data_type_str: String = row.try_get("data_type")?;
        let data_type: AttributeDataType = data_type_str
            .parse()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        Ok(Self {
            name: row.try_get("name")?,
            data_type,
            required: row.try_get("required")?,
            description: row.try_get("description")?,
        })
    }
}
