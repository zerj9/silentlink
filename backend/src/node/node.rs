use crate::ag::AgType;
use crate::config::AppState;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CreateNodeResponse {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Node {
    id: i64,
    label: String,
    properties: HashMap<String, JsonValue>,
}

// Implement FromRow for Node
// This uses the TryFrom implementation to convert AgType to Node
impl<'r> FromRow<'r, PgRow> for Node {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        // Extract the AgType from the row
        let ag_type: AgType = row.try_get("row")?; // Replace "ag_column" with the actual column name

        // Convert AgType to Node using TryFrom
        let node = Node::try_from(ag_type).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        Ok(node)
    }
}

impl Node {
    pub async fn get_by_name(
        state: &AppState,
        label: &str,
        name: &str,
    ) -> Result<Self, sqlx::Error> {
        let escaped_name = name.replace("'", "''");
        let query = format!(
            "SELECT * FROM cypher('{}', $$ MATCH (n:{} {{name: '{}'}}) RETURN n $$) as (row agtype)",
            &state.graph_name,
            &label,
            &escaped_name
        );

        sqlx::query_as::<_, Node>(&query)
            .fetch_one(&*state.pool)
            .await
    }
}

// Implement TryFrom for AgType to convert to Node
impl TryFrom<AgType> for Node {
    type Error = serde_json::Error;

    fn try_from(value: AgType) -> Result<Self, Self::Error> {
        serde_json::from_value(value.0)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct GraphNode {
    id: Option<i64>,
    label: String,
    properties: HashMap<String, JsonValue>,
}
