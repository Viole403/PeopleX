//! Engine persetujuan terkonfigurasi + notifikasi dalam aplikasi.

use rusqlite::{params, Connection, OptionalExtension};

/// Satu tahap rantai yang sudah di-resolve ke user konkret.
pub struct ChainStep {
    pub order: i64,
    pub approver_id: i64,
    pub role: String,
}

/// Bangun rantai dari workflow aktif; tahap tanpa approver dilewati.
pub fn build_chain(
    conn: &Connection,
    module: &str,
    employee_id: i64,
) -> Result<Vec<ChainStep>, String> {
    let wf: Option<i64> = conn
        .query_row(
            "SELECT id FROM approval_workflows WHERE module = ?1 AND is_active = 1",
            params![module],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat alur: {e}"))?;
    let Some(wf_id) = wf else {
        return Ok(Vec::new());
    };
    let mut stmt = conn
        .prepare("SELECT approver_type, role_id, user_id FROM approval_steps WHERE approval_workflow_id = ?1 ORDER BY step_order ASC")
        .map_err(|e| format!("gagal menyiapkan tahap: {e}"))?;
    let rows = stmt
        .query_map(params![wf_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca tahap: {e}"))?;
    let emp: Option<(Option<i64>, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT supervisor_id, manager_id, department_id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![employee_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    let Some((sup, mgr, dept)) = emp else {
        return Ok(Vec::new());
    };
    let mut chain = Vec::new();
    let mut order = 1;
    for row in rows {
        let (atype, role_id, user_id) =
            row.map_err(|e| format!("gagal membaca baris tahap: {e}"))?;
        let approver = match atype.as_str() {
            "supervisor" => sup.and_then(|e| user_of_employee(conn, e).ok().flatten()),
            "manager" => mgr.and_then(|e| user_of_employee(conn, e).ok().flatten()),
            "department_head" => dept
                .and_then(|d| {
                    conn.query_row(
                        "SELECT head_employee_id FROM departments WHERE id = ?1",
                        params![d],
                        |r| r.get::<_, Option<i64>>(0),
                    )
                    .optional()
                    .unwrap_or(None)
                    .flatten()
                })
                .and_then(|e| user_of_employee(conn, e).ok().flatten()),
            "role" => role_id.and_then(|rid| {
                conn.query_row(
                    "SELECT ur.user_id FROM user_roles ur INNER JOIN users u ON u.id = ur.user_id WHERE ur.role_id = ?1 AND u.status = 'active' ORDER BY ur.user_id ASC LIMIT 1",
                    params![rid],
                    |r| r.get::<_, i64>(0),
                )
                .optional()
                .unwrap_or(None)
            }),
            "specific_user" => user_id,
            _ => None,
        };
        if let Some(aid) = approver {
            chain.push(ChainStep {
                order,
                approver_id: aid,
                role: atype,
            });
            order += 1;
        }
    }
    Ok(chain)
}

/// User boleh bertindak bila ia approver tercatat, atau sesama pemegang peran utk tahap role.
pub fn user_has_access(
    conn: &Connection,
    user_id: i64,
    approver_id: i64,
    step_role: &str,
) -> Result<bool, String> {
    if user_id == approver_id {
        return Ok(true);
    }
    if step_role != "role" {
        return Ok(false);
    }
    let shared: Option<i64> = conn
        .query_row(
            "SELECT 1 FROM user_roles ur1 INNER JOIN user_roles ur2 ON ur2.role_id = ur1.role_id WHERE ur1.user_id = ?1 AND ur2.user_id = ?2 LIMIT 1",
            params![approver_id, user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa peran: {e}"))?;
    Ok(shared.is_some())
}

/// user aktif dari employee (untuk resolve approver/notifikasi).
pub fn user_of_employee(conn: &Connection, employee_id: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT id FROM users WHERE employee_id = ?1 AND status = 'active'",
        params![employee_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| format!("gagal mencari user karyawan: {e}"))
}

/// employee dari user login.
pub fn employee_of_user(conn: &Connection, user_id: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT employee_id FROM users WHERE id = ?1",
        params![user_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| format!("gagal memuat akun: {e}"))
}

/// Notifikasi dalam aplikasi.
pub fn notify(
    conn: &Connection,
    user_id: i64,
    notif_type: &str,
    title: &str,
    message: &str,
    link: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO notifications (user_id, type, title, message, link) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, notif_type, title, message, link],
    )
    .map_err(|e| format!("gagal mengirim notifikasi: {e}"))?;
    Ok(())
}
