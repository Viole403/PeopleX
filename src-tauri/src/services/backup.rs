//! Cadangan database: backup manual, terjadwal saat startup, daftar, dan restore.

use rusqlite::{params, Connection, DatabaseName, OptionalExtension};
use std::path::{Path, PathBuf};

/// Satu berkas cadangan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct BackupFile {
    pub name: String,
    pub size_bytes: i32,
    pub modified_at: String,
}

pub fn backup_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

/// Salin isi database hidup ke berkas cadangan baru. Kembalikan nama berkas.
pub fn backup_now(conn: &Connection, dir: &Path) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("gagal membuat folder cadangan: {e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = format!("peoplex-{stamp}.db");
    conn.backup(DatabaseName::Main, dir.join(&name), None)
        .map_err(|e| format!("gagal membuat cadangan: {e}"))?;
    Ok(name)
}

/// Daftar berkas cadangan, terbaru dulu.
pub fn backup_list(dir: &Path) -> Result<Vec<BackupFile>, String> {
    let mut out = Vec::new();
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("gagal membaca folder cadangan: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("gagal membaca entri cadangan: {e}"))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("db") {
            continue;
        }
        let meta = entry
            .metadata()
            .map_err(|e| format!("gagal membaca metadata cadangan: {e}"))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs().to_string())
            .unwrap_or_default();
        out.push(BackupFile {
            name: path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string(),
            size_bytes: crate::to_dto_int(meta.len() as i64, "backup.size")?,
            modified_at: modified,
        });
    }
    out.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(out)
}

/// Pulihkan database hidup dari berkas cadangan. Nama berkas dibersihkan
/// agar tetap di dalam folder cadangan.
pub fn backup_restore(conn: &mut Connection, dir: &Path, name: &str) -> Result<(), String> {
    let clean = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if clean.is_empty() || !clean.ends_with(".db") {
        return Err("Berkas cadangan tidak valid.".to_string());
    }
    let path = dir.join(clean);
    if !path.is_file() {
        return Err("Berkas cadangan tidak ditemukan.".to_string());
    }
    conn.restore(
        DatabaseName::Main,
        &path,
        None::<fn(rusqlite::backup::Progress)>,
    )
    .map_err(|e| format!("gagal memulihkan cadangan: {e}"))?;
    Ok(())
}

fn setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT setting_value FROM system_settings WHERE setting_key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
    .unwrap_or(None)
    .flatten()
}

