//! Koneksi SeaORM + migrasi versioned untuk PeopleX.
//!
//! Satu-satunya jalan akses database: `DatabaseConnection` ini, dipakai lewat `AppState`.
//! Pool SQLite ditegakkan pengaturan berikut:
//! - `foreign_keys = ON` (SQLite default OFF; FK schema harus aktif)
//! - `journal_mode = WAL` (baca konkuren + tulis serial untuk desktop single-user)
//! - `busy_timeout = 5000` (tunggu 5 dtk saat file terkunci, bukan gagal langsung)
//!
//! Driver PostgreSQL dan MySQL dipakai lewat URL dari config tanpa pragma khusus.

use crate::config::AppConfig;
use crate::schema_sql;
use crate::services::sea_raw::{exec, q_all, Value};
use sea_orm::DatabaseConnection;
use std::path::Path;
use std::str::FromStr;
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
    include_str!("migrations/m08_pulse_survey.sql"),
    include_str!("migrations/m09_liveness.sql"),
    include_str!("migrations/m10_shift_swap.sql"),
    include_str!("migrations/m11_offer_letters.sql"),
    include_str!("migrations/m12_ewa.sql"),
    include_str!("migrations/m13_approval_gear.sql"),
    include_str!("migrations/m14_compensation.sql"),
    include_str!("migrations/m15_preboarding.sql"),
    include_str!("migrations/m16_engagement.sql"),
    include_str!("migrations/m17_finance.sql"),
    include_str!("migrations/m18_bpjs.sql"),
    include_str!("migrations/m19_benchmark.sql"),
    include_str!("migrations/m20_compliance.sql"),
    include_str!("migrations/m21_performance_plus.sql"),
    include_str!("migrations/m22_workforce.sql"),
    include_str!("migrations/m23_entity_payroll.sql"),
    include_str!("migrations/m24_licenses.sql"),
    include_str!("migrations/m25_esop.sql"),
    include_str!("migrations/m26_global.sql"),
    include_str!("migrations/m27_fingerprint.sql"),
    include_str!("migrations/m28_api_webhook.sql"),
    include_str!("migrations/m29_fp_outbox.sql"),
    include_str!("migrations/m30_role_device_rules.sql"),
    include_str!("migrations/m31_door_events.sql"),
    include_str!("migrations/m32_card_blocklist.sql"),
    include_str!("migrations/m33_fp_easylink_check.sql"),
];

/// Bangun koneksi SeaORM sesuai driver pada config (`sqlite`/`postgres`/`mysql`).
/// Untuk SQLite, direktori parent file dibuat otomatis bila belum ada.
pub async fn connect_sea(cfg: &AppConfig, data_dir: &Path) -> Result<DatabaseConnection, String> {
    match cfg.database.driver.as_str() {
        "sqlite" => {
            let db_path = match cfg.database.path.as_deref() {
                Some(p) => data_dir.join(p),
                None => data_dir.join("peoplex.db"),
            };
            if let Some(parent) = db_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| format!("gagal membuat direktori database: {e}"))?;
                }
            }
            let options = sea_orm::sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&db_path)
                .create_if_missing(true)
                .foreign_keys(true)
                .journal_mode(sea_orm::sqlx::sqlite::SqliteJournalMode::Wal)
                .busy_timeout(Duration::from_secs(5));
            // Satu koneksi: perubahan tulis langsung terlihat baca berikutnya (WAL lokal).
            let pool = sea_orm::sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(1)
                .connect_with(options)
                .await
                .map_err(|e| format!("gagal membuat connection pool: {e}"))?;
            Ok(sea_orm::SqlxSqliteConnector::from_sqlx_sqlite_pool(
                pool,
            ))
        }
        "postgres" | "postgresql" => {
            let url = cfg.sea_url(data_dir)?;
            let options = sea_orm::sqlx::postgres::PgConnectOptions::from_str(&url)
                .map_err(|e| format!("gagal membaca konfigurasi PostgreSQL: {e}"))?;
            let pool = sea_orm::sqlx::postgres::PgPoolOptions::new()
                .max_connections(10)
                .connect_with(options)
                .await
                .map_err(|e| format!("gagal membuat connection pool: {e}"))?;
            Ok(sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(
                pool,
            ))
        }
        "mysql" => {
            let url = cfg.sea_url(data_dir)?;
            let options = sea_orm::sqlx::mysql::MySqlConnectOptions::from_str(&url)
                .map_err(|e| format!("gagal membaca konfigurasi MySQL: {e}"))?;
            let pool = sea_orm::sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(10)
                .after_connect(|conn, _| {
                    Box::pin(async move {
                        use sea_orm::sqlx::Executor;
                        conn.execute(
                            "SET SESSION sql_mode = CONCAT(@@SESSION.sql_mode, ',PIPES_AS_CONCAT')",
                        )
                        .await
                        .map(|_| ())
                    })
                })
                .connect_with(options)
                .await
                .map_err(|e| format!("gagal membuat connection pool: {e}"))?;
            Ok(sea_orm::SqlxMySqlConnector::from_sqlx_mysql_pool(pool))
        }
        other => Err(format!("driver database tidak dikenal: {other}")),
    }
}

