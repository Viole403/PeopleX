//! Manajemen peran, izin, dan pengguna (admin).

use rusqlite::{params, Connection, OptionalExtension};

use super::audit;
use crate::to_dto_int;

/// Peran untuk daftar dan form.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct Role {
    pub id: i32,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub user_count: i32,
}

/// Izin tunggal.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct Permission {
    pub id: i32,
    pub slug: String,
    pub name: String,
    pub module: String,
}

/// Baris pengguna untuk tabel admin.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone)]
pub struct UserRow {
    pub id: i32,
    pub username: String,
    pub email: String,
    pub status: String,
    pub must_change_password: bool,
    pub employee_name: Option<String>,
    pub roles: Vec<String>,
}

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 60
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn list_roles(conn: &Connection) -> Result<Vec<Role>, String> {
    let mut stmt = conn
        .prepare("SELECT r.id, r.slug, r.name, r.description, r.is_system, (SELECT COUNT(*) FROM user_roles ur WHERE ur.role_id = r.id) FROM roles r ORDER BY r.name")
        .map_err(|e| format!("gagal menyiapkan query peran: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca peran: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, slug, name, description, is_system, users) =
            row.map_err(|e| format!("gagal membaca baris peran: {e}"))?;
        out.push(Role {
            id: to_dto_int(id, "role.id")?,
            slug,
            name,
            description,
            is_system: is_system != 0,
            user_count: to_dto_int(users, "role.user_count")?,
        });
    }
    Ok(out)
}

pub fn create_role(
    conn: &Connection,
    actor_id: i64,
    slug: &str,
    name: &str,
    description: Option<&str>,
) -> Result<i32, String> {
    let slug = slug.trim();
    let name = name.trim();
    if !valid_slug(slug) {
        return Err("Slug peran hanya huruf kecil, angka, dan strip.".to_string());
    }
    if name.is_empty() {
        return Err("Nama peran wajib diisi.".to_string());
    }
    conn.execute(
        "INSERT INTO roles (slug, name, description, is_system) VALUES (?1, ?2, ?3, 0)",
        params![slug, name, description],
    )
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "Slug peran sudah dipakai.".to_string()
        } else {
            format!("gagal membuat peran: {e}")
        }
    })?;
    let id = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "roles",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Peran {name} dibuat")),
    )?;
    to_dto_int(id, "role.id")
}

pub fn update_role(
    conn: &Connection,
    actor_id: i64,
    role_id: i64,
    name: &str,
    description: Option<&str>,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Nama peran wajib diisi.".to_string());
    }
    let n = conn
        .execute(
            "UPDATE roles SET name = ?1, description = ?2 WHERE id = ?3",
            params![name, description, role_id],
        )
        .map_err(|e| format!("gagal memperbarui peran: {e}"))?;
    if n == 0 {
        return Err("Peran tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "roles",
        Some(&role_id.to_string()),
        None,
        None,
        Some(&format!("Peran {name} diperbarui")),
    )?;
    Ok(())
}

