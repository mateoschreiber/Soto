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
        Ok(Self(Arc::new(Mutex::new(connection))))
    }
    pub fn initial_setup_required(&self) -> Result<bool, ApiError> {
        let c = self
            .0
            .lock()
            .map_err(|_| ApiError::internal("SQLite no disponible"))?;
        Ok(c.query_row("SELECT count(*)=0 FROM users", [], |r| r.get(0))?)
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
