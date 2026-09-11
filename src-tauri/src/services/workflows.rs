//! Konfigurasi alur persetujuan per modul.

use rusqlite::{params, Connection, OptionalExtension};

use super::audit;
use crate::to_dto_int;

/// Satu tahap persetujuan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct WorkflowStep {
    pub id: i32,
    pub step_order: i32,
    pub approver_type: String,
    pub role_id: Option<i32>,
    pub user_id: Option<i32>,
    pub role_name: Option<String>,
    pub username: Option<String>,
}

/// Alur beserta tahap-tahapnya.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Workflow {
    pub id: i32,
    pub module: String,
    pub name: String,
    pub is_active: bool,
    pub steps: Vec<WorkflowStep>,
}

const APPROVER_TYPES: &[&str] = &[
    "supervisor",
    "manager",
    "role",
    "specific_user",
    "department_head",
];

pub fn list(conn: &Connection) -> Result<Vec<Workflow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, module, name, is_active FROM approval_workflows ORDER BY module")
        .map_err(|e| format!("gagal menyiapkan query alur: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca alur: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, module, name, active) =
            row.map_err(|e| format!("gagal membaca baris alur: {e}"))?;
        let mut sstmt = conn
            .prepare(
                "SELECT s.id, s.step_order, s.approver_type, s.role_id, s.user_id, r.name, u.username
                 FROM approval_steps s
                 LEFT JOIN roles r ON r.id = s.role_id
                 LEFT JOIN users u ON u.id = s.user_id
                 WHERE s.approval_workflow_id = ?1 ORDER BY s.step_order",
            )
            .map_err(|e| format!("gagal menyiapkan query tahap: {e}"))?;
        let srows = sstmt
            .query_map(params![id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(|e| format!("gagal membaca tahap: {e}"))?;
        let mut steps = Vec::new();
        for srow in srows {
            let (sid, order, atype, role_id, user_id, role_name, username) =
                srow.map_err(|e| format!("gagal membaca baris tahap: {e}"))?;
            steps.push(WorkflowStep {
                id: to_dto_int(sid, "step.id")?,
                step_order: to_dto_int(order, "step.order")?,
                approver_type: atype,
                role_id: role_id.map(|v| to_dto_int(v, "step.role")).transpose()?,
                user_id: user_id.map(|v| to_dto_int(v, "step.user")).transpose()?,
                role_name,
                username,
            });
        }
        out.push(Workflow {
            id: to_dto_int(id, "workflow.id")?,
            module,
            name,
            is_active: active != 0,
            steps,
        });
    }
    Ok(out)
}

/// Tambah tahap di urutan akhir.
pub fn add_step(
    conn: &Connection,
    actor_id: i64,
    workflow_id: i64,
    approver_type: &str,
    role_id: Option<i64>,
    user_id: Option<i64>,
) -> Result<i32, String> {
    if !APPROVER_TYPES.contains(&approver_type) {
        return Err("Tipe approver tidak valid.".to_string());
    }
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM approval_workflows WHERE id = ?1",
            params![workflow_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat alur: {e}"))?;
    if exists.is_none() {
        return Err("Alur tidak ditemukan.".to_string());
    }
    if approver_type == "role" {
        match role_id {
            Some(rid) => {
                let found: Option<i64> = conn
                    .query_row("SELECT id FROM roles WHERE id = ?1", params![rid], |r| {
                        r.get(0)
                    })
                    .optional()
                    .map_err(|e| format!("gagal memeriksa peran: {e}"))?;
                if found.is_none() {
                    return Err("Peran tidak ditemukan.".to_string());
                }
            }
            None => return Err("Tahap peran wajib memilih peran.".to_string()),
        }
    }
    if approver_type == "specific_user" {
        match user_id {
            Some(uid) => {
                let found: Option<i64> = conn
                    .query_row("SELECT id FROM users WHERE id = ?1", params![uid], |r| {
                        r.get(0)
                    })
                    .optional()
                    .map_err(|e| format!("gagal memeriksa pengguna: {e}"))?;
                if found.is_none() {
                    return Err("Pengguna tidak ditemukan.".to_string());
                }
            }
            None => return Err("Tahap pengguna wajib memilih pengguna.".to_string()),
        }
    }
    let max_order: Option<i64> = conn
        .query_row(
            "SELECT MAX(step_order) FROM approval_steps WHERE approval_workflow_id = ?1",
            params![workflow_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung urutan: {e}"))?;
    let order = max_order.unwrap_or(0) + 1;
    conn.execute(
        "INSERT INTO approval_steps (approval_workflow_id, step_order, approver_type, role_id, user_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![workflow_id, order, approver_type, role_id, user_id],
    )
    .map_err(|e| format!("gagal menambah tahap: {e}"))?;
    let id = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "approval_step",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Tahap {order} ({approver_type}) ditambah")),
    )?;
    to_dto_int(id, "step.id")
}

pub fn remove_step(conn: &Connection, actor_id: i64, step_id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM approval_steps WHERE id = ?1", params![step_id])
        .map_err(|e| format!("gagal menghapus tahap: {e}"))?;
    if n == 0 {
        return Err("Tahap tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "approval_step",
        Some(&step_id.to_string()),
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
    fn alur_lengkap_dan_tambah_hapus_tahap() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let all = list(&conn).expect("list");
        assert_eq!(all.len(), 5);
        let leave = all.iter().find(|w| w.module == "leave").expect("cuti");
        assert_eq!(leave.steps.len(), 3);

        let e = add_step(&conn, actor, leave.id as i64, "jelangkung", None, None)
            .expect_err("tipe invalid");
        assert!(e.contains("Tipe approver"));
        let e =
            add_step(&conn, actor, leave.id as i64, "role", None, None).expect_err("peran wajib");
        assert!(e.contains("memilih peran"));

        let role_id: i64 = conn
            .query_row("SELECT id FROM roles WHERE slug = 'finance'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let sid =
            add_step(&conn, actor, leave.id as i64, "role", Some(role_id), None).expect("tambah");
        let again = list(&conn).expect("list");
        let leave2 = again.iter().find(|w| w.module == "leave").expect("cuti");
        assert_eq!(leave2.steps.len(), 4);
        assert_eq!(leave2.steps.last().expect("akhir").step_order, 4);
        remove_step(&conn, actor, sid as i64).expect("hapus");
        assert!(remove_step(&conn, actor, 999999).is_err());
    }
}
