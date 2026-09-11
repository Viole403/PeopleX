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
use std::path::Path;

pub type DbPool = Pool<SqliteConnectionManager>;
pub type DbConn = PooledConnection<SqliteConnectionManager>;

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
}
