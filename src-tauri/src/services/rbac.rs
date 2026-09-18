//! Manajemen peran, izin, dan pengguna (admin).

use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};

use crate::entities::{employee, permission, role, role_permission, user, user_role};
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

pub async fn list_roles(db: &sea_orm::DatabaseConnection) -> Result<Vec<Role>, String> {
    let rows = role::Entity::find()
        .order_by_asc(role::Column::Name)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca peran: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        let users = user_role::Entity::find()
            .filter(user_role::Column::RoleId.eq(r.id))
            .count(db)
            .await
            .map_err(|e| format!("gagal membaca peran: {e}"))?;
        out.push(Role {
            id: to_dto_int(r.id as i64, "role.id")?,
            slug: r.slug,
            name: r.name,
            description: r.description,
            is_system: r.is_system != 0,
            user_count: to_dto_int(users as i64, "role.user_count")?,
        });
    }
    Ok(out)
}

pub async fn create_role(
    db: &sea_orm::DatabaseConnection,
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
    let m = role::ActiveModel {
        slug: Set(slug.to_string()),
        name: Set(name.to_string()),
        description: Set(description.map(str::to_string)),
        is_system: Set(0),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            "Slug peran sudah dipakai.".to_string()
        } else {
            format!("gagal membuat peran: {e}")
        }
    })?;
    to_dto_int(m.id as i64, "role.id")
}

pub async fn update_role(
    db: &sea_orm::DatabaseConnection,
    role_id: i64,
    name: &str,
    description: Option<&str>,
) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Nama peran wajib diisi.".to_string());
    }
    let r = role::Entity::find_by_id(role_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let Some(r) = r else {
        return Err("Peran tidak ditemukan.".to_string());
    };
    let mut am = r.into_active_model();
    am.name = Set(name.to_string());
    am.description = Set(description.map(str::to_string));
    am.update(db)
        .await
        .map_err(|e| format!("gagal memperbarui peran: {e}"))?;
    Ok(())
}

pub async fn delete_role(db: &sea_orm::DatabaseConnection, role_id: i64) -> Result<(), String> {
    let r = role::Entity::find_by_id(role_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let Some(r) = r else {
        return Err("Peran tidak ditemukan.".to_string());
    };
    if r.is_system != 0 {
        return Err("Peran sistem tidak dapat dihapus.".to_string());
    }
    let assigned = user_role::Entity::find()
        .filter(user_role::Column::RoleId.eq(role_id as i32))
        .count(db)
        .await
        .map_err(|e| format!("gagal memeriksa pemakaian peran: {e}"))?;
    if assigned > 0 {
        return Err("Peran masih dipakai pengguna dan tidak dapat dihapus.".to_string());
    }
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::RoleId.eq(role_id as i32))
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus izin peran: {e}"))?;
    role::Entity::delete_by_id(role_id as i32)
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus peran: {e}"))?;
    Ok(())
}

pub async fn list_permissions(db: &sea_orm::DatabaseConnection) -> Result<Vec<Permission>, String> {
    let rows = permission::Entity::find()
        .order_by_asc(permission::Column::Module)
        .order_by_asc(permission::Column::Name)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca izin: {e}"))?;
    let mut out = Vec::new();
    for (id, slug, name, module) in rows
        .into_iter()
        .map(|p| (p.id, p.slug, p.name, p.module))
    {
        out.push(Permission {
            id: to_dto_int(id as i64, "permission.id")?,
            slug,
            name,
            module,
        });
    }
    Ok(out)
}

pub async fn role_permission_ids(
    db: &sea_orm::DatabaseConnection,
    role_id: i64,
) -> Result<Vec<i32>, String> {
    let rows = role_permission::Entity::find()
        .filter(role_permission::Column::RoleId.eq(role_id as i32))
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca izin peran: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(to_dto_int(r.permission_id as i64, "permission.id")?);
    }
    Ok(out)
}

