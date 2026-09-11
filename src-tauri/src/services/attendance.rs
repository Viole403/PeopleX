//! Absensi: shift, jadwal, libur, clock in/out, koreksi, rekap.

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension};

use super::audit;
use crate::to_dto_int;

// ---------------- DTO ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Shift {
    pub id: i32,
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub break_start: Option<String>,
    pub break_end: Option<String>,
    pub grace_period_minutes: i32,
    pub is_overnight: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ShiftInput {
    pub name: String,
    pub start_time: String,
    pub end_time: String,
    pub break_start: Option<String>,
    pub break_end: Option<String>,
    pub grace_period_minutes: i32,
    pub is_overnight: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ScheduleDay {
    pub day_of_week: i32,
    pub day_name: String,
    pub shift_id: Option<i32>,
    pub is_working_day: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Schedule {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub days: Vec<ScheduleDay>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ScheduleInput {
    pub name: String,
    pub description: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Assignment {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub work_schedule_id: Option<i32>,
    pub schedule_name: Option<String>,
    pub shift_id: Option<i32>,
    pub shift_name: Option<String>,
    pub date: Option<String>,
    pub start_date: String,
    pub end_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct AssignmentInput {
    pub employee_id: i32,
    pub work_schedule_id: Option<i32>,
    pub shift_id: Option<i32>,
    pub date: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Holiday {
    pub id: i32,
    pub name: String,
    pub date: String,
    pub holiday_type: String,
    pub description: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct HolidayInput {
    pub name: String,
    pub date: String,
    pub holiday_type: String,
    pub description: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Attendance {
    pub id: i32,
    pub employee_id: i32,
    pub date: String,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub status: String,
    pub late_minutes: i32,
    pub early_minutes: i32,
    pub work_minutes: i32,
    pub notes: Option<String>,
    pub shift_id: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct RecapRow {
    pub employee_id: i32,
    pub employee_number: String,
    pub name: String,
    pub department_name: Option<String>,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub status: Option<String>,
    pub late_minutes: Option<i32>,
    pub work_minutes: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ManualInput {
    pub employee_id: i32,
    pub date: String,
    pub clock_in: Option<String>,
    pub clock_out: Option<String>,
    pub status: String,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Correction {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub date: String,
    pub requested_clock_in: Option<String>,
    pub requested_clock_out: Option<String>,
    pub reason: String,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct CorrectionInput {
    pub date: String,
    pub requested_clock_in: Option<String>,
    pub requested_clock_out: Option<String>,
    pub reason: String,
}

const DAY_NAMES: [&str; 7] = [
    "Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu",
];

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn valid_time(s: &str) -> bool {
    NaiveDate::parse_from_str("2000-01-01", "%Y-%m-%d").is_ok()
        && chrono::NaiveTime::parse_from_str(s, "%H:%M:%S").is_ok()
}

fn notify(
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

/// Cari user id dari employee (untuk notifikasi).
fn user_of_employee(conn: &Connection, employee_id: i64) -> Result<Option<i64>, String> {
    conn.query_row(
        "SELECT id FROM users WHERE employee_id = ?1 AND status = 'active'",
        params![employee_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| format!("gagal mencari user karyawan: {e}"))
}

// ---------------- Shift ----------------

pub fn shift_list(conn: &Connection) -> Result<Vec<Shift>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, start_time, end_time, break_start, break_end, grace_period_minutes, is_overnight FROM shifts WHERE deleted_at IS NULL ORDER BY name")
        .map_err(|e| format!("gagal menyiapkan daftar shift: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca shift: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, start, end, bs, be, grace, over) =
            row.map_err(|e| format!("gagal membaca baris shift: {e}"))?;
        out.push(Shift {
            id: to_dto_int(id, "shift.id")?,
            name,
            start_time: start,
            end_time: end,
            break_start: bs,
            break_end: be,
            grace_period_minutes: to_dto_int(grace, "shift.grace")?,
            is_overnight: over != 0,
        });
    }
    Ok(out)
}

pub fn shift_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &ShiftInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama shift wajib diisi.".to_string());
    }
    for (v, label) in [
        (&input.start_time, "Jam masuk"),
        (&input.end_time, "Jam pulang"),
    ] {
        if !valid_time(v) {
            return Err(format!("{label} harus format JJ:MM:SS."));
        }
    }
    for (v, label) in [
        (&input.break_start, "Istirahat mulai"),
        (&input.break_end, "Istirahat selesai"),
    ] {
        if let Some(s) = v {
            if !s.trim().is_empty() && !valid_time(s.trim()) {
                return Err(format!("{label} harus format JJ:MM:SS."));
            }
        }
    }
    if input.grace_period_minutes < 0 || input.grace_period_minutes > 180 {
        return Err("Toleransi 0-180 menit.".to_string());
    }
    let over = if input.is_overnight { 1 } else { 0 };
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE shifts SET name = ?1, start_time = ?2, end_time = ?3, break_start = ?4, break_end = ?5, grace_period_minutes = ?6, is_overnight = ?7 WHERE id = ?8 AND deleted_at IS NULL",
            params![input.name.trim(), input.start_time, input.end_time, input.break_start.as_deref().map(str::trim).filter(|s| !s.is_empty()), input.break_end.as_deref().map(str::trim).filter(|s| !s.is_empty()), input.grace_period_minutes, over, rid],
        ).map_err(|e| format!("gagal menyimpan shift: {e}"))?;
        if n == 0 {
            return Err("Shift tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "schedule.shift",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "shift.id")
    } else {
        conn.execute(
            "INSERT INTO shifts (name, start_time, end_time, break_start, break_end, grace_period_minutes, is_overnight) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![input.name.trim(), input.start_time, input.end_time, input.break_start.as_deref().map(str::trim).filter(|s| !s.is_empty()), input.break_end.as_deref().map(str::trim).filter(|s| !s.is_empty()), input.grace_period_minutes, over],
        ).map_err(|e| format!("gagal menambah shift: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "schedule.shift",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "shift.id")
    }
}

pub fn shift_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    for (table, col) in [
        ("work_schedule_days", "shift_id"),
        ("shift_assignments", "shift_id"),
        ("attendances", "shift_id"),
    ] {
        let n: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {col} = ?1"),
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
        if n > 0 {
            return Err("Shift masih dipakai dan tidak dapat dihapus.".to_string());
        }
    }
    let n = conn.execute(
        "UPDATE shifts SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus shift: {e}"))?;
    if n == 0 {
        return Err("Shift tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "schedule.shift",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Jadwal ----------------

pub fn schedule_list(conn: &Connection) -> Result<Vec<Schedule>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, description FROM work_schedules WHERE deleted_at IS NULL ORDER BY name")
        .map_err(|e| format!("gagal menyiapkan jadwal: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca jadwal: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, desc) = row.map_err(|e| format!("gagal membaca baris jadwal: {e}"))?;
        out.push(Schedule {
            id: to_dto_int(id, "schedule.id")?,
            name,
            description: desc,
            days: schedule_days(conn, id)?,
        });
    }
    Ok(out)
}

pub fn schedule_days(conn: &Connection, schedule_id: i64) -> Result<Vec<ScheduleDay>, String> {
    let mut stmt = conn
        .prepare("SELECT day_of_week, shift_id, is_working_day FROM work_schedule_days WHERE work_schedule_id = ?1 ORDER BY day_of_week")
        .map_err(|e| format!("gagal menyiapkan hari jadwal: {e}"))?;
    let rows = stmt
        .query_map(params![schedule_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca hari: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (dow, shift, working) = row.map_err(|e| format!("gagal membaca baris hari: {e}"))?;
        out.push(ScheduleDay {
            day_of_week: to_dto_int(dow, "day.dow")?,
            day_name: DAY_NAMES[dow.clamp(0, 6) as usize].to_string(),
            shift_id: shift.map(|v| to_dto_int(v, "day.shift")).transpose()?,
            is_working_day: working != 0,
        });
    }
    Ok(out)
}

pub fn schedule_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &ScheduleInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama jadwal wajib diisi.".to_string());
    }
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE work_schedules SET name = ?1, description = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![input.name.trim(), input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()), rid],
        ).map_err(|e| format!("gagal menyimpan jadwal: {e}"))?;
        if n == 0 {
            return Err("Jadwal tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "schedule.work",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "schedule.id")
    } else {
        conn.execute(
            "INSERT INTO work_schedules (name, description) VALUES (?1, ?2)",
            params![
                input.name.trim(),
                input
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            ],
        )
        .map_err(|e| format!("gagal menambah jadwal: {e}"))?;
        let rid = conn.last_insert_rowid();
        for dow in 0..=6 {
            let working = (1..=5).contains(&dow) as i64;
            conn.execute(
                "INSERT INTO work_schedule_days (work_schedule_id, day_of_week, shift_id, is_working_day) VALUES (?1, ?2, NULL, ?3)",
                params![rid, dow, working],
            ).map_err(|e| format!("gagal membuat hari default: {e}"))?;
        }
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "schedule.work",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "schedule.id")
    }
}

/// Simpan hari: daftar (day_of_week, shift_id|None, is_working).
pub fn schedule_save_days(
    conn: &Connection,
    actor_id: i64,
    schedule_id: i64,
    days: &[(i64, Option<i64>, bool)],
) -> Result<(), String> {
    for (dow, shift, working) in days {
        if !(0..=6).contains(dow) {
            return Err("Hari tidak valid.".to_string());
        }
        if let Some(sid) = shift {
            let found: Option<i64> = conn
                .query_row(
                    "SELECT id FROM shifts WHERE id = ?1 AND deleted_at IS NULL",
                    params![sid],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| format!("gagal memeriksa shift: {e}"))?;
            if found.is_none() {
                return Err("Shift tidak ditemukan.".to_string());
            }
        }
        conn.execute(
            "UPDATE work_schedule_days SET shift_id = ?1, is_working_day = ?2 WHERE work_schedule_id = ?3 AND day_of_week = ?4",
            params![shift, if *working { 1 } else { 0 }, schedule_id, dow],
        ).map_err(|e| format!("gagal menyimpan hari: {e}"))?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "schedule.days",
        Some(&schedule_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn schedule_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM shift_assignments WHERE work_schedule_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if n > 0 {
        return Err("Jadwal masih dipakai penugasan.".to_string());
    }
    conn.execute(
        "DELETE FROM work_schedule_days WHERE work_schedule_id = ?1",
        params![id],
    )
    .map_err(|e| format!("gagal menghapus hari: {e}"))?;
    let n = conn.execute(
        "UPDATE work_schedules SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus jadwal: {e}"))?;
    if n == 0 {
        return Err("Jadwal tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "schedule.work",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Penugasan ----------------

pub fn assignment_list(conn: &Connection) -> Result<Vec<Assignment>, String> {
    let mut stmt = conn
        .prepare("SELECT sa.id, sa.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), sa.work_schedule_id, ws.name, sa.shift_id, s.name, sa.date, sa.start_date, sa.end_date
                  FROM shift_assignments sa
                  JOIN employees e ON e.id = sa.employee_id
                  LEFT JOIN work_schedules ws ON ws.id = sa.work_schedule_id
                  LEFT JOIN shifts s ON s.id = sa.shift_id
                  ORDER BY sa.start_date DESC, e.first_name")
        .map_err(|e| format!("gagal menyiapkan penugasan: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })
        .map_err(|e| format!("gagal membaca penugasan: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, ws, ws_name, sh, sh_name, date, start, end) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "assign.ref")).transpose();
        out.push(Assignment {
            id: to_dto_int(id, "assign.id")?,
            employee_id: to_dto_int(emp, "assign.emp")?,
            employee_name: name,
            work_schedule_id: opt_i(ws)?,
            schedule_name: ws_name,
            shift_id: opt_i(sh)?,
            shift_name: sh_name,
            date,
            start_date: start,
            end_date: end,
        });
    }
    Ok(out)
}

pub fn assignment_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &AssignmentInput,
) -> Result<i32, String> {
    let emp: Option<i64> = conn
        .query_row(
            "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![input.employee_id as i64],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    if input.work_schedule_id.is_none() && input.shift_id.is_none() {
        return Err("Pilih jadwal atau shift.".to_string());
    }
    // tanggal spesifik ATAU rentang; bila keduanya kosong tolak
    let date = input
        .date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let start = input
        .start_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if date.is_none() && start.is_none() {
        return Err("Isi tanggal spesifik atau tanggal mulai rentang.".to_string());
    }
    for (v, label) in [
        (date, "Tanggal"),
        (start, "Tanggal mulai"),
        (
            input
                .end_date
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
            "Tanggal selesai",
        ),
    ] {
        if let Some(s) = v {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{label} harus valid (YYYY-MM-DD)."))?;
        }
    }
    if let (Some(s), Some(e)) = (
        start,
        input
            .end_date
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty()),
    ) {
        if e < s {
            return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
        }
    }
    let end = input
        .end_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let ws = input.work_schedule_id.map(|v| v as i64);
    let sh = input.shift_id.map(|v| v as i64);
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE shift_assignments SET work_schedule_id = ?1, shift_id = ?2, date = ?3, start_date = ?4, end_date = ?5 WHERE id = ?6 AND employee_id = ?7",
            params![ws, sh, date, start.unwrap_or(""), end, rid, input.employee_id as i64],
        ).map_err(|e| format!("gagal menyimpan penugasan: {e}"))?;
        if n == 0 {
            return Err("Penugasan tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "schedule.assign",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "assign.id")
    } else {
        conn.execute(
            "INSERT INTO shift_assignments (employee_id, work_schedule_id, shift_id, date, start_date, end_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![input.employee_id as i64, ws, sh, date, start.unwrap_or(""), end],
        ).map_err(|e| format!("gagal menambah penugasan: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "schedule.assign",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "assign.id")
    }
}

pub fn assignment_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM shift_assignments WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus penugasan: {e}"))?;
    if n == 0 {
        return Err("Penugasan tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "schedule.assign",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Libur ----------------

pub fn holiday_list(conn: &Connection) -> Result<Vec<Holiday>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, date, type, description FROM holidays ORDER BY date DESC")
        .map_err(|e| format!("gagal menyiapkan libur: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca libur: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, date, htype, desc) =
            row.map_err(|e| format!("gagal membaca baris libur: {e}"))?;
        out.push(Holiday {
            id: to_dto_int(id, "holiday.id")?,
            name,
            date,
            holiday_type: htype,
            description: desc,
        });
    }
    Ok(out)
}

pub fn holiday_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &HolidayInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama libur wajib diisi.".to_string());
    }
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal harus valid (YYYY-MM-DD).".to_string())?;
    if !["national", "company", "collective_leave", "custom"].contains(&input.holiday_type.as_str())
    {
        return Err("Jenis libur tidak valid.".to_string());
    }
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE holidays SET name = ?1, date = ?2, type = ?3, description = ?4 WHERE id = ?5",
            params![input.name.trim(), input.date.trim(), input.holiday_type, input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()), rid],
        ).map_err(|e| format!("gagal menyimpan libur: {e}"))?;
        if n == 0 {
            return Err("Libur tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "schedule.holiday",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "holiday.id")
    } else {
        conn.execute(
            "INSERT INTO holidays (name, date, type, description) VALUES (?1, ?2, ?3, ?4)",
            params![
                input.name.trim(),
                input.date.trim(),
                input.holiday_type,
                input
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            ],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Libur tanggal dan nama tersebut sudah ada.".to_string()
            } else {
                format!("gagal menambah libur: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "schedule.holiday",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "holiday.id")
    }
}

pub fn holiday_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM holidays WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus libur: {e}"))?;
    if n == 0 {
        return Err("Libur tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "schedule.holiday",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Resolusi shift ----------------

struct ResolvedShift {
    id: i64,
    start: NaiveDateTime,
    end: NaiveDateTime,
    grace: i64,
}

fn parse_hm(date: NaiveDate, hm: &str) -> Result<NaiveDateTime, String> {
    let t = chrono::NaiveTime::parse_from_str(hm.trim(), "%H:%M:%S")
        .map_err(|_| "Jam shift tidak valid.".to_string())?;
    Ok(date.and_time(t))
}

/// Override tanggal-spesifik menang atas jadwal mingguan dalam rentang.
fn resolve_shift(
    conn: &Connection,
    employee_id: i64,
    date: &str,
) -> Result<Option<ResolvedShift>, String> {
    let day: NaiveDate = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Tanggal tidak valid.".to_string())?;
    if let Some((sid, start, end, grace, over)) = conn
        .query_row(
            "SELECT s.id, s.start_time, s.end_time, s.grace_period_minutes, s.is_overnight
             FROM shift_assignments sa INNER JOIN shifts s ON s.id = sa.shift_id
             WHERE sa.employee_id = ?1 AND sa.date = ?2 AND s.deleted_at IS NULL LIMIT 1",
            params![employee_id, date],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal resolve shift: {e}"))?
    {
        let start = parse_hm(day, &start)?;
        let mut end = parse_hm(day, &end)?;
        if over != 0 {
            end += chrono::Duration::days(1);
        }
        return Ok(Some(ResolvedShift {
            id: sid,
            start,
            end,
            grace,
        }));
    }
    let dow = day.weekday().num_days_from_sunday() as i64;
    let found: Option<(i64, String, String, i64, i64)> = conn
        .query_row(
            "SELECT s.id, s.start_time, s.end_time, s.grace_period_minutes, s.is_overnight
             FROM shift_assignments sa
             INNER JOIN work_schedules ws ON ws.id = sa.work_schedule_id
             INNER JOIN work_schedule_days wsd ON wsd.work_schedule_id = ws.id AND wsd.day_of_week = ?1
             INNER JOIN shifts s ON s.id = wsd.shift_id
             WHERE sa.employee_id = ?2 AND sa.date IS NULL
               AND sa.start_date <= ?3 AND (sa.end_date IS NULL OR sa.end_date >= ?3)
               AND wsd.is_working_day = 1 AND s.deleted_at IS NULL
             ORDER BY sa.start_date DESC LIMIT 1",
            params![dow, employee_id, date],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| format!("gagal resolve jadwal: {e}"))?;
    found
        .map(|(sid, start, end, grace, over)| {
            let s = parse_hm(day, &start)?;
            let mut e = parse_hm(day, &end)?;
            if over != 0 {
                e += chrono::Duration::days(1);
            }
            Ok(ResolvedShift {
                id: sid,
                start: s,
                end: e,
                grace,
            })
        })
        .transpose()
}

fn is_holiday(conn: &Connection, date: &str) -> Result<bool, String> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT id FROM holidays WHERE date = ?1",
            params![date],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa libur: {e}"))?;
    Ok(found.is_some())
}

fn active_employee(conn: &Connection, id: i64) -> Result<(), String> {
    let status: Option<String> = conn
        .query_row(
            "SELECT employment_status FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    match status.as_deref() {
        Some("active") => Ok(()),
        _ => Err("Karyawan tidak aktif, tidak dapat absensi.".to_string()),
    }
}

// ---------------- Clock in/out ----------------

/// Hasil clock in/out: status + pesan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ClockResult {
    pub status: String,
    pub message: String,
}

fn read_attendance(conn: &Connection, id: i64) -> Result<Attendance, String> {
    conn.query_row(
        "SELECT id, employee_id, date, clock_in, clock_out, status, late_minutes, early_minutes, work_minutes, notes, shift_id FROM attendances WHERE id = ?1",
        params![id],
        |r| {
            let aid: i64 = r.get(0)?;
            let emp: i64 = r.get(1)?;
            let late: i64 = r.get(6)?;
            let early: i64 = r.get(7)?;
            let work: i64 = r.get(8)?;
            let shift: Option<i64> = r.get(10)?;
            Ok(Attendance {
                id: to_dto_int(aid, "attendance.id")
                    .map_err(rusqlite::Error::InvalidColumnName)?,
                employee_id: to_dto_int(emp, "attendance.emp")
                    .map_err(rusqlite::Error::InvalidColumnName)?,
                date: r.get(2)?,
                clock_in: r.get(3)?,
                clock_out: r.get(4)?,
                status: r.get(5)?,
                late_minutes: to_dto_int(late, "attendance.late")
                    .map_err(rusqlite::Error::InvalidColumnName)?,
                early_minutes: to_dto_int(early, "attendance.early")
                    .map_err(rusqlite::Error::InvalidColumnName)?,
                work_minutes: to_dto_int(work, "attendance.work")
                    .map_err(rusqlite::Error::InvalidColumnName)?,
                notes: r.get(9)?,
                shift_id: shift
                    .map(|v| to_dto_int(v, "attendance.shift"))
                    .transpose()
                    .map_err(rusqlite::Error::InvalidColumnName)?,
            })
        },
    )
    .map_err(|e| format!("gagal memuat absensi: {e}"))
}

pub fn today(conn: &Connection, employee_id: i64) -> Result<Option<Attendance>, String> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2",
            params![employee_id, today_str()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat absensi hari ini: {e}"))?;
    id.map(|v| read_attendance(conn, v)).transpose()
}

pub fn clock_in(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    lat: Option<f64>,
    lng: Option<f64>,
    device: Option<&str>,
) -> Result<ClockResult, String> {
    active_employee(conn, employee_id)?;
    let today = today_str();
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2",
            params![employee_id, today],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa absensi: {e}"))?;
    if exists.is_some() {
        return Err("Sudah clock in hari ini.".to_string());
    }
    if is_holiday(conn, &today)? {
        return Err("Hari ini libur.".to_string());
    }
    let now = Local::now().naive_local();
    let shift = resolve_shift(conn, employee_id, &today)?;
    let (status, late) = match &shift {
        Some(s) => {
            let limit = s.start + chrono::Duration::minutes(s.grace);
            if now > limit {
                let mins = ((now - limit).num_seconds() + 59) / 60;
                ("late".to_string(), mins)
            } else {
                ("present".to_string(), 0)
            }
        }
        None => ("present".to_string(), 0),
    };
    conn.execute(
        "INSERT INTO attendances (employee_id, date, clock_in, clock_in_lat, clock_in_lng, clock_in_device, shift_id, status, late_minutes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            employee_id,
            today,
            now.format("%Y-%m-%d %H:%M:%S").to_string(),
            lat,
            lng,
            device.map(str::trim).filter(|s| !s.is_empty()),
            shift.as_ref().map(|s| s.id),
            status,
            late,
        ],
    )
    .map_err(|e| format!("gagal clock in: {e}"))?;
    let id = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CLOCK_IN",
        "attendance",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(ClockResult {
        message: if status == "late" {
            format!("Clock in berhasil. Terlambat {late} menit.")
        } else {
            "Clock in berhasil.".to_string()
        },
        status,
    })
}

pub fn clock_out(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    lat: Option<f64>,
    lng: Option<f64>,
    device: Option<&str>,
) -> Result<ClockResult, String> {
    let today = today_str();
    let row: Option<(i64, String, Option<String>, String, Option<i64>)> = conn
        .query_row(
            "SELECT id, clock_in, clock_out, status, shift_id FROM attendances WHERE employee_id = ?1 AND date = ?2",
            params![employee_id, today],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat absensi: {e}"))?;
    let Some((aid, clock_in, clock_out, status, shift_id)) = row else {
        return Err("Belum clock in hari ini.".to_string());
    };
    if clock_out.is_some() {
        return Err("Sudah clock out hari ini.".to_string());
    }
    let now = Local::now().naive_local();
    let fmt = "%Y-%m-%d %H:%M:%S";
    let cin = NaiveDateTime::parse_from_str(&clock_in, fmt)
        .map_err(|_| "Data clock in rusak.".to_string())?;
    let work = (now - cin).num_minutes().max(0);
    let (status, early) = match shift_id {
        Some(sid) => {
            let shift: Option<(String, String, i64)> = conn
                .query_row(
                    "SELECT start_time, end_time, is_overnight FROM shifts WHERE id = ?1",
                    params![sid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()
                .map_err(|e| format!("gagal memuat shift: {e}"))?;
            match shift {
                Some((_, end_s, over)) => {
                    let day = NaiveDate::parse_from_str(&today, "%Y-%m-%d")
                        .map_err(|_| "Tanggal rusak.".to_string())?;
                    let end_t = chrono::NaiveTime::parse_from_str(&end_s, "%H:%M:%S")
                        .map_err(|_| "Jam shift rusak.".to_string())?;
                    let mut end = day.and_time(end_t);
                    if over != 0 {
                        end += chrono::Duration::days(1);
                    }
                    if now < end {
                        let mins = ((end - now).num_seconds() + 59) / 60;
                        let st = if status == "present" {
                            "early_checkout".to_string()
                        } else {
                            status
                        };
                        (st, mins)
                    } else {
                        (status, 0)
                    }
                }
                None => (status, 0),
            }
        }
        None => (status, 0),
    };
    conn.execute(
        "UPDATE attendances SET clock_out = ?1, clock_out_lat = ?2, clock_out_lng = ?3, clock_out_device = ?4, work_minutes = ?5, early_minutes = ?6, status = ?7 WHERE id = ?8",
        params![
            now.format("%Y-%m-%d %H:%M:%S").to_string(),
            lat,
            lng,
            device.map(str::trim).filter(|s| !s.is_empty()),
            work,
            early,
            status,
            aid,
        ],
    )
    .map_err(|e| format!("gagal clock out: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "CLOCK_OUT",
        "attendance",
        Some(&aid.to_string()),
        None,
        None,
        None,
    )?;
    Ok(ClockResult {
        status,
        message: "Clock out berhasil.".to_string(),
    })
}

// ---------------- Riwayat dan rekap ----------------

pub fn history(
    conn: &Connection,
    employee_id: i64,
    month: &str,
) -> Result<Vec<Attendance>, String> {
    if NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").is_err() {
        return Err("Bulan harus format YYYY-MM.".to_string());
    }
    let mut stmt = conn
        .prepare("SELECT id FROM attendances WHERE employee_id = ?1 AND substr(date, 1, 7) = ?2 ORDER BY date DESC")
        .map_err(|e| format!("gagal menyiapkan riwayat: {e}"))?;
    let rows = stmt
        .query_map(params![employee_id, month], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("gagal membaca riwayat: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let id = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(read_attendance(conn, id)?);
    }
    Ok(out)
}

pub fn recap(conn: &Connection, date: &str, search: &str) -> Result<Vec<RecapRow>, String> {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Tanggal harus valid (YYYY-MM-DD).".to_string())?;
    let search = search.trim();
    let has_search = !search.is_empty();
    let base =
        "SELECT e.id, e.employee_number, e.first_name || ' ' || COALESCE(e.last_name, ''), d.name,
                       a.clock_in, a.clock_out, a.status, a.late_minutes, a.work_minutes
                FROM employees e
                LEFT JOIN departments d ON d.id = e.department_id
                LEFT JOIN attendances a ON a.employee_id = e.id AND a.date = ?1
                WHERE e.deleted_at IS NULL";
    let sql = if has_search {
        format!("{base} AND (e.first_name LIKE ?2 OR e.last_name LIKE ?2 OR e.employee_number LIKE ?2) ORDER BY e.first_name ASC")
    } else {
        format!("{base} ORDER BY e.first_name ASC")
    };
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("gagal menyiapkan rekap: {e}"))?;
    let like = format!("%{search}%");
    let map_row = |r: &rusqlite::Row<'_>| {
        let eid: i64 = r.get(0)?;
        let late: Option<i64> = r.get(7)?;
        let work: Option<i64> = r.get(8)?;
        Ok((
            eid,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
            late,
            work,
        ))
    };
    let rows: Vec<(
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
    )> = if has_search {
        stmt.query_map(params![date, like], map_row)
            .map_err(|e| format!("gagal membaca rekap: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("gagal membaca baris rekap: {e}"))?
    } else {
        stmt.query_map(params![date], map_row)
            .map_err(|e| format!("gagal membaca rekap: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("gagal membaca baris rekap: {e}"))?
    };
    let mut out = Vec::new();
    for (eid, number, name, dept, cin, cout, status, late, work) in rows {
        let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "recap.num")).transpose();
        out.push(RecapRow {
            employee_id: to_dto_int(eid, "recap.emp")?,
            employee_number: number,
            name,
            department_name: dept,
            clock_in: cin,
            clock_out: cout,
            status,
            late_minutes: opt_i(late)?,
            work_minutes: opt_i(work)?,
        });
    }
    Ok(out)
}

const MANUAL_STATUSES: &[&str] = &[
    "present",
    "late",
    "absent",
    "sick",
    "permission",
    "leave",
    "wfh",
    "business_trip",
    "early_checkout",
];

pub fn manual_entry(conn: &Connection, actor_id: i64, input: &ManualInput) -> Result<i32, String> {
    let emp: Option<i64> = conn
        .query_row(
            "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![input.employee_id as i64],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal harus valid (YYYY-MM-DD).".to_string())?;
    if !MANUAL_STATUSES.contains(&input.status.as_str()) {
        return Err("Status tidak valid.".to_string());
    }
    let with_date = |t: Option<&String>| {
        t.map(|x| x.trim())
            .filter(|s| !s.is_empty())
            .map(|s| format!("{} {s}", input.date.trim()))
    };
    let cin = with_date(input.clock_in.as_ref());
    let cout = with_date(input.clock_out.as_ref());
    for (v, label) in [(&cin, "Jam masuk"), (&cout, "Jam pulang")] {
        if let Some(s) = v {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M")
                .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S"))
                .map_err(|_| format!("{label} harus format JJ:MM."))?;
        }
    }
    if let Some(n) = input.notes.as_deref() {
        if n.len() > 255 {
            return Err("Catatan maksimal 255 karakter.".to_string());
        }
    }
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2",
            params![input.employee_id as i64, input.date.trim()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa absensi: {e}"))?;
    if let Some(rid) = existing {
        conn.execute(
            "UPDATE attendances SET clock_in = ?1, clock_out = ?2, status = ?3, notes = ?4 WHERE id = ?5",
            params![
                cin,
                cout,
                input.status,
                input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                rid
            ],
        )
        .map_err(|e| format!("gagal memperbarui absensi: {e}"))?;
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "attendance.manual",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "attendance.id")
    } else {
        conn.execute(
            "INSERT INTO attendances (employee_id, date, clock_in, clock_out, status, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                input.employee_id as i64,
                input.date.trim(),
                cin,
                cout,
                input.status,
                input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty())
            ],
        )
        .map_err(|e| format!("gagal mencatat absensi: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "attendance.manual",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "attendance.id")
    }
}

// ---------------- Koreksi ----------------

fn correction_row(
    id: i64,
    emp: i64,
    name: String,
    date: String,
    cin: Option<String>,
    cout: Option<String>,
    reason: String,
    status: String,
    notes: Option<String>,
    created: String,
) -> Result<Correction, String> {
    Ok(Correction {
        id: to_dto_int(id, "correction.id")?,
        employee_id: to_dto_int(emp, "correction.emp")?,
        employee_name: name,
        date,
        requested_clock_in: cin,
        requested_clock_out: cout,
        reason,
        status,
        notes,
        created_at: created,
    })
}

fn read_corrections(
    conn: &Connection,
    where_sql: &str,
    param: i64,
) -> Result<Vec<Correction>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''),
                    c.date, c.requested_clock_in, c.requested_clock_out, c.reason, c.status, c.notes, c.created_at
             FROM attendance_corrections c
             JOIN employees e ON e.id = c.employee_id
             WHERE {where_sql} ORDER BY c.id DESC"
        ))
        .map_err(|e| format!("gagal menyiapkan koreksi: {e}"))?;
    let rows = stmt
        .query_map(params![param], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })
        .map_err(|e| format!("gagal membaca koreksi: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, date, cin, cout, reason, status, notes, created) =
            row.map_err(|e| format!("gagal membaca baris koreksi: {e}"))?;
        out.push(correction_row(
            id, emp, name, date, cin, cout, reason, status, notes, created,
        )?);
    }
    Ok(out)
}

pub fn my_corrections(conn: &Connection, employee_id: i64) -> Result<Vec<Correction>, String> {
    read_corrections(conn, "c.employee_id = ?1", employee_id)
}

pub fn pending_corrections(conn: &Connection) -> Result<Vec<Correction>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''),
                    c.date, c.requested_clock_in, c.requested_clock_out, c.reason, c.status, c.notes, c.created_at
             FROM attendance_corrections c
             JOIN employees e ON e.id = c.employee_id
             WHERE c.status = 'pending' ORDER BY c.id DESC",
        )
        .map_err(|e| format!("gagal menyiapkan koreksi: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, String>(9)?,
            ))
        })
        .map_err(|e| format!("gagal membaca koreksi: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, date, cin, cout, reason, status, notes, created) =
            row.map_err(|e| format!("gagal membaca baris koreksi: {e}"))?;
        out.push(correction_row(
            id, emp, name, date, cin, cout, reason, status, notes, created,
        )?);
    }
    Ok(out)
}

fn with_date(date: &str, time: Option<&String>) -> Result<Option<String>, String> {
    match time.map(|x| x.trim()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(t) => {
            let full = format!("{date} {t}");
            NaiveDateTime::parse_from_str(&full, "%Y-%m-%d %H:%M")
                .or_else(|_| NaiveDateTime::parse_from_str(&full, "%Y-%m-%d %H:%M:%S"))
                .map_err(|_| "Jam koreksi harus format JJ:MM.".to_string())?;
            Ok(Some(full))
        }
    }
}

pub fn request_correction(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    input: &CorrectionInput,
) -> Result<i32, String> {
    active_employee(conn, employee_id)?;
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal harus valid (YYYY-MM-DD).".to_string())?;
    if input.reason.trim().is_empty() {
        return Err("Alasan wajib diisi.".to_string());
    }
    if input.reason.len() > 255 {
        return Err("Alasan maksimal 255 karakter.".to_string());
    }
    let cin = with_date(input.date.trim(), input.requested_clock_in.as_ref())?;
    let cout = with_date(input.date.trim(), input.requested_clock_out.as_ref())?;
    let att: Option<i64> = conn
        .query_row(
            "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2",
            params![employee_id, input.date.trim()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa absensi: {e}"))?;
    conn.execute(
        "INSERT INTO attendance_corrections (employee_id, attendance_id, date, requested_clock_in, requested_clock_out, reason, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')",
        params![employee_id, att, input.date.trim(), cin, cout, input.reason.trim()],
    )
    .map_err(|e| format!("gagal mengajukan koreksi: {e}"))?;
    let rid = conn.last_insert_rowid();
    // beri tahu supervisor bila ada
    let sup: Option<i64> = conn
        .query_row(
            "SELECT supervisor_id FROM employees WHERE id = ?1",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat supervisor: {e}"))?
        .flatten();
    if let Some(sid) = sup {
        if let Some(uid) = user_of_employee(conn, sid)? {
            notify(
                conn,
                uid,
                "attendance_correction",
                "Pengajuan Koreksi Absensi",
                "Ada pengajuan koreksi absensi yang menunggu persetujuan Anda.",
                "/attendance",
            )?;
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "attendance.correction",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "correction.id")
}

/// Putuskan koreksi. approver = supervisor karyawan ybs atau pemegang izin.
pub fn decide_correction(
    conn: &Connection,
    approver_user_id: i64,
    approver_employee_id: Option<i64>,
    is_privileged: bool,
    correction_id: i64,
    decision: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let row: Option<(i64, String, String, Option<String>, Option<String>, Option<i64>)> = conn
        .query_row(
            "SELECT employee_id, date, status, requested_clock_in, requested_clock_out, attendance_id FROM attendance_corrections WHERE id = ?1",
            params![correction_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat koreksi: {e}"))?;
    let Some((emp_id, date, status, req_in, req_out, _att)) = row else {
        return Err("Data koreksi tidak ditemukan.".to_string());
    };
    if status != "pending" {
        return Err("Koreksi sudah diproses.".to_string());
    }
    let supervisor: Option<i64> = conn
        .query_row(
            "SELECT supervisor_id FROM employees WHERE id = ?1",
            params![emp_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat supervisor: {e}"))?
        .flatten();
    let is_supervisor = approver_employee_id == supervisor;
    if !is_supervisor && !is_privileged {
        return Err("Hanya supervisor atau HR yang boleh memutuskan.".to_string());
    }
    let now = now_str();
    conn.execute(
        "UPDATE attendance_corrections SET status = ?1, approved_by = ?2, approved_at = ?3, notes = ?4 WHERE id = ?5",
        params![
            decision,
            approver_user_id,
            now,
            notes.map(str::trim).filter(|s| !s.is_empty()),
            correction_id
        ],
    )
    .map_err(|e| format!("gagal menyimpan keputusan: {e}"))?;
    if decision == "approved" {
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2",
                params![emp_id, date],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memeriksa absensi: {e}"))?;
        match existing {
            Some(aid) => {
                conn.execute(
                    "UPDATE attendances SET clock_in = COALESCE(?1, clock_in), clock_out = COALESCE(?2, clock_out) WHERE id = ?3",
                    params![req_in, req_out, aid],
                )
                .map_err(|e| format!("gagal menerapkan koreksi: {e}"))?;
            }
            None => {
                conn.execute(
                    "INSERT INTO attendances (employee_id, date, clock_in, clock_out, status) VALUES (?1, ?2, ?3, ?4, 'present')",
                    params![emp_id, date, req_in, req_out],
                )
                .map_err(|e| format!("gagal menerapkan koreksi: {e}"))?;
            }
        }
    }
    if let Some(uid) = user_of_employee(conn, emp_id)? {
        notify(
            conn,
            uid,
            "attendance_correction",
            if decision == "approved" {
                "Koreksi Absensi Disetujui"
            } else {
                "Koreksi Absensi Ditolak"
            },
            &format!("Pengajuan koreksi absensi tanggal {date} telah {decision}."),
            "/attendance",
        )?;
    }
    audit::log(
        conn,
        Some(approver_user_id),
        decision.to_uppercase().as_str(),
        "attendance.correction",
        Some(&correction_id.to_string()),
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

    fn live() -> (tempfile::TempDir, crate::db::DbPool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool, files)
    }

    fn actor(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .expect("admin")
    }

    fn admin_employee(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            [],
            |r| r.get(0),
        )
        .expect("emp")
    }

    fn make_employee(conn: &Connection, number: &str, supervisor: Option<i64>) -> i64 {
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, supervisor_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, ?2, '2026-01-01', 'active', 'permanent')",
            params![number, supervisor],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn shift_around_now(conn: &Connection, name: &str, back_h: i64, fwd_h: i64) -> i64 {
        let now = Local::now().naive_local();
        let start = (now - chrono::Duration::hours(back_h))
            .format("%H:%M:%S")
            .to_string();
        let end = (now + chrono::Duration::hours(fwd_h))
            .format("%H:%M:%S")
            .to_string();
        shift_save(
            conn,
            1,
            None,
            &ShiftInput {
                name: name.to_string(),
                start_time: start,
                end_time: end,
                break_start: None,
                break_end: None,
                grace_period_minutes: 15,
                is_overnight: false,
            },
        )
        .expect("shift") as i64
    }

    fn assign_schedule(conn: &Connection, emp: i64, shift: i64) {
        let sched = schedule_save(
            conn,
            1,
            None,
            &ScheduleInput {
                name: "Uji".to_string(),
                description: None,
            },
        )
        .expect("jadwal") as i64;
        let days: Vec<(i64, Option<i64>, bool)> = (0..=6).map(|d| (d, Some(shift), true)).collect();
        schedule_save_days(conn, 1, sched, &days).expect("hari");
        let today = today_str();
        conn.execute(
            "INSERT INTO shift_assignments (employee_id, work_schedule_id, start_date) VALUES (?1, ?2, ?3)",
            params![emp, sched, today],
        )
        .unwrap();
    }

    #[test]
    fn shift_crud_dan_guard() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let sid = shift_save(
            &conn,
            1,
            None,
            &ShiftInput {
                name: "Pagi".to_string(),
                start_time: "08:00:00".to_string(),
                end_time: "17:00:00".to_string(),
                break_start: None,
                break_end: None,
                grace_period_minutes: 10,
                is_overnight: false,
            },
        )
        .expect("buat");
        assert_eq!(shift_list(&conn).expect("list").len(), 3);
        assert!(shift_save(
            &conn,
            1,
            None,
            &ShiftInput {
                name: "X".to_string(),
                start_time: "25:00:00".to_string(),
                end_time: "17:00:00".to_string(),
                break_start: None,
                break_end: None,
                grace_period_minutes: 0,
                is_overnight: false,
            },
        )
        .is_err());
        // pakai di jadwal lalu hapus harus ditolak
        let sched = schedule_save(
            &conn,
            1,
            None,
            &ScheduleInput {
                name: "S".to_string(),
                description: None,
            },
        )
        .expect("jadwal") as i64;
        schedule_save_days(&conn, 1, sched, &[(1, Some(sid as i64), true)]).expect("hari");
        assert!(shift_delete(&conn, 1, sid as i64).is_err());
    }

    #[test]
    fn resolve_override_menang_atas_mingguan() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let emp = admin_employee(&conn);
        let weekly = shift_around_now(&conn, "Mingguan", 1, 8);
        assign_schedule(&conn, emp, weekly);
        let today = today_str();
        let got = resolve_shift(&conn, emp, &today)
            .expect("resolve")
            .expect("ada");
        assert_eq!(got.id, weekly);
        // override tanggal spesifik
        let special = shift_around_now(&conn, "Khusus", 1, 8);
        conn.execute(
            "INSERT INTO shift_assignments (employee_id, shift_id, date, start_date) VALUES (?1, ?2, ?3, ?3)",
            params![emp, special, today],
        )
        .unwrap();
        let got2 = resolve_shift(&conn, emp, &today)
            .expect("resolve")
            .expect("ada");
        assert_eq!(got2.id, special);
    }

    #[test]
    fn clock_in_out_menghitung_menit() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let actor = actor(&conn);
        let emp = make_employee(&conn, "EMP-T2", None);
        // tanpa shift: present
        let r = clock_in(&conn, actor, emp, None, None, None).expect("in");
        assert_eq!(r.status, "present");
        assert!(clock_in(&conn, actor, emp, None, None, None).is_err());
        let r = clock_out(&conn, actor, emp, None, None, None).expect("out");
        assert_eq!(r.status, "present");
        assert!(clock_out(&conn, actor, emp, None, None, None).is_err());
        // dengan shift (masuk 2 jam lalu, pulang 2 jam lagi): late + early
        let emp2 = make_employee(&conn, "EMP-T3", None);
        let shift = shift_around_now(&conn, "Siang", 2, 2);
        assign_schedule(&conn, emp2, shift);
        let r = clock_in(&conn, actor, emp2, Some(-6.2), Some(106.8), None).expect("in2");
        assert_eq!(r.status, "late");
        let att = today(&conn, emp2).expect("today").expect("ada");
        assert!(att.late_minutes > 60);
        let r = clock_out(&conn, actor, emp2, None, None, None).expect("out2");
        assert_eq!(r.status, "late");
        let early: i64 = conn
            .query_row(
                "SELECT early_minutes FROM attendances WHERE employee_id = ?1 AND date = ?2",
                params![emp2, today_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(early > 60);
        // pulang-cepat: masuk dalam toleransi, pulang sebelum shift berakhir
        let emp3 = make_employee(&conn, "EMP-T3B", None);
        let now = Local::now().naive_local();
        let start = (now - chrono::Duration::minutes(10))
            .format("%H:%M:%S")
            .to_string();
        let end = (now + chrono::Duration::hours(2))
            .format("%H:%M:%S")
            .to_string();
        let sh = shift_save(
            &conn,
            1,
            None,
            &ShiftInput {
                name: "Pagi".to_string(),
                start_time: start,
                end_time: end,
                break_start: None,
                break_end: None,
                grace_period_minutes: 30,
                is_overnight: false,
            },
        )
        .expect("shift") as i64;
        assign_schedule(&conn, emp3, sh);
        let r = clock_in(&conn, actor, emp3, None, None, None).expect("in3");
        assert_eq!(r.status, "present");
        let r = clock_out(&conn, actor, emp3, None, None, None).expect("out3");
        assert_eq!(r.status, "early_checkout");
    }

    #[test]
    fn clock_ditolak_libur_dan_nonaktif() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let actor = actor(&conn);
        let emp = make_employee(&conn, "EMP-T4", None);
        conn.execute(
            "INSERT INTO holidays (name, date, type) VALUES ('Uji', date('now','localtime'), 'national')",
            [],
        )
        .unwrap();
        let e = clock_in(&conn, actor, emp, None, None, None).expect_err("libur");
        assert!(e.contains("libur"));
        conn.execute("DELETE FROM holidays", []).unwrap();
        conn.execute(
            "UPDATE employees SET employment_status = 'resigned' WHERE id = ?1",
            params![emp],
        )
        .unwrap();
        let e = clock_in(&conn, actor, emp, None, None, None).expect_err("nonaktif");
        assert!(e.contains("tidak aktif"));
    }

    #[test]
    fn manual_dan_rekap() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let actor = actor(&conn);
        let emp = make_employee(&conn, "EMP-T5", None);
        let input = ManualInput {
            employee_id: to_dto_int(emp, "e").unwrap(),
            date: "2026-03-02".to_string(),
            clock_in: Some("08:00".to_string()),
            clock_out: Some("17:00".to_string()),
            status: "present".to_string(),
            notes: None,
        };
        manual_entry(&conn, actor, &input).expect("manual");
        manual_entry(&conn, actor, &input).expect("upsert");
        let hist = history(&conn, emp, "2026-03").expect("hist");
        assert_eq!(hist.len(), 1);
        assert!(history(&conn, emp, "bulan-salah").is_err());
        let semua = recap(&conn, "2026-03-02", "").expect("rekap");
        assert!(semua.iter().any(|r| r.employee_id as i64 == emp));
        let cari = recap(&conn, "2026-03-02", "zzz-tidak-ada").expect("cari");
        assert!(cari.is_empty());
        let bad = ManualInput {
            status: "ngawur".to_string(),
            ..input
        };
        assert!(manual_entry(&conn, actor, &bad).is_err());
    }

    #[test]
    fn koreksi_sampai_keputusan_dan_notif() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let actor = actor(&conn);
        let sup = admin_employee(&conn);
        let emp = make_employee(&conn, "EMP-T6", Some(sup));
        let mut input = CorrectionInput {
            date: "2026-03-03".to_string(),
            requested_clock_in: Some("08:05".to_string()),
            requested_clock_out: Some("17:00".to_string()),
            reason: "Lupa absen masuk".to_string(),
        };
        let cid = request_correction(&conn, actor, emp, &input).expect("aju");
        assert_eq!(my_corrections(&conn, emp).expect("mine").len(), 1);
        assert_eq!(pending_corrections(&conn).expect("pending").len(), 1);
        // notifikasi ke admin (supervisor)
        let notif: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM notifications WHERE user_id = ?1 AND type = 'attendance_correction'",
                params![actor],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(notif, 1);
        // bukan supervisor dan bukan privileged ditolak
        let other = make_employee(&conn, "EMP-T7", None);
        let e = decide_correction(
            &conn,
            actor,
            Some(other),
            false,
            cid as i64,
            "approved",
            None,
        )
        .expect_err("ditolak");
        assert!(e.contains("supervisor"));
        // supervisor (admin) menyetujui
        decide_correction(
            &conn,
            actor,
            Some(sup),
            false,
            cid as i64,
            "approved",
            Some("ok"),
        )
        .expect("setuju");
        let att: Option<String> = conn
            .query_row(
                "SELECT clock_in FROM attendances WHERE employee_id = ?1 AND date = '2026-03-03'",
                params![emp],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(att.as_deref(), Some("2026-03-03 08:05"));
        // sudah diproses tak bisa lagi
        assert!(
            decide_correction(&conn, actor, Some(sup), false, cid as i64, "rejected", None)
                .is_err()
        );
        input.reason = String::new();
        assert!(request_correction(&conn, actor, emp, &input).is_err());
    }
}
