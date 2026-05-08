//! Chat history persistence — stores conversations per user/session.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub title: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ChatMessageRow {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct ChatStore;

impl ChatStore {
    /// Get or create a session for the user. Returns the most recent active session.
    pub async fn get_or_create_session(
        db: &PgPool,
        user_id: Uuid,
    ) -> Result<ChatSession, sqlx::Error> {
        // Try to find existing session updated within last 30 min
        let existing = sqlx::query_as::<_, ChatSession>(
            "SELECT * FROM chat_sessions WHERE user_id = $1 AND updated_at > now() - interval '30 minutes' ORDER BY updated_at DESC LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(db)
        .await?;

        if let Some(session) = existing {
            return Ok(session);
        }

        // Create new session
        let session = sqlx::query_as::<_, ChatSession>(
            "INSERT INTO chat_sessions (user_id) VALUES ($1) RETURNING *"
        )
        .bind(user_id)
        .fetch_one(db)
        .await?;

        Ok(session)
    }

    /// Create a new session explicitly
    pub async fn create_session(
        db: &PgPool,
        user_id: Uuid,
        title: Option<&str>,
    ) -> Result<ChatSession, sqlx::Error> {
        let session = sqlx::query_as::<_, ChatSession>(
            "INSERT INTO chat_sessions (user_id, title) VALUES ($1, $2) RETURNING *"
        )
        .bind(user_id)
        .bind(title)
        .fetch_one(db)
        .await?;

        Ok(session)
    }

    /// Save a message to the session
    pub async fn save_message(
        db: &PgPool,
        session_id: Uuid,
        role: &str,
        content: Option<&str>,
        tool_calls: Option<&serde_json::Value>,
        tool_call_id: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let row = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO chat_messages (session_id, role, content, tool_calls, tool_call_id) VALUES ($1, $2, $3, $4, $5) RETURNING id"
        )
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(tool_calls)
        .bind(tool_call_id)
        .fetch_one(db)
        .await?;

        // Update session timestamp
        sqlx::query("UPDATE chat_sessions SET updated_at = now() WHERE id = $1")
            .bind(session_id)
            .execute(db)
            .await?;

        Ok(row)
    }

    /// Load recent messages for a session (for context window)
    pub async fn load_messages(
        db: &PgPool,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ChatMessageRow>, sqlx::Error> {
        let messages = sqlx::query_as::<_, ChatMessageRow>(
            "SELECT * FROM chat_messages WHERE session_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(db)
        .await?;

        // Reverse to chronological order
        let mut messages = messages;
        messages.reverse();
        Ok(messages)
    }

    /// List user's chat sessions
    pub async fn list_sessions(
        db: &PgPool,
        user_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ChatSession>, sqlx::Error> {
        sqlx::query_as::<_, ChatSession>(
            "SELECT * FROM chat_sessions WHERE user_id = $1 ORDER BY updated_at DESC LIMIT $2"
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(db)
        .await
    }

    /// Delete a session and all its messages
    pub async fn delete_session(
        db: &PgPool,
        session_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM chat_sessions WHERE id = $1 AND user_id = $2"
        )
        .bind(session_id)
        .bind(user_id)
        .execute(db)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
