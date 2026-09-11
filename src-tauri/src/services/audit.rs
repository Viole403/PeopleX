//! Audit log: catat setiap perubahan penting (append-only, tanpa UI hapus).

use rusqlite::{params, Connection};

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

/// Tulis satu baris audit. before/after berupa JSON string.
pub fn log(
    conn: &Connection,
    user_id: Option<i64>,
    action: &str,
    module: &str,
    record_id: Option<&str>,
    before: Option<&str>,
    after: Option<&str>,
    description: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO audit_logs (user_id, action, module, record_id, description, before_data, after_data, ip_address, user_agent) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'desktop', 'peoplex')",
        params![user_id, action, module, record_id, description, before, after],
    )
    .map_err(|e| format!("gagal menulis audit log: {e}"))?;
    Ok(())
}

/// Daftar audit terbaru, opsional filter modul.
pub fn list(
    conn: &Connection,
    module: Option<&str>,
    limit: i64,
) -> Result<Vec<AuditEntry>, String> {
    let limit = limit.clamp(1, 500);
    let sql = match module {
        Some(_) => "SELECT id, user_id, action, module, record_id, description, created_at FROM audit_logs WHERE module = ?1 ORDER BY id DESC LIMIT ?2",
        None => "SELECT id, user_id, action, module, record_id, description, created_at FROM audit_logs ORDER BY id DESC LIMIT ?1",
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("gagal menyiapkan query audit: {e}"))?;
    let map_row = |r: &rusqlite::Row<'_>| {
        let id: i64 = r.get(0)?;
        let user_id: Option<i64> = r.get(1)?;
        Ok((
            id,
            user_id,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, String>(6)?,
        ))
    };
    let rows = match module {
        Some(m) => stmt
            .query_map(params![m, limit], map_row)
            .map_err(|e| format!("gagal membaca audit log: {e}"))?,
        None => stmt
            .query_map(params![limit], map_row)
            .map_err(|e| format!("gagal membaca audit log: {e}"))?,
    };
    // query_map dengan branch berbeda menghasilkan tipe iterator berbeda;
    // satukan lewat Vec per branch.
    let mut out = Vec::new();
    for row in rows {
        let (id, user_id, action, module, record_id, description, created_at) =
            row.map_err(|e| format!("gagal membaca baris audit: {e}"))?;
        let user_id = match user_id {
            Some(v) => Some(to_dto_int(v, "audit.user_id")?),
            None => None,
        };
        out.push(AuditEntry {
            id: to_dto_int(id, "audit.id")?,
            user_id,
            action,
            module,
            record_id,
            description,
            created_at,
        });
    }
    Ok(out)
}
