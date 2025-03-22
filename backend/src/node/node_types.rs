use super::NewAttributeDefinition;
use crate::utils::create_id;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Postgres, Row, Transaction};
use strum_macros::{AsRefStr, Display, EnumString};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeType {
    pub id: String,
    pub graph_id: String,
    pub name: String,
    pub normalized_name: String,
    pub description: String,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeTypeSummary {
    pub id: String,
    pub name: String,
    pub description: String,
}

impl From<&NodeType> for NodeTypeSummary {
    fn from(node_type: &NodeType) -> Self {
        Self {
            id: node_type.id.clone(),
            name: node_type.name.clone(),
            description: node_type.description.clone(),
        }
    }
}

impl NodeType {
    pub fn new(
        graph_id: &str,
        name: &str,
        description: String,
        created_by: Uuid,
    ) -> Result<Self, String> {
        // Validate that type_name contains only letters and spaces
        if !name.chars().all(|c| c.is_alphabetic() || c.is_whitespace()) {
            return Err(format!("Node type name '{}' contains invalid characters. Only letters and spaces are allowed.", name));
        }

        // Validate that type_name is not empty
        if name.trim().is_empty() {
            return Err("Node name cannot be empty.".to_string());
        }

        Ok(Self {
            id: format!("v{}", create_id(8)),
            graph_id: graph_id.to_string(),
            name: name.to_string(),
            normalized_name: crate::utils::normalize(name),
            created_by,
            created_at: chrono::Utc::now(),
            description,
        })
    }

    pub async fn save(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), sqlx::Error> {
        // In AGE, node types are implemented as vertex labels
        let age_query = "SELECT ag_catalog.create_vlabel($1, $2)";
        sqlx::query(age_query)
            .bind(&self.graph_id)
            .bind(&self.normalized_name)
            .execute(&mut **transaction)
            .await?;

        // Store node type metadata
        let insert_node_type_meta = "
        INSERT INTO app_data.node_types (
            id,
            graph_id, 
            name, 
            normalized_name,
            description, 
            created_by, 
            created_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)";

        sqlx::query(insert_node_type_meta)
            .bind(&self.id)
            .bind(&self.graph_id)
            .bind(&self.name)
            .bind(&self.normalized_name)
            .bind(&self.description)
            .bind(&self.created_by)
            .bind(&self.created_at)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }

    pub async fn from_id(
        pool: &sqlx::PgPool,
        graph_id: &str,
        node_type_id: &str,
    ) -> Result<Self, sqlx::Error> {
        let query = r#"
            SELECT * FROM app_data.node_types
            WHERE graph_id = $1 AND id = $2
        "#;

        let node_type = sqlx::query_as::<_, NodeType>(query)
            .bind(graph_id)
            .bind(node_type_id)
            .fetch_one(pool)
            .await?;

        Ok(node_type)
    }

    pub async fn from_name(
        pool: &sqlx::PgPool,
        graph_id: &str,
        name: &str,
    ) -> Result<Self, sqlx::Error> {
        let query = r#"
            SELECT * FROM app_data.node_types
            WHERE graph_id = $1 AND normalized_name = $2
        "#;

        let node_type = sqlx::query_as::<_, NodeType>(query)
            .bind(graph_id)
            .bind(crate::utils::normalize(&name))
            .fetch_one(pool)
            .await?;

        Ok(node_type)
    }
}

// Implement FromRow for NodeType
impl<'r> FromRow<'r, PgRow> for NodeType {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            graph_id: row.try_get("graph_id")?,
            name: row.try_get("name")?,
            normalized_name: row.try_get("normalized_name")?,
            description: row.try_get("description")?,
            created_by: row.try_get("created_by")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct AttributeDefinition {
    pub id: Uuid,
    pub type_id: String,
    pub name: String,
    pub normalized_name: String,
    pub data_type: AttributeDataType,
    pub required: bool,
    pub description: String,
}

impl AttributeDefinition {
    pub fn from_request(req: &NewAttributeDefinition, type_id: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            type_id: type_id.to_string(),
            name: req.name.clone(),
            normalized_name: crate::utils::normalize(&req.name),
            data_type: req.data_type.clone(),
            required: req.required,
            description: req.description.clone(),
        }
    }

    pub async fn save(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), sqlx::Error> {
        let insert_query = r#"
            INSERT INTO app_data.node_type_attributes (
                id,
                type_id,
                name,
                normalized_name,
                data_type,
                required,
                description
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#;

        sqlx::query(insert_query)
            .bind(&self.id)
            .bind(&self.type_id)
            .bind(&self.name)
            .bind(&self.normalized_name)
            .bind(&self.data_type.to_string())
            .bind(&self.required)
            .bind(&self.description)
            .execute(&mut **transaction)
            .await?;

        Ok(())
    }
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
            id: row.try_get("id")?,
            type_id: row.try_get("type_id")?,
            name: row.try_get("name")?,
            normalized_name: row.try_get("normalized_name")?,
            data_type,
            required: row.try_get("required")?,
            description: row.try_get("description")?,
        })
    }
}
