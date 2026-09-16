//! Izin non-cuti: satu tahap oleh atasan.

use chrono::{Local, NaiveDate};

use super::approval;
use super::audit;
use crate::to_dto_int;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PermissionType {
    pub id: i32,
    pub code: String,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PermissionRequest {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub permission_type_id: i32,
    pub permission_type_name: String,
    pub date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub reason: String,
    pub status: String,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct PermissionCreate {
    pub permission_type_id: i32,
    pub date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub reason: String,
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};

const SELECT_SEA: &str = "SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pr.permission_type_id, pt.name, pr.date, pr.start_time, pr.end_time, pr.reason, pr.status, pr.created_at FROM permission_requests pr INNER JOIN permission_types pt ON pt.id = pr.permission_type_id INNER JOIN employees e ON e.id = pr.employee_id";

fn map_perm_sea(r: &[Value]) -> Result<PermissionRequest, String> {
    Ok(PermissionRequest {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "perm.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "perm.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        permission_type_id: to_dto_int(value_i64(&r[4]).unwrap_or(0), "perm.type")?,
        permission_type_name: value_to_string(&r[5]),
        date: value_to_string(&r[6]),
        start_time: match &r[7] {
            Value::Null => None,
            _ => Some(value_to_string(&r[7])),
        },
        end_time: match &r[8] {
            Value::Null => None,
            _ => Some(value_to_string(&r[8])),
        },
        reason: value_to_string(&r[9]),
        status: value_to_string(&r[10]),
        created_at: value_to_string(&r[11]),
    })
}

pub async fn types_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<PermissionType>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name FROM permission_types ORDER BY name".to_string(),
        vec![],
        3,
        "perm.types",
    )
    .await
    .map_err(|e| format!("gagal membaca tipe: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(PermissionType {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "permtype.id")?,
            code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
        });
    }
    Ok(out)
}

pub async fn my_requests_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<PermissionRequest>, String> {
    let rows = q_all(
        db,
        format!("{SELECT_SEA} WHERE pr.employee_id = ?1 ORDER BY pr.created_at DESC"),
        vec![Value::Int(employee_id)],
        12,
        "perm.mine",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_perm_sea(r)).collect()
}

/// Menunggu putusan user: ia supervisor langsung ATAU pemegang izin.
pub async fn pending_for_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    privileged: bool,
) -> Result<Vec<PermissionRequest>, String> {
    let emp = approval::employee_of_user_sea(db, user_id).await?.unwrap_or(0);
    let rows = q_all(
        db,
        format!("{SELECT_SEA} WHERE pr.status = 'pending' AND e.supervisor_id = ?1 ORDER BY pr.created_at ASC"),
        vec![Value::Int(emp)],
        12,
        "perm.pending",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out: Vec<PermissionRequest> =
        rows.iter().map(|r| map_perm_sea(r)).collect::<Result<_, _>>()?;
    if privileged {
        let rows2 = q_all(
            db,
            format!("{SELECT_SEA} WHERE pr.status = 'pending' AND (e.supervisor_id IS NULL OR e.supervisor_id != ?1) ORDER BY pr.created_at ASC"),
            vec![Value::Int(emp)],
            12,
            "perm.pendinghr",
        )
        .await
        .map_err(|e| format!("gagal membaca daftar: {e}"))?;
        let mut extra: Vec<PermissionRequest> =
            rows2.iter().map(|r| map_perm_sea(r)).collect::<Result<_, _>>()?;
        out.append(&mut extra);
    }
    Ok(out)
}

pub async fn all_requests_sea(
    db: &sea_orm::DatabaseConnection,
    department_id: Option<i64>,
) -> Result<Vec<PermissionRequest>, String> {
    let mut sql = format!("{SELECT_SEA} WHERE 1 = 1");
    let mut vals = Vec::new();
    if let Some(did) = department_id {
        sql.push_str(" AND e.department_id = ?1");
        vals.push(Value::Int(did));
    }
    sql.push_str(" ORDER BY CASE pr.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END, pr.created_at DESC");
    let rows = q_all(db, sql, vals, 12, "perm.all")
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_perm_sea(r)).collect()
}