pub fn delete_role(conn: &Connection, actor_id: i64, role_id: i64) -> Result<(), String> {
    let row: Option<(String, i64)> = conn
        .query_row(
            "SELECT slug, is_system FROM roles WHERE id = ?1",
            params![role_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let Some((slug, is_system)) = row else {
        return Err("Peran tidak ditemukan.".to_string());
    };
    if is_system != 0 {
        return Err("Peran sistem tidak dapat dihapus.".to_string());
    }
    let assigned: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM user_roles WHERE role_id = ?1",
            params![role_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa pemakaian peran: {e}"))?;
    if assigned > 0 {
        return Err("Peran masih dipakai pengguna dan tidak dapat dihapus.".to_string());
    }
    conn.execute(
        "DELETE FROM role_permissions WHERE role_id = ?1",
        params![role_id],
    )
    .map_err(|e| format!("gagal menghapus izin peran: {e}"))?;
    conn.execute("DELETE FROM roles WHERE id = ?1", params![role_id])
        .map_err(|e| format!("gagal menghapus peran: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "roles",
        Some(&role_id.to_string()),
        None,
        None,
        Some(&format!("Peran {slug} dihapus")),
    )?;
    Ok(())
}

pub fn list_permissions(conn: &Connection) -> Result<Vec<Permission>, String> {
    let mut stmt = conn
        .prepare("SELECT id, slug, name, module FROM permissions ORDER BY module, name")
        .map_err(|e| format!("gagal menyiapkan query izin: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca izin: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, slug, name, module) = row.map_err(|e| format!("gagal membaca baris izin: {e}"))?;
        out.push(Permission {
            id: to_dto_int(id, "permission.id")?,
            slug,
            name,
            module,
        });
    }
    Ok(out)
}

pub fn role_permission_ids(conn: &Connection, role_id: i64) -> Result<Vec<i32>, String> {
    let mut stmt = conn
        .prepare("SELECT permission_id FROM role_permissions WHERE role_id = ?1")
        .map_err(|e| format!("gagal menyiapkan query izin peran: {e}"))?;
    let rows = stmt
        .query_map(params![role_id], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("gagal membaca izin peran: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let id: i64 = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(to_dto_int(id, "permission.id")?);
    }
    Ok(out)
}

/// Ganti seluruh izin sebuah peran (kecuali peran sistem).
pub fn sync_role_permissions(
    conn: &Connection,
    actor_id: i64,
    role_id: i64,
    permission_ids: &[i64],
) -> Result<(), String> {
    let is_system: Option<i64> = conn
        .query_row(
            "SELECT is_system FROM roles WHERE id = ?1",
            params![role_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    match is_system {
        None => return Err("Peran tidak ditemukan.".to_string()),
        Some(1) => return Err("Izin peran sistem tidak dapat diubah.".to_string()),
        _ => {}
    }
    conn.execute(
        "DELETE FROM role_permissions WHERE role_id = ?1",
        params![role_id],
    )
    .map_err(|e| format!("gagal menghapus izin lama: {e}"))?;
    for pid in permission_ids {
        conn.execute(
            "INSERT INTO role_permissions (role_id, permission_id) VALUES (?1, ?2)",
            params![role_id, pid],
        )
        .map_err(|e| format!("gagal menyimpan izin peran: {e}"))?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "role_permissions",
        Some(&role_id.to_string()),
        None,
        None,
        Some(&format!(
            "Izin peran {role_id} disinkron ({})",
            permission_ids.len()
        )),
    )?;
    Ok(())
}

pub fn list_users(conn: &Connection) -> Result<Vec<UserRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT u.id, u.username, u.email, u.status, u.must_change_password,
                    TRIM(COALESCE(e.first_name, '') || ' ' || COALESCE(e.last_name, ''))
             FROM users u LEFT JOIN employees e ON e.id = u.employee_id
             ORDER BY u.username",
        )
        .map_err(|e| format!("gagal menyiapkan query pengguna: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let name: String = r.get(5)?;
            Ok((
                id,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                if name.trim().is_empty() {
                    None
                } else {
                    Some(name)
                },
            ))
        })
        .map_err(|e| format!("gagal membaca pengguna: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, username, email, status, must, employee_name) =
            row.map_err(|e| format!("gagal membaca baris pengguna: {e}"))?;
        let mut stmt = conn
            .prepare("SELECT slug FROM roles r JOIN user_roles ur ON ur.role_id = r.id WHERE ur.user_id = ?1 ORDER BY slug")
            .map_err(|e| format!("gagal memuat peran pengguna: {e}"))?;
        let roles: Vec<String> = stmt
            .query_map(params![id], |r| r.get(0))
            .map_err(|e| format!("gagal memuat peran pengguna: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("gagal memuat peran pengguna: {e}"))?;
        out.push(UserRow {
            id: to_dto_int(id, "user.id").map_err(|e| format!("{e}"))?,
            username,
            email,
            status,
            must_change_password: must != 0,
            employee_name,
            roles,
        });
    }
    Ok(out)
}

pub fn user_role_ids(conn: &Connection, user_id: i64) -> Result<Vec<i32>, String> {
    let mut stmt = conn
        .prepare("SELECT role_id FROM user_roles WHERE user_id = ?1")
        .map_err(|e| format!("gagal menyiapkan query peran pengguna: {e}"))?;
    let rows = stmt
        .query_map(params![user_id], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("gagal membaca peran pengguna: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let id: i64 = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(to_dto_int(id, "role.id")?);
    }
    Ok(out)
}

/// Ganti seluruh peran pengguna. Cegah user melepas peran terakhirnya sendiri?
/// Aturan: admin tak boleh mencabut super-administrator dari dirinya sendiri.
pub fn sync_user_roles(
    conn: &Connection,
    actor_id: i64,
    user_id: i64,
    role_ids: &[i64],
) -> Result<(), String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengguna: {e}"))?;
    if exists.is_none() {
        return Err("Pengguna tidak ditemukan.".to_string());
    }
    if user_id == actor_id {
        let super_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM roles WHERE slug = 'super-administrator'",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memuat peran: {e}"))?;
        if let Some(sid) = super_id {
            let had: Option<i64> = conn
                .query_row(
                    "SELECT role_id FROM user_roles WHERE user_id = ?1 AND role_id = ?2",
                    params![user_id, sid],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| format!("gagal memeriksa peran: {e}"))?;
            if had.is_some() && !role_ids.contains(&sid) {
                return Err(
                    "Tidak dapat mencabut peran super-administrator dari diri sendiri.".to_string(),
                );
            }
        }
    }
    conn.execute(
        "DELETE FROM user_roles WHERE user_id = ?1",
        params![user_id],
    )
    .map_err(|e| format!("gagal menghapus peran lama: {e}"))?;
    for rid in role_ids {
        conn.execute(
            "INSERT INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
            params![user_id, rid],
        )
        .map_err(|e| format!("gagal menyimpan peran pengguna: {e}"))?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "user_roles",
        Some(&user_id.to_string()),
        None,
        None,
        Some(&format!("Peran pengguna {user_id} disinkron")),
    )?;
    Ok(())
}

pub fn toggle_user_status(
    conn: &Connection,
    actor_id: i64,
    user_id: i64,
    status: &str,
) -> Result<(), String> {
    if status != "active" && status != "inactive" {
        return Err("Status tidak valid.".to_string());
    }
    if user_id == actor_id && status == "inactive" {
        return Err("Tidak dapat menonaktifkan akun sendiri.".to_string());
    }
    let n = conn
        .execute(
            "UPDATE users SET status = ?1 WHERE id = ?2",
            params![status, user_id],
        )
        .map_err(|e| format!("gagal memperbarui status: {e}"))?;
    if n == 0 {
        return Err("Pengguna tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "users",
        Some(&user_id.to_string()),
        None,
        None,
        Some(&format!("Status pengguna {user_id} menjadi {status}")),
    )?;
    Ok(())
}

/// Reset oleh admin: set password baru + wajib ganti saat login berikut.
pub fn admin_reset_password(
    conn: &Connection,
    actor_id: i64,
    user_id: i64,
    new_password: &str,
) -> Result<(), String> {
    if new_password.chars().count() < 8 {
        return Err("Password baru minimal 8 karakter.".to_string());
    }
    let hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password: {e}"))?;
    let n = conn
        .execute(
            "UPDATE users SET password = ?1, must_change_password = 1, failed_login_attempts = 0, locked_until = NULL, password_reset_token = NULL, password_reset_expires_at = NULL WHERE id = ?2",
            params![hash, user_id],
        )
        .map_err(|e| format!("gagal mereset password: {e}"))?;
    if n == 0 {
        return Err("Pengguna tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "PASSWORD_RESET",
        "users",
        Some(&user_id.to_string()),
        None,
        None,
        Some(&format!("Password pengguna {user_id} direset admin")),
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

    fn admin_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .expect("admin")
    }

    fn make_user(conn: &Connection, username: &str) -> i64 {
        let hash = bcrypt::hash("PasswordAwal1", bcrypt::DEFAULT_COST).unwrap();
        conn.execute(
            "INSERT INTO users (username, email, password, status, must_change_password) VALUES (?1, ?2, ?3, 'active', 0)",
            params![username, format!("{username}@x.local"), hash],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn peran_baru_sinkron_izin_lalu_hapus() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin_id(&conn);
        let rid = create_role(&conn, actor, "qa-lead", "QA Lead", None).expect("create");
        let perms = list_permissions(&conn).expect("perms");
        let pick: Vec<i64> = perms
            .iter()
            .filter(|p| p.slug == "employee.view" || p.slug == "report.view")
            .map(|p| p.id as i64)
            .collect();
        assert_eq!(pick.len(), 2);
        sync_role_permissions(&conn, actor, rid as i64, &pick).expect("sync");
        let got = role_permission_ids(&conn, rid as i64).expect("ids");
        assert_eq!(got.len(), 2);
        sync_role_permissions(&conn, actor, rid as i64, &[]).expect("kosongkan");
        assert!(role_permission_ids(&conn, rid as i64)
            .expect("ids")
            .is_empty());
        delete_role(&conn, actor, rid as i64).expect("delete");
        assert!(list_roles(&conn)
            .expect("list")
            .iter()
            .all(|r| r.slug != "qa-lead"));
    }

    #[test]
    fn peran_sistem_dan_bertuan_dilindungi() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin_id(&conn);
        let super_id: i64 = conn
            .query_row(
                "SELECT id FROM roles WHERE slug = 'super-administrator'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(delete_role(&conn, actor, super_id).is_err());
        assert!(sync_role_permissions(&conn, actor, super_id, &[]).is_err());
        assert!(delete_role(&conn, actor, super_id).is_err());
        // super-administrator dipakai admin -> hapus ditolak
        let e = delete_role(&conn, actor, super_id).expect_err("bertuan");
        assert!(e.contains("sistem") || e.contains("dipakai"));
    }

    #[test]
    fn proteksi_diri_sendiri() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin_id(&conn);
        let e = toggle_user_status(&conn, actor, actor, "inactive").expect_err("diri sendiri");
        assert!(e.contains("sendiri"));
        let super_id: i64 = conn
            .query_row(
                "SELECT id FROM roles WHERE slug = 'super-administrator'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let e = sync_user_roles(&conn, actor, actor, &[]).expect_err("cabut peran sendiri");
        assert!(e.contains("sendiri"));
        // status user lain boleh diubah, lalu login-nya ditolak
        let other = make_user(&conn, "budi");
        toggle_user_status(&conn, actor, other, "inactive").expect("nonaktifkan");
        let status: String = conn
            .query_row(
                "SELECT status FROM users WHERE id = ?1",
                params![other],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "inactive");
        let _ = super_id;
    }

    #[test]
    fn admin_reset_password_memaksa_ganti() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin_id(&conn);
        let other = make_user(&conn, "siti");
        let e = admin_reset_password(&conn, actor, other, "pendek").expect_err("kebijakan");
        assert!(e.contains("minimal 8"));
        admin_reset_password(&conn, actor, other, "Sementara99").expect("reset");
        let (hash, must): (String, i64) = conn
            .query_row(
                "SELECT password, must_change_password FROM users WHERE id = ?1",
                params![other],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(must, 1);
        assert!(bcrypt::verify("Sementara99", &hash).unwrap());
    }

    #[test]
    fn audit_mencatat_aksi_rbac() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin_id(&conn);
        create_role(&conn, actor, "audit-probe", "Audit Probe", None).expect("create");
        let entries = super::super::audit::list(&conn, Some("roles"), 10).expect("list");
        assert!(entries.iter().any(|e| e.action == "CREATE"));
    }
}
