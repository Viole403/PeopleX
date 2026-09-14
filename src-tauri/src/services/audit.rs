//! Audit log: catat setiap perubahan penting (append-only, tanpa UI hapus).

use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::entities::audit_log;
use crate::to_dto_int;

/// Baris audit untuk daftar.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct AuditEntry {
    pub id: i32,
    pub user_id: Option<i32>,
    pub action: String,
    pub module: String,
    pub record_id: Option<String>,
    pub description: Option<String>,
    pub created_at: String,
}

/// Tulis satu baris audit lewat SeaORM. Jalur baru untuk command yang sudah pindah.
#[allow(clippy::too_many_arguments)]
pub async fn log_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: Option<i64>,
    action: &str,
    module: &str,
    record_id: Option<&str>,
    before: Option<&str>,
    after: Option<&str>,
    description: Option<&str>,
) -> Result<(), String> {
    audit_log::ActiveModel {
        user_id: Set(user_id.map(|v| v as i32)),
        action: Set(action.to_string()),
        module: Set(module.to_string()),
        record_id: Set(record_id.map(str::to_string)),
        description: Set(description.map(str::to_string)),
        before_data: Set(before.map(str::to_string)),
        after_data: Set(after.map(str::to_string)),
        ip_address: Set(Some("desktop".to_string())),
        user_agent: Set(Some("peoplex".to_string())),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("gagal menulis audit log: {e}"))?;
    Ok(())
}

fn map_entry(m: audit_log::Model) -> Result<AuditEntry, String> {
    Ok(AuditEntry {
        id: to_dto_int(m.id as i64, "audit.id")?,
        user_id: m
            .user_id
            .map(|v| to_dto_int(v as i64, "audit.user_id"))
            .transpose()?,
        action: m.action,
        module: m.module,
        record_id: m.record_id,
        description: m.description,
        created_at: m.created_at,
    })
}

/// Daftar audit terbaru lewat SeaORM, opsional filter modul.
pub async fn list_sea(
    db: &sea_orm::DatabaseConnection,
    module: Option<&str>,
    limit: i64,
) -> Result<Vec<AuditEntry>, String> {
    let limit = limit.clamp(1, 500) as u64;
    let mut q = audit_log::Entity::find().order_by_desc(audit_log::Column::Id);
    if let Some(m) = module {
        q = q.filter(audit_log::Column::Module.eq(m));
    }
    q.limit(limit)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca audit log: {e}"))?
        .into_iter()
        .map(map_entry)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_state;

    #[tokio::test]
    async fn tulis_dan_baca_lewat_seaorm() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        log_sea(db, Some(1), "CREATE", "uji", Some("7"), None, None, Some("uji tulis"))
            .await
            .expect("tulis");
        let rows = list_sea(db, Some("uji"), 10).await.expect("baca");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action, "CREATE");
        assert_eq!(rows[0].user_id, Some(1));
        let all = list_sea(db, None, 10).await.expect("semua");
        assert!(all.iter().any(|e| e.module == "uji"));
    }
}
