use crate::utils::create_id;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Postgres, Row, Transaction};
use strum_macros::{AsRefStr, Display, EnumString};
use uuid::Uuid;

use super::CreateEdgeTypeRequest;

#[derive(Debug, Serialize, Deserialize)]
pub struct EdgeType {
    pub id: String,
    pub graph_id: String,
    pub name: String,
    pub normalized_name: String,
    pub description: String,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl EdgeType {
    pub fn new(
        graph_id: &str,
        name: &str,
        description: String,
        created_by: Uuid,
    ) -> Result<Self, String> {
        // Validate that type_name contains only letters and spaces
        if !name.chars().all(|c| c.is_alphabetic() || c.is_whitespace()) {
            return Err(format!("Edge type name '{}' contains invalid characters. Only letters and spaces are allowed.", name));
        }

        // Validate that type_name is not empty
        if name.trim().is_empty() {
            return Err("Edge name cannot be empty.".to_string());
        }

        let now = chrono::Utc::now();
        // Convert name to uppercase and replace spaces with underscores
        let normalized_name = name.to_uppercase().replace(' ', "_");

        Ok(Self {
            id: format!("e{}", create_id(8)),
            graph_id: graph_id.to_string(),
            name: name.to_string(),
            normalized_name,
            created_by,
            created_at: now,
            description,
        })
    }

    pub async fn from_request(
        req: CreateEdgeTypeRequest,
        graph_id: &str,
        created_by: Uuid,
    ) -> Result<Self, String> {
        let edge_type = Self::new(graph_id, &req.name, req.description, created_by)?;
        Ok(edge_type)
    }

    pub async fn save(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        // In AGE, edge types are implemented as edge labels
        let age_query = "SELECT ag_catalog.create_elabel($1, $2)";
        sqlx::query(age_query)
            .bind(&self.graph_id)
            .bind(&self.normalized_name)
            .execute(&mut **transaction)
            .await?;

        let insert_edge_type_query = "INSERT INTO app_data.edge_type (id, graph_id, name, normalized_name, description, created_by, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)";
        sqlx::query(insert_edge_type_query)
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
        edge_type_id: &str,
    ) -> Result<Self, sqlx::Error> {
        let query = "SELECT * FROM app_data.edge_type WHERE graph_id = $1 AND id = $2";
        let edge_type = sqlx::query_as::<_, EdgeType>(query)
            .bind(graph_id)
            .bind(edge_type_id)
            .fetch_one(pool)
            .await?;

        Ok(edge_type)
    }

    pub async fn from_name(
        pool: &sqlx::PgPool,
        graph_id: &str,
        edge_type_name: &str,
    ) -> Result<Self, sqlx::Error> {
        let query = "SELECT * FROM app_data.edge_type WHERE graph_id = $1 AND normalized_name = $2";
        let edge_type = sqlx::query_as::<_, EdgeType>(query)
            .bind(graph_id)
            .bind(crate::utils::normalize(edge_type_name))
            .fetch_one(pool)
            .await?;

        Ok(edge_type)
    }

    pub async fn list(pool: &sqlx::PgPool, graph_id: &str) -> Result<Vec<EdgeType>, sqlx::Error> {
        let query = "SELECT * FROM app_data.edge_type WHERE graph_id = $1";
        let rows = sqlx::query(query).bind(graph_id).fetch_all(pool).await?;
        let edge_types: Vec<EdgeType> = rows
            .iter()
            .map(|row| EdgeType::from_row(row).unwrap())
            .collect();
        Ok(edge_types)
    }
}

// Implement FromRow for EdgeType
impl<'r> FromRow<'r, PgRow> for EdgeType {
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

#[derive(Debug, Clone, Deserialize, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum EdgeTypeAttributeDataType {
    String,
    Number,
    Boolean,
    Date,
}

#[derive(Debug, Deserialize)]
pub struct NewEdgeTypeAttributeDefinition {
    pub name: String,
    pub data_type: EdgeTypeAttributeDataType,
    pub required: bool,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct EdgeTypeAttributeDefinition {
    pub id: Uuid,
    pub type_id: String,
    pub name: String,
    pub normalized_name: String,
    pub data_type: EdgeTypeAttributeDataType,
    pub required: bool,
    pub description: String,
}

impl EdgeTypeAttributeDefinition {
    pub fn from_request(req: &NewEdgeTypeAttributeDefinition, type_id: &str) -> Self {
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
            INSERT INTO app_data.edge_type_attribute (
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