pub async fn create_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &PermissionCreate,
) -> Result<i32, String> {
    let type_row = q_one(
        db,
        "SELECT id FROM permission_types WHERE id = ?1".to_string(),
        vec![Value::Int(input.permission_type_id as i64)],
        1,
        "perm.typecheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa tipe: {e}"))?;
    if type_row.is_none() {
        return Err("Tipe izin tidak ditemukan.".to_string());
    }
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal tidak valid.".to_string())?;
    if input.reason.trim().is_empty() {
        return Err("Alasan wajib diisi.".to_string());
    }
    if input.reason.len() > 255 {
        return Err("Alasan maksimal 255 karakter.".to_string());
    }
    let start = input
        .start_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let end = input
        .end_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rid = exec_insert(
        db,
        "INSERT INTO permission_requests (employee_id, permission_type_id, date, start_time, end_time, reason, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(input.permission_type_id as i64),
            Value::Text(input.date.trim().to_string()),
            match start {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            match end {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Text(input.reason.trim().to_string()),
        ],
        "perm.create",
    )
    .await
    .map_err(|e| format!("gagal mengajukan izin: {e}"))?;
    let sup_row = q_one(
        db,
        "SELECT supervisor_id FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "perm.sup",
    )
    .await
    .map_err(|e| format!("gagal memuat supervisor: {e}"))?;
    let sup = sup_row.as_ref().and_then(|r| value_i64(&r[0]));
    if let Some(sid) = sup {
        if let Some(uid) = approval::user_of_employee_sea(db, sid).await? {
            let name_row = q_one(
                db,
                "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
                vec![Value::Int(employee_id)],
                1,
                "perm.empname",
            )
            .await
            .unwrap_or(None);
            let name = name_row
                .as_ref()
                .map(|r| value_to_string(&r[0]))
                .unwrap_or_default();
            approval::notify_sea(
                db,
                uid,
                "permission_request",
                "Pengajuan Izin Baru",
                &format!("{name} mengajukan izin."),
                "/leave",
            )
            .await?;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "permission_request",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "perm.id")
}

pub async fn decide_sea(
    db: &sea_orm::DatabaseConnection,
    approver_user_id: i64,
    approver_employee_id: Option<i64>,
    privileged: bool,
    id: i64,
    decision: &str,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let req = q_one(
        db,
        "SELECT e.supervisor_id, pr.status FROM permission_requests pr INNER JOIN employees e ON e.id = pr.employee_id WHERE pr.id = ?1".to_string(),
        vec![Value::Int(id)],
        2,
        "perm.req",
    )
    .await
    .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some(req) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    let (supervisor, status) = (value_i64(&req[0]), value_to_string(&req[1]));
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let is_supervisor = approver_employee_id == supervisor;
    if !is_supervisor && !privileged {
        return Err("Hanya supervisor atau HR yang boleh memutuskan.".to_string());
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    exec(
        db,
        "UPDATE permission_requests SET status = ?1, approved_by = ?2, approved_at = ?3 WHERE id = ?4".to_string(),
        vec![
            Value::Text(decision.to_string()),
            Value::Int(approver_user_id),
            Value::Text(now),
            Value::Int(id),
        ],
        "perm.decide",
    )
    .await
    .map_err(|e| format!("gagal menyimpan putusan: {e}"))?;
    let emp_row = q_one(
        db,
        "SELECT employee_id FROM permission_requests WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "perm.emp",
    )
    .await
    .unwrap_or(None);
    let emp_id = emp_row
        .as_ref()
        .and_then(|r| value_i64(&r[0]))
        .unwrap_or(0);
    if let Some(uid) = approval::user_of_employee_sea(db, emp_id).await? {
        approval::notify_sea(
            db,
            uid,
            "permission_request",
            if decision == "approved" {
                "Izin Disetujui"
            } else {
                "Izin Ditolak"
            },
            &format!("Pengajuan izin Anda {decision}."),
            "/leave",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(approver_user_id),
        decision.to_uppercase().as_str(),
        "permission_request",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sea_raw::{exec, q_one, value_i64, Value};

    async fn mkuser(
        db: &sea_orm::DatabaseConnection,
        username: &str,
        number: &str,
        supervisor: Option<i64>,
    ) -> (i64, i64) {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, supervisor_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, ?2, '2026-01-01', 'active', 'permanent')".to_string(),
            vec![
                Value::Text(number.to_string()),
                match supervisor {
                    Some(s) => Value::Int(s),
                    None => Value::Null,
                },
            ],
            "test.mkemp",
        )
        .await
        .expect("emp");
        let eid = q_one(db, "SELECT last_insert_rowid()".to_string(), vec![], 1, "test.rowid")
            .await
            .expect("rowid")
            .and_then(|r| value_i64(&r[0]))
            .expect("eid");
        let hash = bcrypt::hash("Rahasia123", 4).expect("hash");
        exec(
            db,
            "INSERT INTO users (employee_id, username, email, password, status, must_change_password) VALUES (?1, ?2, ?3, ?4, 'active', 0)".to_string(),
            vec![
                Value::Int(eid),
                Value::Text(username.to_string()),
                Value::Text(format!("{username}@x.local")),
                Value::Text(hash),
            ],
            "test.mkuser",
        )
        .await
        .expect("user");
        let uid = q_one(db, "SELECT last_insert_rowid()".to_string(), vec![], 1, "test.rowid")
            .await
            .expect("rowid")
            .and_then(|r| value_i64(&r[0]))
            .expect("uid");
        (uid, eid)
    }

    async fn type_id(db: &sea_orm::DatabaseConnection, code: &str) -> i64 {
        q_one(
            db,
            "SELECT id FROM permission_types WHERE code = ?1".to_string(),
            vec![Value::Text(code.to_string())],
            1,
            "test.typeid",
        )
        .await
        .expect("typeid")
        .and_then(|r| value_i64(&r[0]))
        .expect("tid")
    }

    async fn status_of(db: &sea_orm::DatabaseConnection, pid: i64) -> String {
        q_one(
            db,
            "SELECT status FROM permission_requests WHERE id = ?1".to_string(),
            vec![Value::Int(pid)],
            1,
            "test.status",
        )
        .await
        .expect("status")
        .map(|r| value_to_string(&r[0]))
        .expect("val")
    }

    #[tokio::test]
    async fn izin_sea_satu_tahap_oleh_supervisor() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (sup_uid, sup_eid) = mkuser(db, "spv2", "EMP-P1", None).await;
        let (uid, eid) = mkuser(db, "staff2", "EMP-P2", Some(sup_eid)).await;
        assert!(!types_sea(db).await.expect("tipe").is_empty());
        let tid = type_id(db, "PERSONAL").await;
        let pid = create_sea(
            db,
            uid,
            eid,
            &PermissionCreate {
                permission_type_id: tid as i32,
                date: "2026-05-04".to_string(),
                start_time: Some("10:00".to_string()),
                end_time: Some("12:00".to_string()),
                reason: "Keperluan keluarga".to_string(),
            },
        )
        .await
        .expect("buat") as i64;
        assert_eq!(my_requests_sea(db, eid).await.expect("mine").len(), 1);
        assert_eq!(
            pending_for_sea(db, sup_uid, false)
                .await
                .expect("antrean")
                .len(),
            1
        );
        // rekan kerja ditolak
        let (other_uid, _) = mkuser(db, "lain2", "EMP-P3", None).await;
        let other_emp = approval::employee_of_user_sea(db, other_uid)
            .await
            .expect("emp")
            .expect("ada");
        let e = decide_sea(db, other_uid, Some(other_emp), false, pid, "approved")
            .await
            .expect_err("otorisasi");
        assert!(e.contains("supervisor"));
        // supervisor menyetujui langsung final
        decide_sea(db, sup_uid, Some(sup_eid), false, pid, "approved")
            .await
            .expect("setuju");
        assert_eq!(status_of(db, pid).await, "approved");
        // sudah diproses: putusan kedua ditolak
        assert!(decide_sea(db, sup_uid, Some(sup_eid), false, pid, "approved")
            .await
            .is_err());
        assert_eq!(all_requests_sea(db, None).await.expect("all").len(), 1);
        let _ = (sup_uid, uid);
    }

    #[tokio::test]
    async fn izin_sea_tanpa_supervisor_masuk_antrean_hr() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (uid, eid) = mkuser(db, "solo2", "EMP-P4", None).await;
        let tid = type_id(db, "SICK").await;
        create_sea(
            db,
            uid,
            eid,
            &PermissionCreate {
                permission_type_id: tid as i32,
                date: "2026-05-05".to_string(),
                start_time: None,
                end_time: None,
                reason: "Sakit".to_string(),
            },
        )
        .await
        .expect("buat");
        // tanpa supervisor: antrean supervisor kosong, tapi HR privileged melihatnya
        assert!(pending_for_sea(db, uid, false).await.expect("a").is_empty());
        let admin = q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "test.admin",
        )
        .await
        .expect("admin")
        .and_then(|r| value_i64(&r[0]))
        .expect("aid");
        assert_eq!(pending_for_sea(db, admin, true).await.expect("b").len(), 1);
    }

    #[tokio::test]
    async fn izin_sea_validasi_ditolak() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (uid, eid) = mkuser(db, "val1", "EMP-P5", None).await;
        let tid = type_id(db, "PERSONAL").await;
        let base = || PermissionCreate {
            permission_type_id: tid as i32,
            date: "2026-05-06".to_string(),
            start_time: None,
            end_time: None,
            reason: "Perlu".to_string(),
        };
        // tipe tidak dikenal
        assert!(create_sea(
            db,
            uid,
            eid,
            &PermissionCreate {
                permission_type_id: 99999,
                ..base()
            },
        )
        .await
        .is_err());
        // tanggal buruk
        assert!(create_sea(
            db,
            uid,
            eid,
            &PermissionCreate {
                date: "bukan-tanggal".to_string(),
                ..base()
            },
        )
        .await
        .is_err());
        // alasan kosong
        assert!(create_sea(
            db,
            uid,
            eid,
            &PermissionCreate {
                reason: "   ".to_string(),
                ..base()
            },
        )
        .await
        .is_err());
        // alasan > 255 karakter
        assert!(create_sea(
            db,
            uid,
            eid,
            &PermissionCreate {
                reason: "x".repeat(256),
                ..base()
            },
        )
        .await
        .is_err());
        let pid = create_sea(db, uid, eid, &base()).await.expect("buat") as i64;
        // keputusan tidak valid
        assert!(decide_sea(db, uid, Some(eid), true, pid, "maybe").await.is_err());
        // id tidak dikenal
        assert!(decide_sea(db, uid, Some(eid), true, 99999, "approved")
            .await
            .is_err());
        // penolakan valid oleh HR mengubah status
        decide_sea(db, uid, Some(eid), true, pid, "rejected")
            .await
            .expect("tolak");
        assert_eq!(status_of(db, pid).await, "rejected");
    }
}
