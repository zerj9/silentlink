use crate::{node::NodeType, org::Org, user::User, utils::create_id};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, FromRow, Row};
use strum_macros::{Display, EnumString};
use uuid::Uuid;

#[derive(PartialEq, Clone, Serialize, Deserialize, Debug, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum GraphRole {
    Admin,
    Member,
}

pub struct GraphMember {
    pub graph_id: String,
    pub user_id: Uuid,
    pub role: GraphRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl GraphMember {
    pub fn new(graph_id: String, user_id: Uuid, role: GraphRole) -> Self {
        let now = chrono::Utc::now();
        Self {
            graph_id,
            user_id,
            role,
            created_at: now,
            updated_at: now,
        }
    }
}

pub struct GraphInfo {
    // Unique randomly generated identifier for the graph name to pass to AGE
    // AGE graph names are unique. This allows us to have multiple graphs with the same name
    // Has to start with a letter
    pub graph_id: String,
    pub org_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// Implement FromRow for GraphInfo to convert from PgRow to GraphInfo
impl<'r> FromRow<'r, PgRow> for GraphInfo {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            graph_id: row.try_get("graph_id")?,
            org_id: row.try_get("org_id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

// Create error enum for graph creation
// TODO: Handle duplicate graph id's, and add retry logic
#[derive(Debug)]
pub enum GraphError {
    // Error when the graph name is not unique
    ValidationError(String),
}

impl GraphInfo {
    pub fn new(org: &Org, name: &str, description: Option<&str>) -> Result<Self, GraphError> {
        let now = chrono::Utc::now();
        // Prefix g to the random id. Required by AGE to start with a letter
        let graph_id = "g".to_string() + &create_id(8);

        // If name is empty, return a validation error
        if name.is_empty() {
            return Err(GraphError::ValidationError(
                "Name cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            graph_id: graph_id,
            org_id: org.id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    pub async fn persist(&self, pool: &sqlx::PgPool, admin_user: User) -> Result<(), sqlx::Error> {
        // Start a transaction
        let mut transaction = pool.begin().await?;

        // Create the graph in AGE
        let age_query = "SELECT ag_catalog.create_graph($1)";
        sqlx::query(age_query)
            .bind(&self.graph_id)
            .execute(&mut *transaction)
            .await?;

        // Insert the graph info into the database
        let graph_info_query =
            "INSERT INTO app_data.graph_info (graph_id, org_id, name, description, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)";
        sqlx::query(graph_info_query)
            .bind(&self.graph_id)
            .bind(&self.org_id)
            .bind(&self.name)
            .bind(&self.description)
            .bind(&self.created_at)
            .bind(&self.updated_at)
            .execute(&mut *transaction)
            .await?;

        let graph_member = GraphMember::new(self.graph_id.clone(), admin_user.id, GraphRole::Admin);

        let graph_member_query =
            "INSERT INTO app_data.graph_member (graph_id, user_id, role, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)";
        sqlx::query(&graph_member_query)
            .bind(&graph_member.graph_id)
            .bind(&graph_member.user_id)
            .bind(&graph_member.role.to_string())
            .bind(&graph_member.created_at)
            .bind(&graph_member.updated_at)
            .execute(&mut *transaction)
            .await?;

        transaction.commit().await?;
        Ok(())
    }

    pub async fn get_all(pool: &sqlx::PgPool, org_id: Uuid) -> Result<Vec<GraphInfo>, sqlx::Error> {
        let query = "SELECT * FROM app_data.graph_info WHERE org_id = $1";
        let rows = sqlx::query(query).bind(&org_id).fetch_all(pool).await?;
        let graphs: Vec<GraphInfo> = rows
            .iter()
            .map(|row| GraphInfo::from_row(row).unwrap())
            .collect();
        Ok(graphs)
    }

    pub async fn from_id(pool: &sqlx::PgPool, graph_id: &str) -> Result<Self, sqlx::Error> {
        let query = "SELECT * FROM app_data.graph_info WHERE graph_id = $1";
        sqlx::query_as::<_, GraphInfo>(query)
            .bind(graph_id)
            .fetch_one(pool)
            .await
    }

    pub async fn get_node_types(&self, pool: &sqlx::PgPool) -> Result<Vec<NodeType>, sqlx::Error> {
        let query = "SELECT * FROM app_data.node_types WHERE graph_id = $1";
        let rows = sqlx::query_as::<_, NodeType>(query)
            .bind(&self.graph_id)
            .fetch_all(pool)
            .await?;
        Ok(rows)
    }
}
