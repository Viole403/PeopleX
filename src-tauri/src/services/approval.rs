//! Engine persetujuan terkonfigurasi + notifikasi dalam aplikasi.

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

/// Satu tahap rantai yang sudah di-resolve ke user konkret.
pub struct ChainStep {
    pub order: i64,
    pub approver_id: i64,
    pub role: String,
}

// ---------------- Varian SeaORM ----------------

pub async fn user_of_employee_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Option<i64>, String> {
    let row = q_one(
        db,
        "SELECT id FROM users WHERE employee_id = ?1 AND status = 'active'".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "approval.userof",
    )
    .await
    .map_err(|e| format!("gagal mencari user karyawan: {e}"))?;
    Ok(row.as_ref().and_then(|r| value_i64(&r[0])))
}

pub async fn employee_of_user_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Option<i64>, String> {
    let row = q_one(
        db,
        "SELECT employee_id FROM users WHERE id = ?1".to_string(),
        vec![Value::Int(user_id)],
        1,
        "approval.empof",
    )
    .await
    .map_err(|e| format!("gagal memuat akun: {e}"))?;
    Ok(row.as_ref().and_then(|r| value_i64(&r[0])))
}

pub async fn notify_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    notif_type: &str,
    title: &str,
    message: &str,
    link: &str,
) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO notifications (user_id, type, title, message, link) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(user_id),
            Value::Text(notif_type.to_string()),
            Value::Text(title.to_string()),
            Value::Text(message.to_string()),
            Value::Text(link.to_string()),
        ],
        "approval.notify",
    )
    .await
    .map_err(|e| format!("gagal mengirim notifikasi: {e}"))?;
    Ok(())
}

pub async fn user_has_access_sea(
    db: &sea_orm::DatabaseConnection,
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
    let row = q_one(
        db,
        "SELECT 1 FROM user_roles ur1 INNER JOIN user_roles ur2 ON ur2.role_id = ur1.role_id WHERE ur1.user_id = ?1 AND ur2.user_id = ?2 LIMIT 1".to_string(),
        vec![Value::Int(approver_id), Value::Int(user_id)],
        1,
        "approval.shared",
    )
    .await
    .map_err(|e| format!("gagal memeriksa peran: {e}"))?;
    Ok(row.is_some())
}

/// Bangun rantai dari workflow aktif; tahap tanpa approver dilewati.
pub async fn build_chain_sea(
    db: &sea_orm::DatabaseConnection,
    module: &str,
    employee_id: i64,
) -> Result<Vec<ChainStep>, String> {
    let wf = q_one(
        db,
        "SELECT id FROM approval_workflows WHERE module = ?1 AND is_active = 1".to_string(),
        vec![Value::Text(module.to_string())],
        1,
        "approval.wf",
    )
    .await
    .map_err(|e| format!("gagal memuat alur: {e}"))?;
    let Some(wf_row) = wf else {
        return Ok(Vec::new());
    };
    let wf_id = value_i64(&wf_row[0]).unwrap_or(0);
    let steps = q_all(
        db,
        "SELECT approver_type, role_id, user_id FROM approval_steps WHERE approval_workflow_id = ?1 ORDER BY step_order ASC".to_string(),
        vec![Value::Int(wf_id)],
        3,
        "approval.steps",
    )
    .await
    .map_err(|e| format!("gagal membaca tahap: {e}"))?;
    let emp = q_one(
        db,
        "SELECT supervisor_id, manager_id, department_id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(employee_id)],
        3,
        "approval.emp",
    )
    .await
    .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    let Some(emp_row) = emp else {
        return Ok(Vec::new());
    };
    let (sup, mgr, dept) = (
        value_i64(&emp_row[0]),
        value_i64(&emp_row[1]),
        value_i64(&emp_row[2]),
    );
    let mut chain = Vec::new();
    let mut order = 1;
    for s in &steps {
        let atype = value_to_string(&s[0]);
        let role_id = value_i64(&s[1]);
        let user_id = value_i64(&s[2]);
        let approver = match atype.as_str() {
            "supervisor" => match sup {
                Some(e) => user_of_employee_sea(db, e).await.ok().flatten(),
                None => None,
            },
            "manager" => match mgr {
                Some(e) => user_of_employee_sea(db, e).await.ok().flatten(),
                None => None,
            },
            "department_head" => {
                let head = match dept {
                    Some(d) => {
                        let r = q_one(
                            db,
                            "SELECT head_employee_id FROM departments WHERE id = ?1".to_string(),
                            vec![Value::Int(d)],
                            1,
                            "approval.head",
                        )
                        .await
                        .unwrap_or(None);
                        r.as_ref().and_then(|x| value_i64(&x[0]))
                    }
                    None => None,
                };
                match head {
                    Some(e) => user_of_employee_sea(db, e).await.ok().flatten(),
                    None => None,
                }
            }
            "role" => match role_id {
                Some(rid) => {
                    let r = q_one(
                        db,
                        "SELECT ur.user_id FROM user_roles ur INNER JOIN users u ON u.id = ur.user_id WHERE ur.role_id = ?1 AND u.status = 'active' ORDER BY ur.user_id ASC LIMIT 1".to_string(),
                        vec![Value::Int(rid)],
                        1,
                        "approval.role",
                    )
                    .await
                    .unwrap_or(None);
                    r.as_ref().and_then(|x| value_i64(&x[0]))
                }
                None => None,
            },
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
