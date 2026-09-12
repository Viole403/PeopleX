//! Cuti: saldo, pengajuan snapshot, putusan bertahap, pembatalan, kalender.

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

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

// ---------------- Saldo ----------------

pub fn balances(conn: &Connection, employee_id: i64, year: i32) -> Result<Vec<Balance>, String> {
    let mut stmt = conn
        .prepare("SELECT lt.id, lt.code, lt.name, lt.is_paid,
                         COALESCE(lb.allocated_days, lt.default_days_per_year),
                         COALESCE(lb.used_days, 0), COALESCE(lb.carried_days, 0), COALESCE(lb.adjustment_days, 0)
                  FROM leave_types lt
                  LEFT JOIN leave_balances lb ON lb.leave_type_id = lt.id AND lb.employee_id = ?1 AND lb.year = ?2
                  WHERE lt.deleted_at IS NULL ORDER BY lt.name")
        .map_err(|e| format!("gagal menyiapkan saldo: {e}"))?;
    let rows = stmt
        .query_map(params![employee_id, year], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, f64>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca saldo: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name, paid, alloc, used, carried, adj) =
            row.map_err(|e| format!("gagal membaca baris saldo: {e}"))?;
        out.push(Balance {
            leave_type_id: to_dto_int(id, "balance.type")?,
            code,
            name,
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

fn ensure_balance_row(
    conn: &Connection,
    employee_id: i64,
    leave_type_id: i64,
    year: i32,
) -> Result<i64, String> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3",
            params![employee_id, leave_type_id, year],
            |r| r.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa saldo: {e}"))?
    {
        return Ok(id);
    }
    let def: f64 = conn
        .query_row(
            "SELECT default_days_per_year FROM leave_types WHERE id = ?1",
            params![leave_type_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat tipe cuti: {e}"))?
        .ok_or("Tipe cuti tidak ditemukan.".to_string())?;
    conn.execute(
        "INSERT INTO leave_balances (employee_id, leave_type_id, year, allocated_days, used_days, carried_days, adjustment_days) VALUES (?1, ?2, ?3, ?4, 0, 0, 0)",
        params![employee_id, leave_type_id, year, def],
    )
    .map_err(|e| format!("gagal membuat saldo: {e}"))?;
    Ok(conn.last_insert_rowid())
}

fn apply_usage(
    conn: &Connection,
    employee_id: i64,
    leave_type_id: i64,
    year: i32,
    days: f64,
) -> Result<(), String> {
    let id = ensure_balance_row(conn, employee_id, leave_type_id, year)?;
    conn.execute(
        "UPDATE leave_balances SET used_days = used_days + ?1 WHERE id = ?2",
        params![days, id],
    )
    .map_err(|e| format!("gagal memakai saldo: {e}"))?;
    Ok(())
}

fn reverse_usage(
    conn: &Connection,
    employee_id: i64,
    leave_type_id: i64,
    year: i32,
    days: f64,
) -> Result<(), String> {
    let id = ensure_balance_row(conn, employee_id, leave_type_id, year)?;
    conn.execute(
        "UPDATE leave_balances SET used_days = MAX(0, used_days - ?1) WHERE id = ?2",
        params![days, id],
    )
    .map_err(|e| format!("gagal mengembalikan saldo: {e}"))?;
    Ok(())
}

/// Hitung hari kerja Senin-Jumat minus libur.
pub fn count_business_days(conn: &Connection, start: &str, end: &str) -> Result<i64, String> {
    let mut day = NaiveDate::parse_from_str(start, "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    let last = NaiveDate::parse_from_str(end, "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if last < day {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    let hols: Vec<String> = conn
        .prepare("SELECT date FROM holidays WHERE date BETWEEN ?1 AND ?2")
        .map_err(|e| format!("gagal memuat libur: {e}"))?
        .query_map(params![start, end], |r| r.get(0))
        .map_err(|e| format!("gagal membaca libur: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("gagal membaca libur: {e}"))?;
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

// ---------------- Daftar ----------------

fn map_request(
    id: i64,
    emp: i64,
    name: String,
    number: String,
    tid: i64,
    tname: String,
    start: String,
    end: String,
    total: f64,
    reason: Option<String>,
    status: String,
    step: i64,
    created: String,
) -> Result<LeaveRequest, String> {
    Ok(LeaveRequest {
        id: to_dto_int(id, "leave.id")?,
        employee_id: to_dto_int(emp, "leave.emp")?,
        employee_name: name,
        employee_number: number,
        leave_type_id: to_dto_int(tid, "leave.type")?,
        leave_type_name: tname,
        start_date: start,
        end_date: end,
        total_days: total,
        reason,
        status,
        current_step: to_dto_int(step, "leave.step")?,
        created_at: created,
    })
}

pub fn my_requests(conn: &Connection, employee_id: i64) -> Result<Vec<LeaveRequest>, String> {
    let mut stmt = conn
        .prepare("SELECT lr.id, lr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, lr.leave_type_id, lt.name, lr.start_date, lr.end_date, lr.total_days, lr.reason, lr.status, lr.current_step, lr.created_at
                  FROM leave_requests lr INNER JOIN leave_types lt ON lt.id = lr.leave_type_id
                  INNER JOIN employees e ON e.id = lr.employee_id
                  WHERE lr.employee_id = ?1 ORDER BY lr.created_at DESC")
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    collect_requests(conn, &mut stmt, employee_id)
}

pub fn pending_for(conn: &Connection, user_id: i64) -> Result<Vec<LeaveRequest>, String> {
    let mut stmt = conn
        .prepare("SELECT lr.id, lr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, lr.leave_type_id, lt.name, lr.start_date, lr.end_date, lr.total_days, lr.reason, lr.status, lr.current_step, lr.created_at
                  FROM leave_approvals la
                  INNER JOIN leave_requests lr ON lr.id = la.leave_request_id
                  INNER JOIN leave_types lt ON lt.id = lr.leave_type_id
                  INNER JOIN employees e ON e.id = lr.employee_id
                  WHERE la.approver_id = ?1 AND la.status = 'pending' AND lr.status = 'pending' AND lr.current_step = la.step_order
                  ORDER BY lr.created_at ASC")
        .map_err(|e| format!("gagal menyiapkan antrean: {e}"))?;
    collect_requests(conn, &mut stmt, user_id)
}

pub fn all_requests(conn: &Connection) -> Result<Vec<LeaveRequest>, String> {
    let mut stmt = conn
        .prepare("SELECT lr.id, lr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, lr.leave_type_id, lt.name, lr.start_date, lr.end_date, lr.total_days, lr.reason, lr.status, lr.current_step, lr.created_at
                  FROM leave_requests lr
                  INNER JOIN leave_types lt ON lt.id = lr.leave_type_id
                  INNER JOIN employees e ON e.id = lr.employee_id
                  ORDER BY CASE lr.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 WHEN 'rejected' THEN 2 ELSE 3 END, lr.created_at DESC")
        .map_err(|e| format!("gagal menyiapkan semua: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, f64>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, String>(12)?,
            ))
        })
        .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let t = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(map_request(
            t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7, t.8, t.9, t.10, t.11, t.12,
        )?);
    }
    Ok(out)
}

fn collect_requests(
    _conn: &Connection,
    stmt: &mut rusqlite::Statement,
    param: i64,
) -> Result<Vec<LeaveRequest>, String> {
    let rows = stmt
        .query_map(params![param], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, f64>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, i64>(11)?,
                r.get::<_, String>(12)?,
            ))
        })
        .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let t = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(map_request(
            t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7, t.8, t.9, t.10, t.11, t.12,
        )?);
    }
    Ok(out)
}

// ---------------- Buat ----------------

pub fn create(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    input: &LeaveCreate,
) -> Result<i32, String> {
    let ltype: Option<(i64, String, i64)> = conn
        .query_row(
            "SELECT id, name, is_paid FROM leave_types WHERE id = ?1 AND deleted_at IS NULL",
            params![input.leave_type_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat tipe cuti: {e}"))?;
    let Some((tid, tname, is_paid)) = ltype else {
        return Err("Tipe cuti tidak ditemukan.".to_string());
    };
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
    let total = count_business_days(conn, start, end)?;
    if total <= 0 {
        return Err("Rentang tanggal tidak memiliki hari kerja.".to_string());
    }
    let year: i32 = start[..4]
        .parse()
        .map_err(|_| "Tahun tidak valid.".to_string())?;
    if is_paid != 0 {
        let bals = balances(conn, employee_id, year)?;
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
    let overlap: Option<i64> = conn
        .query_row(
            "SELECT id FROM leave_requests WHERE employee_id = ?1 AND status IN ('pending','approved') AND NOT (end_date < ?2 OR start_date > ?3)",
            params![employee_id, start, end],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa tumpang tindih: {e}"))?;
    if overlap.is_some() {
        return Err("Sudah ada pengajuan cuti pada rentang tanggal tersebut.".to_string());
    }
    conn.execute(
        "INSERT INTO leave_requests (employee_id, leave_type_id, start_date, end_date, total_days, reason, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 1)",
        params![employee_id, tid, start, end, total as f64, input.reason.as_deref().map(str::trim).filter(|s| !s.is_empty())],
    )
    .map_err(|e| format!("gagal mengajukan cuti: {e}"))?;
    let rid = conn.last_insert_rowid();
    let chain = approval::build_chain(conn, "leave", employee_id)?;
    if chain.is_empty() {
        conn.execute(
            "UPDATE leave_requests SET status = 'approved', current_step = 0 WHERE id = ?1",
            params![rid],
        )
        .map_err(|e| format!("gagal menyetujui langsung: {e}"))?;
        if is_paid != 0 {
            apply_usage(conn, employee_id, tid, year, total as f64)?;
        }
    } else {
        for step in &chain {
            conn.execute(
                "INSERT INTO leave_approvals (leave_request_id, approver_id, step_order, step_role, status) VALUES (?1, ?2, ?3, ?4, 'pending')",
                params![rid, step.approver_id, step.order, step.role],
            )
            .map_err(|e| format!("gagal menyimpan rantai: {e}"))?;
        }
        let emp_name: String = conn
            .query_row(
                "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1",
                params![employee_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        approval::notify(
            conn,
            chain[0].approver_id,
            "leave_approval",
            "Pengajuan Cuti Baru",
            &format!("{emp_name} mengajukan cuti {tname}."),
            "/leave",
        )?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "leave",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "leave.id")
}

// ---------------- Putusan ----------------

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
    let req: Option<(i64, i64, i64, String, String, f64)> = conn
        .query_row(
            "SELECT employee_id, leave_type_id, current_step, status, start_date, total_days FROM leave_requests WHERE id = ?1",
            params![request_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some((emp_id, type_id, step, status, start_date, total)) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let approval: Option<(i64, i64, String)> = conn
        .query_row(
            "SELECT id, approver_id, step_role FROM leave_approvals WHERE leave_request_id = ?1 AND step_order = ?2 AND status = 'pending'",
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
    conn.execute(
        "UPDATE leave_approvals SET status = ?1, approver_id = ?2, notes = ?3, acted_at = datetime('now','localtime') WHERE id = ?4",
        params![decision, approver_user_id, notes.map(str::trim).filter(|s| !s.is_empty()), approval_id],
    )
    .map_err(|e| format!("gagal menyimpan putusan: {e}"))?;
    let emp_user = approval::user_of_employee(conn, emp_id)?;
    if decision == "rejected" {
        conn.execute(
            "UPDATE leave_requests SET status = 'rejected' WHERE id = ?1",
            params![request_id],
        )
        .map_err(|e| format!("gagal menolak: {e}"))?;
        if let Some(uid) = emp_user {
            approval::notify(
                conn,
                uid,
                "leave_approval",
                "Pengajuan Cuti Ditolak",
                "Pengajuan cuti Anda ditolak.",
                "/leave",
            )?;
        }
    } else {
        let next: Option<(i64, i64)> = conn
            .query_row(
                "SELECT step_order, approver_id FROM leave_approvals WHERE leave_request_id = ?1 AND step_order > ?2 ORDER BY step_order LIMIT 1",
                params![request_id, step],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("gagal mencari tahap berikut: {e}"))?;
        match next {
            Some((next_order, next_approver)) => {
                conn.execute(
                    "UPDATE leave_requests SET current_step = ?1 WHERE id = ?2",
                    params![next_order, request_id],
                )
                .map_err(|e| format!("gagal maju tahap: {e}"))?;
                approval::notify(
                    conn,
                    next_approver,
                    "leave_approval",
                    "Cuti Menunggu Persetujuan",
                    "Ada pengajuan cuti menunggu persetujuan Anda.",
                    "/leave",
                )?;
            }
            None => {
                conn.execute(
                    "UPDATE leave_requests SET status = 'approved' WHERE id = ?1",
                    params![request_id],
                )
                .map_err(|e| format!("gagal menyetujui: {e}"))?;
                let paid: i64 = conn
                    .query_row(
                        "SELECT is_paid FROM leave_types WHERE id = ?1",
                        params![type_id],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if paid != 0 {
                    let year: i32 = start_date[..4]
                        .parse()
                        .unwrap_or_else(|_| Local::now().year());
                    apply_usage(conn, emp_id, type_id, year, total)?;
                }
                if let Some(uid) = emp_user {
                    approval::notify(
                        conn,
                        uid,
                        "leave_approval",
                        "Cuti Disetujui",
                        "Pengajuan cuti Anda disetujui.",
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
        "leave",
        Some(&request_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Batal ----------------

pub fn cancel(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    request_id: i64,
) -> Result<(), String> {
    let req: Option<(i64, String, String, f64, String)> = conn
        .query_row(
            "SELECT leave_type_id, status, start_date, total_days, employee_id FROM leave_requests WHERE id = ?1 AND employee_id = ?2",
            params![request_id, employee_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)?.to_string())),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some((type_id, status, start_date, total, _)) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    if status != "pending" && status != "approved" {
        return Err("Pengajuan tidak dapat dibatalkan.".to_string());
    }
    let today = Local::now().format("%Y-%m-%d").to_string();
    if start_date < today {
        return Err("Cuti yang sudah berjalan tidak dapat dibatalkan.".to_string());
    }
    if status == "approved" {
        let paid: i64 = conn
            .query_row(
                "SELECT is_paid FROM leave_types WHERE id = ?1",
                params![type_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if paid != 0 {
            let year: i32 = start_date[..4]
                .parse()
                .unwrap_or_else(|_| Local::now().year());
            reverse_usage(conn, employee_id, type_id, year, total)?;
        }
    }
    conn.execute(
        "UPDATE leave_requests SET status = 'cancelled' WHERE id = ?1",
        params![request_id],
    )
    .map_err(|e| format!("gagal membatalkan: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "CANCEL",
        "leave",
        Some(&request_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Kalender ----------------

pub fn calendar(conn: &Connection, month: &str) -> Result<Vec<CalendarDay>, String> {
    if NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").is_err() {
        return Err("Bulan harus format YYYY-MM.".to_string());
    }
    let mut out = Vec::new();
    let mut stmt = conn
        .prepare("SELECT lr.start_date, lr.end_date, e.first_name || ' ' || COALESCE(e.last_name, ''), lt.name FROM leave_requests lr INNER JOIN employees e ON e.id = lr.employee_id INNER JOIN leave_types lt ON lt.id = lr.leave_type_id WHERE lr.status = 'approved' AND substr(lr.start_date, 1, 7) <= ?1 AND substr(lr.end_date, 1, 7) >= ?1")
        .map_err(|e| format!("gagal menyiapkan kalender: {e}"))?;
    let rows = stmt
        .query_map(params![month], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca kalender: {e}"))?;
    for row in rows {
        let (start, end, name, tname) =
            row.map_err(|e| format!("gagal membaca baris kalender: {e}"))?;
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
    let mut hstmt = conn
        .prepare("SELECT date, name FROM holidays WHERE substr(date, 1, 7) = ?1")
        .map_err(|e| format!("gagal menyiapkan libur: {e}"))?;
    let hrows = hstmt
        .query_map(params![month], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .map_err(|e| format!("gagal membaca libur: {e}"))?;
    for row in hrows {
        let (date, name) = row.map_err(|e| format!("gagal membaca baris libur: {e}"))?;
        out.push(CalendarDay {
            date,
            label: name,
            kind: "holiday".to_string(),
        });
    }
    out.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(out)
}

// ---------------- Tipe cuti ----------------

pub fn type_list(conn: &Connection) -> Result<Vec<LeaveType>, String> {
    let mut stmt = conn
        .prepare("SELECT id, code, name, default_days_per_year, is_paid, carry_forward, carry_forward_max_days, requires_attachment FROM leave_types WHERE deleted_at IS NULL ORDER BY name")
        .map_err(|e| format!("gagal menyiapkan tipe: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca tipe: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name, def, paid, carry, carry_max, attach) =
            row.map_err(|e| format!("gagal membaca baris tipe: {e}"))?;
        out.push(LeaveType {
            id: to_dto_int(id, "leavetype.id")?,
            code,
            name,
            default_days_per_year: def,
            is_paid: paid != 0,
            carry_forward: carry != 0,
            carry_forward_max_days: carry_max,
            requires_attachment: attach != 0,
        });
    }
    Ok(out)
}

pub fn type_save(
    conn: &Connection,
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
        let n = conn.execute(
            "UPDATE leave_types SET code = ?1, name = ?2, default_days_per_year = ?3, is_paid = ?4, carry_forward = ?5, carry_forward_max_days = ?6, requires_attachment = ?7 WHERE id = ?8 AND deleted_at IS NULL",
            params![input.code.trim(), input.name.trim(), input.default_days_per_year, paid, carry, input.carry_forward_max_days, attach, rid],
        ).map_err(|e| format!("gagal menyimpan tipe: {e}"))?;
        if n == 0 {
            return Err("Tipe cuti tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "leave_type",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "leavetype.id")
    } else {
        conn.execute(
            "INSERT INTO leave_types (code, name, default_days_per_year, is_paid, carry_forward, carry_forward_max_days, requires_attachment) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![input.code.trim(), input.name.trim(), input.default_days_per_year, paid, carry, input.carry_forward_max_days, attach],
        ).map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Kode tipe sudah dipakai.".to_string()
            } else {
                format!("gagal menambah tipe: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "leave_type",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "leavetype.id")
    }
}

pub fn type_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let used: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM leave_requests WHERE leave_type_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if used > 0 {
        return Err("Tipe sudah dipakai pengajuan.".to_string());
    }
    let n = conn.execute(
        "UPDATE leave_types SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus tipe: {e}"))?;
    if n == 0 {
        return Err("Tipe cuti tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "leave_type",
        Some(&id.to_string()),
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
    use chrono::Datelike;

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
        number: &str,
        supervisor: Option<i64>,
    ) -> (i64, i64) {
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, supervisor_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, ?2, '2026-01-01', 'active', 'permanent')",
            params![number, supervisor],
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

    fn al_type(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM leave_types WHERE code = 'AL'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn admin(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .unwrap()
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

    #[test]
    fn saldo_dipotong_saat_final_dan_kembali_saat_batal() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let (sup_uid, sup_eid) = mkuser(&conn, "cuti-spv", "EMP-C1", None);
        let (uid, eid) = mkuser(&conn, "cuti-staff", "EMP-C2", Some(sup_eid));
        let tid = al_type(&conn);
        let adm: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let hr_role: i64 = conn
            .query_row(
                "SELECT id FROM roles WHERE slug = 'hr-administrator'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute(
            "INSERT INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
            params![adm, hr_role],
        )
        .unwrap();
        let id = create(
            &conn,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(fwd_monday()),
                end_date: ds(fwd_monday() + chrono::Duration::days(4)),
                reason: Some("Liburan".to_string()),
            },
        )
        .expect("buat") as i64;
        decide(&conn, sup_uid, id, "approved", None).expect("tahap1");
        // manager tak ada -> dilewati; tahap 2 = role hr-administrator (admin)
        decide(&conn, admin(&conn), id, "approved", None).expect("tahap2");
        let status: String = conn
            .query_row(
                "SELECT status FROM leave_requests WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "approved");
        let used: f64 = conn
            .query_row(
                "SELECT used_days FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3",
                params![eid, tid, fwd_monday().year()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(used, 5.0);
        cancel(&conn, uid, eid, id).expect("batal");
        let used: f64 = conn
            .query_row(
                "SELECT used_days FROM leave_balances WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3",
                params![eid, tid, fwd_monday().year()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(used, 0.0);
        let _ = sup_uid;
    }

    #[test]
    fn saldo_kurang_overlap_dan_hari_kerja_ditolak() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let (_, sup_eid) = mkuser(&conn, "cuti-spv3", "EMP-C0", None);
        let (uid, eid) = mkuser(&conn, "cuti-solo", "EMP-C3", Some(sup_eid));
        let tid = al_type(&conn);
        let mon = fwd_monday();
        let id = create(
            &conn,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(mon),
                end_date: ds(mon + chrono::Duration::days(11)),
                reason: None,
            },
        )
        .expect("buat") as i64;
        let e = create(
            &conn,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(mon + chrono::Duration::days(2)),
                end_date: ds(mon + chrono::Duration::days(4)),
                reason: None,
            },
        )
        .expect_err("overlap");
        assert!(e.contains("rentang"), "dapat: {e}");
        let sat = mon + chrono::Duration::days(5);
        let e = create(
            &conn,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(sat),
                end_date: ds(sat + chrono::Duration::days(1)),
                reason: None,
            },
        )
        .expect_err("weekend");
        assert!(e.contains("hari kerja"));
        cancel(&conn, uid, eid, id).expect("batal pending");
        let e = create(
            &conn,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid as i32,
                start_date: ds(mon),
                end_date: ds(mon + chrono::Duration::days(60)),
                reason: None,
            },
        )
        .expect_err("saldo");
        assert!(e.contains("Saldo"));
    }

    #[test]
    fn kalender_dan_tipe_crud() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let adm = admin(&conn);
        let tid = type_save(
            &conn,
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
        .expect("tipe");
        assert!(type_list(&conn)
            .expect("list")
            .iter()
            .any(|t| t.code == "TS"));
        let (uid, eid) = mkuser(&conn, "cuti-ts", "EMP-C4", None);
        let lid = create(
            &conn,
            uid,
            eid,
            &LeaveCreate {
                leave_type_id: tid,
                start_date: "2026-07-06".to_string(),
                end_date: "2026-07-06".to_string(),
                reason: None,
            },
        )
        .expect("pakai") as i64;
        assert!(type_delete(&conn, adm, tid as i64).is_err());
        let _ = lid;
        let cal = calendar(&conn, "2026-07").expect("kalender");
        assert!(cal
            .iter()
            .any(|d| d.kind == "leave" && d.date == "2026-07-06"));
    }

    #[test]
    fn bisnis_days_lewati_akhir_pekan_dan_libur() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        assert_eq!(
            count_business_days(&conn, "2026-06-06", "2026-06-07").unwrap(),
            0
        );
        assert_eq!(
            count_business_days(&conn, "2026-06-01", "2026-06-05").unwrap(),
            5
        );
        conn.execute(
            "INSERT INTO holidays (name, date, type) VALUES ('Uji', '2026-06-03', 'national')",
            [],
        )
        .unwrap();
        assert_eq!(
            count_business_days(&conn, "2026-06-01", "2026-06-05").unwrap(),
            4
        );
    }
}
