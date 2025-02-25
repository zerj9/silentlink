use crate::auth::AuthProvider;
use crate::org::OrgMember;
use chrono::{DateTime, Utc};
use openidconnect::SubjectIdentifier;
use sqlx::Row;
use std::env;
use uuid::Uuid;

#[derive(Debug, Clone, strum_macros::EnumString, strum_macros::Display)]
#[strum(serialize_all = "lowercase")]
pub enum GlobalRole {
    SuperAdmin,
    Viewer,
    Writer,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid, // Random alphanumeric string
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub global_role: Option<GlobalRole>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for User {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let role_str: Option<String> = row.try_get("global_role")?;
        // Convert the string to the enum, or bind None if no role.
        let global_role = role_str.map(|role| role.parse::<GlobalRole>().unwrap());
        Ok(Self {
            id: row.try_get("id")?,
            email: row.try_get("email")?,
            first_name: row.try_get("first_name")?,
            last_name: row.try_get("last_name")?,
            global_role,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            is_active: row.try_get("is_active")?,
        })
    }
}

impl User {
    pub fn new(email: String, first_name: String, last_name: String) -> Self {
        let created_at = Utc::now();
        let updated_at = created_at;
        Self {
            id: Uuid::new_v4(),
            email,
            first_name,
            last_name,
            global_role: None,
            created_at,
            updated_at,
            is_active: true,
        }
    }

    pub async fn persist(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        let global_role: Option<GlobalRole> = if let Ok(sl_superadmins) = env::var("SL_SUPERADMINS")
        {
            let superadmin_emails: Vec<&str> =
                sl_superadmins.split(',').map(|s| s.trim()).collect();
            if superadmin_emails.contains(&self.email.as_str()) {
                Some(GlobalRole::SuperAdmin)
            } else {
                None
            }
        } else {
            None
        };

        let query = "INSERT INTO app_data.user (id, email, first_name, last_name, is_active, global_role) VALUES ($1, $2, $3, $4, $5, $6)";
        sqlx::query(query)
            .bind(&self.id)
            .bind(&self.email)
            .bind(&self.first_name)
            .bind(&self.last_name)
            .bind(&self.is_active)
            // Convert the enum to its string representation, or bind None if no role.
            .bind(global_role.map(|role| role.to_string()))
            .execute(&mut **tx)
            .await?;

        Ok(())
    }

    pub async fn from_id(pg_pool: &sqlx::PgPool, user_id: Uuid) -> Result<User, sqlx::Error> {
        let query = "SELECT * FROM app_data.user WHERE id = $1";
        let user = sqlx::query_as::<_, User>(query)
            .bind(user_id)
            .fetch_one(pg_pool)
            .await?;

        Ok(user)
    }

    pub async fn from_email(pg_pool: &sqlx::PgPool, email: &str) -> Result<User, sqlx::Error> {
        let query = "SELECT * FROM app_data.user WHERE email = $1";
        let user = sqlx::query_as::<_, User>(query)
            .bind(email)
            .fetch_one(pg_pool)
            .await?;

        Ok(user)
    }

    pub async fn get_org_memberships(
        &self,
        pg_pool: &sqlx::PgPool,
    ) -> Result<Vec<OrgMember>, sqlx::Error> {
        let query = "
        SELECT * FROM app_data.org_member WHERE user_id = $1";
        let org_memberships = sqlx::query_as::<_, OrgMember>(query)
            .bind(self.id)
            .fetch_all(pg_pool)
            .await?;

        Ok(org_memberships)
    }
}

#[derive(Debug, Clone)]
pub struct FederatedUser {
    pub id: Uuid,
    pub user_id: Uuid, // References `app_data.user(id)`
    pub provider: AuthProvider,
    pub sub: SubjectIdentifier, // Unique ID from the provider (e.g. Google sub)
    pub email: Option<String>,
    pub picture_url: Option<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for FederatedUser {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        let provider = row
            .try_get::<String, _>("provider")?
            .parse::<AuthProvider>()
            .map_err(|_| sqlx::Error::Decode("Invalid provider".into()))?;
        let sub = row.try_get("sub")?;
        let sub = SubjectIdentifier::new(sub);

        Ok(Self {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            provider,
            sub,
            email: row.try_get("email")?,
            picture_url: row.try_get("picture_url")?,
        })
    }
}

impl FederatedUser {
    pub fn new(
        user_id: Uuid,
        provider: AuthProvider,
        sub: SubjectIdentifier,
        email: Option<String>,
        picture_url: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            provider,
            sub,
            email,
            picture_url,
        }
    }

    pub async fn from_sub(
        pg_pool: &sqlx::PgPool,
        provider: AuthProvider,
        sub: SubjectIdentifier,
    ) -> Result<Option<FederatedUser>, sqlx::Error> {
        let query = "
        SELECT * FROM app_data.federated_user WHERE provider = $1 AND sub = $2";

        // `fetch_optional` returns `Ok(Some(record))` if found, or `Ok(None)` if no row exists.
        let result = sqlx::query_as::<_, FederatedUser>(query)
            .bind(provider.to_string())
            .bind(sub.to_string())
            .fetch_optional(pg_pool)
            .await?;

        Ok(result)
    }

    pub async fn persist(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        let query = "
        INSERT INTO app_data.federated_user (id, user_id, provider, sub, email, picture_url)
        VALUES ($1, $2, $3, $4, $5, $6)";

        sqlx::query(query)
            .bind(&self.id)
            .bind(&self.user_id)
            .bind(self.provider.to_string())
            .bind(&self.sub.to_string())
            .bind(&self.email)
            .bind(&self.picture_url)
            .execute(&mut **tx)
            .await?;

        Ok(())
    }
}
