pub mod encrypted_backup;
pub mod migrations;
pub mod repositories;
pub mod settings;

use std::{
    fmt, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::{Connection, OpenFlags, MAIN_DB};
use tauri::{AppHandle, Manager};

pub struct Database {
    connection: Mutex<Connection>,
    path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum DbError {
    AppDataPath(String),
    BackupInvalid(String),
    Csv(csv::Error),
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
            Self::BackupInvalid(message) => {
                write!(formatter, "backup validation failed: {message}")
            }
            Self::Csv(error) => write!(formatter, "CSV export failed: {error}"),
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
            Self::Csv(error) => Some(error),
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
impl From<csv::Error> for DbError {
    fn from(error: csv::Error) -> Self {
        Self::Csv(error)
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
            path: Some(path.to_path_buf()),
        })
    }

    pub fn database_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        let mut connection = Connection::open_in_memory()?;
        configure_connection(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            path: None,
        })
    }

    pub fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, DbError>,
    ) -> Result<T, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::LockPoisoned)?;
        operation(&connection)
    }

    pub fn backup_to(&self, path: impl AsRef<Path>) -> Result<(), DbError> {
        let path = path.as_ref();
        if path.extension().and_then(|value| value.to_str()) != Some("sqlite3") {
            return Err(DbError::Validation(
                "backup must use a .sqlite3 filename".into(),
            ));
        }
        let connection = self.connection.lock().map_err(|_| DbError::LockPoisoned)?;
        connection.backup(MAIN_DB, path, rusqlite::backup::NO_PROGRESS)?;
        restrict_file_permissions(path)?;
        Ok(())
    }

    pub fn restore_from(&self, path: impl AsRef<Path>) -> Result<(), DbError> {
        let path = path.as_ref();
        validate_backup(path)?;
        let mut connection = self.connection.lock().map_err(|_| DbError::LockPoisoned)?;
        connection.restore(MAIN_DB, path, rusqlite::backup::NO_PROGRESS)?;
        configure_connection(&connection)?;
        migrations::apply(&mut connection)?;
        Ok(())
    }

    pub fn clear_personal_data(&self) -> Result<(), DbError> {
        let mut connection = self.connection.lock().map_err(|_| DbError::LockPoisoned)?;
        let transaction = connection.transaction()?;
        transaction.execute_batch("DELETE FROM transactions; DELETE FROM import_batches; DELETE FROM budgets; DELETE FROM merchant_rules; DELETE FROM merchants; DELETE FROM accounts;")?;
        transaction.commit()?;
        Ok(())
    }

    pub fn export_transactions_csv(&self, path: impl AsRef<Path>) -> Result<usize, DbError> {
        let path = path.as_ref();
        if path.extension().and_then(|value| value.to_str()) != Some("csv") {
            return Err(DbError::Validation(
                "export must use a .csv filename".into(),
            ));
        }
        let connection = self.connection.lock().map_err(|_| DbError::LockPoisoned)?;
        let mut writer = csv::Writer::from_path(path)?;
        writer.write_record([
            "date",
            "direction",
            "amount_paise",
            "kind",
            "description",
            "merchant_override",
            "category",
            "reference",
        ])?;
        let mut statement = connection.prepare("SELECT t.transaction_date,t.direction,t.amount_paise,t.transaction_kind,t.raw_description,t.merchant_override,COALESCE(c.name,''),COALESCE(t.reference_raw,'') FROM transactions t LEFT JOIN categories c ON c.id=t.category_id ORDER BY t.transaction_date,t.created_at")?;
        let mut rows = statement.query([])?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            writer.write_record([
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?.to_string(),
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ])?;
            count += 1;
        }
        writer.flush().map_err(DbError::Io)?;
        restrict_file_permissions(path)?;
        Ok(count)
    }
}

fn validate_backup(path: &Path) -> Result<(), DbError> {
    if !path.is_file() {
        return Err(DbError::BackupInvalid(
            "selected file does not exist".into(),
        ));
    }
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(DbError::BackupInvalid(format!(
            "integrity check returned {integrity}"
        )));
    }
    let migrations: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(|_| DbError::BackupInvalid("not a Ledger backup".into()))?;
    if migrations == 0 {
        return Err(DbError::BackupInvalid("not a Ledger backup".into()));
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::Database;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("ledger-{name}-{}.sqlite3", Uuid::new_v4()))
    }

    #[test]
    fn backup_restore_export_and_clear_are_local_and_validated() {
        let database = Database::open_in_memory().unwrap();
        database.with_connection(|connection| { connection.execute("INSERT INTO accounts (id,name,institution,created_at,updated_at) VALUES ('a','Synthetic','Canara Bank','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",[])?; connection.execute("INSERT INTO transactions (id,account_id,transaction_date,direction,amount_paise,raw_description,transaction_kind,is_reviewed,fingerprint,created_at,updated_at) VALUES ('t','a','2026-01-02','debit',1234,'Synthetic merchant','expense',1,'fp','2026-01-02T00:00:00Z','2026-01-02T00:00:00Z')",[])?; Ok(()) }).unwrap();
        let backup = temp_path("backup");
        let export = std::env::temp_dir().join(format!("ledger-export-{}.csv", Uuid::new_v4()));
        database.backup_to(&backup).unwrap();
        assert_eq!(database.export_transactions_csv(&export).unwrap(), 1);
        assert!(fs::read_to_string(&export)
            .unwrap()
            .contains("Synthetic merchant"));
        database.clear_personal_data().unwrap();
        let count: i64 = database
            .with_connection(|connection| {
                Ok(connection
                    .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(count, 0);
        database.restore_from(&backup).unwrap();
        let restored: i64 = database
            .with_connection(|connection| {
                Ok(connection
                    .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))?)
            })
            .unwrap();
        assert_eq!(restored, 1);
        let invalid = temp_path("invalid");
        fs::write(&invalid, b"not a database").unwrap();
        assert!(database.restore_from(&invalid).is_err());
        fs::remove_file(backup).unwrap();
        fs::remove_file(export).unwrap();
        fs::remove_file(invalid).unwrap();
    }
}
