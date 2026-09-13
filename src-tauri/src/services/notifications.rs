//! Notifikasi pengguna: daftar terbaru, daftar penuh, tandai dibaca.

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

use crate::entities::notification;
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

fn map(m: notification::Model) -> Result<Notification, String> {
    Ok(Notification {
        id: to_dto_int(m.id as i64, "notification.id")?,
        kind: m.kind,
        title: m.title,
        message: m.message,
        link: m.link,
        is_read: m.is_read != 0,
        created_at: m.created_at,
    })
}

pub async fn recent(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Vec<Notification>, String> {
    list(db, user_id, 8).await
}

pub async fn all(db: &sea_orm::DatabaseConnection, user_id: i64) -> Result<Vec<Notification>, String> {
    list(db, user_id, 100).await
}

async fn list(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    limit: u64,
) -> Result<Vec<Notification>, String> {
    notification::Entity::find()
        .filter(notification::Column::UserId.eq(user_id as i32))
        .order_by_desc(notification::Column::Id)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca notifikasi: {e}"))?
        .into_iter()
        .map(map)
        .collect()
}

pub async fn unread_count(db: &sea_orm::DatabaseConnection, user_id: i64) -> Result<i32, String> {
    let n = notification::Entity::find()
        .filter(notification::Column::UserId.eq(user_id as i32))
        .filter(notification::Column::IsRead.eq(0))
        .count(db)
        .await
        .map_err(|e| format!("gagal menghitung notifikasi: {e}"))?;
    to_dto_int(n as i64, "notification.unread")
}

pub async fn mark_read(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    id: i64,
) -> Result<(), String> {
    let found = notification::Entity::find_by_id(id as i32)
        .filter(notification::Column::UserId.eq(user_id as i32))
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat notifikasi: {e}"))?;
    let Some(m) = found else {
        return Err("Notifikasi tidak ditemukan.".to_string());
    };
    let mut am = m.into_active_model();
    am.is_read = Set(1);
    am.read_at = Set(Some(Local::now().format("%Y-%m-%d %H:%M:%S").to_string()));
    am.update(db)
        .await
        .map_err(|e| format!("gagal menandai notifikasi: {e}"))?;
    Ok(())
}

pub async fn mark_all(db: &sea_orm::DatabaseConnection, user_id: i64) -> Result<(), String> {
    use sea_orm::sea_query::Expr;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    notification::Entity::update_many()
        .col_expr(notification::Column::IsRead, Expr::value(1))
        .col_expr(notification::Column::ReadAt, Expr::value(now))
        .filter(notification::Column::UserId.eq(user_id as i32))
        .filter(notification::Column::IsRead.eq(0))
        .exec(db)
        .await
        .map_err(|e| format!("gagal menandai notifikasi: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::notification::{ActiveModel, Column};
    use crate::init_state;

    async fn seed10(db: &sea_orm::DatabaseConnection, uid: i32) {
        for i in 0..10 {
            ActiveModel {
                user_id: Set(uid),
                kind: Set("info".to_string()),
                title: Set(format!("N{i}")),
                ..Default::default()
            }
            .insert(db)
            .await
            .expect("insert");
        }
        // kolom wajib lain diisi default database
        let _ = Column::Id;
    }

    #[tokio::test]
    async fn notifikasi_terbaru_dan_tandai_baca() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let uid: i32 = 1;
        seed10(db, uid).await;
        assert_eq!(recent(db, uid as i64).await.expect("recent").len(), 8);
        assert_eq!(all(db, uid as i64).await.expect("all").len(), 10);
        assert_eq!(unread_count(db, uid as i64).await.expect("n"), 10);
        let first = recent(db, uid as i64).await.expect("r")[0].clone();
        mark_read(db, uid as i64, first.id as i64)
            .await
            .expect("read");
        assert_eq!(unread_count(db, uid as i64).await.expect("n2"), 9);
        mark_all(db, uid as i64).await.expect("all");
        assert_eq!(unread_count(db, uid as i64).await.expect("n3"), 0);
    }
}
