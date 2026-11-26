use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::ApiError;

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub password_hash: String,
}

/// SQLite storage for accounts and projects metadata.
///
/// One connection behind a mutex is enough for the MVP: the database only sees
/// short account queries, never sandbox work.
pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &std::path::Path) -> Result<Self, ApiError> {
        let conn = Connection::open(path)
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("ouverture de la base: {e}")))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("journal_mode: {e}")))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                id            TEXT PRIMARY KEY,
                email         TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                created_at    INTEGER NOT NULL
            );",
        )
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("création du schéma: {e}")))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_user(&self, user: &User) -> Result<(), ApiError> {
        let conn = self.lock();
        let now = chrono_like_timestamp();
        conn.execute(
            "INSERT INTO users (id, email, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![user.id, user.email, user.password_hash, now],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                ApiError::Conflict("un compte existe déjà avec cette adresse e-mail".into())
            } else {
                ApiError::Internal(anyhow::anyhow!("insertion utilisateur: {e}"))
            }
        })?;
        Ok(())
    }

    pub fn find_user_by_email(&self, email: &str) -> Result<Option<User>, ApiError> {
        let conn = self.lock();
        conn.query_row(
            "SELECT id, email, password_hash FROM users WHERE email = ?1",
            params![email],
            |row| {
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    password_hash: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("lecture utilisateur: {e}")))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // A poisoned mutex only means another request panicked; the SQLite
        // connection itself is still usable.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn chrono_like_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
