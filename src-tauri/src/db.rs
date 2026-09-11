//! SQLite connection pool untuk PeopleX.
//!
//! Satu-satunya jalan akses database: pool ini, dipakai lewat `AppState`.
//! Setiap koneksi dari pool ditegakkan PRAGMA berikut:
//! - `foreign_keys = ON` (SQLite default OFF; FK schema harus aktif)
//! - `journal_mode = WAL` (baca konkuren + tulis serial untuk desktop single-user)
//! - `busy_timeout = 5000` (tunggu 5 dtk saat file terkunci, bukan gagal langsung)

use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use std::path::Path;
use std::sync::LazyLock;

pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConn = PooledConnection<SqliteConnectionManager>;

/// Migrasi versioned. M01 = skema awal (87 tabel). M02 = jenjang pendidikan.
pub static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(include_str!("schema_sqlite.sql")),
        M::up(include_str!("migrations/m02_education_levels.sql")),
    ])
});

/// Jalankan semua migrasi yang belum teraplikasi (atomik).
pub fn migrate(conn: &mut Connection) -> Result<(), String> {
    MIGRATIONS
        .to_latest(conn)
        .map_err(|e| format!("migrasi database gagal: {e}"))
}

/// PRAGMA wajib tiap koneksi baru dari pool.
fn configure(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    Ok(())
}

/// Bangun pool untuk file SQLite di `db_path`.
/// Direktori parent dibuat otomatis bila belum ada.
pub fn init_pool(db_path: &Path) -> Result<DbPool, String> {
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("gagal membuat direktori database: {e}"))?;
        }
    }
    let manager = SqliteConnectionManager::file(db_path).with_init(configure);
    Pool::builder()
        .max_size(4)
        .build(manager)
        .map_err(|e| format!("gagal membuat connection pool: {e}"))
}

/// Cek cepat pool bisa dipakai: ambil koneksi + jalankan query trivial.
pub fn ping(pool: &DbPool) -> Result<(), String> {
    let conn = pool
        .get()
        .map_err(|e| format!("gagal mengambil koneksi database: {e}"))?;
    let one: i64 = conn
        .query_row("SELECT 1", [], |row| row.get(0))
        .map_err(|e| format!("query database gagal: {e}"))?;
    if one == 1 {
        Ok(())
    } else {
        Err("ping database mengembalikan nilai tak terduga".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_bisa_dibuat_dan_ping_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = init_pool(&dir.path().join("pool_ping.db")).expect("init_pool");
        ping(&pool).expect("ping");
    }

    #[test]
    fn foreign_keys_aktif_di_setiap_koneksi() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("fk.db");
        let pool = init_pool(&path).expect("init_pool");
        for _ in 0..3 {
            let conn = pool.get().expect("get");
            let fk: i64 = conn
                .pragma_query_value(None, "foreign_keys", |row| row.get(0))
                .expect("pragma");
            assert_eq!(fk, 1, "foreign_keys harus ON");
        }
    }

    #[test]
    fn fk_ditegakkan_parent_harus_ada_dulu() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("fk_enforce.db");
        let pool = init_pool(&path).expect("init_pool");
        let conn = pool.get().expect("get");
        conn.execute_batch(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER NOT NULL
               REFERENCES parent(id));",
        )
        .expect("ddl");
        let bad = conn.execute("INSERT INTO child (parent_id) VALUES (999)", []);
        assert!(bad.is_err(), "insert yatim harus ditolak FK");
        conn.execute("INSERT INTO parent (id) VALUES (1)", [])
            .expect("insert parent");
        conn.execute("INSERT INTO child (parent_id) VALUES (1)", [])
            .expect("insert child valid");
    }

    #[test]
    fn direktori_parent_dibuat_otomatis() {
        let dir = tempfile::tempdir().expect("tempdir");
        let nested = dir.path().join("a").join("b").join("nested.db");
        init_pool(&nested).expect("init_pool");
        assert!(nested.exists(), "file db harus tercipta");
    }

    #[test]
    fn definisi_migrasi_valid() {
        MIGRATIONS.validate().expect("migrasi harus valid");
    }

    fn migrated_pool() -> (tempfile::TempDir, DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = init_pool(&dir.path().join("migrated.db")).expect("init_pool");
        let mut conn = pool.get().expect("get");
        migrate(&mut conn).expect("migrate");
        (dir, pool)
    }

    #[test]
    fn migrate_membuat_87_tabel() {
        let (_dir, pool) = migrated_pool();
        let conn = pool.get().expect("get");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(n, 87, "skema harus memuat 87 tabel");
    }

    #[test]
    fn fk_tetap_ditegakkan_setelah_migrate() {
        let (_dir, pool) = migrated_pool();
        let conn = pool.get().expect("get");
        let bad = conn.execute(
            "INSERT INTO branches (company_id, code, name) VALUES (999, 'X', 'Yatim')",
            [],
        );
        assert!(bad.is_err(), "branch tanpa company harus ditolak FK");
    }

    #[test]
    fn m02_jenjang_baru_diterima_data_lama_aman() {
        let (_dir, pool) = migrated_pool();
        let conn = pool.get().expect("get");
        conn.execute(
            "INSERT INTO companies (code, name) VALUES ('T1', 'Tes')",
            [],
        )
        .expect("insert company");
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type) VALUES ('EMP-T1', 'Uji', 'male', 'single', 1, '2026-01-01', 'active', 'permanent')",
            [],
        )
        .expect("insert employee");
        let eid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO employee_educations (employee_id, level, school_name) VALUES (?1, 's1', 'Kampus Lama')",
            rusqlite::params![eid],
        )
        .expect("insert lama");
        for lvl in ["smk", "d1", "d4", "sma"] {
            conn.execute(
                "INSERT INTO employee_educations (employee_id, level, school_name) VALUES (?1, ?2, 'Sekolah')",
                rusqlite::params![eid, lvl],
            )
            .unwrap_or_else(|_| panic!("jenjang {lvl} harus diterima"));
        }
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM employee_educations WHERE employee_id = ?1",
                rusqlite::params![eid],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(n, 5);
    }
}
