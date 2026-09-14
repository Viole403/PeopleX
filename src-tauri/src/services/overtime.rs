//! Lembur: pengajuan lintas-midnight, rantai snapshot, putusan bertahap.

use chrono::{Local, NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};

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

fn map_row(
    id: i64,
    emp: i64,
    name: String,
    number: String,
    date: String,
    start: String,
    end: String,
    dur: i64,
    reason: Option<String>,
    status: String,
    step: i64,
    created: String,
) -> Result<Overtime, String> {
    Ok(Overtime {
        id: to_dto_int(id, "overtime.id")?,
        employee_id: to_dto_int(emp, "overtime.emp")?,
        employee_name: name,
        employee_number: number,
        date,
        start_time: start,
        end_time: end,
        duration_minutes: to_dto_int(dur, "overtime.dur")?,
        reason,
        status,
        current_step: to_dto_int(step, "overtime.step")?,
        created_at: created,
    })
}

pub fn my_requests(conn: &Connection, employee_id: i64) -> Result<Vec<Overtime>, String> {
    let mut stmt = conn
        .prepare("SELECT ot.id, ot.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, ot.date, ot.start_time, ot.end_time, ot.duration_minutes, ot.reason, ot.status, ot.current_step, ot.created_at FROM overtime_requests ot INNER JOIN employees e ON e.id = ot.employee_id WHERE ot.employee_id = ?1 ORDER BY ot.created_at DESC")
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    collect(conn, &mut stmt, Some(employee_id), true)
}

pub fn pending_for(conn: &Connection, user_id: i64) -> Result<Vec<Overtime>, String> {
    let mut stmt = conn
        .prepare("SELECT ot.id, ot.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, ot.date, ot.start_time, ot.end_time, ot.duration_minutes, ot.reason, ot.status, ot.current_step, ot.created_at FROM overtime_approvals oa INNER JOIN overtime_requests ot ON ot.id = oa.overtime_request_id INNER JOIN employees e ON e.id = ot.employee_id WHERE oa.approver_id = ?1 AND oa.status = 'pending' AND ot.status = 'pending' AND ot.current_step = oa.step_order ORDER BY ot.created_at ASC")
        .map_err(|e| format!("gagal menyiapkan antrean: {e}"))?;
    collect(conn, &mut stmt, Some(user_id), true)
}

pub fn all_requests(conn: &Connection) -> Result<Vec<Overtime>, String> {
    let mut stmt = conn
        .prepare("SELECT ot.id, ot.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, ot.date, ot.start_time, ot.end_time, ot.duration_minutes, ot.reason, ot.status, ot.current_step, ot.created_at FROM overtime_requests ot INNER JOIN employees e ON e.id = ot.employee_id ORDER BY CASE ot.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END, ot.created_at DESC")
        .map_err(|e| format!("gagal menyiapkan semua: {e}"))?;
    collect(conn, &mut stmt, None, false)
}

type OtRow = (
    i64,
    i64,
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<String>,
    String,
    i64,
    String,
);

fn map_ot_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<OtRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
    ))
}

fn collect(
    _conn: &Connection,
    stmt: &mut rusqlite::Statement,
    param: Option<i64>,
    has_param: bool,
) -> Result<Vec<Overtime>, String> {
    let rows = if has_param {
        stmt.query_map(params![param.unwrap_or(0)], map_ot_row)
            .map_err(|e| format!("gagal membaca daftar: {e}"))?
    } else {
        stmt.query_map([], map_ot_row)
            .map_err(|e| format!("gagal membaca daftar: {e}"))?
    };
    let mut out = Vec::new();
    for row in rows {
        let t = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(map_row(
            t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7, t.8, t.9, t.10, t.11,
        )?);
    }
    Ok(out)
}