/// Ganti seluruh izin sebuah peran (kecuali peran sistem).
pub async fn sync_role_permissions(
    db: &sea_orm::DatabaseConnection,
    role_id: i64,
    permission_ids: &[i64],
) -> Result<(), String> {
    let r = role::Entity::find_by_id(role_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let Some(r) = r else {
        return Err("Peran tidak ditemukan.".to_string());
    };
    if r.is_system != 0 {
        return Err("Izin peran sistem tidak dapat diubah.".to_string());
    }
    role_permission::Entity::delete_many()
        .filter(role_permission::Column::RoleId.eq(role_id as i32))
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus izin lama: {e}"))?;
    for pid in permission_ids {
        role_permission::ActiveModel {
            role_id: Set(role_id as i32),
            permission_id: Set(*pid as i32),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal menyimpan izin peran: {e}"))?;
    }
    Ok(())
}

pub async fn list_users(db: &sea_orm::DatabaseConnection) -> Result<Vec<UserRow>, String> {
    let rows = user::Entity::find()
        .order_by_asc(user::Column::Username)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca pengguna: {e}"))?;
    let mut out = Vec::new();
    for u in rows {
        let employee_name = match u.employee_id {
            Some(eid) => employee::Entity::find_by_id(eid)
                .one(db)
                .await
                .map_err(|e| format!("gagal memuat karyawan: {e}"))?
                .map(|e| {
                    format!(
                        "{} {}",
                        e.first_name,
                        e.last_name.unwrap_or_default()
                    )
                    .trim()
                    .to_string()
                })
                .filter(|s| !s.is_empty()),
            None => None,
        };
        let urs = user_role::Entity::find()
            .filter(user_role::Column::UserId.eq(u.id))
            .all(db)
            .await
            .map_err(|e| format!("gagal memuat peran pengguna: {e}"))?;
        let mut roles = Vec::new();
        for ur in urs {
            if let Some(r) = role::Entity::find_by_id(ur.role_id)
                .one(db)
                .await
                .map_err(|e| format!("gagal memuat peran pengguna: {e}"))?
            {
                roles.push(r.slug);
            }
        }
        roles.sort();
        out.push(UserRow {
            id: to_dto_int(u.id as i64, "user.id").map_err(|e| format!("{e}"))?,
            username: u.username,
            email: u.email,
            status: u.status,
            must_change_password: u.must_change_password != 0,
            employee_name,
            roles,
        });
    }
    Ok(out)
}

pub async fn user_role_ids(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Vec<i32>, String> {
    let rows = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id as i32))
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca peran pengguna: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(to_dto_int(r.role_id as i64, "role.id")?);
    }
    Ok(out)
}

