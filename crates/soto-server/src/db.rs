use crate::app::ApiError;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct Store(Arc<Mutex<rusqlite::Connection>>);
impl Store {
    pub fn open(path: &Path) -> Result<Self, ApiError> {
        let connection = rusqlite::Connection::open(path)
            .map_err(|_| ApiError::internal("No se pudo abrir la base de datos"))?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| ApiError::internal("SQLite ocupado"))?;
        connection
            .execute_batch(include_str!("schema.sql"))
            .map_err(|_| ApiError::internal("No se pudo aplicar la migración"))?;
        // Collapse any previous accounts into Soto's single local owner so saved
        // items and viewing progress survive the transition away from user login.
        connection
            .execute_batch(
                "BEGIN IMMEDIATE;
                 INSERT OR IGNORE INTO users(id,username,password_hash,admin) VALUES('local','local','',1);
                 INSERT OR IGNORE INTO collections(user_id,media_id) SELECT 'local',media_id FROM collections;
                 INSERT OR IGNORE INTO progress(user_id,media_id,position,duration) SELECT 'local',media_id,position,duration FROM progress;
                 DROP TABLE IF EXISTS sessions;
                 DELETE FROM playback;
                 DROP TABLE IF EXISTS profiles;
                 DELETE FROM users WHERE id!='local';
                 COMMIT;",
            )
            .map_err(|_| ApiError::internal("No se pudo migrar al modo sin usuarios"))?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }
    pub async fn call<T, F>(&self, f: F) -> Result<T, ApiError>
    where
        T: Send + 'static,
        F: FnOnce(&mut rusqlite::Connection) -> Result<T, ApiError> + Send + 'static,
    {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut connection = db
                .lock()
                .map_err(|_| ApiError::internal("Base de datos no disponible"))?;
            f(&mut connection)
        })
        .await
        .map_err(|_| ApiError::internal("Operación de base de datos interrumpida"))?
    }
}
