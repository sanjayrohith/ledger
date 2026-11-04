pub mod migrations;
pub mod repositories;

use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub enum DbError {
    AppDataPath(String),
    Io(std::io::Error),
    LockPoisoned,
    NotFound,
    Sql(rusqlite::Error),
    Validation(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppDataPath(message) => write!(
                formatter,
                "could not resolve Ledger data directory: {message}"
            ),
            Self::Io(error) => write!(formatter, "database file operation failed: {error}"),
            Self::LockPoisoned => formatter.write_str("database connection lock was poisoned"),
            Self::NotFound => formatter.write_str("record was not found"),
            Self::Sql(error) => write!(formatter, "database operation failed: {error}"),
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sql(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

impl From<std::io::Error> for DbError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl Database {
    pub fn open_in_app_data(app: &AppHandle) -> Result<Self, DbError> {
        let app_data_directory = app
            .path()
            .app_local_data_dir()
            .map_err(|error| DbError::AppDataPath(error.to_string()))?;
        Self::open_at(app_data_directory.join("ledger.sqlite3"))
    }

    pub fn open_at(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            restrict_directory_permissions(parent)?;
        }

        let mut connection = Connection::open(path)?;
        configure_connection(&connection)?;
        migrations::apply(&mut connection)?;
        restrict_file_permissions(path)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, DbError>,
    ) -> Result<T, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::LockPoisoned)?;
        operation(&connection)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), DbError> {
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(())
}

#[cfg(unix)]
fn restrict_directory_permissions(path: &Path) -> Result<(), DbError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_directory_permissions(_path: &Path) -> Result<(), DbError> {
    Ok(())
}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<(), DbError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> Result<(), DbError> {
    Ok(())
}

pub fn application_database_filename() -> &'static str {
    "ledger.sqlite3"
}

pub fn database_path_in(directory: PathBuf) -> PathBuf {
    directory.join(application_database_filename())
}
