//! Cuti: saldo, pengajuan snapshot, putusan bertahap, pembatalan, kalender.

use chrono::{Datelike, Local, NaiveDate};

use super::approval;
use super::audit;
use crate::to_dto_int;

// ---------------- DTO ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Balance {
    pub leave_type_id: i32,
    pub code: String,
    pub name: String,
    pub is_paid: bool,
    pub allocated_days: f64,
    pub used_days: f64,
    pub carried_days: f64,
    pub adjustment_days: f64,
    pub remaining: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct LeaveType {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub default_days_per_year: f64,
    pub is_paid: bool,
    pub carry_forward: bool,
    pub carry_forward_max_days: f64,
    pub requires_attachment: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct LeaveTypeInput {
    pub code: String,
    pub name: String,
    pub default_days_per_year: f64,
    pub is_paid: bool,
    pub carry_forward: bool,
    pub carry_forward_max_days: f64,
    pub requires_attachment: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct LeaveRequest {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub leave_type_id: i32,
    pub leave_type_name: String,
    pub start_date: String,
    pub end_date: String,
    pub total_days: f64,
    pub reason: Option<String>,
    pub status: String,
    pub current_step: i32,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct LeaveCreate {
    pub leave_type_id: i32,
    pub start_date: String,
    pub end_date: String,
    pub reason: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CalendarDay {
    pub date: String,
    pub label: String,
    pub kind: String,
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};

fn sea_f64(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        Value::Text(s) => s.parse().unwrap_or(0.0),
        Value::Null => 0.0,
    }
}

fn sea_opt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

pub async fn balances_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    year: i32,
) -> Result<Vec<Balance>, String> {
    let rows = q_all(
        db,
        "SELECT lt.id, lt.code, lt.name, lt.is_paid,
                       COALESCE(lb.allocated_days, lt.default_days_per_year),
                       COALESCE(lb.used_days, 0), COALESCE(lb.carried_days, 0), COALESCE(lb.adjustment_days, 0)
                FROM leave_types lt
                LEFT JOIN leave_balances lb ON lb.leave_type_id = lt.id AND lb.employee_id = ?1 AND lb.year = ?2
                WHERE lt.deleted_at IS NULL ORDER BY lt.name"
            .to_string(),
        vec![Value::Int(employee_id), Value::Int(year as i64)],
        8,
        "leave.balances",
    )
    .await
    .map_err(|e| format!("gagal membaca saldo: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let (id, paid) = (value_i64(&r[0]).unwrap_or(0), value_i64(&r[3]).unwrap_or(0));
        let (alloc, used, carried, adj) = (sea_f64(&r[4]), sea_f64(&r[5]), sea_f64(&r[6]), sea_f64(&r[7]));
        out.push(Balance {
            leave_type_id: to_dto_int(id, "balance.type")?,
            code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
            is_paid: paid != 0,
            allocated_days: alloc,
            used_days: used,
            carried_days: carried,
            adjustment_days: adj,
            remaining: alloc + carried + adj - used,
        });
    }
    Ok(out)
}

async fn ensure_balance_row_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    leave_type_id: i64,
    year: i32,
) -> Result<i64, String> {
    let row = q_one(
        db,
        "SELECT id FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(leave_type_id),
            Value::Int(year as i64),
        ],
        1,
        "leave.balcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa saldo: {e}"))?;
    if let Some(r) = row {
        return Ok(value_i64(&r[0]).unwrap_or(0));
    }
    let def_row = q_one(
        db,
        "SELECT default_days_per_year FROM leave_types WHERE id = ?1".to_string(),
        vec![Value::Int(leave_type_id)],
        1,
        "leave.typedef",
    )
    .await
    .map_err(|e| format!("gagal memuat tipe cuti: {e}"))?;
    let Some(def_row) = def_row else {
        return Err("Tipe cuti tidak ditemukan.".to_string());
    };
    let def = sea_f64(&def_row[0]);
    exec_insert(
        db,
        "INSERT INTO leave_balances (employee_id, leave_type_id, year, allocated_days, used_days, carried_days, adjustment_days) VALUES (?1, ?2, ?3, ?4, 0, 0, 0)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(leave_type_id),
            Value::Int(year as i64),
            Value::Float(def),
        ],
        "leave.balcreate",
    )
    .await
    .map_err(|e| format!("gagal membuat saldo: {e}"))
}

async fn apply_usage_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    leave_type_id: i64,
    year: i32,
    days: f64,
) -> Result<(), String> {
    let id = ensure_balance_row_sea(db, employee_id, leave_type_id, year).await?;
    exec(
        db,
        "UPDATE leave_balances SET used_days = used_days + ?1 WHERE id = ?2".to_string(),
        vec![Value::Float(days), Value::Int(id)],
        "leave.use",
    )
    .await
    .map_err(|e| format!("gagal memakai saldo: {e}"))?;
    Ok(())
}

async fn reverse_usage_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    leave_type_id: i64,
    year: i32,
    days: f64,
) -> Result<(), String> {
    let id = ensure_balance_row_sea(db, employee_id, leave_type_id, year).await?;
    exec(
        db,
        "UPDATE leave_balances SET used_days = MAX(0, used_days - ?1) WHERE id = ?2".to_string(),
        vec![Value::Float(days), Value::Int(id)],
        "leave.unuse",
    )
    .await
    .map_err(|e| format!("gagal mengembalikan saldo: {e}"))?;
    Ok(())
}

