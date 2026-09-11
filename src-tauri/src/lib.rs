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
#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SessionUser {
    pub id: i32,
    pub username: String,
}

/// Status database untuk layar diagnosa.
#[derive(serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DbStatus {
    pub ok: bool,
    pub tables: i32,
    pub users: i32,
    pub data_dir: String,
}

/// i64 dari SQLite ke i32 untuk DTO (tolak overflow, jangan silent-truncate).
fn to_dto_int(v: i64, field: &str) -> Result<i32, String> {
    i32::try_from(v).map_err(|_| format!("nilai {field} di luar jangkauan"))
}

#[tauri::command]
#[specta::specta]
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
        tables: to_dto_int(tables, "tables")?,
        users: to_dto_int(users, "users")?,
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

/// Builder specta: satu-satunya daftar command yang diekspos ke frontend.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new().commands(tauri_specta::collect_commands![db_status])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();
    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .expect("gagal export TypeScript bindings");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|e| format!("gagal resolve direktori data: {e}"))?;
            let state = init_state(data_dir)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
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

    #[test]
    fn specta_export_memuat_db_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("bindings.ts");
        specta_builder()
            .export(specta_typescript::Typescript::default(), &out)
            .expect("export");
        let ts = std::fs::read_to_string(&out).expect("read");
        assert!(
            ts.contains("dbStatus"),
            "bindings harus memuat command dbStatus"
        );
        assert!(
            ts.contains("DbStatus"),
            "bindings harus memuat tipe DbStatus"
        );
    }

    #[test]
    #[ignore]
    fn export_bindings_ke_src() {
        specta_builder()
            .export(
                specta_typescript::Typescript::default(),
                "../src/bindings.ts",
            )
            .expect("export");
    }
}