fn parse_time(date: &str, time: &str) -> Result<NaiveDateTime, String> {
    let full = format!("{} {}", date.trim(), time.trim());
    NaiveDateTime::parse_from_str(&full, "%Y-%m-%d %H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(&full, "%Y-%m-%d %H:%M:%S"))
        .map_err(|_| "Jam harus format JJ:MM.".to_string())
}

pub fn create(
    conn: &Connection,
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
    conn.execute(
        "INSERT INTO overtime_requests (employee_id, date, start_time, end_time, duration_minutes, reason, rate_multiplier, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', 1)",
        params![
            employee_id,
            input.date.trim(),
            start.format(fmt).to_string(),
            end.format(fmt).to_string(),
            duration,
            input.reason.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            RATE,
        ],
    )
    .map_err(|e| format!("gagal mengajukan lembur: {e}"))?;
    let rid = conn.last_insert_rowid();
    let chain = approval::build_chain(conn, "overtime", employee_id)?;
    if chain.is_empty() {
        conn.execute(
            "UPDATE overtime_requests SET status = 'approved', current_step = 0 WHERE id = ?1",
            params![rid],
        )
        .map_err(|e| format!("gagal menyetujui langsung: {e}"))?;
    } else {
        for step in &chain {
            conn.execute(
                "INSERT INTO overtime_approvals (overtime_request_id, approver_id, step_order, step_role, status) VALUES (?1, ?2, ?3, ?4, 'pending')",
                params![rid, step.approver_id, step.order, step.role],
            )
            .map_err(|e| format!("gagal menyimpan rantai: {e}"))?;
        }
        let name: String = conn
            .query_row(
                "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1",
                params![employee_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        approval::notify(
            conn,
            chain[0].approver_id,
            "overtime_approval",
            "Pengajuan Lembur Baru",
            &format!("{name} mengajukan lembur."),
            "/leave",
        )?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "overtime",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "overtime.id")
}

pub fn decide(
    conn: &Connection,
    approver_user_id: i64,
    request_id: i64,
    decision: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let req: Option<(i64, String)> = conn
        .query_row(
            "SELECT current_step, status FROM overtime_requests WHERE id = ?1",
            params![request_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some((step, status)) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let approval: Option<(i64, i64, String)> = conn
        .query_row(
            "SELECT id, approver_id, step_role FROM overtime_approvals WHERE overtime_request_id = ?1 AND step_order = ?2 AND status = 'pending'",
            params![request_id, step],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat tahap: {e}"))?;
    let Some((approval_id, approver_id, step_role)) = approval else {
        return Err("Tahap persetujuan tidak ditemukan.".to_string());
    };
    if !approval::user_has_access(conn, approver_user_id, approver_id, &step_role)? {
        return Err("Tidak berwenang memutus pengajuan ini.".to_string());
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE overtime_approvals SET status = ?1, approver_id = ?2, notes = ?3, acted_at = ?4 WHERE id = ?5",
        params![decision, approver_user_id, notes.map(str::trim).filter(|s| !s.is_empty()), now, approval_id],
    )
    .map_err(|e| format!("gagal menyimpan putusan: {e}"))?;
    let emp_id: i64 = conn
        .query_row(
            "SELECT employee_id FROM overtime_requests WHERE id = ?1",
            params![request_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let emp_user = approval::user_of_employee(conn, emp_id).unwrap_or(None);
    if decision == "rejected" {
        conn.execute(
            "UPDATE overtime_requests SET status = 'rejected' WHERE id = ?1",
            params![request_id],
        )
        .map_err(|e| format!("gagal menolak: {e}"))?;
        if let Some(uid) = emp_user {
            approval::notify(
                conn,
                uid,
                "overtime_approval",
                "Lembur Ditolak",
                "Pengajuan lembur Anda ditolak.",
                "/leave",
            )?;
        }
    } else {
        let next: Option<(i64, i64)> = conn
            .query_row(
                "SELECT step_order, approver_id FROM overtime_approvals WHERE overtime_request_id = ?1 AND step_order > ?2 ORDER BY step_order LIMIT 1",
                params![request_id, step],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("gagal mencari tahap berikut: {e}"))?;
        match next {
            Some((next_order, next_approver)) => {
                conn.execute(
                    "UPDATE overtime_requests SET current_step = ?1 WHERE id = ?2",
                    params![next_order, request_id],
                )
                .map_err(|e| format!("gagal maju tahap: {e}"))?;
                approval::notify(
                    conn,
                    next_approver,
                    "overtime_approval",
                    "Lembur Menunggu Persetujuan",
                    "Ada pengajuan lembur menunggu persetujuan Anda.",
                    "/leave",
                )?;
            }
            None => {
                conn.execute(
                    "UPDATE overtime_requests SET status = 'approved' WHERE id = ?1",
                    params![request_id],
                )
                .map_err(|e| format!("gagal menyetujui: {e}"))?;
                if let Some(uid) = emp_user {
                    approval::notify(
                        conn,
                        uid,
                        "overtime_approval",
                        "Lembur Disetujui",
                        "Pengajuan lembur Anda disetujui.",
                        "/leave",
                    )?;
                }
            }
        }
    }
    audit::log(
        conn,
        Some(approver_user_id),
        decision.to_uppercase().as_str(),
        "overtime",
        Some(&request_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
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
    use crate::{db, seed};

    fn live() -> (tempfile::TempDir, crate::db::DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool)
    }

    fn mkuser(
        conn: &Connection,
        username: &str,
        employee_number: &str,
        supervisor: Option<i64>,
    ) -> (i64, i64) {
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, supervisor_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, ?2, '2026-01-01', 'active', 'permanent')",
            params![employee_number, supervisor],
        )
        .unwrap();
        let eid = conn.last_insert_rowid();
        let hash = bcrypt::hash("Rahasia123", 4).unwrap();
        conn.execute(
            "INSERT INTO users (employee_id, username, email, password, status, must_change_password) VALUES (?1, ?2, ?3, ?4, 'active', 0)",
            params![eid, username, format!("{username}@x.local"), hash],
        )
        .unwrap();
        (conn.last_insert_rowid(), eid)
    }

    #[test]
    fn lembur_midnight_dan_minimal_30() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let (uid, eid) = mkuser(&conn, "lembur1", "EMP-L1", None);
        // 22:00-01:00 = 180 menit, auto-approved (tanpa supervisor)
        let id = create(
            &conn,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-06".to_string(),
                start_time: "22:00".to_string(),
                end_time: "01:00".to_string(),
                reason: None,
            },
        )
        .expect("buat");
        let status: String = conn
            .query_row(
                "SELECT status FROM overtime_requests WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "approved");
        let (dur, mult): (i64, f64) = conn
            .query_row(
                "SELECT duration_minutes, rate_multiplier FROM overtime_requests WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(dur, 180);
        assert_eq!(mult, 1.5);
        // di bawah 30 menit ditolak
        let e = create(
            &conn,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-07".to_string(),
                start_time: "18:00".to_string(),
                end_time: "18:20".to_string(),
                reason: None,
            },
        )
        .expect_err("minimal");
        assert!(e.contains("30 menit"));
    }

    #[test]
    fn lembur_rantai_dua_tahap() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let (sup_uid, sup_eid) = mkuser(&conn, "spv1", "EMP-S1", None);
        // sup perlu peran agar rantai role terisi? supervisor step cukup user aktif
        let (mgr_uid, mgr_eid) = mkuser(&conn, "mgr1", "EMP-M1", None);
        let (uid, eid) = mkuser(&conn, "staff1", "EMP-T1", Some(sup_eid));
        conn.execute(
            "UPDATE employees SET manager_id = ?1 WHERE id = ?2",
            params![mgr_eid, eid],
        )
        .unwrap();
        let id = create(
            &conn,
            uid,
            eid,
            &OvertimeCreate {
                date: "2026-04-08".to_string(),
                start_time: "18:00".to_string(),
                end_time: "20:00".to_string(),
                reason: None,
            },
        )
        .expect("buat") as i64;
        // tahap 1 oleh supervisor
        decide(&conn, sup_uid, id, "approved", None).expect("tahap1");
        let (status, step): (String, i64) = conn
            .query_row(
                "SELECT status, current_step FROM overtime_requests WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "pending");
        assert_eq!(step, 2);
        // pihak tak berwenang ditolak
        let e = decide(&conn, uid, id, "approved", None).expect_err("otorisasi");
        assert!(e.contains("berwenang"));
        // tahap 2 oleh manajer -> approved
        decide(&conn, mgr_uid, id, "approved", None).expect("tahap2");
        let status: String = conn
            .query_row(
                "SELECT status FROM overtime_requests WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "approved");
        let _ = sup_uid;
    }

    #[tokio::test]
    async fn lembur_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let (uid, eid) = mkuser(&conn, "seaot", "EMP-SEAO", None);
        let oid = create_sea(
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
        assert!(oid > 0);
        let m_sync = serde_json::to_string(&my_requests(&conn, eid).expect("ms")).unwrap();
        let m_sea = serde_json::to_string(&my_requests_sea(db, eid).await.expect("mse")).unwrap();
        assert_eq!(m_sync, m_sea);
        assert!(m_sea.contains("\"duration_minutes\":180"));
        let a_sync = serde_json::to_string(&all_requests(&conn).expect("as")).unwrap();
        let a_sea = serde_json::to_string(&all_requests_sea(db).await.expect("ase")).unwrap();
        assert_eq!(a_sync, a_sea);
        let pendek = OvertimeCreate {
            date: "2026-04-07".to_string(),
            start_time: "18:00".to_string(),
            end_time: "18:20".to_string(),
            reason: None,
        };
        let e_sea = create_sea(db, uid, eid, &pendek).await.expect_err("minimal");
        let e_sync = create(&conn, uid, eid, &pendek).expect_err("minimal sync");
        assert_eq!(e_sea, e_sync);
    }
}
