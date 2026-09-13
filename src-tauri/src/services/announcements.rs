//! Pengumuman: daftar terlihat, kelola, dan tandai baca.

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::entities::{
    announcement, announcement_read, announcement_target, branch, department, employee, role,
    user, user_role,
};
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

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

async fn employee_of(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Option<i32>, String> {
    user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat akun: {e}"))
        .map(|u| u.and_then(|u| u.employee_id))
}

async fn user_roles(db: &sea_orm::DatabaseConnection, user_id: i64) -> Result<Vec<i32>, String> {
    user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id as i32))
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca peran: {e}"))
        .map(|rows| rows.into_iter().map(|r| r.role_id).collect())
}

async fn target_name(
    db: &sea_orm::DatabaseConnection,
    tt: &str,
    tid: i32,
) -> Result<Option<String>, String> {
    let err = |e: sea_orm::DbErr| format!("gagal memuat target: {e}");
    match tt {
        "department" => department::Entity::find_by_id(tid)
            .one(db)
            .await
            .map_err(err)
            .map(|d| d.map(|d| d.name)),
        "branch" => branch::Entity::find_by_id(tid)
            .one(db)
            .await
            .map_err(err)
            .map(|b| b.map(|b| b.name)),
        "role" => role::Entity::find_by_id(tid)
            .one(db)
            .await
            .map_err(err)
            .map(|r| r.map(|r| r.name)),
        "employee" => employee::Entity::find_by_id(tid)
            .one(db)
            .await
            .map_err(err)
            .map(|e| {
                e.map(|e| {
                    format!("{} {}", e.first_name, e.last_name.unwrap_or_default()).trim().to_string()
                })
            }),
        _ => Ok(None),
    }
}

async fn targets_of(
    db: &sea_orm::DatabaseConnection,
    id: i32,
) -> Result<Vec<Target>, String> {
    let rows = announcement_target::Entity::find()
        .filter(announcement_target::Column::AnnouncementId.eq(id))
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca target: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(Target {
            target_name: target_name(db, &r.target_type, r.target_id).await?,
            target_type: r.target_type,
            target_id: r.target_id,
        });
    }
    Ok(out)
}

async fn created_by_name(
    db: &sea_orm::DatabaseConnection,
    uid: Option<i32>,
) -> Result<Option<String>, String> {
    match uid {
        Some(id) => {
            user::Entity::find_by_id(id)
                .one(db)
                .await
                .map_err(|e| format!("gagal memuat pembuat: {e}"))
                .map(|u| u.map(|u| u.username))
        }
        None => Ok(None),
    }
}

fn in_window(m: &announcement::Model, now: &str) -> bool {
    if let Some(p) = m.publish_at.as_deref() {
        if p > now {
            return false;
        }
    }
    if let Some(e) = m.expire_at.as_deref() {
        if e < now {
            return false;
        }
    }
    true
}

fn target_hit(
    m: &announcement::Model,
    targets: &[announcement_target::Model],
    emp: Option<i32>,
    dept: Option<i32>,
    branch: Option<i32>,
    roles: &[i32],
) -> bool {
    if m.target_type == "all" {
        return true;
    }
    targets.iter().any(|t| match t.target_type.as_str() {
        "department" => dept == Some(t.target_id),
        "branch" => branch == Some(t.target_id),
        "role" => roles.contains(&t.target_id),
        "employee" => emp == Some(t.target_id),
        _ => false,
    })
}

async fn to_dto(
    db: &sea_orm::DatabaseConnection,
    m: announcement::Model,
    read_at: Option<String>,
) -> Result<Announcement, String> {
    Ok(Announcement {
        id: to_dto_int(m.id as i64, "announcement.id")?,
        title: m.title,
        content: m.content,
        target_type: m.target_type,
        publish_at: m.publish_at,
        expire_at: m.expire_at,
        status: m.status,
        created_by_name: created_by_name(db, m.created_by).await?,
        created_at: m.created_at,
        read_at,
        targets: targets_of(db, m.id).await?,
    })
}

