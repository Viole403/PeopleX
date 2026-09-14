//! Koneksi SeaORM + migrasi versioned untuk PeopleX.
//!
//! Satu-satunya jalan akses database: `DatabaseConnection` ini, dipakai lewat `AppState`.
//! Setiap pool ditegakkan pengaturan berikut:
//! - `foreign_keys = ON` (SQLite default OFF; FK schema harus aktif)
//! - `journal_mode = WAL` (baca konkuren + tulis serial untuk desktop single-user)
//! - `busy_timeout = 5000` (tunggu 5 dtk saat file terkunci, bukan gagal langsung)

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::path::Path;
use std::time::Duration;

/// Migrasi versioned. M01 = skema awal (87 tabel). M02 = jenjang pendidikan. M03 = MFA. M04 = skor exit. M05 = band gaji. M06 = materi training. M07 = nilai kuis.
static MIGRATIONS: &[&str] = &[
    include_str!("schema_sqlite.sql"),
    include_str!("migrations/m02_education_levels.sql"),
    include_str!("migrations/m03_mfa.sql"),
    include_str!("migrations/m04_exit_score.sql"),
    include_str!("migrations/m05_salary_band.sql"),
    include_str!("migrations/m06_training_materials.sql"),
    include_str!("migrations/m07_quiz_score.sql"),
];

/// Bangun koneksi SeaORM untuk file SQLite di `db_path`.
/// Direktori parent dibuat otomatis bila belum ada.
pub async fn connect_sea(db_path: &Path) -> Result<DatabaseConnection, String> {
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("gagal membuat direktori database: {e}"))?;
        }
    }
    let options = sea_orm::sqlx::sqlite::SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sea_orm::sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = sea_orm::sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|e| format!("gagal membuat connection pool: {e}"))?;
    Ok(sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(
        pool,
    ))
}

/// Jalankan semua migrasi yang belum teraplikasi (atomik per versi).
pub async fn migrate_sea(db: &DatabaseConnection) -> Result<(), String> {
    db.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE IF NOT EXISTS sea_migrations (version INTEGER PRIMARY KEY)".to_string(),
    ))
    .await
    .map_err(|e| format!("migrasi database gagal: {e}"))?;
    let applied: Vec<i64> = {
        let rows = db
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT version FROM sea_migrations ORDER BY version".to_string(),
            ))
            .await
            .map_err(|e| format!("migrasi database gagal: {e}"))?;
        let mut out = Vec::new();
        for r in rows {
            if let Ok(v) = r.try_get::<i64>("", "version") {
                out.push(v);
            }
        }
        out
    };
    for (idx, script) in MIGRATIONS.iter().enumerate() {
        let version = idx as i64 + 1;
        if applied.contains(&version) {
            continue;
        }
        for part in script.split(';') {
            let stmt = part.trim();
            if stmt.is_empty() || stmt.starts_with("--") && !stmt.contains('\n') {
                continue;
            }
            db.execute(Statement::from_string(DbBackend::Sqlite, stmt.to_string()))
                .await
                .map_err(|e| format!("migrasi M{version:02} gagal: {e}"))?;
        }
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            format!("INSERT INTO sea_migrations (version) VALUES ({version})"),
        ))
        .await
        .map_err(|e| format!("migrasi database gagal: {e}"))?;
    }
    Ok(())
}

/// Cek cepat koneksi bisa dipakai: jalankan query trivial.
pub async fn ping_sea(db: &DatabaseConnection) -> Result<(), String> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT 1".to_string(),
        ))
        .await
        .map_err(|e| format!("query database gagal: {e}"))?;
    match row {
        Some(r) => {
            let one: i32 = r
                .try_get("", "1")
                .map_err(|_| "ping database mengembalikan nilai tak terduga".to_string())?;
            if one == 1 {
                Ok(())
            } else {
                Err("ping database mengembalikan nilai tak terduga".to_string())
            }
        }
        None => Err("ping database mengembalikan nilai tak terduga".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn migrated_db() -> (tempfile::TempDir, DatabaseConnection) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = connect_sea(&dir.path().join("migrated.db"))
            .await
            .expect("connect");
        migrate_sea(&db).await.expect("migrate");
        (dir, db)
    }

    #[tokio::test]
    async fn migrate_membuat_88_tabel() {
        let (_dir, db) = migrated_db().await;
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'".to_string(),
            ))
            .await
            .expect("count")
            .expect("row");
        let n: i64 = row.try_get("", "n").expect("n");
        assert!(n >= 88, "skema harus memuat 88 tabel, dapat {n}");
    }

    #[tokio::test]
    async fn migrasi_idempoten_dua_kali() {
        let (_dir, db) = migrated_db().await;
        migrate_sea(&db).await.expect("migrate ulang harus ok");
    }

    #[tokio::test]
    async fn fk_ditegakkan_parent_harus_ada_dulu() {
        let (_dir, db) = migrated_db().await;
        let bad = db
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "INSERT INTO branches (company_id, code, name) VALUES (999, 'X', 'Yatim')".to_string(),
            ))
            .await;
        assert!(bad.is_err(), "branch tanpa company harus ditolak FK");
    }
}