/// Ganti seluruh peran pengguna.
/// Aturan: admin tak boleh mencabut super-administrator dari dirinya sendiri.
pub async fn sync_user_roles(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    user_id: i64,
    role_ids: &[i64],
) -> Result<(), String> {
    let exists = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat pengguna: {e}"))?;
    if exists.is_none() {
        return Err("Pengguna tidak ditemukan.".to_string());
    }
    if user_id == actor_id {
        let super_role = role::Entity::find()
            .filter(role::Column::Slug.eq("super-administrator"))
            .one(db)
            .await
            .map_err(|e| format!("gagal memuat peran: {e}"))?;
        if let Some(sr) = super_role {
            let had = user_role::Entity::find()
                .filter(user_role::Column::UserId.eq(user_id as i32))
                .filter(user_role::Column::RoleId.eq(sr.id))
                .one(db)
                .await
                .map_err(|e| format!("gagal memeriksa peran: {e}"))?;
            if had.is_some() && !role_ids.contains(&(sr.id as i64)) {
                return Err(
                    "Tidak dapat mencabut peran super-administrator dari diri sendiri.".to_string(),
                );
            }
        }
    }
    user_role::Entity::delete_many()
        .filter(user_role::Column::UserId.eq(user_id as i32))
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus peran lama: {e}"))?;
    for rid in role_ids {
        user_role::ActiveModel {
            user_id: Set(user_id as i32),
            role_id: Set(*rid as i32),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal menyimpan peran pengguna: {e}"))?;
    }
    Ok(())
}

pub async fn toggle_user_status(
    db: &sea_orm::DatabaseConnection,
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
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat pengguna: {e}"))?;
    let Some(u) = u else {
        return Err("Pengguna tidak ditemukan.".to_string());
    };
    let mut am = u.into_active_model();
    am.status = Set(status.to_string());
    am.update(db)
        .await
        .map_err(|e| format!("gagal memperbarui status: {e}"))?;
    Ok(())
}

/// Reset oleh admin: set password baru + wajib ganti saat login berikut.
pub async fn admin_reset_password(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    new_password: &str,
) -> Result<(), String> {
    if new_password.chars().count() < 8 {
        return Err("Password baru minimal 8 karakter.".to_string());
    }
    let hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password: {e}"))?;
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat pengguna: {e}"))?;
    let Some(u) = u else {
        return Err("Pengguna tidak ditemukan.".to_string());
    };
    let mut am = u.into_active_model();
    am.password = Set(hash);
    am.must_change_password = Set(1);
    am.failed_login_attempts = Set(0);
    am.locked_until = Set(None);
    am.password_reset_token = Set(None);
    am.password_reset_expires_at = Set(None);
    am.update(db)
        .await
        .map_err(|e| format!("gagal mereset password: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::permission;
    use crate::init_state;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        user::Entity::find()
            .filter(user::Column::Username.eq("admin"))
            .one(db)
            .await
            .expect("admin")
            .expect("ada")
            .id as i64
    }

    async fn make_user(db: &sea_orm::DatabaseConnection, username: &str) -> i64 {
        let hash = bcrypt::hash("PasswordAwal1", bcrypt::DEFAULT_COST).unwrap();
        let m = user::ActiveModel {
            username: Set(username.to_string()),
            email: Set(format!("{username}@x.local")),
            password: Set(hash),
            status: Set("active".to_string()),
            must_change_password: Set(0),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
        m.id as i64
    }

    #[tokio::test]
    async fn peran_baru_sinkron_izin_lalu_hapus() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let rid = create_role(db, "qa-lead", "QA Lead", None).await.expect("create");
        let perms = list_permissions(db).await.expect("perms");
        let pick: Vec<i64> = perms
            .iter()
            .filter(|p| p.slug == "employee.view" || p.slug == "report.view")
            .map(|p| p.id as i64)
            .collect();
        assert_eq!(pick.len(), 2);
        sync_role_permissions(db, rid as i64, &pick).await.expect("sync");
        let got = role_permission_ids(db, rid as i64).await.expect("ids");
        assert_eq!(got.len(), 2);
        sync_role_permissions(db, rid as i64, &[]).await.expect("kosongkan");
        assert!(role_permission_ids(db, rid as i64)
            .await
            .expect("ids")
            .is_empty());
        delete_role(db, rid as i64).await.expect("delete");
        assert!(list_roles(db)
            .await
            .expect("list")
            .iter()
            .all(|r| r.slug != "qa-lead"));
    }

    #[tokio::test]
    async fn peran_sistem_dan_bertuan_dilindungi() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin_id(db).await;
        let super_id: i64 = role::Entity::find()
            .filter(role::Column::Slug.eq("super-administrator"))
            .one(db)
            .await
            .unwrap()
            .expect("ada")
            .id as i64;
        assert!(delete_role(db, super_id).await.is_err());
        assert!(sync_role_permissions(db, super_id, &[]).await.is_err());
        let _ = actor;
    }

    #[tokio::test]
    async fn proteksi_diri_sendiri() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin_id(db).await;
        let e = toggle_user_status(db, actor, actor, "inactive")
            .await
            .expect_err("diri sendiri");
        assert!(e.contains("sendiri"));
        let e = sync_user_roles(db, actor, actor, &[])
            .await
            .expect_err("cabut peran sendiri");
        assert!(e.contains("sendiri"));
        let other = make_user(db, "budi").await;
        toggle_user_status(db, actor, other, "inactive")
            .await
            .expect("nonaktifkan");
        let u = user::Entity::find_by_id(other as i32)
            .one(db)
            .await
            .unwrap()
            .expect("ada");
        assert_eq!(u.status, "inactive");
    }

    #[tokio::test]
    async fn admin_reset_password_memaksa_ganti() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let _actor = admin_id(db).await;
        let other = make_user(db, "siti").await;
        let e = admin_reset_password(db, other, "pendek")
            .await
            .expect_err("kebijakan");
        assert!(e.contains("minimal 8"));
        admin_reset_password(db, other, "Sementara99")
            .await
            .expect("reset");
        let u = user::Entity::find_by_id(other as i32)
            .one(db)
            .await
            .unwrap()
            .expect("ada");
        assert_eq!(u.must_change_password, 1);
        assert!(bcrypt::verify("Sementara99", &u.password).unwrap());
    }

    #[tokio::test]
    async fn audit_mencatat_aksi_rbac() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin_id(db).await;
        create_role(db, "audit-probe", "Audit Probe", None)
            .await
            .expect("create");
        crate::services::audit::log_sea(
            db,
            Some(actor),
            "CREATE",
            "roles",
            None,
            None,
            None,
            None,
        )
        .await
        .expect("log");
        let entries = crate::services::audit::list_sea(db, Some("roles"), 10)
            .await
            .expect("list");
        assert!(entries.iter().any(|e| e.action == "CREATE"));
    }

    #[tokio::test]
    async fn daftar_pengguna_memuat_peran() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let rows = list_users(db).await.expect("users");
        assert!(rows.iter().any(|u| u.username == "admin"
            && u.roles.iter().any(|r| r == "super-administrator")));
        let _ = permission::Entity::find()
            .one(db)
            .await
            .expect("perm");
    }
}
