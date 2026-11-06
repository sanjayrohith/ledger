use std::{fs, path::Path};

use age::{scrypt, secrecy::SecretString};
use uuid::Uuid;

use super::{restrict_file_permissions, Database, DbError};

pub const ENCRYPTED_BACKUP_EXTENSION: &str = "age";
const MIN_PASSPHRASE_CHARS: usize = 8;

impl Database {
    /// Writes a passphrase-encrypted backup (age, scrypt). The plaintext snapshot
    /// only exists briefly in Ledger's own private directory and is removed afterwards.
    pub fn backup_encrypted_to(
        &self,
        path: impl AsRef<Path>,
        passphrase: &str,
    ) -> Result<(), DbError> {
        self.backup_encrypted_with(path, passphrase, None)
    }

    pub(crate) fn backup_encrypted_with(
        &self,
        path: impl AsRef<Path>,
        passphrase: &str,
        work_factor: Option<u8>,
    ) -> Result<(), DbError> {
        let path = path.as_ref();
        if path.extension().and_then(|value| value.to_str()) != Some(ENCRYPTED_BACKUP_EXTENSION) {
            return Err(DbError::Validation(
                "encrypted backup must use a .age filename".into(),
            ));
        }
        validate_passphrase(passphrase)?;
        let snapshot = self.scratch_path();
        self.backup_to(&snapshot)?;
        let result = (|| {
            let plaintext = fs::read(&snapshot)?;
            let mut recipient = scrypt::Recipient::new(SecretString::from(passphrase.to_owned()));
            if let Some(log_n) = work_factor {
                recipient.set_work_factor(log_n);
            }
            let ciphertext = age::encrypt(&recipient, &plaintext)
                .map_err(|error| DbError::BackupInvalid(error.to_string()))?;
            fs::write(path, ciphertext)?;
            restrict_file_permissions(path)
        })();
        let _ = fs::remove_file(&snapshot);
        result
    }

    pub fn restore_encrypted_from(
        &self,
        path: impl AsRef<Path>,
        passphrase: &str,
    ) -> Result<(), DbError> {
        let ciphertext = fs::read(path.as_ref())?;
        let identity = scrypt::Identity::new(SecretString::from(passphrase.to_owned()));
        let plaintext = age::decrypt(&identity, &ciphertext).map_err(|_| {
            DbError::BackupInvalid("wrong passphrase or not an encrypted Ledger backup".into())
        })?;
        let snapshot = self.scratch_path();
        fs::write(&snapshot, plaintext)?;
        restrict_file_permissions(&snapshot)?;
        let result = self.restore_from(&snapshot);
        let _ = fs::remove_file(&snapshot);
        result
    }

    fn scratch_path(&self) -> std::path::PathBuf {
        let directory = self
            .database_path()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(std::env::temp_dir);
        directory.join(format!(".ledger-scratch-{}.sqlite3", Uuid::new_v4()))
    }
}

fn validate_passphrase(passphrase: &str) -> Result<(), DbError> {
    if passphrase.chars().count() < MIN_PASSPHRASE_CHARS {
        return Err(DbError::Validation(format!(
            "passphrase must be at least {MIN_PASSPHRASE_CHARS} characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use crate::db::Database;

    fn seed(database: &Database) {
        database.with_connection(|connection| {
            connection.execute("INSERT INTO accounts (id,name,institution,created_at,updated_at) VALUES ('a','Synthetic','Canara Bank','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", [])?;
            Ok(())
        }).unwrap();
    }

    fn account_count(database: &Database) -> i64 {
        database
            .with_connection(|connection| {
                Ok(connection.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?)
            })
            .unwrap()
    }

    #[test]
    fn encrypted_backup_round_trips_and_rejects_wrong_passphrase_without_changes() {
        let database = Database::open_in_memory().unwrap();
        seed(&database);
        let file = std::env::temp_dir().join(format!("ledger-enc-{}.age", Uuid::new_v4()));
        database
            .backup_encrypted_with(&file, "correct horse battery", Some(10))
            .unwrap();
        let bytes = fs::read(&file).unwrap();
        assert!(!bytes.windows(9).any(|window| window == b"Synthetic"));
        assert!(!bytes.starts_with(b"SQLite format 3"));

        database.clear_personal_data().unwrap();
        assert_eq!(account_count(&database), 0);
        assert!(database
            .restore_encrypted_from(&file, "wrong passphrase")
            .is_err());
        assert_eq!(account_count(&database), 0);
        database
            .restore_encrypted_from(&file, "correct horse battery")
            .unwrap();
        assert_eq!(account_count(&database), 1);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn encrypted_backup_requires_age_extension_and_a_real_passphrase() {
        let database = Database::open_in_memory().unwrap();
        let dir = std::env::temp_dir();
        assert!(database
            .backup_encrypted_with(dir.join("x.sqlite3"), "long enough pass", Some(10))
            .is_err());
        assert!(database
            .backup_encrypted_with(dir.join("x.age"), "short", Some(10))
            .is_err());
    }
}
