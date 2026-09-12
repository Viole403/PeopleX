//! Pengumuman: daftar terlihat, kelola, dan tandai baca.

use rusqlite::{params, Connection};

use super::audit;
use crate::to_dto_int;

const TARGETS: &[&str] = &["department", "branch", "role", "employee"];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Target {
    pub target_type: String,
    pub target_id: i32,
    pub target_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Announcement {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub target_type: String,
    pub publish_at: Option<String>,
    pub expire_at: Option<String>,
    pub status: String,
    pub created_by_name: Option<String>,
    pub created_at: String,
    pub read_at: Option<String>,
    pub targets: Vec<Target>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct TargetInput {
    pub target_type: String,
    pub target_id: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct AnnouncementInput {
    pub title: String,
    pub content: String,
    pub target_type: String,
    pub publish_at: Option<String>,
    pub expire_at: Option<String>,
    pub targets: Vec<TargetInput>,
}

fn valid_target(t: &str) -> bool {
    t == "all" || TARGETS.contains(&t)
}

fn employee_of(conn: &Connection, user_id: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT employee_id FROM users WHERE id = ?1",
        params![user_id],
        |r| r.get::<_, Option<i64>>(0),
    )
    .map_err(|e| format!("gagal memuat akun: {e}"))
}

fn user_roles(conn: &Connection, user_id: i64) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare("SELECT role_id FROM user_roles WHERE user_id = ?1")
        .map_err(|e| format!("gagal menyiapkan peran: {e}"))?;
    let rows = stmt
        .query_map(params![user_id], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("gagal membaca peran: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(out)
}

fn targets_of(conn: &Connection, id: i64) -> Result<Vec<Target>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT target_type, target_id FROM announcement_targets WHERE announcement_id = ?1",
        )
        .map_err(|e| format!("gagal menyiapkan target: {e}"))?;
    let rows = stmt
        .query_map(params![id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("gagal membaca target: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (tt, tid) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        let name = target_name(conn, &tt, tid);
        out.push(Target {
            target_type: tt,
            target_id: to_dto_int(tid, "announcement.target_id")?,
            target_name: name,
        });
    }
    Ok(out)
}

fn target_name(conn: &Connection, tt: &str, tid: i64) -> Option<String> {
    let sql = match tt {
        "department" => "SELECT name FROM departments WHERE id = ?1",
        "branch" => "SELECT name FROM branches WHERE id = ?1",
        "role" => "SELECT name FROM roles WHERE id = ?1",
        "employee" => {
            return conn
                .query_row(
                    "SELECT TRIM(first_name || ' ' || COALESCE(last_name,'')) FROM employees WHERE id = ?1",
                    params![tid],
                    |r| r.get::<_, String>(0),
                )
                .ok();
        }
        _ => return None,
    };
    conn.query_row(sql, params![tid], |r| r.get::<_, String>(0))
        .ok()
}

const VISIBILITY: &str = "a.status = 'published' AND a.deleted_at IS NULL AND (a.publish_at IS NULL OR a.publish_at <= datetime('now','localtime')) AND (a.expire_at IS NULL OR a.expire_at >= datetime('now','localtime'))";

/// Pengumuman yang terlihat untuk pengguna (ditambah status baca bila tertaut karyawan).
pub fn visible(conn: &Connection, user_id: i64) -> Result<Vec<Announcement>, String> {
    let emp = employee_of(conn, user_id)?;
    let roles = user_roles(conn, user_id)?;
    let (dept, branch): (Option<i64>, Option<i64>) = match emp {
        Some(e) => conn
            .query_row(
                "SELECT department_id, branch_id FROM employees WHERE id = ?1",
                params![e],
                |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, Option<i64>>(1)?)),
            )
            .map_err(|e| format!("gagal memuat karyawan: {e}"))?,
        None => (None, None),
    };
    let role_list = roles
        .iter()
        .map(|r| r.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT DISTINCT a.id, a.title, a.content, a.target_type, a.publish_at, a.expire_at, a.status, u.username, a.created_at, ar.read_at FROM announcements a LEFT JOIN users u ON u.id = a.created_by LEFT JOIN announcement_reads ar ON ar.announcement_id = a.id AND ar.employee_id = ?1 LEFT JOIN announcement_targets t ON t.announcement_id = a.id WHERE {VISIBILITY} AND (a.target_type = 'all' OR (t.target_type = 'department' AND t.target_id = ?2) OR (t.target_type = 'branch' AND t.target_id = ?3) OR (t.target_type = 'role' AND instr(',' || ?4 || ',', ',' || t.target_id || ',') > 0) OR (t.target_type = 'employee' AND t.target_id = ?1)) ORDER BY a.id DESC LIMIT 100"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("gagal menyiapkan pengumuman: {e}"))?;
    let rows = stmt
        .query_map(params![emp, dept, branch, role_list], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(|e| format!("gagal membaca pengumuman: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, title, content, tt, pub_at, exp_at, status, by, created, read) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Announcement {
            id: to_dto_int(id, "announcement.id")?,
            title,
            content,
            target_type: tt,
            publish_at: pub_at,
            expire_at: exp_at,
            status,
            created_by_name: by,
            created_at: created,
            read_at: read,
            targets: targets_of(conn, id)?,
        });
    }
    Ok(out)
}

/// Daftar kelola (semua status).
pub fn all(conn: &Connection) -> Result<Vec<Announcement>, String> {
    let mut stmt = conn
        .prepare("SELECT a.id, a.title, a.content, a.target_type, a.publish_at, a.expire_at, a.status, u.username, a.created_at FROM announcements a LEFT JOIN users u ON u.id = a.created_by WHERE a.deleted_at IS NULL ORDER BY a.id DESC LIMIT 200")
        .map_err(|e| format!("gagal menyiapkan pengumuman: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| format!("gagal membaca pengumuman: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, title, content, tt, pub_at, exp_at, status, by, created) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Announcement {
            id: to_dto_int(id, "announcement.id")?,
            title,
            content,
            target_type: tt,
            publish_at: pub_at,
            expire_at: exp_at,
            status,
            created_by_name: by,
            created_at: created,
            read_at: None,
            targets: targets_of(conn, id)?,
        });
    }
    Ok(out)
}

/// Buka satu pengumuman; tandai baca bila akun tertaut karyawan.
pub fn get(conn: &Connection, user_id: i64, id: i64) -> Result<Announcement, String> {
    let list = visible(conn, user_id)?;
    let mut found = list
        .into_iter()
        .find(|a| a.id as i64 == id)
        .ok_or("Pengumuman tidak ditemukan.".to_string())?;
    if let Some(emp) = employee_of(conn, user_id)? {
        conn.execute(
            "INSERT OR IGNORE INTO announcement_reads (announcement_id, employee_id, read_at) VALUES (?1, ?2, datetime('now','localtime'))",
            params![id, emp],
        )
        .map_err(|e| format!("gagal menandai baca: {e}"))?;
        found.read_at = Some("baru saja".to_string());
    }
    Ok(found)
}

pub fn create(conn: &Connection, actor_id: i64, input: &AnnouncementInput) -> Result<i32, String> {
    let title = input.title.trim();
    let content = input.content.trim();
    if title.is_empty() || content.is_empty() {
        return Err("Judul dan isi pengumuman wajib diisi.".to_string());
    }
    if !valid_target(&input.target_type) {
        return Err("Target pengumuman tidak valid.".to_string());
    }
    if input.target_type != "all" && input.targets.is_empty() {
        return Err("Pilih minimal satu target pengumuman.".to_string());
    }
    for t in &input.targets {
        if !TARGETS.contains(&t.target_type.as_str()) {
            return Err("Jenis target tidak valid.".to_string());
        }
    }
    conn.execute(
        "INSERT INTO announcements (title, content, target_type, publish_at, expire_at, status, created_by) VALUES (?1, ?2, ?3, ?4, ?5, 'published', ?6)",
        params![
            title,
            content,
            input.target_type,
            input.publish_at.as_deref(),
            input.expire_at.as_deref(),
            actor_id
        ],
    )
    .map_err(|e| format!("gagal menyimpan pengumuman: {e}"))?;
    let id = conn.last_insert_rowid();
    for t in &input.targets {
        conn.execute(
            "INSERT INTO announcement_targets (announcement_id, target_type, target_id) VALUES (?1, ?2, ?3)",
            params![id, t.target_type, t.target_id as i64],
        )
        .map_err(|e| format!("gagal menyimpan target: {e}"))?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "announcement",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(id, "announcement.id")
}

pub fn delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM announcement_targets WHERE announcement_id = ?1",
        params![id],
    )
    .map_err(|e| format!("gagal menghapus target: {e}"))?;
    let n = conn
        .execute("DELETE FROM announcements WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus pengumuman: {e}"))?;
    if n == 0 {
        return Err("Pengumuman tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "announcement",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
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
    fn pengumuman_target_dan_baca() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(create(
            &conn,
            admin,
            &AnnouncementInput {
                title: "".to_string(),
                content: "x".to_string(),
                target_type: "all".to_string(),
                publish_at: None,
                expire_at: None,
                targets: vec![],
            },
        )
        .is_err());
        assert!(create(
            &conn,
            admin,
            &AnnouncementInput {
                title: "Libur".to_string(),
                content: "Isi".to_string(),
                target_type: "department".to_string(),
                publish_at: None,
                expire_at: None,
                targets: vec![],
            },
        )
        .is_err());
        let dept: i64 = conn
            .query_row("SELECT id FROM departments LIMIT 1", [], |r| r.get(0))
            .unwrap();
        let id = create(
            &conn,
            admin,
            &AnnouncementInput {
                title: "Semua".to_string(),
                content: "Isi semua".to_string(),
                target_type: "all".to_string(),
                publish_at: None,
                expire_at: None,
                targets: vec![],
            },
        )
        .expect("all") as i64;
        let did = create(
            &conn,
            admin,
            &AnnouncementInput {
                title: "Dept".to_string(),
                content: "Isi dept".to_string(),
                target_type: "department".to_string(),
                publish_at: None,
                expire_at: None,
                targets: vec![TargetInput {
                    target_type: "department".to_string(),
                    target_id: to_dto_int(dept, "t").unwrap(),
                }],
            },
        )
        .expect("dept") as i64;
        assert_eq!(all(&conn).expect("all").len(), 2);
        // admin tak tertaut karyawan: hanya target semua yang terlihat
        let vis = visible(&conn, admin).expect("vis");
        assert!(vis.iter().any(|a| a.id as i64 == id));
        assert!(!vis.iter().any(|a| a.id as i64 == did));
        delete(&conn, admin, did).expect("hapus");
        assert_eq!(all(&conn).expect("all2").len(), 1);
    }
}
