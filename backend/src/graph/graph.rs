use crate::{user::User, utils::create_id};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use uuid::Uuid;

#[derive(PartialEq, Clone, Serialize, Deserialize, Debug, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum GraphRole {
    Admin,
    Member,
}

pub struct GraphMember {
    pub app_graphid: String,
    pub user_id: Uuid,
    pub role: GraphRole,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl GraphMember {
    pub fn new(app_graphid: String, user_id: Uuid, role: GraphRole) -> Self {
        let now = chrono::Utc::now();
        Self {
            app_graphid,
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
    pub app_graphid: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// Create error enum for graph creation
// TODO: Handle duplicate graph id's, and add retry logic
#[derive(Debug)]
pub enum GraphError {
    // Error when the graph name is not unique
    ValidationError(String),
}

impl GraphInfo {
    pub fn new(name: &str, description: Option<&str>) -> Result<Self, GraphError> {
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
            app_graphid: graph_id,
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
            .bind(&self.app_graphid)
            .execute(&mut *transaction)
            .await?;

        // Insert the graph info into the database
        let graph_info_query =
            "INSERT INTO graph_info (app_graphid, name, description, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)";
        sqlx::query(graph_info_query)
            .bind(&self.app_graphid)
            .bind(&self.name)
            .bind(&self.description)
            .bind(&self.created_at)
            .bind(&self.updated_at)
            .execute(&mut *transaction)
            .await?;

        let graph_member =
            GraphMember::new(self.app_graphid.clone(), admin_user.id, GraphRole::Admin);

        let graph_member_query =
            "INSERT INTO graph_member (app_graphid, user_id, role, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)";
        sqlx::query(&graph_member_query)
            .bind(&graph_member.app_graphid)
            .bind(&graph_member.user_id)
            .bind(&graph_member.role.to_string())
            .bind(&graph_member.created_at)
            .bind(&graph_member.updated_at)
            .execute(&mut *transaction)
            .await?;

        transaction.commit().await?;
        Ok(())
    }
}