/// Jalankan backup terjadwal saat startup bila sudah jatuh tempo.
/// `backup_schedule`: off, daily, weekly. Kegagalan diabaikan agar startup tetap jalan.
pub fn ensure_scheduled(conn: &Connection, dir: &Path) {
    let schedule = setting(conn, "backup_schedule").unwrap_or_default();
    if schedule != "daily" && schedule != "weekly" {
        return;
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let last = setting(conn, "backup_last_at").unwrap_or_default();
    let due = if last >= today {
        false
    } else if schedule == "daily" {
        true
    } else {
        last.is_empty() || last < week_ago()
    };
    if !due {
        return;
    }
    if backup_now(conn, dir).is_ok() {
        let _ = conn.execute(
            "UPDATE system_settings SET setting_value = ?1 WHERE setting_key = 'backup_last_at'",
            params![today],
        );
    }
}

fn week_ago() -> String {
    (chrono::Local::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string()
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_one, value_to_string, Value};

/// Salin isi database hidup ke berkas cadangan baru. Kembalikan nama berkas.
pub async fn backup_now_sea(
    db: &sea_orm::DatabaseConnection,
    dir: &Path,
) -> Result<String, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("gagal membuat folder cadangan: {e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = format!("peoplex-{stamp}.db");
    let dest = dir.join(&name);
    let dest_str = dest
        .to_str()
        .ok_or("Path cadangan tidak valid.".to_string())?;
    let safe = dest_str.replace('\'', "''");
    exec(
        db,
        format!("VACUUM INTO '{safe}'"),
        vec![],
        "backup.vacuum",
    )
    .await
    .map_err(|e| format!("gagal membuat cadangan: {e}"))?;
    Ok(name)
}

/// Pulihkan database dari berkas cadangan dengan salin berkas.
/// Pemanggil WAJIB memulai ulang aplikasi setelah ini agar pool koneksi
/// dibuka ulang terhadap berkas yang dipulihkan.
pub fn backup_restore_sea(db_path: &Path, dir: &Path, name: &str) -> Result<(), String> {
    let clean = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if clean.is_empty() || !clean.ends_with(".db") {
        return Err("Berkas cadangan tidak valid.".to_string());
    }
    let path = dir.join(clean);
    if !path.is_file() {
        return Err("Berkas cadangan tidak ditemukan.".to_string());
    }
    std::fs::copy(&path, db_path).map_err(|e| format!("gagal memulihkan cadangan: {e}"))?;
    Ok(())
}

async fn setting_sea(db: &sea_orm::DatabaseConnection, key: &str) -> Option<String> {
    q_one(
        db,
        "SELECT setting_value FROM system_settings WHERE setting_key = ?1".to_string(),
        vec![Value::Text(key.to_string())],
        1,
        "backup.setting",
    )
    .await
    .ok()
    .flatten()
    .as_ref()
    .and_then(|r| match &r[0] {
        Value::Null => None,
        _ => Some(value_to_string(&r[0])),
    })
}

/// Jalankan backup terjadwal saat startup bila sudah jatuh tempo.
/// `backup_schedule`: off, daily, weekly. Kegagalan diabaikan agar startup tetap jalan.
pub async fn ensure_scheduled_sea(db: &sea_orm::DatabaseConnection, dir: &Path) {
    let schedule = setting_sea(db, "backup_schedule").await.unwrap_or_default();
    if schedule != "daily" && schedule != "weekly" {
        return;
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let last = setting_sea(db, "backup_last_at").await.unwrap_or_default();
    let due = if last >= today {
        false
    } else if schedule == "daily" {
        true
    } else {
        last.is_empty() || last < week_ago()
    };
    if !due {
        return;
    }
    if backup_now_sea(db, dir).await.is_ok() {
        let _ = exec(
            db,
            "UPDATE system_settings SET setting_value = ?1 WHERE setting_key = 'backup_last_at'"
                .to_string(),
            vec![Value::Text(today)],
            "backup.stamp",
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, seed};

    fn live() -> (tempfile::TempDir, crate::db::DbPool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("dir");
        let backups = tempfile::tempdir().expect("backups");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool, backups)
    }

    #[test]
    fn backup_lalu_restore_mengembalikan_data() {
        let (_d, pool, backups) = live();
        let conn = pool.get().expect("get");
        let name = backup_now(&conn, backups.path()).expect("backup");
        assert!(name.starts_with("peoplex-"));
        let list = backup_list(backups.path()).expect("daftar");
        assert_eq!(list.len(), 1);
        conn.execute(
            "INSERT INTO companies (code, name) VALUES ('JUNK', 'Sampah')",
            [],
        )
        .unwrap();
        drop(conn);
        let mut c2 = pool.get().expect("get");
        backup_restore(&mut c2, backups.path(), &name).expect("restore");
        let n: i64 = c2
            .query_row(
                "SELECT COUNT(*) FROM companies WHERE code = 'JUNK'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        assert!(backup_restore(&mut c2, backups.path(), "../x.db").is_err());
        assert!(backup_restore(&mut c2, backups.path(), "hilang.db").is_err());
    }

    #[test]
    fn jadwal_harian_jatuh_tempo_sekali_sehari() {
        let (_d, pool, backups) = live();
        let conn = pool.get().expect("get");
        conn.execute(
            "UPDATE system_settings SET setting_value = 'daily' WHERE setting_key = 'backup_schedule'",
            [],
        )
        .unwrap();
        ensure_scheduled(&conn, backups.path());
        assert_eq!(backup_list(backups.path()).expect("daftar").len(), 1);
        ensure_scheduled(&conn, backups.path());
        assert_eq!(backup_list(backups.path()).expect("daftar").len(), 1);
        conn.execute(
            "UPDATE system_settings SET setting_value = 'off' WHERE setting_key = 'backup_schedule'",
            [],
        )
        .unwrap();
    }

    #[tokio::test]
    async fn backup_sea_lalu_restore_sea() {
        let dir = tempfile::tempdir().expect("dir");
        let backups = tempfile::tempdir().expect("backups");
        let db_path = dir.path().join("peoplex.db");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let name = backup_now_sea(db, backups.path()).await.expect("backup");
        assert!(name.starts_with("peoplex-"));
        let chk = rusqlite::Connection::open(backups.path().join(&name)).unwrap();
        let n: i64 = chk
            .query_row("SELECT COUNT(*) FROM companies", [], |r| r.get(0))
            .unwrap();
        assert!(n >= 1);
        drop(chk);
        let list = backup_list(backups.path()).expect("daftar");
        assert_eq!(list.len(), 1);
        let conn = state.db.get().expect("get");
        conn.execute(
            "INSERT INTO companies (code, name) VALUES ('JUNK', 'Sampah')",
            [],
        )
        .unwrap();
        drop(conn);
        drop(state);
        backup_restore_sea(&db_path, backups.path(), &name).expect("restore");
        let c2 = rusqlite::Connection::open(&db_path).unwrap();
        let n: i64 = c2
            .query_row(
                "SELECT COUNT(*) FROM companies WHERE code = 'JUNK'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        assert!(backup_restore_sea(&db_path, backups.path(), "../x.db").is_err());
        assert!(backup_restore_sea(&db_path, backups.path(), "hilang.db").is_err());
    }

    #[tokio::test]
    async fn jadwal_sea_harian_sekali_sehari() {
        let dir = tempfile::tempdir().expect("dir");
        let backups = tempfile::tempdir().expect("backups");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        exec_sea_setting(db, "daily").await;
        ensure_scheduled_sea(db, backups.path()).await;
        assert_eq!(backup_list(backups.path()).expect("daftar").len(), 1);
        ensure_scheduled_sea(db, backups.path()).await;
        assert_eq!(backup_list(backups.path()).expect("daftar").len(), 1);
        exec_sea_setting(db, "off").await;
    }

    async fn exec_sea_setting(db: &sea_orm::DatabaseConnection, v: &str) {
        use super::sea_raw::{exec, Value};
        exec(
            db,
            "UPDATE system_settings SET setting_value = ?1 WHERE setting_key = 'backup_schedule'".to_string(),
            vec![Value::Text(v.to_string())],
            "backup.test",
        )
        .await
        .unwrap();
    }
}