/// Kumpulan naskah migrasi berurut (versi = indeks + 1), untuk pemakai luar.
pub fn migration_scripts() -> &'static [&'static str] {
    MIGRATIONS
}

/// Benar bila potongan naskah hanya berisi komentar atau baris kosong.
fn nama_backend(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Postgres => "postgres",
        sea_orm::DbBackend::MySql => "mysql",
        _ => "sqlite",
    }
}

/// Saring satu potongan migrasi: buang baris komentar `--`, baca direktif
/// `-- only: sqlite,postgres` (berlaku untuk SQL dalam potongan yang sama),
/// kembalikan None bila backend tidak tercantum atau sisa SQL kosong.
fn saring_direktif(potongan: &str, backend: &str) -> Option<String> {
    let mut hanya: Option<Vec<String>> = None;
    let mut sql = String::new();
    for baris in potongan.lines() {
        let t = baris.trim();
        if t.starts_with("--") {
            let isi = t[2..].trim();
            if let Some(daftar) = isi.strip_prefix("only:") {
                hanya = Some(
                    daftar
                        .split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            continue;
        }
        sql.push_str(baris);
        sql.push('\n');
    }
    let sql = sql.trim().to_string();
    if sql.is_empty() {
        return None;
    }
    if let Some(daftar) = hanya {
        if !daftar.iter().any(|b| b == backend) {
            return None;
        }
    }
    Some(sql)
}


/// Jalankan migrasi DDL yang belum tercatat di tabel `sea_migrations`.
///
/// Naskah SQLite ditranspil sesuai backend koneksi sebelum dieksekusi,
/// sehingga satu sumber skema berlaku untuk SQLite, PostgreSQL, dan MySQL.
pub async fn migrate_sea(db: &DatabaseConnection) -> Result<(), String> {
    let backend = db.get_database_backend();

    // Kumpulkan kolom TEXT terindeks dari seluruh naskah (indeks bisa
    // merujuk tabel yang dibuat di file migrasi lain).
    let mut semua = String::new();
    for script in MIGRATIONS {
        semua.push_str(script);
        semua.push('\n');
    }
    let indexed = schema_sql::collect_indexed_text(&semua);

    exec(
        db,
        schema_sql::transpile(
            backend,
            "CREATE TABLE IF NOT EXISTS sea_migrations (version INTEGER PRIMARY KEY)",
            &indexed,
        ),
        vec![],
        "migrasi database gagal: membuat sea_migrations",
    )
    .await?;

    let rows = q_all(
        db,
        "SELECT version FROM sea_migrations ORDER BY version".to_string(),
        vec![],
        1,
        "migrasi database gagal: membaca sea_migrations",
    )
    .await?;
    let applied: std::collections::HashSet<i64> = rows
        .iter()
        .filter_map(|r| match r.first() {
            Some(Value::Int(v)) => Some(*v),
            _ => None,
        })
        .collect();

    for (idx, script) in MIGRATIONS.iter().enumerate() {
        let version = idx as i64 + 1;
        if applied.contains(&version) {
            continue;
        }
        // Eksekusi tiap pernyataan DDL satu per satu (SQLite menolak multi-statement).
        // Potongan boleh membawa direktif `-- only: sqlite,postgres` per backend.
        let nama = nama_backend(backend);
        for part in script.split(';') {
            let Some(stmt) = saring_direktif(part, nama) else {
                continue;
            };
            let stmt = schema_sql::transpile(backend, &stmt, &indexed);
            exec(db, stmt, vec![], &format!("migrasi M{version:02} gagal")).await?;
        }
        exec(
            db,
            format!("INSERT INTO sea_migrations (version) VALUES ({version})"),
            vec![],
            "migrasi database gagal: mencatat versi",
        )
        .await?;
    }
    Ok(())
}

/// Cek cepat koneksi bisa dipakai: jalankan query trivial.
pub async fn ping_sea(db: &DatabaseConnection) -> Result<(), String> {
    let rows = q_all(db, "SELECT 1".to_string(), vec![], 1, "query database").await?;
    let unexpected = || "ping database mengembalikan nilai tak terduga".to_string();
    match rows.first().and_then(|r| r.first()) {
        Some(Value::Int(v)) if *v == 1 => Ok(()),
        _ => Err(unexpected()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn migrated_db() -> (tempfile::TempDir, DatabaseConnection) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = connect_sea(&AppConfig::default(), dir.path())
            .await
            .expect("connect");
        migrate_sea(&db).await.expect("migrate");
        (dir, db)
    }

    #[test]
    fn saring_direktif_backend() {
        assert_eq!(
            saring_direktif("-- komentar biasa\nSELECT 1", "sqlite"),
            Some("SELECT 1".to_string())
        );
        assert_eq!(saring_direktif("-- hanya komentar", "sqlite"), None);
        assert_eq!(saring_direktif("   ", "sqlite"), None);
        assert_eq!(
            saring_direktif("-- only: sqlite\nPRAGMA foreign_keys=OFF", "sqlite"),
            Some("PRAGMA foreign_keys=OFF".to_string())
        );
        assert_eq!(
            saring_direktif("-- only: sqlite\nPRAGMA foreign_keys=OFF", "postgres"),
            None
        );
        assert_eq!(
            saring_direktif("-- only: postgres,mysql\nALTER TABLE t ADD CHECK (a IN (1,2))", "mysql"),
            Some("ALTER TABLE t ADD CHECK (a IN (1,2))".to_string())
        );
        assert_eq!(
            saring_direktif("-- only: postgres,mysql\nALTER TABLE t ADD CHECK (a IN (1,2))", "sqlite"),
            None
        );
    }

    #[tokio::test]
    async fn migrate_membuat_88_tabel() {
        let (_dir, db) = migrated_db().await;
        let rows = q_all(
            &db,
            schema_sql::count_tables_sql(db.get_database_backend()),
            vec![],
            1,
            "count",
        )
        .await
        .expect("count");
        let n: i64 = match rows.first().and_then(|r| r.first()) {
            Some(Value::Int(v)) => *v,
            _ => panic!("n"),
        };
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
        let bad = q_all(
            &db,
            "INSERT INTO branches (company_id, code, name) VALUES (999, 'X', 'Yatim')".to_string(),
            vec![],
            1,
            "fk",
        )
        .await;
        assert!(bad.is_err(), "branch tanpa company harus ditolak FK");
    }

    fn server_cfg(
        driver: &str,
        port: u16,
        pass_env: &str,
    ) -> Option<(AppConfig, tempfile::TempDir)> {
        if std::env::var(pass_env).is_err() {
            return None;
        }
        let dir = tempfile::tempdir().expect("tempdir");
        let cfg = AppConfig {
            database: crate::config::DbConfig {
                driver: driver.to_string(),
                path: None,
                host: Some("127.0.0.1".to_string()),
                port: Some(port),
                user: Some("peoplex".to_string()),
                password_env: Some(pass_env.to_string()),
                database: Some("peoplex_test".to_string()),
            },
        };
        Some((cfg, dir))
    }

    async fn smoke_migrasi(cfg: &AppConfig, dir: &tempfile::TempDir, label: &str) {
        let db = connect_sea(cfg, dir.path())
            .await
            .unwrap_or_else(|e| panic!("sambung {label}: {e}"));
        migrate_sea(&db)
            .await
            .unwrap_or_else(|e| panic!("migrasi {label}: {e}"));
        migrate_sea(&db)
            .await
            .unwrap_or_else(|e| panic!("migrasi ulang {label}: {e}"));
        let rows = q_all(
            &db,
            schema_sql::count_tables_sql(db.get_database_backend()),
            vec![],
            1,
            "count",
        )
        .await
        .expect("count");
        let n: i64 = match rows.first().and_then(|r| r.first()) {
            Some(Value::Int(v)) => *v,
            _ => panic!("n"),
        };
        assert!(n >= 88, "skema {label} harus memuat 88 tabel, dapat {n}");
    }

    #[tokio::test]
    async fn smoke_postgres_bila_server_ada() {
        let Some((cfg, dir)) = server_cfg("postgres", 5432, "PEOPLEX_PG_PASSWORD") else {
            return;
        };
        smoke_migrasi(&cfg, &dir, "postgres").await;
    }

    #[tokio::test]
    async fn smoke_mysql_bila_server_ada() {
        let Some((cfg, dir)) = server_cfg("mysql", 3306, "PEOPLEX_MYSQL_PASSWORD") else {
            return;
        };
        smoke_migrasi(&cfg, &dir, "mysql").await;
    }
}
