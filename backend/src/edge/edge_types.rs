use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use strum_macros::{AsRefStr, Display, EnumString};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeType {
    pub type_name: String,
    pub display_name: String,
    pub description: String,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// Implement FromRow for NodeType
impl<'r> FromRow<'r, PgRow> for EdgeType {
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
pub struct EdgeAttributeDefinition {
    pub name: String,
    pub data_type: EdgeAttributeDataType,
    pub required: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum EdgeAttributeDataType {
    String,
    Number,
    Boolean,
    Date,
    // Add other types as needed
}

// Implement FromRow for EdgeAttributeDefinition
impl<'r> FromRow<'r, PgRow> for EdgeAttributeDefinition {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let data_type_str: String = row.try_get("data_type")?;
        let data_type: EdgeAttributeDataType = data_type_str
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

impl EdgeType {
    pub fn new(
        type_name: &str,
        display_name: &str,
        description: &str,
        created_by: &Uuid,
    ) -> Result<Self, String> {
        if type_name.is_empty() {
            return Err("type_name cannot be empty".to_string());
        }
        if display_name.is_empty() {
            return Err("display_name cannot be empty".to_string());
        }
        if description.is_empty() {
            return Err("description cannot be empty".to_string());
        }
        let created_at = chrono::Utc::now();
        Ok(Self {
            type_name: type_name.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            created_by: created_by.clone(),
            created_at,
        })
    }

    pub async fn get_all(pool: &sqlx::PgPool, graph_id: &str) -> Result<Vec<Self>, sqlx::Error> {
        let query = format!(
            "SELECT * FROM app_data.edge_types WHERE graph_id = '{}' ORDER BY created_at DESC",
            graph_id
        );

        sqlx::query_as::<_, EdgeType>(&query)
            .fetch_all(&*pool)
            .await
    }
}
