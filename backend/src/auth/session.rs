use chrono::{DateTime, Utc};
use oauth2::RefreshToken;
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use uuid::Uuid;

#[derive(Debug)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub federated_user_id: Uuid,
    pub refresh_token: Option<RefreshToken>,
    pub token_expiry: chrono::DateTime<chrono::Utc>,
    pub session_expiry: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl<'r> FromRow<'r, PgRow> for Session {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        // Extract each field from the row
        // Refresh token may be null, so we need to handle it as an Option
        let id: Uuid = row.try_get("id")?;
        let user_id = row.try_get("user_id")?;
        let federated_user_id = row.try_get("federated_user_id")?;
        let refresh_token: Option<String> = row.try_get("refresh_token")?;
        let refresh_token = refresh_token.map(|t| RefreshToken::new(t));
        let token_expiry: DateTime<Utc> = row.try_get("token_expiry")?;
        let session_expiry: DateTime<Utc> = row.try_get("session_expiry")?;
        let created_at: DateTime<Utc> = row.try_get("created_at")?;

        // Construct the Session struct
        Ok(Session {
            id,
            user_id,
            federated_user_id,
            refresh_token,
            token_expiry,
            session_expiry,
            created_at,
        })
    }
}

impl Session {
    pub async fn create(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        federated_user_id: Uuid,
        refresh_token: Option<&RefreshToken>,
        token_expiry: DateTime<Utc>,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        // Convert the refresh token to a string if it exists, otherwise None
        let sql_refresh_token = refresh_token.clone().map(|t| t.secret().to_string());
        let session_expiry = chrono::Utc::now() + chrono::Duration::days(365);
        let query =
            "INSERT INTO app_data.session (id, user_id, federated_user_id, refresh_token, token_expiry, session_expiry) VALUES ($1, $2, $3, $4, $5, $6)";
        sqlx::query(query)
            .bind(id)
            .bind(user_id)
            .bind(federated_user_id)
            .bind(sql_refresh_token)
            .bind(token_expiry)
            .bind(session_expiry)
            .execute(pool)
            .await?;

        Ok(Self {
            id,
            user_id,
            federated_user_id,
            refresh_token: refresh_token.cloned(),
            token_expiry,
            session_expiry,
            created_at: chrono::Utc::now(),
        })
    }

    pub async fn from_id(pool: &sqlx::PgPool, id: Uuid) -> Result<Self, sqlx::Error> {
        let query = "SELECT * FROM app_data.session WHERE id = $1";
        let row = sqlx::query_as::<_, Session>(query)
            .bind(id)
            .fetch_one(pool)
            .await?;
        Ok(row)
    }
}