/// Pengumuman yang terlihat untuk pengguna (ditambah status baca bila tertaut karyawan).
pub async fn visible(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Vec<Announcement>, String> {
    let emp = employee_of(db, user_id).await?;
    let roles = user_roles(db, user_id).await?;
    let (dept, branch) = match emp {
        Some(e) => employee::Entity::find_by_id(e)
            .one(db)
            .await
            .map_err(|e| format!("gagal memuat karyawan: {e}"))?
            .map(|m| (m.department_id, m.branch_id))
            .unwrap_or((None, None)),
        None => (None, None),
    };
    let now = now_str();
    let rows = announcement::Entity::find()
        .filter(announcement::Column::Status.eq("published"))
        .filter(announcement::Column::DeletedAt.is_null())
        .order_by_desc(announcement::Column::Id)
        .limit(200)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca pengumuman: {e}"))?;
    let mut out = Vec::new();
    for m in rows {
        if !in_window(&m, &now) {
            continue;
        }
        let targets = announcement_target::Entity::find()
            .filter(announcement_target::Column::AnnouncementId.eq(m.id))
            .all(db)
            .await
            .map_err(|e| format!("gagal membaca target: {e}"))?;
        if !target_hit(&m, &targets, emp, dept, branch, &roles) {
            continue;
        }
        let read = match emp {
            Some(e) => announcement_read::Entity::find()
                .filter(announcement_read::Column::AnnouncementId.eq(m.id))
                .filter(announcement_read::Column::EmployeeId.eq(e))
                .one(db)
                .await
                .map_err(|e| format!("gagal membaca status baca: {e}"))?
                .map(|r| r.read_at),
            None => None,
        };
        out.push(to_dto(db, m, read).await?);
        if out.len() >= 100 {
            break;
        }
    }
    Ok(out)
}

/// Daftar kelola (semua status).
pub async fn all(db: &sea_orm::DatabaseConnection) -> Result<Vec<Announcement>, String> {
    let rows = announcement::Entity::find()
        .filter(announcement::Column::DeletedAt.is_null())
        .order_by_desc(announcement::Column::Id)
        .limit(200)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca pengumuman: {e}"))?;
    let mut out = Vec::new();
    for m in rows {
        out.push(to_dto(db, m, None).await?);
    }
    Ok(out)
}

/// Buka satu pengumuman; tandai baca bila akun tertaut karyawan.
pub async fn get(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    id: i64,
) -> Result<Announcement, String> {
    let list = visible(db, user_id).await?;
    let mut found = list
        .into_iter()
        .find(|a| a.id as i64 == id)
        .ok_or("Pengumuman tidak ditemukan.".to_string())?;
    if let Some(emp) = employee_of(db, user_id).await? {
        let exists = announcement_read::Entity::find()
            .filter(announcement_read::Column::AnnouncementId.eq(id as i32))
            .filter(announcement_read::Column::EmployeeId.eq(emp))
            .one(db)
            .await
            .map_err(|e| format!("gagal memeriksa baca: {e}"))?;
        if exists.is_none() {
            announcement_read::ActiveModel {
                announcement_id: Set(id as i32),
                employee_id: Set(emp),
                read_at: Set(now_str()),
                ..Default::default()
            }
            .insert(db)
            .await
            .map_err(|e| format!("gagal menandai baca: {e}"))?;
        }
        found.read_at = Some("baru saja".to_string());
    }
    Ok(found)
}

pub async fn create(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    input: &AnnouncementInput,
) -> Result<i32, String> {
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
    let m = announcement::ActiveModel {
        title: Set(title.to_string()),
        content: Set(content.to_string()),
        target_type: Set(input.target_type.clone()),
        publish_at: Set(input.publish_at.clone()),
        expire_at: Set(input.expire_at.clone()),
        status: Set("published".to_string()),
        created_by: Set(Some(actor_id as i32)),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("gagal menyimpan pengumuman: {e}"))?;
    for t in &input.targets {
        announcement_target::ActiveModel {
            announcement_id: Set(m.id),
            target_type: Set(t.target_type.clone()),
            target_id: Set(t.target_id),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal menyimpan target: {e}"))?;
    }
    to_dto_int(m.id as i64, "announcement.id")
}

pub async fn delete(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(), String> {
    announcement_target::Entity::delete_many()
        .filter(announcement_target::Column::AnnouncementId.eq(id as i32))
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus target: {e}"))?;
    let r = announcement::Entity::delete_by_id(id as i32)
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus pengumuman: {e}"))?;
    if r.rows_affected == 0 {
        return Err("Pengumuman tidak ditemukan.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_state;

    async fn other_dept(db: &sea_orm::DatabaseConnection) -> i32 {
        use sea_orm::ColumnTrait;
        use sea_orm::EntityTrait;
        use sea_orm::QueryFilter;
        // departemen admin adalah 1; pilih yang lain agar target tak terlihat admin
        department::Entity::find()
            .filter(department::Column::Id.ne(1))
            .one(db)
            .await
            .expect("dept")
            .expect("ada")
            .id
    }

    #[tokio::test]
    async fn pengumuman_target_dan_baca() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin = 1;
        assert!(create(
            db,
            admin,
            &AnnouncementInput {
                title: "".to_string(),
                content: "x".to_string(),
                target_type: "all".to_string(),
                ..Default::default()
            },
        )
        .await
        .is_err());
        assert!(create(
            db,
            admin,
            &AnnouncementInput {
                title: "Libur".to_string(),
                content: "Isi".to_string(),
                target_type: "department".to_string(),
                ..Default::default()
            },
        )
        .await
        .is_err());
        let dept = other_dept(db).await;
        let id = create(
            db,
            admin,
            &AnnouncementInput {
                title: "Semua".to_string(),
                content: "Isi semua".to_string(),
                target_type: "all".to_string(),
                ..Default::default()
            },
        )
        .await
        .expect("all");
        let did = create(
            db,
            admin,
            &AnnouncementInput {
                title: "Dept".to_string(),
                content: "Isi dept".to_string(),
                target_type: "department".to_string(),
                targets: vec![TargetInput {
                    target_type: "department".to_string(),
                    target_id: dept,
                }],
                ..Default::default()
            },
        )
        .await
        .expect("dept");
        assert_eq!(all(db).await.expect("all").len(), 2);
        let vis = visible(db, admin).await.expect("vis");
        assert!(vis.iter().any(|a| a.id == id));
        assert!(!vis.iter().any(|a| a.id == did));
        delete(db, did as i64).await.expect("hapus");
        assert_eq!(all(db).await.expect("all2").len(), 1);
    }
}