/// Hitung hari kerja Senin-Jumat minus libur.
pub async fn count_business_days_sea(
    db: &sea_orm::DatabaseConnection,
    start: &str,
    end: &str,
) -> Result<i64, String> {
    let mut day = NaiveDate::parse_from_str(start, "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    let last = NaiveDate::parse_from_str(end, "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if last < day {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    let rows = q_all(
        db,
        "SELECT date FROM holidays WHERE date BETWEEN ?1 AND ?2".to_string(),
        vec![
            Value::Text(start.to_string()),
            Value::Text(end.to_string()),
        ],
        1,
        "leave.hols",
    )
    .await
    .map_err(|e| format!("gagal membaca libur: {e}"))?;
    let hols: Vec<String> = rows.iter().map(|r| value_to_string(&r[0])).collect();
    let mut count = 0;
    while day <= last {
        let dow = day.weekday().num_days_from_sunday();
        let s = day.format("%Y-%m-%d").to_string();
        if dow != 0 && dow != 6 && !hols.contains(&s) {
            count += 1;
        }
        day += chrono::Duration::days(1);
    }
    Ok(count)
}

fn map_request_sea(r: &[Value]) -> Result<LeaveRequest, String> {
    Ok(LeaveRequest {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "leave.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "leave.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        leave_type_id: to_dto_int(value_i64(&r[4]).unwrap_or(0), "leave.type")?,
        leave_type_name: value_to_string(&r[5]),
        start_date: value_to_string(&r[6]),
        end_date: value_to_string(&r[7]),
        total_days: sea_f64(&r[8]),
        reason: sea_opt_text(&r[9]),
        status: value_to_string(&r[10]),
        current_step: to_dto_int(value_i64(&r[11]).unwrap_or(0), "leave.step")?,
        created_at: value_to_string(&r[12]),
    })
}

pub async fn my_requests_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<LeaveRequest>, String> {
    let rows = q_all(
        db,
        "SELECT lr.id, lr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, lr.leave_type_id, lt.name, lr.start_date, lr.end_date, lr.total_days, lr.reason, lr.status, lr.current_step, lr.created_at
                FROM leave_requests lr INNER JOIN leave_types lt ON lt.id = lr.leave_type_id
                INNER JOIN employees e ON e.id = lr.employee_id
                WHERE lr.employee_id = ?1 ORDER BY lr.created_at DESC"
            .to_string(),
        vec![Value::Int(employee_id)],
        13,
        "leave.mine",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_request_sea(r)).collect()
}

pub async fn pending_for_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Vec<LeaveRequest>, String> {
    let rows = q_all(
        db,
        "SELECT lr.id, lr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, lr.leave_type_id, lt.name, lr.start_date, lr.end_date, lr.total_days, lr.reason, lr.status, lr.current_step, lr.created_at
                FROM leave_approvals la
                INNER JOIN leave_requests lr ON lr.id = la.leave_request_id
                INNER JOIN leave_types lt ON lt.id = lr.leave_type_id
                INNER JOIN employees e ON e.id = lr.employee_id
                WHERE la.approver_id = ?1 AND la.status = 'pending' AND lr.status = 'pending' AND lr.current_step = la.step_order
                ORDER BY lr.created_at ASC"
            .to_string(),
        vec![Value::Int(user_id)],
        13,
        "leave.pending",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_request_sea(r)).collect()
}

pub async fn all_requests_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<LeaveRequest>, String> {
    let rows = q_all(
        db,
        "SELECT lr.id, lr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, lr.leave_type_id, lt.name, lr.start_date, lr.end_date, lr.total_days, lr.reason, lr.status, lr.current_step, lr.created_at
                FROM leave_requests lr
                INNER JOIN leave_types lt ON lt.id = lr.leave_type_id
                INNER JOIN employees e ON e.id = lr.employee_id
                ORDER BY CASE lr.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 WHEN 'rejected' THEN 2 ELSE 3 END, lr.created_at DESC"
            .to_string(),
        vec![],
        13,
        "leave.all",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_request_sea(r)).collect()
}

async fn leave_chain_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    type_id: i64,
) -> Result<Vec<approval::ChainStep>, String> {
    let row = q_one(
        db,
        "SELECT COALESCE(department_id, 0) FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "leave.dept",
    )
    .await
    .map_err(|e| format!("gagal membaca departemen: {e}"))?;
    let dept = row.and_then(|r| value_i64(&r[0])).unwrap_or(0);
    let scopes = if dept == 0 { vec![0] } else { vec![dept, 0] };
    for scope in scopes {
        let rows = q_all(
            db,
            "SELECT step_order, approver_user_id FROM leave_type_levels WHERE leave_type_id = ?1 AND department_id = ?2 ORDER BY step_order".to_string(),
            vec![Value::Int(type_id), Value::Int(scope)],
            2,
            "leave.levels",
        )
        .await
        .map_err(|e| format!("gagal membaca jenjang: {e}"))?;
        if !rows.is_empty() {
            return Ok(rows
                .iter()
                .map(|r| approval::ChainStep {
                    order: value_i64(&r[0]).unwrap_or(0),
                    approver_id: value_i64(&r[1]).unwrap_or(0),
                    role: "specific_user".to_string(),
                })
                .collect());
        }
    }
    Ok(vec![])
}

pub async fn create_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &LeaveCreate,
) -> Result<i32, String> {
    let ltype = q_one(
        db,
        "SELECT id, name, is_paid FROM leave_types WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(input.leave_type_id as i64)],
        3,
        "leave.type",
    )
    .await
    .map_err(|e| format!("gagal memuat tipe cuti: {e}"))?;
    let Some(ltype) = ltype else {
        return Err("Tipe cuti tidak ditemukan.".to_string());
    };
    let (tid, tname, is_paid) = (
        value_i64(&ltype[0]).unwrap_or(0),
        value_to_string(&ltype[1]),
        value_i64(&ltype[2]).unwrap_or(0),
    );
    let start = input.start_date.trim();
    let end = input.end_date.trim();
    NaiveDate::parse_from_str(start, "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    NaiveDate::parse_from_str(end, "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if end < start {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    if let Some(reason) = input.reason.as_deref() {
        if reason.len() > 255 {
            return Err("Alasan maksimal 255 karakter.".to_string());
        }
    }
    let total = count_business_days_sea(db, start, end).await?;
    if total <= 0 {
        return Err("Rentang tanggal tidak memiliki hari kerja.".to_string());
    }
    let year: i32 = start[..4]
        .parse()
        .map_err(|_| "Tahun tidak valid.".to_string())?;
    if is_paid != 0 {
        let bals = balances_sea(db, employee_id, year).await?;
        let remaining = bals
            .iter()
            .find(|b| b.leave_type_id as i64 == tid)
            .map(|b| b.remaining)
            .unwrap_or(0.0);
        if total as f64 > remaining {
            return Err(format!(
                "Saldo cuti tidak mencukupi. Sisa: {remaining} hari."
            ));
        }
    }
    let overlap = q_one(
        db,
        "SELECT id FROM leave_requests WHERE employee_id = ?1 AND status IN ('pending','approved') AND NOT (end_date < ?2 OR start_date > ?3)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(start.to_string()),
            Value::Text(end.to_string()),
        ],
        1,
        "leave.overlap",
    )
    .await
    .map_err(|e| format!("gagal memeriksa tumpang tindih: {e}"))?;
    if overlap.is_some() {
        return Err("Sudah ada pengajuan cuti pada rentang tanggal tersebut.".to_string());
    }
    let reason = input
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let rid = exec_insert(
        db,
        "INSERT INTO leave_requests (employee_id, leave_type_id, start_date, end_date, total_days, reason, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 1)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(tid),
            Value::Text(start.to_string()),
            Value::Text(end.to_string()),
            Value::Float(total as f64),
            match reason {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
        ],
        "leave.create",
    )
    .await
    .map_err(|e| format!("gagal mengajukan cuti: {e}"))?;
    let mut chain = leave_chain_sea(db, employee_id, tid).await?;
    if chain.is_empty() {
        chain = approval::build_chain_sea(db, "leave", employee_id).await?;
    }
    let hari_ini = Local::now().date_naive().format("%Y-%m-%d").to_string();
    for step in chain.iter_mut() {
        if let Some(d) =
            approval::active_delegate_sea(db, "leave", step.approver_id, &hari_ini).await?
        {
            step.approver_id = d;
        }
    }
    if chain.is_empty() {
        exec(
            db,
            "UPDATE leave_requests SET status = 'approved', current_step = 0 WHERE id = ?1".to_string(),
            vec![Value::Int(rid)],
            "leave.auto",
        )
        .await
        .map_err(|e| format!("gagal menyetujui langsung: {e}"))?;
        if is_paid != 0 {
            apply_usage_sea(db, employee_id, tid, year, total as f64).await?;
        }
    } else {
        for step in &chain {
            exec(
                db,
                "INSERT INTO leave_approvals (leave_request_id, approver_id, step_order, step_role, status) VALUES (?1, ?2, ?3, ?4, 'pending')".to_string(),
                vec![
                    Value::Int(rid),
                    Value::Int(step.approver_id),
                    Value::Int(step.order),
                    Value::Text(step.role.clone()),
                ],
                "leave.chain",
            )
            .await
            .map_err(|e| format!("gagal menyimpan rantai: {e}"))?;
        }
        let emp_row = q_one(
            db,
            "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
            vec![Value::Int(employee_id)],
            1,
            "leave.empname",
        )
        .await
        .unwrap_or(None);
        let emp_name = emp_row
            .as_ref()
            .map(|r| value_to_string(&r[0]))
            .unwrap_or_default();
        approval::notify_sea(
            db,
            chain[0].approver_id,
            "leave_approval",
            "Pengajuan Cuti Baru",
            &format!("{emp_name} mengajukan cuti {tname}."),
            "/leave",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "leave",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "leave.id")
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
        "SELECT employee_id, leave_type_id, current_step, status, start_date, total_days FROM leave_requests WHERE id = ?1".to_string(),
        vec![Value::Int(request_id)],
        6,
        "leave.req",
    )
    .await
    .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some(req) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    let (emp_id, type_id, step, status, start_date, total) = (
        value_i64(&req[0]).unwrap_or(0),
        value_i64(&req[1]).unwrap_or(0),
        value_i64(&req[2]).unwrap_or(0),
        value_to_string(&req[3]),
        value_to_string(&req[4]),
        sea_f64(&req[5]),
    );
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let ap = q_one(
        db,
        "SELECT id, approver_id, step_role FROM leave_approvals WHERE leave_request_id = ?1 AND step_order = ?2 AND status = 'pending'".to_string(),
        vec![Value::Int(request_id), Value::Int(step)],
        3,
        "leave.step",
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
    let note = notes.map(str::trim).filter(|s| !s.is_empty());
    exec(
        db,
        "UPDATE leave_approvals SET status = ?1, approver_id = ?2, notes = ?3, acted_at = datetime('now','localtime') WHERE id = ?4".to_string(),
        vec![
            Value::Text(decision.to_string()),
            Value::Int(approver_user_id),
            match note {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Int(approval_id),
        ],
        "leave.decide",
    )
    .await
    .map_err(|e| format!("gagal menyimpan putusan: {e}"))?;
    let emp_user = approval::user_of_employee_sea(db, emp_id).await?;
    if decision == "rejected" {
        exec(
            db,
            "UPDATE leave_requests SET status = 'rejected' WHERE id = ?1".to_string(),
            vec![Value::Int(request_id)],
            "leave.reject",
        )
        .await
        .map_err(|e| format!("gagal menolak: {e}"))?;
        if let Some(uid) = emp_user {
            approval::notify_sea(
                db,
                uid,
                "leave_approval",
                "Pengajuan Cuti Ditolak",
                "Pengajuan cuti Anda ditolak.",
                "/leave",
            )
            .await?;
        }
    } else {
        let next = q_one(
            db,
            "SELECT step_order, approver_id FROM leave_approvals WHERE leave_request_id = ?1 AND step_order > ?2 ORDER BY step_order LIMIT 1".to_string(),
            vec![Value::Int(request_id), Value::Int(step)],
            2,
            "leave.next",
        )
        .await
        .map_err(|e| format!("gagal mencari tahap berikut: {e}"))?;
        match next {
            Some(n) => {
                let (next_order, next_approver) =
                    (value_i64(&n[0]).unwrap_or(0), value_i64(&n[1]).unwrap_or(0));
                exec(
                    db,
                    "UPDATE leave_requests SET current_step = ?1 WHERE id = ?2".to_string(),
                    vec![Value::Int(next_order), Value::Int(request_id)],
                    "leave.advance",
                )
                .await
                .map_err(|e| format!("gagal maju tahap: {e}"))?;
                approval::notify_sea(
                    db,
                    next_approver,
                    "leave_approval",
                    "Cuti Menunggu Persetujuan",
                    "Ada pengajuan cuti menunggu persetujuan Anda.",
                    "/leave",
                )
                .await?;
            }
            None => {
                exec(
                    db,
                    "UPDATE leave_requests SET status = 'approved' WHERE id = ?1".to_string(),
                    vec![Value::Int(request_id)],
                    "leave.approve",
                )
                .await
                .map_err(|e| format!("gagal menyetujui: {e}"))?;
                let paid_row = q_one(
                    db,
                    "SELECT is_paid FROM leave_types WHERE id = ?1".to_string(),
                    vec![Value::Int(type_id)],
                    1,
                    "leave.paid",
                )
                .await
                .unwrap_or(None);
                let paid = paid_row
                    .as_ref()
                    .and_then(|r| value_i64(&r[0]))
                    .unwrap_or(0);
                if paid != 0 {
                    let year: i32 = start_date[..4]
                        .parse()
                        .unwrap_or_else(|_| Local::now().year());
                    apply_usage_sea(db, emp_id, type_id, year, total).await?;
                }
                if let Some(uid) = emp_user {
                    approval::notify_sea(
                        db,
                        uid,
                        "leave_approval",
                        "Cuti Disetujui",
                        "Pengajuan cuti Anda disetujui.",
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
        "leave",
        Some(&request_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn cancel_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    request_id: i64,
) -> Result<(), String> {
    let req = q_one(
        db,
        "SELECT leave_type_id, status, start_date, total_days FROM leave_requests WHERE id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(request_id), Value::Int(employee_id)],
        4,
        "leave.cancelreq",
    )
    .await
    .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some(req) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    let (type_id, status, start_date, total) = (
        value_i64(&req[0]).unwrap_or(0),
        value_to_string(&req[1]),
        value_to_string(&req[2]),
        sea_f64(&req[3]),
    );
    if status != "pending" && status != "approved" {
        return Err("Pengajuan tidak dapat dibatalkan.".to_string());
    }
    let today = Local::now().format("%Y-%m-%d").to_string();
    if start_date < today {
        return Err("Cuti yang sudah berjalan tidak dapat dibatalkan.".to_string());
    }
    if status == "approved" {
        let paid_row = q_one(
            db,
            "SELECT is_paid FROM leave_types WHERE id = ?1".to_string(),
            vec![Value::Int(type_id)],
            1,
            "leave.paid",
        )
        .await
        .unwrap_or(None);
        let paid = paid_row
            .as_ref()
            .and_then(|r| value_i64(&r[0]))
            .unwrap_or(0);
        if paid != 0 {
            let year: i32 = start_date[..4]
                .parse()
                .unwrap_or_else(|_| Local::now().year());
            reverse_usage_sea(db, employee_id, type_id, year, total).await?;
        }
    }
    exec(
        db,
        "UPDATE leave_requests SET status = 'cancelled' WHERE id = ?1".to_string(),
        vec![Value::Int(request_id)],
        "leave.cancel",
    )
    .await
    .map_err(|e| format!("gagal membatalkan: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CANCEL",
        "leave",
        Some(&request_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn calendar_sea(
    db: &sea_orm::DatabaseConnection,
    month: &str,
) -> Result<Vec<CalendarDay>, String> {
    if NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").is_err() {
        return Err("Bulan harus format YYYY-MM.".to_string());
    }
    let mut out = Vec::new();
    let rows = q_all(
        db,
        "SELECT lr.start_date, lr.end_date, e.first_name || ' ' || COALESCE(e.last_name, ''), lt.name FROM leave_requests lr INNER JOIN employees e ON e.id = lr.employee_id INNER JOIN leave_types lt ON lt.id = lr.leave_type_id WHERE lr.status = 'approved' AND substr(lr.start_date, 1, 7) <= ?1 AND substr(lr.end_date, 1, 7) >= ?1".to_string(),
        vec![Value::Text(month.to_string())],
        4,
        "leave.cal",
    )
    .await
    .map_err(|e| format!("gagal membaca kalender: {e}"))?;
    for r in &rows {
        let (start, end, name, tname) = (
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            value_to_string(&r[3]),
        );
        let mut day = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
            .map_err(|_| "Tanggal cuti rusak.".to_string())?;
        let last = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
            .map_err(|_| "Tanggal cuti rusak.".to_string())?;
        while day <= last {
            let s = day.format("%Y-%m-%d").to_string();
            if s.starts_with(month) {
                out.push(CalendarDay {
                    date: s,
                    label: format!("{name} ({tname})"),
                    kind: "leave".to_string(),
                });
            }
            day += chrono::Duration::days(1);
        }
    }
    let hrows = q_all(
        db,
        "SELECT date, name FROM holidays WHERE substr(date, 1, 7) = ?1".to_string(),
        vec![Value::Text(month.to_string())],
        2,
        "leave.calhol",
    )
    .await
    .map_err(|e| format!("gagal membaca libur: {e}"))?;
    for r in &hrows {
        out.push(CalendarDay {
            date: value_to_string(&r[0]),
            label: value_to_string(&r[1]),
            kind: "holiday".to_string(),
        });
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

pub async fn carryover_run_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    year: i32,
) -> Result<i32, String> {
    if !(1990..=2100).contains(&year) {
        return Err("Tahun harus antara 1990 sampai 2100.".to_string());
    }
    let rows = q_all(
        db,
        "SELECT b.employee_id, b.leave_type_id, b.allocated_days + b.carried_days + b.adjustment_days - b.used_days, t.carry_forward_max_days FROM leave_balances b INNER JOIN leave_types t ON t.id = b.leave_type_id WHERE t.deleted_at IS NULL AND t.carry_forward = 1 AND b.year = ?1 AND b.allocated_days + b.carried_days + b.adjustment_days - b.used_days > 0 AND NOT EXISTS (SELECT 1 FROM leave_carryovers lc WHERE lc.employee_id = b.employee_id AND lc.leave_type_id = b.leave_type_id AND lc.from_year = b.year) ORDER BY b.id".to_string(),
        vec![Value::Int(year as i64)],
        4,
        "leave.carryover.q",
    )
    .await
    .map_err(|e| format!("gagal membaca saldo: {e}"))?;
    let mut diproses = 0i64;
    for r in &rows {
        let emp = value_i64(&r[0]).unwrap_or(0);
        let tid = value_i64(&r[1]).unwrap_or(0);
        let sisa = sea_f64(&r[2]);
        let maks = sea_f64(&r[3]);
        let dibawa = if maks > 0.0 { sisa.min(maks) } else { sisa };
        let hangus = sisa - dibawa;
        let next = year + 1;
        let exist = q_one(
            db,
            "SELECT id FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3".to_string(),
            vec![Value::Int(emp), Value::Int(tid), Value::Int(next as i64)],
            1,
            "leave.carryover.bal",
        )
        .await
        .map_err(|e| format!("gagal memeriksa saldo: {e}"))?;
        if let Some(b) = exist {
            exec(
                db,
                "UPDATE leave_balances SET carried_days = ?1 WHERE id = ?2".to_string(),
                vec![Value::Float(dibawa), Value::Int(value_i64(&b[0]).unwrap_or(0))],
                "leave.carryover.upd",
            )
            .await
            .map_err(|e| format!("gagal memperbarui saldo: {e}"))?;
        } else {
            exec(
                db,
                "INSERT INTO leave_balances (employee_id, leave_type_id, year, allocated_days, used_days, carried_days, adjustment_days) VALUES (?1, ?2, ?3, 0, 0, ?4, 0)".to_string(),
                vec![Value::Int(emp), Value::Int(tid), Value::Int(next as i64), Value::Float(dibawa)],
                "leave.carryover.ins",
            )
            .await
            .map_err(|e| format!("gagal membuat saldo: {e}"))?;
        }
        exec(
            db,
            "INSERT INTO leave_carryovers (employee_id, leave_type_id, from_year, to_year, days_carried, days_expired) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
            vec![
                Value::Int(emp),
                Value::Int(tid),
                Value::Int(year as i64),
                Value::Int(next as i64),
                Value::Float(dibawa),
                Value::Float(hangus),
            ],
            "leave.carryover.rec",
        )
        .await
        .map_err(|e| format!("gagal mencatat carry-over: {e}"))?;
        diproses += 1;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "leave.carryover",
        Some(&year.to_string()),
        None,
        None,
        None,
    )
    .await;
    to_dto_int(diproses, "leave.carryover")
}

pub async fn type_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<LeaveType>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name, default_days_per_year, is_paid, carry_forward, carry_forward_max_days, requires_attachment FROM leave_types WHERE deleted_at IS NULL ORDER BY name".to_string(),
        vec![],
        8,
        "leave.typelist",
    )
    .await
    .map_err(|e| format!("gagal membaca tipe: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let (id, paid, carry, attach) = (
            value_i64(&r[0]).unwrap_or(0),
            value_i64(&r[4]).unwrap_or(0),
            value_i64(&r[5]).unwrap_or(0),
            value_i64(&r[7]).unwrap_or(0),
        );
        out.push(LeaveType {
            id: to_dto_int(id, "leavetype.id")?,
            code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
            default_days_per_year: sea_f64(&r[3]),
            is_paid: paid != 0,
            carry_forward: carry != 0,
            carry_forward_max_days: sea_f64(&r[6]),
            requires_attachment: attach != 0,
        });
    }
    Ok(out)
}

pub async fn type_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &LeaveTypeInput,
) -> Result<i32, String> {
    if input.code.trim().is_empty() || input.name.trim().is_empty() {
        return Err("Kode dan nama tipe wajib diisi.".to_string());
    }
    if input.default_days_per_year < 0.0 || input.carry_forward_max_days < 0.0 {
        return Err("Jumlah hari tidak boleh negatif.".to_string());
    }
    let paid = if input.is_paid { 1 } else { 0 };
    let carry = if input.carry_forward { 1 } else { 0 };
    let attach = if input.requires_attachment { 1 } else { 0 };
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE leave_types SET code = ?1, name = ?2, default_days_per_year = ?3, is_paid = ?4, carry_forward = ?5, carry_forward_max_days = ?6, requires_attachment = ?7 WHERE id = ?8 AND deleted_at IS NULL".to_string(),
            vec![
                Value::Text(input.code.trim().to_string()),
                Value::Text(input.name.trim().to_string()),
                Value::Float(input.default_days_per_year),
                Value::Int(paid),
                Value::Int(carry),
                Value::Float(input.carry_forward_max_days),
                Value::Int(attach),
                Value::Int(rid),
            ],
            "leave.typeupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan tipe: {e}"))?;
        if n == 0 {
            return Err("Tipe cuti tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "leave_type",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "leavetype.id")
    } else {
        let rid = exec_insert(
            db,
            "INSERT INTO leave_types (code, name, default_days_per_year, is_paid, carry_forward, carry_forward_max_days, requires_attachment) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)".to_string(),
            vec![
                Value::Text(input.code.trim().to_string()),
                Value::Text(input.name.trim().to_string()),
                Value::Float(input.default_days_per_year),
                Value::Int(paid),
                Value::Int(carry),
                Value::Float(input.carry_forward_max_days),
                Value::Int(attach),
            ],
            "leave.typeadd",
        )
        .await
        .map_err(|e| {
            if e.contains("UNIQUE") {
                "Kode tipe sudah dipakai.".to_string()
            } else {
                format!("gagal menambah tipe: {e}")
            }
        })?;
        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "leave_type",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        return to_dto_int(rid, "leavetype.id");
    }
}

pub async fn type_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let used_row = q_one(
        db,
        "SELECT COUNT(*) FROM leave_requests WHERE leave_type_id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "leave.typerel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    let used = used_row
        .as_ref()
        .and_then(|r| value_i64(&r[0]))
        .unwrap_or(0);
    if used > 0 {
        return Err("Tipe sudah dipakai pengajuan.".to_string());
    }
    let n = exec(
        db,
        "UPDATE leave_types SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "leave.typedel",
    )
    .await
    .map_err(|e| format!("gagal menghapus tipe: {e}"))?;
    if n == 0 {
        return Err("Tipe cuti tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "leave_type",
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
    use crate::services::sea_raw::{exec, q_one, value_i64};

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

    async fn one(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> i64 {
        q_one(db, sql.to_string(), vec![], 1, label)
            .await
            .expect("one")
            .and_then(|r| value_i64(&r[0]))
            .expect("id")
    }

    async fn text(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> String {
        q_one(db, sql.to_string(), vec![], 1, label)
            .await
            .expect("text")
            .map(|r| value_to_string(&r[0]))
            .expect("val")
    }

    async fn al_type(db: &sea_orm::DatabaseConnection) -> i64 {
        one(db, "SELECT id FROM leave_types WHERE code = 'AL'", "test.al").await
    }

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        one(
            db,
            "SELECT id FROM users WHERE username = 'admin'",
            "test.admin",
        )
        .await
    }

    async fn used_days(db: &sea_orm::DatabaseConnection, eid: i64, tid: i64, year: i32) -> f64 {
        let r = q_one(
            db,
            "SELECT used_days FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3".to_string(),
            vec![Value::Int(eid), Value::Int(tid), Value::Int(year as i64)],
            1,
            "test.used",
        )
        .await
        .expect("used")
        .expect("baris saldo");
        sea_f64(&r[0])
    }

    fn fwd_monday() -> chrono::NaiveDate {
        use chrono::Datelike;
        let mut d = Local::now().date_naive() + chrono::Duration::days(8);
        while d.weekday().num_days_from_monday() != 0 {
            d += chrono::Duration::days(1);
        }
        d
    }

    fn ds(d: chrono::NaiveDate) -> String {
        d.format("%Y-%m-%d").to_string()
    }

    async fn state() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[tokio::test]
    async fn alur_pengajuan_persetujuan_dan_saldo() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let (sup_uid, sup_eid) = mkuser(db, "seasup", "EMP-SEAS", None).await;
        let (uid, eid) = mkuser(db, "seastaff", "EMP-SEAE", Some(sup_eid)).await;
        let tid = al_type(db).await;
        let rid = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: "2026-06-01".to_string(),
                end_date: "2026-06-02".to_string(),
                reason: Some("Uji saldo".to_string()),
            },
        )
        .await
        .expect("buat") as i64;
        assert!(rid > 0);
        let mine = my_requests_sea(db, eid).await.expect("mine");
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].status, "pending");
        assert_eq!(mine[0].total_days, 2.0);
        assert_eq!(mine[0].reason.as_deref(), Some("Uji saldo"));
        let pend = pending_for_sea(db, sup_uid).await.expect("pend");
        assert_eq!(pend.len(), 1);
        assert_eq!(pend[0].id, rid as i32);
        decide_sea(db, sup_uid, rid, "approved", None)
            .await
            .expect("tahap1");
        loop {
            let next = q_one(
                db,
                "SELECT la.approver_id FROM leave_approvals la INNER JOIN leave_requests lr ON lr.id = la.leave_request_id WHERE la.leave_request_id = ?1 AND la.status = 'pending' AND lr.current_step = la.step_order".to_string(),
                vec![Value::Int(rid)],
                1,
                "test.next",
            )
            .await
            .expect("next");
            match next {
                Some(r) => {
                    let a2 = value_i64(&r[0]).expect("approver");
                    decide_sea(db, a2, rid, "approved", None)
                        .await
                        .expect("tahap berikutnya");
                }
                None => break,
            }
        }
        let status = text(
            db,
            &format!("SELECT status FROM leave_requests WHERE id = {rid}"),
            "test.status",
        )
        .await;
        assert_eq!(status, "approved");
        let all = all_requests_sea(db).await.expect("all");
        assert!(all.iter().any(|r| r.id as i64 == rid && r.status == "approved"));
        let bals = balances_sea(db, eid, 2026).await.expect("bals");
        let al = bals.iter().find(|b| b.leave_type_id == tid as i32).expect("AL");
        assert_eq!(al.used_days, 2.0);
        assert_eq!(al.remaining, al.allocated_days + al.carried_days + al.adjustment_days - 2.0);
        let cal = calendar_sea(db, "2026-06").await.expect("kalender");
        assert!(cal
            .iter()
            .any(|d| d.kind == "leave" && d.date == "2026-06-01"));
    }

    #[tokio::test]
    async fn saldo_dipotong_saat_final_dan_kembali_saat_batal() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let (sup_uid, sup_eid) = mkuser(db, "cuti-spv", "EMP-C1", None).await;
        let (uid, eid) = mkuser(db, "cuti-staff", "EMP-C2", Some(sup_eid)).await;
        let tid = al_type(db).await;
        let adm = admin_id(db).await;
        let hr_role = one(
            db,
            "SELECT id FROM roles WHERE slug = 'hr-administrator'",
            "test.hrrole",
        )
        .await;
        exec(
            db,
            "INSERT INTO user_roles (user_id, role_id) VALUES (?1, ?2)".to_string(),
            vec![Value::Int(adm), Value::Int(hr_role)],
            "test.grant",
        )
        .await
        .expect("role");
        let id = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(fwd_monday()),
                end_date: ds(fwd_monday() + chrono::Duration::days(4)),
                reason: Some("Liburan".to_string()),
            },
        )
        .await
        .expect("buat") as i64;
        decide_sea(db, sup_uid, id, "approved", None)
            .await
            .expect("tahap1");
        decide_sea(db, adm, id, "approved", None)
            .await
            .expect("tahap2");
        let status = text(
            db,
            &format!("SELECT status FROM leave_requests WHERE id = {id}"),
            "test.status",
        )
        .await;
        assert_eq!(status, "approved");
        assert_eq!(used_days(db, eid, tid, fwd_monday().year()).await, 5.0);
        cancel_sea(db, uid, eid, id).await.expect("batal");
        assert_eq!(used_days(db, eid, tid, fwd_monday().year()).await, 0.0);
    }

    #[tokio::test]
    async fn saldo_kurang_overlap_dan_hari_kerja_ditolak() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let (_, sup_eid) = mkuser(db, "cuti-spv3", "EMP-C0", None).await;
        let (uid, eid) = mkuser(db, "cuti-solo", "EMP-C3", Some(sup_eid)).await;
        let tid = al_type(db).await;
        let mon = fwd_monday();
        let id = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(mon),
                end_date: ds(mon + chrono::Duration::days(11)),
                reason: None,
            },
        )
        .await
        .expect("buat") as i64;
        let e = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(mon + chrono::Duration::days(2)),
                end_date: ds(mon + chrono::Duration::days(4)),
                reason: None,
            },
        )
        .await
        .expect_err("overlap");
        assert!(e.contains("rentang"), "dapat: {e}");
        let sat = mon + chrono::Duration::days(5);
        let e = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(sat),
                end_date: ds(sat + chrono::Duration::days(1)),
                reason: None,
            },
        )
        .await
        .expect_err("weekend");
        assert!(e.contains("hari kerja"));
        cancel_sea(db, uid, eid, id).await.expect("batal pending");
        let e = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(mon),
                end_date: ds(mon + chrono::Duration::days(60)),
                reason: None,
            },
        )
        .await
        .expect_err("saldo");
        assert!(e.contains("Saldo"));
    }

    #[tokio::test]
    async fn kalender_dan_tipe_crud() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let adm = admin_id(db).await;
        let tid = type_save_sea(
            db,
            adm,
            None,
            &LeaveTypeInput {
                code: "TS".to_string(),
                name: "Tes".to_string(),
                default_days_per_year: 1.0,
                is_paid: true,
                carry_forward: false,
                carry_forward_max_days: 0.0,
                requires_attachment: false,
            },
        )
        .await
        .expect("tipe");
        assert!(type_list_sea(db)
            .await
            .expect("list")
            .iter()
            .any(|t| t.code == "TS"));
        let (uid, eid) = mkuser(db, "cuti-ts", "EMP-C4", None).await;
        let lid = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid,
                start_date: "2026-07-06".to_string(),
                end_date: "2026-07-06".to_string(),
                reason: None,
            },
        )
        .await
        .expect("pakai") as i64;
        assert!(type_delete_sea(db, adm, tid as i64).await.is_err());
        let _ = lid;
        let cal = calendar_sea(db, "2026-07").await.expect("kalender");
        assert!(cal
            .iter()
            .any(|d| d.kind == "leave" && d.date == "2026-07-06"));
    }

    #[tokio::test]
    async fn bisnis_days_lewati_akhir_pekan_dan_libur() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        assert_eq!(
            count_business_days_sea(db, "2026-06-06", "2026-06-07")
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            count_business_days_sea(db, "2026-06-01", "2026-06-05")
                .await
                .unwrap(),
            5
        );
        exec(
            db,
            "INSERT INTO holidays (name, date, type) VALUES ('Uji', '2026-06-03', 'national')".to_string(),
            vec![],
            "test.hol",
        )
        .await
        .expect("libur");
        assert_eq!(
            count_business_days_sea(db, "2026-06-01", "2026-06-05")
                .await
                .unwrap(),
            4
        );
    }

    #[tokio::test]
    async fn carryover_membawa_sisa_dan_mencatat_hangus() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin_id(db).await;
        let tid = type_save_sea(
            db,
            actor,
            None,
            &LeaveTypeInput {
                code: "CFO".to_string(),
                name: "Cuti Carry".to_string(),
                default_days_per_year: 12.0,
                is_paid: true,
                carry_forward: true,
                carry_forward_max_days: 3.0,
                requires_attachment: false,
            },
        )
        .await
        .expect("tipe") as i64;
        let (_uid, eid) = mkuser(db, "cfo1", "EMP-CFO1", None).await;
        exec(
            db,
            "INSERT INTO leave_balances (employee_id, leave_type_id, year, allocated_days, used_days, carried_days, adjustment_days) VALUES (?1, ?2, 2025, 12, 5, 0, 0)".to_string(),
            vec![Value::Int(eid), Value::Int(tid)],
            "t.bl",
        )
        .await
        .expect("saldo");
        assert_eq!(carryover_run_sea(db, actor, 2025).await.expect("jalankan"), 1);
        let bal = q_one(
            db,
            "SELECT carried_days FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = 2026".to_string(),
            vec![Value::Int(eid), Value::Int(tid)],
            "t.b26",
        )
        .await
        .expect("b26")
        .expect("ada");
        assert!((sea_f64(&bal[0]) - 3.0).abs() < 1e-9);
        let rec = q_all(
            db,
            "SELECT days_carried, days_expired FROM leave_carryovers WHERE employee_id = ?1".to_string(),
            vec![Value::Int(eid)],
            2,
            "t.rec",
        )
        .await
        .expect("rec");
        assert!((sea_f64(&rec[0][0]) - 3.0).abs() < 1e-9);
        assert!((sea_f64(&rec[0][1]) - 4.0).abs() < 1e-9);
        assert_eq!(carryover_run_sea(db, actor, 2025).await.expect("idempoten"), 0);
    }

    #[tokio::test]
    async fn jenjang_jenis_dan_delegasi_memindahkan_antrean() {
        let dir = state().await;
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin_id(db).await;
        let tid = type_save_sea(
            db,
            actor,
            None,
            &LeaveTypeInput {
                code: "JEN".to_string(),
                name: "Cuti Jenjang".to_string(),
                default_days_per_year: 12.0,
                is_paid: true,
                carry_forward: false,
                carry_forward_max_days: 0.0,
                requires_attachment: false,
            },
        )
        .await
        .expect("tipe") as i64;
        let (x_uid, _) = mkuser(db, "jenkx", "EMP-JNKX", None).await;
        let (z_uid, _) = mkuser(db, "jenzk", "EMP-JNZK", None).await;
        let (uid, eid) = mkuser(db, "jenap", "EMP-JNAP", None).await;
        exec(
            db,
            "INSERT INTO leave_type_levels (leave_type_id, department_id, step_order, approver_user_id) VALUES (?1, 0, 1, ?2)".to_string(),
            vec![Value::Int(tid), Value::Int(x_uid)],
            "t.lvl",
        )
        .await
        .expect("jenjang");
        approval::delegate_sea(db, x_uid, z_uid, "leave", "2020-01-01", "2099-12-31", None)
            .await
            .expect("delegasi");
        let start = fwd_monday();
        let end = start + chrono::Duration::days(1);
        let year: i32 = ds(start)[..4].parse().expect("tahun");
        exec(
            db,
            "INSERT INTO leave_balances (employee_id, leave_type_id, year, allocated_days, used_days, carried_days, adjustment_days) VALUES (?1, ?2, ?3, 12, 0, 0, 0)".to_string(),
            vec![Value::Int(eid), Value::Int(tid), Value::Int(year as i64)],
            "t.bl2",
        )
        .await
        .expect("saldo");
        let rid = create_sea(
            db,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(start),
                end_date: ds(end),
                reason: Some("Perlu tugas keluarga yang mendesak.".to_string()),
            },
        )
        .await
        .expect("ajukan") as i64;
        assert_eq!(pending_for_sea(db, z_uid).await.expect("antrean z").len(), 1);
        assert_eq!(pending_for_sea(db, x_uid).await.expect("antrean x").len(), 0);
        decide_sea(db, z_uid, rid, "approved", None)
            .await
            .expect("setujui");
        assert_eq!(
            text(
                db,
                &format!("SELECT status FROM leave_requests WHERE id = {rid}"),
                "t.st",
            ),
            "approved"
        );
        assert!((used_days(db, eid, tid, year).await - 2.0).abs() < 1e-9);
        let err = decide_sea(db, x_uid, rid, "approved", None)
            .await
            .expect_err("ganda");
        assert!(err.contains("sudah diproses"));
    }
}
