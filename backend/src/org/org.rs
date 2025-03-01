use crate::user::User;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, FromRow, Row};
use strum_macros::{Display, EnumString};
use uuid::Uuid;

#[derive(PartialEq, Clone, Serialize, Deserialize, Debug, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Role {
    Admin,
    Viewer,
}

#[derive(Debug, Serialize)]
pub struct OrgMember {
    pub org_id: Uuid,
    pub user_id: Uuid,
    pub role: Role,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

// Implement FromRow for OauthSession to convert from PgRow to OauthSession
impl<'r> FromRow<'r, PgRow> for OrgMember {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        let role: String = row.try_get("role")?;
        let role = role.parse::<Role>().unwrap();

        Ok(Self {
            org_id: row.try_get("org_id")?,
            user_id: row.try_get("user_id")?,
            role,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Org {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, PgRow> for Org {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            description: row.try_get("description")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

impl OrgMember {
    pub fn new(org_id: Uuid, user_id: Uuid, role: Role) -> Self {
        let now = chrono::Utc::now();
        Self {
            org_id,
            user_id,
            role,
            created_at: now,
            updated_at: now,
        }
    }
}

impl Org {
    pub fn new(name: &str, description: &str) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.to_string(),
            created_at: now,
            updated_at: now,
        }
    }

    pub async fn persist(&self, pool: &sqlx::PgPool, admin_user: User) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        let org_query = "INSERT INTO app_data.org (id, name, description, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)";
        sqlx::query(org_query)
            .bind(&self.id)
            .bind(&self.name)
            .bind(&self.description)
            .bind(&self.created_at)
            .bind(&self.updated_at)
            .execute(&mut *tx)
            .await?;

        let org_user = OrgMember::new(self.id, admin_user.id, Role::Admin);
        let org_user_query =
            "INSERT INTO app_data.org_member (org_id, user_id, role, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)";
        sqlx::query(org_user_query)
            .bind(&org_user.org_id)
            .bind(&org_user.user_id)
            .bind(&org_user.role.to_string())
            .bind(&org_user.created_at)
            .bind(&org_user.updated_at)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    pub async fn from_id(pool: &sqlx::PgPool, org_id: Uuid) -> Result<Self, sqlx::Error> {
        let org_query = "SELECT * FROM app_data.org WHERE id = $1";
        sqlx::query_as::<_, Org>(org_query)
            .bind(&org_id)
            .fetch_one(pool)
            .await
    }

    // Get multiple orgs given a list of org ids
    pub async fn get_many(
        pool: &sqlx::PgPool,
        org_ids: Vec<Uuid>,
    ) -> Result<Vec<Self>, sqlx::Error> {
        let org_query = "SELECT * FROM app_data.org WHERE id = ANY($1)";
        sqlx::query_as::<_, Org>(org_query)
            .bind(&org_ids)
            .fetch_all(pool)
            .await
    }

    pub async fn get_member(
        &self,
        pool: &sqlx::PgPool,
        user_id: Uuid,
    ) -> Result<Option<OrgMember>, sqlx::Error> {
        let org_user_query = "SELECT * FROM app_data.org_member WHERE org_id = $1 AND user_id = $2";
        sqlx::query_as::<_, OrgMember>(org_user_query)
            .bind(&self.id)
            .bind(&user_id)
            .fetch_optional(pool)
            .await
    }

    pub async fn get_members(&self, pool: &sqlx::PgPool) -> Result<Vec<OrgMember>, sqlx::Error> {
        let org_user_query = "SELECT * FROM app_data.org_member WHERE org_id = $1";
        sqlx::query_as::<_, OrgMember>(org_user_query)
            .bind(&self.id)
            .fetch_all(pool)
            .await
    }

    pub async fn add_member(
        &self,
        pool: &sqlx::PgPool,
        user: User,
        role: Role,
    ) -> Result<(), sqlx::Error> {
        let org_user = OrgMember::new(self.id, user.id, role);
        let org_user_query =
            "INSERT INTO app_data.org_member (org_id, user_id, role, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)";
        sqlx::query(org_user_query)
            .bind(&org_user.org_id)
            .bind(&org_user.user_id)
            .bind(&org_user.role.to_string())
            .bind(&org_user.created_at)
            .bind(&org_user.updated_at)
            .execute(pool)
            .await?;

        Ok(())
    }
}
