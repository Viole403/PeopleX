pub mod db;
pub mod seed;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

/// State global: koneksi database, direktori data, dan sesi login.
pub struct AppState {
    pub db: db::DbPool,
    pub data_dir: PathBuf,
    pub session: Mutex<Option<SessionUser>>,
}

/// Pengguna yang sedang login (diisi penuh di modul auth).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionUser {
    pub id: i64,
    pub username: String,
}

/// Status database untuk layar diagnosa.
#[derive(serde::Serialize)]
pub struct DbStatus {
    pub ok: bool,
    pub tables: i64,
    pub users: i64,
    pub data_dir: String,
}

#[tauri::command]
fn db_status(state: tauri::State<AppState>) -> Result<DbStatus, String> {
    let conn = state
        .db
        .get()
        .map_err(|e| format!("gagal mengambil koneksi database: {e}"))?;
    let tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung tabel: {e}"))?;
    let users: i64 = conn
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .map_err(|e| format!("gagal menghitung users: {e}"))?;
    Ok(DbStatus {
        ok: true,
        tables,
        users,
        data_dir: state.data_dir.display().to_string(),
    })
}

/// Siapkan direktori data, pool, migrasi, dan seed awal.
fn init_state(data_dir: PathBuf) -> Result<AppState, String> {
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("gagal membuat direktori data: {e}"))?;
    let pool = db::init_pool(&data_dir.join("peoplex.db"))?;
    {
        let mut conn = pool
            .get()
            .map_err(|e| format!("gagal mengambil koneksi database: {e}"))?;
        db::migrate(&mut conn)?;
        seed::seed(&mut conn)?;
    }
    Ok(AppState {
        db: pool,
        data_dir,
        session: Mutex::new(None),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|e| format!("gagal resolve direktori data: {e}"))?;
            let state = init_state(data_dir)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![db_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_state_menyiapkan_db_lengkap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = init_state(dir.path().to_path_buf()).expect("init_state");
        assert!(dir.path().join("peoplex.db").exists());
        assert_eq!(state.data_dir, dir.path());
        assert!(state.session.lock().unwrap().is_none());
        let conn = state.db.get().expect("get");
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let admin: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE username = 'admin'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 87);
        assert_eq!(admin, 1);
    }
}
