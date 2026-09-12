//! Notifikasi pengguna: daftar terbaru, daftar penuh, tandai dibaca.

use rusqlite::{params, Connection};

use crate::to_dto_int;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Notification {
    pub id: i32,
    pub kind: String,
    pub title: String,
    pub message: Option<String>,
    pub link: Option<String>,
    pub is_read: bool,
    pub created_at: String,
}

fn map_row(r: &rusqlite::Row<'_>) -> Result<Notification, rusqlite::Error> {
    let id: i64 = r.get(0)?;
    let read: i64 = r.get(5)?;
    Ok(Notification {
        id: 0,
        kind: r.get(1)?,
        title: r.get(2)?,
        message: r.get(3)?,
        link: r.get(4)?,
        is_read: read != 0,
        created_at: r.get(6)?,
    }
    .with_id(id))
}

impl Notification {
    fn with_id(mut self, id: i64) -> Self {
        self.id = to_dto_int(id, "notification.id").unwrap_or(0);
        self
    }
}

const BASE: &str = "SELECT id, type, title, message, link, is_read, created_at FROM notifications WHERE user_id = ?1 ORDER BY id DESC LIMIT ?2";

pub fn recent(conn: &Connection, user_id: i64) -> Result<Vec<Notification>, String> {
    list(conn, user_id, 8)
}

pub fn all(conn: &Connection, user_id: i64) -> Result<Vec<Notification>, String> {
    list(conn, user_id, 100)
}

fn list(conn: &Connection, user_id: i64, limit: i64) -> Result<Vec<Notification>, String> {
    let mut stmt = conn
        .prepare(BASE)
        .map_err(|e| format!("gagal menyiapkan notifikasi: {e}"))?;
    let rows = stmt
        .query_map(params![user_id, limit], map_row)
        .map_err(|e| format!("gagal membaca notifikasi: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(out)
}

pub fn unread_count(conn: &Connection, user_id: i64) -> Result<i32, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ?1 AND is_read = 0",
            params![user_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung notifikasi: {e}"))?;
    to_dto_int(n, "notification.unread")
}

pub fn mark_read(conn: &Connection, user_id: i64, id: i64) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE notifications SET is_read = 1, read_at = datetime('now','localtime') WHERE id = ?1 AND user_id = ?2",
            params![id, user_id],
        )
        .map_err(|e| format!("gagal menandai notifikasi: {e}"))?;
    if n == 0 {
        return Err("Notifikasi tidak ditemukan.".to_string());
    }
    Ok(())
}

pub fn mark_all(conn: &Connection, user_id: i64) -> Result<(), String> {
    conn.execute(
        "UPDATE notifications SET is_read = 1, read_at = datetime('now','localtime') WHERE user_id = ?1 AND is_read = 0",
        params![user_id],
    )
    .map_err(|e| format!("gagal menandai notifikasi: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, seed};

    fn live() -> (tempfile::TempDir, crate::db::DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool)
    }

    #[test]
    fn notifikasi_terbaru_dan_tandai_baca() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let uid: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        for i in 0..10 {
            conn.execute(
                "INSERT INTO notifications (user_id, type, title, message) VALUES (?1, 'info', ?2, NULL)",
                params![uid, format!("N{i}")],
            )
            .unwrap();
        }
        assert_eq!(recent(&conn, uid).expect("recent").len(), 8);
        assert_eq!(all(&conn, uid).expect("all").len(), 10);
        assert_eq!(unread_count(&conn, uid).expect("n"), 10);
        let first = recent(&conn, uid).expect("r")[0].clone();
        mark_read(&conn, uid, first.id as i64).expect("read");
        assert_eq!(unread_count(&conn, uid).expect("n2"), 9);
        mark_all(&conn, uid).expect("all");
        assert_eq!(unread_count(&conn, uid).expect("n3"), 0);
    }
}
