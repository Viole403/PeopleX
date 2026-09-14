//! Lembur: pengajuan lintas-midnight, rantai snapshot, putusan bertahap.

use chrono::{Local, NaiveDate, NaiveDateTime};

use super::approval;
use super::audit;
use crate::to_dto_int;

const RATE: f64 = 1.5;
const MIN_MINUTES: i64 = 30;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Overtime {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: i32,
    pub reason: Option<String>,
    pub status: String,
    pub current_step: i32,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct OvertimeCreate {
    pub date: String,
    pub start_time: String,
    pub end_time: String,
    pub reason: Option<String>,
}

fn parse_time(date: &str, time: &str) -> Result<NaiveDateTime, String> {
    let full = format!("{} {}", date.trim(), time.trim());
    NaiveDateTime::parse_from_str(&full, "%Y-%m-%d %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(&full, "%Y-%m-%d %H:%M:%S"))
        .map_err(|_| "Jam harus format JJ:MM.".to_string())
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

async fn sea_rowid_ot(db: &sea_orm::DatabaseConnection) -> i64 {
    q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        "overtime.rowid",
    )
    .await
    .ok()
    .flatten()
    .as_ref()
    .and_then(|r| value_i64(&r[0]))
    .unwrap_or(0)
}

fn map_overtime_sea(r: &[Value]) -> Result<Overtime, String> {
    Ok(Overtime {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "overtime.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "overtime.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        date: value_to_string(&r[4]),
        start_time: value_to_string(&r[5]),
        end_time: value_to_string(&r[6]),
        duration_minutes: to_dto_int(value_i64(&r[7]).unwrap_or(0), "overtime.dur")?,
        reason: match &r[8] {
            Value::Null => None,
            _ => Some(value_to_string(&r[8])),
        },
        status: value_to_string(&r[9]),
        current_step: to_dto_int(value_i64(&r[10]).unwrap_or(0), "overtime.step")?,
        created_at: value_to_string(&r[11]),
    })
}

pub async fn my_requests_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<Overtime>, String> {
    let rows = q_all(
        db,
        "SELECT ot.id, ot.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, ot.date, ot.start_time, ot.end_time, ot.duration_minutes, ot.reason, ot.status, ot.current_step, ot.created_at FROM overtime_requests ot INNER JOIN employees e ON e.id = ot.employee_id WHERE ot.employee_id = ?1 ORDER BY ot.created_at DESC".to_string(),
        vec![Value::Int(employee_id)],
        12,
        "overtime.mine",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_overtime_sea(r)).collect()
}

pub async fn pending_for_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Vec<Overtime>, String> {
    let rows = q_all(
        db,
        "SELECT ot.id, ot.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, ot.date, ot.start_time, ot.end_time, ot.duration_minutes, ot.reason, ot.status, ot.current_step, ot.created_at FROM overtime_approvals oa INNER JOIN overtime_requests ot ON ot.id = oa.overtime_request_id INNER JOIN employees e ON e.id = ot.employee_id WHERE oa.approver_id = ?1 AND oa.status = 'pending' AND ot.status = 'pending' AND ot.current_step = oa.step_order ORDER BY ot.created_at ASC".to_string(),
        vec![Value::Int(user_id)],
        12,
        "overtime.pending",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_overtime_sea(r)).collect()
}

pub async fn all_requests_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Overtime>, String> {
    let rows = q_all(
        db,
        "SELECT ot.id, ot.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, ot.date, ot.start_time, ot.end_time, ot.duration_minutes, ot.reason, ot.status, ot.current_step, ot.created_at FROM overtime_requests ot INNER JOIN employees e ON e.id = ot.employee_id ORDER BY CASE ot.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END, ot.created_at DESC".to_string(),
        vec![],
        12,
        "overtime.all",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_overtime_sea(r)).collect()
}

pub async fn create_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &OvertimeCreate,
) -> Result<i32, String> {
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal tidak valid.".to_string())?;
    if let Some(reason) = input.reason.as_deref() {
        if reason.len() > 255 {
            return Err("Alasan maksimal 255 karakter.".to_string());
        }
    }
    let start = parse_time(&input.date, &input.start_time)?;
    let mut end = parse_time(&input.date, &input.end_time)?;
    if end <= start {
        end += chrono::Duration::days(1);
    }
    let duration = ((end - start).num_seconds() + 30) / 60;
    if duration < MIN_MINUTES {
        return Err("Durasi lembur minimal 30 menit.".to_string());
    }
    let fmt = "%Y-%m-%d %H:%M:%S";
    let reason = input
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    exec(
        db,
        "INSERT INTO overtime_requests (employee_id, date, start_time, end_time, duration_minutes, reason, rate_multiplier, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 1)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(input.date.trim().to_string()),
            Value::Text(start.format(fmt).to_string()),
            Value::Text(end.format(fmt).to_string()),
            Value::Int(duration),
            match reason {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Float(RATE),
        ],
        "overtime.create",
    )
    .await
    .map_err(|e| format!("gagal mengajukan lembur: {e}"))?;
    let rid = sea_rowid_ot(db).await;
    let chain = approval::build_chain_sea(db, "overtime", employee_id).await?;
    if chain.is_empty() {
        exec(
            db,
            "UPDATE overtime_requests SET status = 'approved', current_step = 0 WHERE id = ?1".to_string(),
            vec![Value::Int(rid)],
            "overtime.auto",
        )
        .await
        .map_err(|e| format!("gagal menyetujui langsung: {e}"))?;
    } else {
        for step in &chain {
            exec(
                db,
                "INSERT INTO overtime_approvals (overtime_request_id, approver_id, step_order, step_role, status) VALUES (?1, ?2, ?3, ?4, 'pending')".to_string(),
                vec![
                    Value::Int(rid),
                    Value::Int(step.approver_id),
                    Value::Int(step.order),
                    Value::Text(step.role.clone()),
                ],
                "overtime.chain",
            )
            .await
            .map_err(|e| format!("gagal menyimpan rantai: {e}"))?;
        }
        let name_row = q_one(
            db,
            "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
            vec![Value::Int(employee_id)],
            1,
            "overtime.empname",
        )
        .await
        .unwrap_or(None);
        let name = name_row
            .as_ref()
            .map(|r| value_to_string(&r[0]))
            .unwrap_or_default();
        approval::notify_sea(
            db,
            chain[0].approver_id,
            "overtime_approval",
            "Pengajuan Lembur Baru",
            &format!("{name} mengajukan lembur."),
            "/leave",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "overtime",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "overtime.id")
}

pub async fn decide_sea(
    db: &sea_orm::DatabaseConnection,
    approver_user_id: i64,
    request_id: i64,
    decision: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let req = q_one(
        db,
        "SELECT current_step, status FROM overtime_requests WHERE id = ?1".to_string(),
        vec![Value::Int(request_id)],
        2,
        "overtime.req",
    )
    .await
    .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some(req) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    let (step, status) = (
        value_i64(&req[0]).unwrap_or(0),
        value_to_string(&req[1]),
    );
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let ap = q_one(
        db,
        "SELECT id, approver_id, step_role FROM overtime_approvals WHERE overtime_request_id = ?1 AND step_order = ?2 AND status = 'pending'".to_string(),
        vec![Value::Int(request_id), Value::Int(step)],
        3,
        "overtime.step",
    )
    .await
    .map_err(|e| format!("gagal memuat tahap: {e}"))?;
    let Some(ap) = ap else {
        return Err("Tahap persetujuan tidak ditemukan.".to_string());
    };
    let (approval_id, approver_id, step_role) = (
        value_i64(&ap[0]).unwrap_or(0),
        value_i64(&ap[1]).unwrap_or(0),
        value_to_string(&ap[2]),
    );
    if !approval::user_has_access_sea(db, approver_user_id, approver_id, &step_role).await? {
        return Err("Tidak berwenang memutus pengajuan ini.".to_string());
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let note = notes.map(str::trim).filter(|s| !s.is_empty());
    exec(
        db,
        "UPDATE overtime_approvals SET status = ?1, approver_id = ?2, notes = ?3, acted_at = ?4 WHERE id = ?5".to_string(),
        vec![
            Value::Text(decision.to_string()),
            Value::Int(approver_user_id),
            match note {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Text(now),
            Value::Int(approval_id),
        ],
        "overtime.decide",
    )
    .await
    .map_err(|e| format!("gagal menyimpan putusan: {e}"))?;
    let emp_row = q_one(
        db,
        "SELECT employee_id FROM overtime_requests WHERE id = ?1".to_string(),
        vec![Value::Int(request_id)],
        1,
        "overtime.emp",
    )
    .await
    .unwrap_or(None);
    let emp_id = emp_row
        .as_ref()
        .and_then(|r| value_i64(&r[0]))
        .unwrap_or(0);
    let emp_user = approval::user_of_employee_sea(db, emp_id).await.unwrap_or(None);
    if decision == "rejected" {
        exec(
            db,
            "UPDATE overtime_requests SET status = 'rejected' WHERE id = ?1".to_string(),
            vec![Value::Int(request_id)],
            "overtime.reject",
        )
        .await
        .map_err(|e| format!("gagal menolak: {e}"))?;
        if let Some(uid) = emp_user {
            approval::notify_sea(
                db,
                uid,
                "overtime_approval",
                "Lembur Ditolak",
                "Pengajuan lembur Anda ditolak.",
                "/leave",
            )
            .await?;
        }
    } else {
        let next = q_one(
            db,
            "SELECT step_order, approver_id FROM overtime_approvals WHERE overtime_request_id = ?1 AND step_order > ?2 ORDER BY step_order LIMIT 1".to_string(),
            vec![Value::Int(request_id), Value::Int(step)],
            2,
            "overtime.next",
        )
        .await
        .map_err(|e| format!("gagal mencari tahap berikut: {e}"))?;
        match next {
            Some(n) => {
                let (next_order, next_approver) =
                    (value_i64(&n[0]).unwrap_or(0), value_i64(&n[1]).unwrap_or(0));
                exec(
                    db,
                    "UPDATE overtime_requests SET current_step = ?1 WHERE id = ?2".to_string(),
                    vec![Value::Int(next_order), Value::Int(request_id)],
                    "overtime.advance",
                )
                .await
                .map_err(|e| format!("gagal maju tahap: {e}"))?;
                approval::notify_sea(
                    db,
                    next_approver,
                    "overtime_approval",
                    "Lembur Menunggu Persetujuan",
                    "Ada pengajuan lembur menunggu persetujuan Anda.",
                    "/leave",
                )
                .await?;
            }
            None => {
                exec(
                    db,
                    "UPDATE overtime_requests SET status = 'approved' WHERE id = ?1".to_string(),
                    vec![Value::Int(request_id)],
                    "overtime.approve",
                )
                .await
                .map_err(|e| format!("gagal menyetujui: {e}"))?;
                if let Some(uid) = emp_user {
                    approval::notify_sea(
                        db,
                        uid,
                        "overtime_approval",
                        "Lembur Disetujui",
                        "Pengajuan lembur Anda disetujui.",
                        "/leave",
                    )
                    .await?;
                }
            }
        }
    }
    audit::log_sea(
        db,
        Some(approver_user_id),
        decision.to_uppercase().as_str(),
        "overtime",
        Some(&request_id.to_string()),
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

    async fn rowid(db: &sea_orm::DatabaseConnection) -> i64 {
        q_one(
            db,
            "SELECT last_insert_rowid()".to_string(),
            vec![],
            1,
            "test.rowid",
        )
        .await
        .expect("rowid")
        .and_then(|r| value_i64(&r[0]))
        .expect("id")
    }

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
        let eid = rowid(db).await;
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
        let uid = rowid(db).await;
        (uid, eid)
    }

    async fn text_of(db: &sea_orm::DatabaseConnection, sql: &str) -> String {
        q_one(
            db,
            sql.to_string(),
            vec![],
            1,
            "test.text",
        )
        .await
        .expect("one")
        .map(|r| value_to_string(&r[0]))
        .expect("val")
    }

    async fn int_of(db: &sea_orm::DatabaseConnection, sql: &str) -> i64 {
        q_one(db, sql.to_string(), vec![], 1, "test.int")
            .await
            .expect("one")
            .and_then(|r| value_i64(&r[0]))
            .expect("int")
    }

    async fn float_of(db: &sea_orm::DatabaseConnection, sql: &str) -> f64 {
        let r = q_one(db, sql.to_string(), vec![], 1, "test.float")
            .await
            .expect("one")
            .expect("row");
        match &r[0] {
            Value::Float(f) => *f,
            Value::Int(i) => *i as f64,
            other => panic!("bukan angka: {other:?}"),
        }
    }

    #[tokio::test]
    async fn lembur_sea_midnight_dan_minimal_30() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (uid, eid) = mkuser(db, "seaot1", "EMP-SEO1", None).await;
        // 22:00-01:00 = 180 menit, auto-approved (tanpa atasan -> rantai kosong)
        let id = create_sea(
            db,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-06".to_string(),
                start_time: "22:00".to_string(),
                end_time: "01:00".to_string(),
                reason: None,
            },
        )
        .await
        .expect("buat");
        assert!(id > 0);
        let id64 = id as i64;
        assert_eq!(
            text_of(db, &format!("SELECT status FROM overtime_requests WHERE id = {id64}")).await,
            "approved"
        );
        assert_eq!(
            int_of(db, &format!("SELECT duration_minutes FROM overtime_requests WHERE id = {id64}")).await,
            180
        );
        assert_eq!(
            float_of(db, &format!("SELECT rate_multiplier FROM overtime_requests WHERE id = {id64}")).await,
            1.5
        );
        let mine = my_requests_sea(db, eid).await.expect("mine");
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].duration_minutes, 180);
        assert_eq!(mine[0].status, "approved");
        assert_eq!(mine[0].current_step, 0);
        assert_eq!(mine[0].id, id);
        // di bawah 30 menit ditolak
        let e = create_sea(
            db,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-07".to_string(),
                start_time: "18:00".to_string(),
                end_time: "18:20".to_string(),
                reason: None,
            },
        )
        .await
        .expect_err("minimal");
        assert!(e.contains("30 menit"));
    }

    #[tokio::test]
    async fn lembur_sea_rantai_dua_tahap_dan_perhitungan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (sup_uid, sup_eid) = mkuser(db, "seasup", "EMP-SSUP", None).await;
        let (mgr_uid, mgr_eid) = mkuser(db, "seamgr", "EMP-SMGR", None).await;
        let (uid, eid) = mkuser(db, "seastaff", "EMP-SSTF", Some(sup_eid)).await;
        exec(
            db,
            "UPDATE employees SET manager_id = ?1 WHERE id = ?2".to_string(),
            vec![Value::Int(mgr_eid), Value::Int(eid)],
            "test.mgrid",
        )
        .await
        .expect("manager");
        // 18:00-20:00 = 120 menit
        let id = create_sea(
            db,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-08".to_string(),
                start_time: "18:00".to_string(),
                end_time: "20:00".to_string(),
                reason: Some("Target mepet".to_string()),
            },
        )
        .await
        .expect("buat") as i64;
        assert_eq!(
            text_of(db, &format!("SELECT status FROM overtime_requests WHERE id = {id}")).await,
            "pending"
        );
        assert_eq!(
            int_of(db, &format!("SELECT current_step FROM overtime_requests WHERE id = {id}")).await,
            1
        );
        // antrean tahap 1 ada di supervisor
        let q1 = pending_for_sea(db, sup_uid).await.expect("antrean1");
        assert_eq!(q1.len(), 1);
        assert_eq!(q1[0].duration_minutes, 120);
        assert_eq!(q1[0].reason.as_deref(), Some("Target mepet"));
        assert_eq!(q1[0].current_step, 1);
        // tahap 1 oleh supervisor -> maju ke tahap 2
        decide_sea(db, sup_uid, id, "approved", None)
            .await
            .expect("tahap1");
        assert_eq!(
            text_of(db, &format!("SELECT status FROM overtime_requests WHERE id = {id}")).await,
            "pending"
        );
        assert_eq!(
            int_of(db, &format!("SELECT current_step FROM overtime_requests WHERE id = {id}")).await,
            2
        );
        assert!(pending_for_sea(db, sup_uid).await.expect("q1b").is_empty());
        assert_eq!(pending_for_sea(db, mgr_uid).await.expect("q2").len(), 1);
        // pihak tak berwenang ditolak
        let e = decide_sea(db, uid, id, "approved", None)
            .await
            .expect_err("otorisasi");
        assert!(e.contains("berwenang"));
        // keputusan tidak valid
        let e = decide_sea(db, mgr_uid, id, "mungkin", None)
            .await
            .expect_err("validasi");
        assert!(e.contains("Keputusan tidak valid"));
        // tahap 2 oleh manajer -> approved; durasi dan rate tetap terhitung
        decide_sea(db, mgr_uid, id, "approved", Some("ok"))
            .await
            .expect("tahap2");
        assert_eq!(
            text_of(db, &format!("SELECT status FROM overtime_requests WHERE id = {id}")).await,
            "approved"
        );
        assert_eq!(
            int_of(db, &format!("SELECT duration_minutes FROM overtime_requests WHERE id = {id}")).await,
            120
        );
        assert_eq!(
            float_of(db, &format!("SELECT rate_multiplier FROM overtime_requests WHERE id = {id}")).await,
            1.5
        );
        // catatan reviewer tersimpan
        assert_eq!(
            text_of(
                db,
                &format!("SELECT notes FROM overtime_approvals WHERE overtime_request_id = {id} AND step_order = 2")
            )
            .await,
            "ok"
        );
        // pengajuan kedua untuk alur penolakan
        let id2 = create_sea(
            db,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-09".to_string(),
                start_time: "20:00".to_string(),
                end_time: "23:30".to_string(),
                reason: None,
            },
        )
        .await
        .expect("buat2") as i64;
        // 20:00-23:30 = 210 menit
        assert_eq!(
            int_of(db, &format!("SELECT duration_minutes FROM overtime_requests WHERE id = {id2}")).await,
            210
        );
        decide_sea(db, sup_uid, id2, "rejected", None)
            .await
            .expect("tolak");
        assert_eq!(
            text_of(db, &format!("SELECT status FROM overtime_requests WHERE id = {id2}")).await,
            "rejected"
        );
        // putusan ulang atas yang sudah diproses ditolak
        let e = decide_sea(db, mgr_uid, id2, "approved", None)
            .await
            .expect_err("proses ulang");
        assert!(e.contains("sudah diproses"));
        // daftar semua: pending -> approved -> rejected; saat ini tidak ada pending
        let all = all_requests_sea(db).await.expect("all");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id as i64, id);
        assert_eq!(all[0].status, "approved");
        assert_eq!(all[1].id as i64, id2);
        assert_eq!(all[1].status, "rejected");
        let mine = my_requests_sea(db, eid).await.expect("mine");
        assert_eq!(mine.len(), 2);
    }
}
