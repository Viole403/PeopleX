//! Absensi: shift, jadwal, libur, clock in/out, koreksi, rekap.

use chrono::{Datelike, Local, NaiveDate, NaiveDateTime};
use rand::RngCore;
use std::path::Path;

use super::employees::FileUpload;
use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};
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

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Swap {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub target_employee_id: i32,
    pub target_name: String,
    pub shift_date: String,
    pub from_shift_name: Option<String>,
    pub to_shift_name: Option<String>,
    pub reason: String,
    pub status: String,
    pub requested_at: String,
    pub decided_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct SwapInput {
    pub target_employee_id: i32,
    pub shift_date: String,
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

// ---------------- Resolusi shift ----------------

struct ResolvedShift {
    id: i64,
    start: NaiveDateTime,
    grace: i64,
}

fn parse_hm(date: NaiveDate, hm: &str) -> Result<NaiveDateTime, String> {
    let t = chrono::NaiveTime::parse_from_str(hm.trim(), "%H:%M:%S")
        .map_err(|_| "Jam shift tidak valid.".to_string())?;
    Ok(date.and_time(t))
}

// ---------------- Clock in/out ----------------

/// Hasil clock in/out: status + pesan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ClockResult {
    pub status: String,
    pub message: String,
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

// ---------------- Varian SeaORM (master data) ----------------

fn sea_text(row: &[Value], i: usize) -> String {
    value_to_string(&row[i])
}

fn sea_int(row: &[Value], i: usize) -> i64 {
    value_i64(&row[i]).unwrap_or(0)
}

fn sea_opt_text(row: &[Value], i: usize) -> Option<String> {
    match &row[i] {
        Value::Null => None,
        v => {
            let s = value_to_string(v);
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
    }
}

fn sea_opt_int(row: &[Value], i: usize) -> Option<i64> {
    value_i64(&row[i])
}

fn sea_text_val(v: &str) -> Value {
    Value::Text(v.to_string())
}

fn sea_opt_str(v: Option<&str>) -> Value {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    }
}

fn sea_opt_int_val(v: Option<i64>) -> Value {
    match v {
        Some(x) => Value::Int(x),
        None => Value::Null,
    }
}

pub async fn shift_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Shift>, String> {
    let rows = q_all(db, "SELECT id, name, start_time, end_time, break_start, break_end, grace_period_minutes, is_overnight FROM shifts WHERE deleted_at IS NULL ORDER BY name".to_string(), vec![], 8, "attendance.shift.list").await?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Shift {
            id: to_dto_int(sea_int(r, 0), "shift.id")?,
            name: sea_text(r, 1),
            start_time: sea_text(r, 2),
            end_time: sea_text(r, 3),
            break_start: sea_opt_text(r, 4),
            break_end: sea_opt_text(r, 5),
            grace_period_minutes: to_dto_int(sea_int(r, 6), "shift.grace")?,
            is_overnight: sea_int(r, 7) != 0,
        });
    }
    Ok(out)
}

pub async fn shift_save_sea(
    db: &sea_orm::DatabaseConnection,
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
        let n = exec(db, "UPDATE shifts SET name = ?1, start_time = ?2, end_time = ?3, break_start = ?4, break_end = ?5, grace_period_minutes = ?6, is_overnight = ?7 WHERE id = ?8 AND deleted_at IS NULL".to_string(), vec![sea_text_val(input.name.trim()), sea_text_val(&input.start_time), sea_text_val(&input.end_time), sea_opt_str(input.break_start.as_deref()), sea_opt_str(input.break_end.as_deref()), Value::Int(input.grace_period_minutes as i64), Value::Int(over), Value::Int(rid)], "attendance.shift.update").await.map_err(|e| format!("gagal menyimpan shift: {e}"))?;
        if n == 0 {
            return Err("Shift tidak ditemukan.".to_string());
        }
        super::audit::log_sea(db, Some(actor_id), "UPDATE", "schedule.shift", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "shift.id")
    } else {
        let rid = exec_insert(db, "INSERT INTO shifts (name, start_time, end_time, break_start, break_end, grace_period_minutes, is_overnight) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)".to_string(), vec![sea_text_val(input.name.trim()), sea_text_val(&input.start_time), sea_text_val(&input.end_time), sea_opt_str(input.break_start.as_deref()), sea_opt_str(input.break_end.as_deref()), Value::Int(input.grace_period_minutes as i64), Value::Int(over)], "attendance.shift.insert").await.map_err(|e| format!("gagal menambah shift: {e}"))?;

        super::audit::log_sea(db, Some(actor_id), "CREATE", "schedule.shift", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "shift.id")
    }
}

pub async fn shift_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    for (table, col) in [
        ("work_schedule_days", "shift_id"),
        ("shift_assignments", "shift_id"),
        ("attendances", "shift_id"),
    ] {
        let n = q_one(db, format!("SELECT COUNT(*) FROM {table} WHERE {col} = ?1"), vec![Value::Int(id)], 1, "attendance.shift.rel").await.map_err(|e| format!("gagal memeriksa relasi: {e}"))?.map(|r| sea_int(&r, 0)).unwrap_or(0);
        if n > 0 {
            return Err("Shift masih dipakai dan tidak dapat dihapus.".to_string());
        }
    }
    let n = exec(db, "UPDATE shifts SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(id)], "attendance.shift.delete").await.map_err(|e| format!("gagal menghapus shift: {e}"))?;
    if n == 0 {
        return Err("Shift tidak ditemukan.".to_string());
    }
    super::audit::log_sea(db, Some(actor_id), "DELETE", "schedule.shift", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn schedule_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Schedule>, String> {
    let rows = q_all(db, "SELECT id, name, description FROM work_schedules WHERE deleted_at IS NULL ORDER BY name".to_string(), vec![], 3, "attendance.schedule.list").await?;
    let mut out = Vec::new();
    for r in &rows {
        let id = sea_int(r, 0);
        out.push(Schedule {
            id: to_dto_int(id, "schedule.id")?,
            name: sea_text(r, 1),
            description: sea_opt_text(r, 2),
            days: schedule_days_sea(db, id).await?,
        });
    }
    Ok(out)
}

pub async fn schedule_days_sea(
    db: &sea_orm::DatabaseConnection,
    schedule_id: i64,
) -> Result<Vec<ScheduleDay>, String> {
    let rows = q_all(db, "SELECT day_of_week, shift_id, is_working_day FROM work_schedule_days WHERE work_schedule_id = ?1 ORDER BY day_of_week".to_string(), vec![Value::Int(schedule_id)], 3, "attendance.schedule.days").await?;
    let mut out = Vec::new();
    for r in &rows {
        let dow = sea_int(r, 0);
        let shift = sea_opt_int(r, 1);
        out.push(ScheduleDay {
            day_of_week: to_dto_int(dow, "day.dow")?,
            day_name: DAY_NAMES[dow.clamp(0, 6) as usize].to_string(),
            shift_id: shift.map(|v| to_dto_int(v, "day.shift")).transpose()?,
            is_working_day: sea_int(r, 2) != 0,
        });
    }
    Ok(out)
}

pub async fn schedule_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &ScheduleInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama jadwal wajib diisi.".to_string());
    }
    if let Some(rid) = id {
        let n = exec(db, "UPDATE work_schedules SET name = ?1, description = ?2 WHERE id = ?3 AND deleted_at IS NULL".to_string(), vec![sea_text_val(input.name.trim()), sea_opt_str(input.description.as_deref()), Value::Int(rid)], "attendance.schedule.update").await.map_err(|e| format!("gagal menyimpan jadwal: {e}"))?;
        if n == 0 {
            return Err("Jadwal tidak ditemukan.".to_string());
        }
        super::audit::log_sea(db, Some(actor_id), "UPDATE", "schedule.work", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "schedule.id")
    } else {
        let rid = exec_insert(db, "INSERT INTO work_schedules (name, description) VALUES (?1, ?2)".to_string(), vec![sea_text_val(input.name.trim()), sea_opt_str(input.description.as_deref())], "attendance.schedule.insert").await.map_err(|e| format!("gagal menambah jadwal: {e}"))?;

        for dow in 0..=6 {
            let working = (1..=5).contains(&dow) as i64;
            exec(db, "INSERT INTO work_schedule_days (work_schedule_id, day_of_week, shift_id, is_working_day) VALUES (?1, ?2, NULL, ?3)".to_string(), vec![Value::Int(rid), Value::Int(dow), Value::Int(working)], "attendance.schedule.initdays").await.map_err(|e| format!("gagal membuat hari default: {e}"))?;
        }
        super::audit::log_sea(db, Some(actor_id), "CREATE", "schedule.work", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "schedule.id")
    }
}

pub async fn schedule_save_days_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    schedule_id: i64,
    days: &[(i64, Option<i64>, bool)],
) -> Result<(), String> {
    for (dow, shift, working) in days {
        if !(0..=6).contains(dow) {
            return Err("Hari tidak valid.".to_string());
        }
        if let Some(sid) = shift {
            let found = q_one(db, "SELECT id FROM shifts WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(*sid)], 1, "attendance.schedule.checkshift").await.map_err(|e| format!("gagal memeriksa shift: {e}"))?;
            if found.is_none() {
                return Err("Shift tidak ditemukan.".to_string());
            }
        }
        exec(db, "UPDATE work_schedule_days SET shift_id = ?1, is_working_day = ?2 WHERE work_schedule_id = ?3 AND day_of_week = ?4".to_string(), vec![sea_opt_int_val(*shift), Value::Int(if *working { 1 } else { 0 }), Value::Int(schedule_id), Value::Int(*dow)], "attendance.schedule.savedays").await.map_err(|e| format!("gagal menyimpan hari: {e}"))?;
    }
    super::audit::log_sea(db, Some(actor_id), "UPDATE", "schedule.days", Some(&schedule_id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn schedule_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let n = q_one(db, "SELECT COUNT(*) FROM shift_assignments WHERE work_schedule_id = ?1".to_string(), vec![Value::Int(id)], 1, "attendance.schedule.rel").await.map_err(|e| format!("gagal memeriksa relasi: {e}"))?.map(|r| sea_int(&r, 0)).unwrap_or(0);
    if n > 0 {
        return Err("Jadwal masih dipakai penugasan.".to_string());
    }
    exec(db, "DELETE FROM work_schedule_days WHERE work_schedule_id = ?1".to_string(), vec![Value::Int(id)], "attendance.schedule.cleardays").await.map_err(|e| format!("gagal menghapus hari: {e}"))?;
    let n = exec(db, "UPDATE work_schedules SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(id)], "attendance.schedule.delete").await.map_err(|e| format!("gagal menghapus jadwal: {e}"))?;
    if n == 0 {
        return Err("Jadwal tidak ditemukan.".to_string());
    }
    super::audit::log_sea(db, Some(actor_id), "DELETE", "schedule.work", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn assignment_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Assignment>, String> {
    let rows = q_all(db, "SELECT sa.id, sa.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), sa.work_schedule_id, ws.name, sa.shift_id, s.name, sa.date, sa.start_date, sa.end_date FROM shift_assignments sa JOIN employees e ON e.id = sa.employee_id LEFT JOIN work_schedules ws ON ws.id = sa.work_schedule_id LEFT JOIN shifts s ON s.id = sa.shift_id ORDER BY sa.start_date DESC, e.first_name".to_string(), vec![], 10, "attendance.assign.list").await?;
    let mut out = Vec::new();
    for r in &rows {
        let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "assign.ref")).transpose();
        out.push(Assignment {
            id: to_dto_int(sea_int(r, 0), "assign.id")?,
            employee_id: to_dto_int(sea_int(r, 1), "assign.emp")?,
            employee_name: sea_text(r, 2),
            work_schedule_id: opt_i(sea_opt_int(r, 3))?,
            schedule_name: sea_opt_text(r, 4),
            shift_id: opt_i(sea_opt_int(r, 5))?,
            shift_name: sea_opt_text(r, 6),
            date: sea_opt_text(r, 7),
            start_date: sea_text(r, 8),
            end_date: sea_opt_text(r, 9),
        });
    }
    Ok(out)
}

pub async fn assignment_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &AssignmentInput,
) -> Result<i32, String> {
    let emp = q_one(db, "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(input.employee_id as i64)], 1, "attendance.assign.checkemp").await.map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    if input.work_schedule_id.is_none() && input.shift_id.is_none() {
        return Err("Pilih jadwal atau shift.".to_string());
    }
    let date = input.date.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let start = input.start_date.as_deref().map(str::trim).filter(|s| !s.is_empty());
    if date.is_none() && start.is_none() {
        return Err("Isi tanggal spesifik atau tanggal mulai rentang.".to_string());
    }
    for (v, label) in [
        (date, "Tanggal"),
        (start, "Tanggal mulai"),
        (input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty()), "Tanggal selesai"),
    ] {
        if let Some(s) = v {
            NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| format!("{label} harus valid (YYYY-MM-DD)."))?;
        }
    }
    if let (Some(s), Some(e)) = (start, input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty())) {
        if e < s {
            return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
        }
    }
    let end = input.end_date.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let ws = input.work_schedule_id.map(|v| v as i64);
    let sh = input.shift_id.map(|v| v as i64);
    if let Some(rid) = id {
        let n = exec(db, "UPDATE shift_assignments SET work_schedule_id = ?1, shift_id = ?2, date = ?3, start_date = ?4, end_date = ?5 WHERE id = ?6 AND employee_id = ?7".to_string(), vec![sea_opt_int_val(ws), sea_opt_int_val(sh), sea_opt_str(date), sea_text_val(start.unwrap_or("")), sea_opt_str(end), Value::Int(rid), Value::Int(input.employee_id as i64)], "attendance.assign.update").await.map_err(|e| format!("gagal menyimpan penugasan: {e}"))?;
        if n == 0 {
            return Err("Penugasan tidak ditemukan.".to_string());
        }
        super::audit::log_sea(db, Some(actor_id), "UPDATE", "schedule.assign", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "assign.id")
    } else {
        let rid = exec_insert(db, "INSERT INTO shift_assignments (employee_id, work_schedule_id, shift_id, date, start_date, end_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(), vec![Value::Int(input.employee_id as i64), sea_opt_int_val(ws), sea_opt_int_val(sh), sea_opt_str(date), sea_text_val(start.unwrap_or("")), sea_opt_str(end)], "attendance.assign.insert").await.map_err(|e| format!("gagal menambah penugasan: {e}"))?;

        super::audit::log_sea(db, Some(actor_id), "CREATE", "schedule.assign", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "assign.id")
    }
}

pub async fn assignment_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let n = exec(db, "DELETE FROM shift_assignments WHERE id = ?1".to_string(), vec![Value::Int(id)], "attendance.assign.delete").await.map_err(|e| format!("gagal menghapus penugasan: {e}"))?;
    if n == 0 {
        return Err("Penugasan tidak ditemukan.".to_string());
    }
    super::audit::log_sea(db, Some(actor_id), "DELETE", "schedule.assign", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn holiday_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Holiday>, String> {
    let rows = q_all(db, "SELECT id, name, date, type, description FROM holidays ORDER BY date DESC".to_string(), vec![], 5, "attendance.holiday.list").await?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Holiday {
            id: to_dto_int(sea_int(r, 0), "holiday.id")?,
            name: sea_text(r, 1),
            date: sea_text(r, 2),
            holiday_type: sea_text(r, 3),
            description: sea_opt_text(r, 4),
        });
    }
    Ok(out)
}

pub async fn holiday_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &HolidayInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama libur wajib diisi.".to_string());
    }
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d").map_err(|_| "Tanggal harus valid (YYYY-MM-DD).".to_string())?;
    if !["national", "company", "collective_leave", "custom"].contains(&input.holiday_type.as_str()) {
        return Err("Jenis libur tidak valid.".to_string());
    }
    if let Some(rid) = id {
        let n = exec(db, "UPDATE holidays SET name = ?1, date = ?2, type = ?3, description = ?4 WHERE id = ?5".to_string(), vec![sea_text_val(input.name.trim()), sea_text_val(input.date.trim()), sea_text_val(&input.holiday_type), sea_opt_str(input.description.as_deref()), Value::Int(rid)], "attendance.holiday.update").await.map_err(|e| format!("gagal menyimpan libur: {e}"))?;
        if n == 0 {
            return Err("Libur tidak ditemukan.".to_string());
        }
        super::audit::log_sea(db, Some(actor_id), "UPDATE", "schedule.holiday", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "holiday.id")
    } else {
        let rid = exec_insert(db, "INSERT INTO holidays (name, date, type, description) VALUES (?1, ?2, ?3, ?4)".to_string(), vec![sea_text_val(input.name.trim()), sea_text_val(input.date.trim()), sea_text_val(&input.holiday_type), sea_opt_str(input.description.as_deref())], "attendance.holiday.insert").await.map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Libur tanggal dan nama tersebut sudah ada.".to_string()
            } else {
                format!("gagal menambah libur: {e}")
            }
        })?;

        super::audit::log_sea(db, Some(actor_id), "CREATE", "schedule.holiday", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "holiday.id")
    }
}

pub async fn holiday_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let n = exec(db, "DELETE FROM holidays WHERE id = ?1".to_string(), vec![Value::Int(id)], "attendance.holiday.delete").await.map_err(|e| format!("gagal menghapus libur: {e}"))?;
    if n == 0 {
        return Err("Libur tidak ditemukan.".to_string());
    }
    super::audit::log_sea(db, Some(actor_id), "DELETE", "schedule.holiday", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn holidays_autofill_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    year: i32,
) -> Result<i32, String> {
    if year < 1990 || year > 2100 {
        return Err("Tahun harus antara 1990 dan 2100.".to_string());
    }
    let tetap: [(&str, &str); 5] = [
        ("01-01", "Tahun Baru Masehi"),
        ("05-01", "Hari Buruh Internasional"),
        ("06-01", "Hari Lahir Pancasila"),
        ("08-17", "Hari Kemerdekaan Republik Indonesia"),
        ("12-25", "Hari Raya Natal"),
    ];
    let mut added = 0i64;
    for (mmdd, name) in tetap {
        let date = format!("{year}-{mmdd}");
        added += exec(
            db,
            "INSERT OR IGNORE INTO holidays (name, date, type, description) VALUES (?1, ?2, 'national', 'Ditambahkan otomatis')".to_string(),
            vec![Value::Text(name.to_string()), Value::Text(date)],
            "attendance.holiday.autofill",
        )
        .await
        .map_err(|e| format!("gagal menambah libur: {e}"))? as i64;
    }
    super::audit::log_sea(db, Some(actor_id), "CREATE", "schedule.holiday.autofill", Some(&year.to_string()), None, None, None).await?;
    to_dto_int(added, "jumlah libur")
}

// ---------------- Varian SeaORM (transaksi) ----------------

fn sea_opt_float(v: Option<f64>) -> Value {
    match v {
        Some(x) => Value::Float(x),
        None => Value::Null,
    }
}

async fn notify_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    notif_type: &str,
    title: &str,
    message: &str,
    link: &str,
) -> Result<(), String> {
    exec(db, "INSERT INTO notifications (user_id, type, title, message, link) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(), vec![Value::Int(user_id), sea_text_val(notif_type), sea_text_val(title), sea_text_val(message), sea_text_val(link)], "attendance.notify").await.map_err(|e| format!("gagal mengirim notifikasi: {e}"))?;
    Ok(())
}

async fn user_of_employee_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Option<i64>, String> {
    let rows = q_all(db, "SELECT id FROM users WHERE employee_id = ?1 AND status = 'active'".to_string(), vec![Value::Int(employee_id)], 1, "attendance.userof").await.map_err(|e| format!("gagal mencari user karyawan: {e}"))?;
    Ok(rows.into_iter().next().map(|r| sea_int(&r, 0)))
}

async fn resolve_shift_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    date: &str,
) -> Result<Option<ResolvedShift>, String> {
    let day: NaiveDate = NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Tanggal tidak valid.".to_string())?;
    let rows = q_all(db, "SELECT s.id, s.start_time, s.end_time, s.grace_period_minutes, s.is_overnight FROM shift_assignments sa INNER JOIN shifts s ON s.id = sa.shift_id WHERE sa.employee_id = ?1 AND sa.date = ?2 AND s.deleted_at IS NULL LIMIT 1".to_string(), vec![Value::Int(employee_id), sea_text_val(date)], 5, "attendance.resolve").await.map_err(|e| format!("gagal resolve shift: {e}"))?;
    if let Some(r) = rows.into_iter().next() {
        let (sid, start, end, grace) = (sea_int(&r, 0), sea_text(&r, 1), sea_text(&r, 2), sea_int(&r, 3));
        let s = parse_hm(day, &start)?;
        parse_hm(day, &end)?;
        return Ok(Some(ResolvedShift { id: sid, start: s, grace }));
    }
    let dow = day.weekday().num_days_from_sunday() as i64;
    let rows = q_all(db, "SELECT s.id, s.start_time, s.end_time, s.grace_period_minutes, s.is_overnight FROM shift_assignments sa INNER JOIN work_schedules ws ON ws.id = sa.work_schedule_id INNER JOIN work_schedule_days wsd ON wsd.work_schedule_id = ws.id AND wsd.day_of_week = ?1 INNER JOIN shifts s ON s.id = wsd.shift_id WHERE sa.employee_id = ?2 AND sa.date IS NULL AND sa.start_date <= ?3 AND (sa.end_date IS NULL OR sa.end_date >= ?3) AND wsd.is_working_day = 1 AND s.deleted_at IS NULL ORDER BY sa.start_date DESC LIMIT 1".to_string(), vec![Value::Int(dow), Value::Int(employee_id), sea_text_val(date)], 5, "attendance.resolvesched").await.map_err(|e| format!("gagal resolve jadwal: {e}"))?;
    rows.into_iter().next().map(|r| {
        let (sid, start, end, grace) = (sea_int(&r, 0), sea_text(&r, 1), sea_text(&r, 2), sea_int(&r, 3));
        let s = parse_hm(day, &start)?;
        parse_hm(day, &end)?;
        Ok(ResolvedShift { id: sid, start: s, grace })
    }).transpose()
}

async fn is_holiday_sea(db: &sea_orm::DatabaseConnection, date: &str) -> Result<bool, String> {
    let rows = q_all(db, "SELECT id FROM holidays WHERE date = ?1".to_string(), vec![sea_text_val(date)], 1, "attendance.isholiday").await.map_err(|e| format!("gagal memeriksa libur: {e}"))?;
    Ok(!rows.is_empty())
}

async fn active_employee_sea(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(), String> {
    let rows = q_all(db, "SELECT employment_status FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(id)], 1, "attendance.active").await.map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    match rows.into_iter().next().map(|r| sea_text(&r, 0)) {
        Some(s) if s == "active" => Ok(()),
        _ => Err("Karyawan tidak aktif, tidak dapat absensi.".to_string()),
    }
}

fn jarak_meter(lat1: f64, lng1: f64, lat2: f64, lng2: f64) -> f64 {
    let r = 6_371_000.0_f64;
    let dlat = (lat2 - lat1).to_radians();
    let dlng = (lng2 - lng1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlng / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().asin()
}

fn angka_opt(v: &Value) -> Option<f64> {
    match v {
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse::<f64>().ok(),
        Value::Null => None,
    }
}

async fn cek_geofence(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<(), String> {
    let rows = q_all(db, "SELECT work_location_id FROM employees WHERE id = ?1".to_string(), vec![Value::Int(employee_id)], 1, "attendance.lokasi").await.map_err(|e| format!("gagal memuat lokasi kerja: {e}"))?;
    let loc = rows.into_iter().next().and_then(|r| sea_opt_int(&r, 0));
    let loc = match loc {
        Some(v) => v,
        None => return Ok(()),
    };
    let rows = q_all(db, "SELECT latitude, longitude, radius_meter FROM work_locations WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(loc)], 3, "attendance.geofence").await.map_err(|e| format!("gagal memuat geofence: {e}"))?;
    let r = rows.into_iter().next().ok_or_else(|| "Lokasi kerja tidak ditemukan.".to_string())?;
    let (olat, olng) = match (angka_opt(&r[0]), angka_opt(&r[1])) {
        (Some(a), Some(b)) => (a, b),
        _ => return Ok(()),
    };
    let radius = value_i64(&r[2]).unwrap_or(100).max(1) as f64;
    let (plat, plng) = match (lat, lng) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err("Lokasi clock wajib diisi karena karyawan terikat lokasi kerja.".to_string()),
    };
    let d = jarak_meter(olat, olng, plat, plng);
    if d > radius {
        return Err(format!(
            "Di luar jangkauan lokasi kerja ({:.0} m dari batas {} m).",
            d, radius as i64
        ));
    }
    Ok(())
}

const LIVENESS_MIMES: &[&str] = &["image/jpeg", "image/png"];
const LIVENESS_MAX_BYTES: usize = 3 * 1024 * 1024;
const CHALLENGE_TTL_SECS: i64 = 300;
const LIVENESS_INSTRUCTIONS: &[&str] = &[
    "Kedipkan mata dua kali",
    "Tengok ke kiri",
    "Tengok ke kanan",
    "Tersenyum ke kamera",
];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct LivenessChallenge {
    pub nonce: String,
    pub instruction: String,
}

pub async fn liveness_challenge_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<LivenessChallenge, String> {
    active_employee_sea(db, employee_id).await?;
    let mut raw = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut raw);
    let nonce: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    let instruction = LIVENESS_INSTRUCTIONS[(raw[0] as usize) % LIVENESS_INSTRUCTIONS.len()].to_string();
    exec(db, "INSERT INTO liveness_challenges (nonce, employee_id, instruction, issued_at, used) VALUES (?1, ?2, ?3, ?4, 0)".to_string(), vec![Value::Text(nonce.clone()), Value::Int(employee_id), sea_text_val(&instruction), sea_text_val(&now_str())], "attendance.challenge").await.map_err(|e| format!("gagal membuat tantangan verifikasi: {e}"))?;
    Ok(LivenessChallenge { nonce, instruction })
}

fn nama_aman(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("foto");
    let clean: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "foto".to_string()
    } else {
        clean
    }
}

async fn periksa_liveness(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    employee_id: i64,
    photo: Option<&FileUpload>,
    nonce: Option<&str>,
) -> Result<String, String> {
    let code = nonce.ok_or_else(|| "Verifikasi wajah wajib dilakukan sebelum clock in.".to_string())?;
    let row = q_one(db, "SELECT employee_id, issued_at, used FROM liveness_challenges WHERE nonce = ?1".to_string(), vec![Value::Text(code.to_string())], 3, "attendance.challenge.read").await.map_err(|e| format!("gagal memeriksa verifikasi: {e}"))?.ok_or_else(|| "Kode verifikasi tidak valid.".to_string())?;
    if sea_int(&row, 2) != 0 {
        return Err("Kode verifikasi sudah dipakai, minta yang baru.".to_string());
    }
    if sea_int(&row, 0) != employee_id {
        return Err("Kode verifikasi tidak valid.".to_string());
    }
    let issued = NaiveDateTime::parse_from_str(&sea_text(&row, 1), "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "Data verifikasi rusak.".to_string())?;
    if (Local::now().naive_local() - issued).num_seconds() > CHALLENGE_TTL_SECS {
        return Err("Kode verifikasi kedaluwarsa, minta yang baru.".to_string());
    }
    let file = photo.ok_or_else(|| "Swafoto wajib diambil saat clock in.".to_string())?;
    if !LIVENESS_MIMES.contains(&file.mime.as_str()) {
        return Err("Swafoto harus berformat JPG atau PNG.".to_string());
    }
    if file.bytes.is_empty() {
        return Err("Swafoto kosong.".to_string());
    }
    if file.bytes.len() > LIVENESS_MAX_BYTES {
        return Err("Ukuran swafoto maksimal 3MB.".to_string());
    }
    let dir = files.join("attendance");
    std::fs::create_dir_all(&dir).map_err(|e| format!("gagal membuat folder swafoto: {e}"))?;
    let stamp = Local::now().format("%Y%m%d%H%M%S%f").to_string();
    let rel = format!("attendance/{stamp}_{employee_id}_{}", nama_aman(&file.name));
    std::fs::write(files.join(&rel), &file.bytes)
        .map_err(|e| format!("gagal menyimpan swafoto: {e}"))?;
    exec(db, "UPDATE liveness_challenges SET used = 1 WHERE nonce = ?1".to_string(), vec![Value::Text(code.to_string())], "attendance.challenge.pakai").await.map_err(|e| format!("gagal menandai verifikasi: {e}"))?;
    Ok(rel)
}

async fn read_attendance_sea(db: &sea_orm::DatabaseConnection, id: i64) -> Result<Attendance, String> {
    let row = q_one(db, "SELECT id, employee_id, date, clock_in, clock_out, status, late_minutes, early_minutes, work_minutes, notes, shift_id FROM attendances WHERE id = ?1".to_string(), vec![Value::Int(id)], 11, "attendance.read").await.map_err(|e| format!("gagal memuat absensi: {e}"))?.ok_or_else(|| "Absensi tidak ditemukan.".to_string())?;
    Ok(Attendance {
        id: to_dto_int(sea_int(&row, 0), "attendance.id")?,
        employee_id: to_dto_int(sea_int(&row, 1), "attendance.emp")?,
        date: sea_text(&row, 2),
        clock_in: sea_opt_text(&row, 3),
        clock_out: sea_opt_text(&row, 4),
        status: sea_text(&row, 5),
        late_minutes: to_dto_int(sea_int(&row, 6), "attendance.late")?,
        early_minutes: to_dto_int(sea_int(&row, 7), "attendance.early")?,
        work_minutes: to_dto_int(sea_int(&row, 8), "attendance.work")?,
        notes: sea_opt_text(&row, 9),
        shift_id: sea_opt_int(&row, 10).map(|v| to_dto_int(v, "attendance.shift")).transpose()?,
    })
}

pub async fn today_sea(db: &sea_orm::DatabaseConnection, employee_id: i64) -> Result<Option<Attendance>, String> {
    let rows = q_all(db, "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(), vec![Value::Int(employee_id), sea_text_val(&today_str())], 1, "attendance.today").await.map_err(|e| format!("gagal memuat absensi hari ini: {e}"))?;
    match rows.into_iter().next() {
        Some(r) => Ok(Some(read_attendance_sea(db, sea_int(&r, 0)).await?)),
        None => Ok(None),
    }
}

pub async fn clock_in_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    lat: Option<f64>,
    lng: Option<f64>,
    device: Option<&str>,
    files: &Path,
    photo: Option<&FileUpload>,
    nonce: Option<&str>,
) -> Result<ClockResult, String> {
    active_employee_sea(db, employee_id).await?;
    let today = today_str();
    let rows = q_all(db, "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(), vec![Value::Int(employee_id), sea_text_val(&today)], 1, "attendance.check").await.map_err(|e| format!("gagal memeriksa absensi: {e}"))?;
    if !rows.is_empty() {
        return Err("Sudah clock in hari ini.".to_string());
    }
    if is_holiday_sea(db, &today).await? {
        return Err("Hari ini libur.".to_string());
    }
    cek_geofence(db, employee_id, lat, lng).await?;
    let foto = periksa_liveness(db, files, employee_id, photo, nonce).await?;
    let now = Local::now().naive_local();
    let shift = resolve_shift_sea(db, employee_id, &today).await?;
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
    let id = exec_insert(db, "INSERT INTO attendances (employee_id, date, clock_in, clock_in_lat, clock_in_lng, clock_in_device, shift_id, status, late_minutes, clock_in_photo) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)".to_string(), vec![Value::Int(employee_id), sea_text_val(&today), sea_text_val(&now.format("%Y-%m-%d %H:%M:%S").to_string()), sea_opt_float(lat), sea_opt_float(lng), sea_opt_str(device), match shift.as_ref().map(|s| s.id) { Some(x) => Value::Int(x), None => Value::Null }, sea_text_val(&status), Value::Int(late), sea_text_val(&foto)], "attendance.clockin").await.map_err(|e| format!("gagal clock in: {e}"))?;

    super::audit::log_sea(db, Some(actor_id), "CLOCK_IN", "attendance", Some(&id.to_string()), None, None, None).await?;
    Ok(ClockResult {
        message: if status == "late" {
            format!("Clock in berhasil. Terlambat {late} menit.")
        } else {
            "Clock in berhasil.".to_string()
        },
        status,
    })
}

pub async fn clock_out_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    lat: Option<f64>,
    lng: Option<f64>,
    device: Option<&str>,
) -> Result<ClockResult, String> {
    let today = today_str();
    let rows = q_all(db, "SELECT id, clock_in, clock_out, status, shift_id FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(), vec![Value::Int(employee_id), sea_text_val(&today)], 5, "attendance.clockout.read").await.map_err(|e| format!("gagal memuat absensi: {e}"))?;
    let r = rows.into_iter().next().ok_or_else(|| "Belum clock in hari ini.".to_string())?;
    let (aid, clock_in, clock_out, status, shift_id) = (sea_int(&r, 0), sea_text(&r, 1), sea_opt_text(&r, 2), sea_text(&r, 3), sea_opt_int(&r, 4));
    if clock_out.is_some() {
        return Err("Sudah clock out hari ini.".to_string());
    }
    cek_geofence(db, employee_id, lat, lng).await?;
    let now = Local::now().naive_local();
    let fmt = "%Y-%m-%d %H:%M:%S";
    let cin = NaiveDateTime::parse_from_str(&clock_in, fmt)
        .map_err(|_| "Data clock in rusak.".to_string())?;
    let work = (now - cin).num_minutes().max(0);
    let (status, early) = match shift_id {
        Some(sid) => {
            let srows = q_all(db, "SELECT start_time, end_time, is_overnight FROM shifts WHERE id = ?1".to_string(), vec![Value::Int(sid)], 3, "attendance.shift").await.map_err(|e| format!("gagal memuat shift: {e}"))?;
            match srows.into_iter().next() {
                Some(sr) => {
                    let end_s = sea_text(&sr, 1);
                    let over = sea_int(&sr, 2);
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
    exec(db, "UPDATE attendances SET clock_out = ?1, clock_out_lat = ?2, clock_out_lng = ?3, clock_out_device = ?4, work_minutes = ?5, early_minutes = ?6, status = ?7 WHERE id = ?8".to_string(), vec![sea_text_val(&now.format("%Y-%m-%d %H:%M:%S").to_string()), sea_opt_float(lat), sea_opt_float(lng), sea_opt_str(device), Value::Int(work), Value::Int(early), sea_text_val(&status), Value::Int(aid)], "attendance.clockout").await.map_err(|e| format!("gagal clock out: {e}"))?;
    super::audit::log_sea(db, Some(actor_id), "CLOCK_OUT", "attendance", Some(&aid.to_string()), None, None, None).await?;
    Ok(ClockResult {
        status,
        message: "Clock out berhasil.".to_string(),
    })
}

pub async fn history_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    month: &str,
) -> Result<Vec<Attendance>, String> {
    if NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").is_err() {
        return Err("Bulan harus format YYYY-MM.".to_string());
    }
    let rows = q_all(db, "SELECT id FROM attendances WHERE employee_id = ?1 AND substr(date, 1, 7) = ?2 ORDER BY date DESC".to_string(), vec![Value::Int(employee_id), sea_text_val(month)], 1, "attendance.history").await.map_err(|e| format!("gagal membaca riwayat: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(read_attendance_sea(db, sea_int(r, 0)).await?);
    }
    Ok(out)
}

pub async fn recap_sea(db: &sea_orm::DatabaseConnection, date: &str, search: &str) -> Result<Vec<RecapRow>, String> {
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
    let like = format!("%{search}%");
    let mut vals = vec![sea_text_val(date)];
    if has_search {
        vals.push(sea_text_val(&like));
    }
    let rows = q_all(db, sql, vals, 9, "attendance.recap").await.map_err(|e| format!("gagal membaca rekap: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "recap.num")).transpose();
        out.push(RecapRow {
            employee_id: to_dto_int(sea_int(r, 0), "recap.emp")?,
            employee_number: sea_text(r, 1),
            name: sea_text(r, 2),
            department_name: sea_opt_text(r, 3),
            clock_in: sea_opt_text(r, 4),
            clock_out: sea_opt_text(r, 5),
            status: sea_opt_text(r, 6),
            late_minutes: opt_i(sea_opt_int(r, 7))?,
            work_minutes: opt_i(sea_opt_int(r, 8))?,
        });
    }
    Ok(out)
}

pub async fn manual_entry_sea(db: &sea_orm::DatabaseConnection, actor_id: i64, input: &ManualInput) -> Result<i32, String> {
    let rows = q_all(db, "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(input.employee_id as i64)], 1, "attendance.manual.emp").await.map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if rows.is_empty() {
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
    let existing = q_all(db, "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(), vec![Value::Int(input.employee_id as i64), sea_text_val(input.date.trim())], 1, "attendance.manual.check").await.map_err(|e| format!("gagal memeriksa absensi: {e}"))?;
    let notes_val = sea_opt_str(input.notes.as_deref());
    if let Some(r) = existing.into_iter().next() {
        let rid = sea_int(&r, 0);
        exec(db, "UPDATE attendances SET clock_in = ?1, clock_out = ?2, status = ?3, notes = ?4 WHERE id = ?5".to_string(), vec![match &cin { Some(s) => sea_text_val(s), None => Value::Null }, match &cout { Some(s) => sea_text_val(s), None => Value::Null }, sea_text_val(&input.status), notes_val, Value::Int(rid)], "attendance.manual.update").await.map_err(|e| format!("gagal memperbarui absensi: {e}"))?;
        super::audit::log_sea(db, Some(actor_id), "UPDATE", "attendance.manual", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "attendance.id")
    } else {
        let rid = exec_insert(db, "INSERT INTO attendances (employee_id, date, clock_in, clock_out, status, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(), vec![Value::Int(input.employee_id as i64), sea_text_val(input.date.trim()), match &cin { Some(s) => sea_text_val(s), None => Value::Null }, match &cout { Some(s) => sea_text_val(s), None => Value::Null }, sea_text_val(&input.status), notes_val], "attendance.manual.insert").await.map_err(|e| format!("gagal mencatat absensi: {e}"))?;

        super::audit::log_sea(db, Some(actor_id), "CREATE", "attendance.manual", Some(&rid.to_string()), None, None, None).await?;
        to_dto_int(rid, "attendance.id")
    }
}

async fn read_corrections_sea(
    db: &sea_orm::DatabaseConnection,
    where_sql: &str,
    param: i64,
) -> Result<Vec<Correction>, String> {
    let rows = q_all(db, format!("SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), c.date, c.requested_clock_in, c.requested_clock_out, c.reason, c.status, c.notes, c.created_at FROM attendance_corrections c JOIN employees e ON e.id = c.employee_id WHERE {where_sql} ORDER BY c.id DESC"), vec![Value::Int(param)], 10, "attendance.corrections").await.map_err(|e| format!("gagal membaca koreksi: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(correction_row(
            sea_int(r, 0), sea_int(r, 1), sea_text(r, 2), sea_text(r, 3),
            sea_opt_text(r, 4), sea_opt_text(r, 5), sea_text(r, 6), sea_text(r, 7),
            sea_opt_text(r, 8), sea_text(r, 9),
        )?);
    }
    Ok(out)
}

pub async fn my_corrections_sea(db: &sea_orm::DatabaseConnection, employee_id: i64) -> Result<Vec<Correction>, String> {
    read_corrections_sea(db, "c.employee_id = ?1", employee_id).await
}

pub async fn pending_corrections_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Correction>, String> {
    let rows = q_all(db, "SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), c.date, c.requested_clock_in, c.requested_clock_out, c.reason, c.status, c.notes, c.created_at FROM attendance_corrections c JOIN employees e ON e.id = c.employee_id WHERE c.status = 'pending' ORDER BY c.id DESC".to_string(), vec![], 10, "attendance.corrections.pending").await.map_err(|e| format!("gagal membaca koreksi: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(correction_row(
            sea_int(r, 0), sea_int(r, 1), sea_text(r, 2), sea_text(r, 3),
            sea_opt_text(r, 4), sea_opt_text(r, 5), sea_text(r, 6), sea_text(r, 7),
            sea_opt_text(r, 8), sea_text(r, 9),
        )?);
    }
    Ok(out)
}

pub async fn request_correction_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &CorrectionInput,
) -> Result<i32, String> {
    active_employee_sea(db, employee_id).await?;
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
    let att = q_all(db, "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(), vec![Value::Int(employee_id), sea_text_val(input.date.trim())], 1, "attendance.correction.check").await.map_err(|e| format!("gagal memeriksa absensi: {e}"))?.into_iter().next().map(|r| sea_int(&r, 0));
    let rid = exec_insert(db, "INSERT INTO attendance_corrections (employee_id, attendance_id, date, requested_clock_in, requested_clock_out, reason, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')".to_string(), vec![Value::Int(employee_id), match att { Some(x) => Value::Int(x), None => Value::Null }, sea_text_val(input.date.trim()), match &cin { Some(s) => sea_text_val(s), None => Value::Null }, match &cout { Some(s) => sea_text_val(s), None => Value::Null }, sea_text_val(input.reason.trim())], "attendance.correction.insert").await.map_err(|e| format!("gagal mengajukan koreksi: {e}"))?;

    let sup = q_all(db, "SELECT supervisor_id FROM employees WHERE id = ?1".to_string(), vec![Value::Int(employee_id)], 1, "attendance.supervisor").await.map_err(|e| format!("gagal memuat supervisor: {e}"))?.into_iter().next().and_then(|r| sea_opt_int(&r, 0));
    if let Some(sid) = sup {
        if let Some(uid) = user_of_employee_sea(db, sid).await? {
            notify_sea(
                db,
                uid,
                "attendance_correction",
                "Pengajuan Koreksi Absensi",
                "Ada pengajuan koreksi absensi yang menunggu persetujuan Anda.",
                "/attendance",
            ).await?;
        }
    }
    super::audit::log_sea(db, Some(actor_id), "CREATE", "attendance.correction", Some(&rid.to_string()), None, None, None).await?;
    to_dto_int(rid, "correction.id")
}

pub async fn decide_correction_sea(
    db: &sea_orm::DatabaseConnection,
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
    let rows = q_all(db, "SELECT employee_id, date, status, requested_clock_in, requested_clock_out, attendance_id FROM attendance_corrections WHERE id = ?1".to_string(), vec![Value::Int(correction_id)], 6, "attendance.decide.read").await.map_err(|e| format!("gagal memuat koreksi: {e}"))?;
    let r = rows.into_iter().next().ok_or_else(|| "Data koreksi tidak ditemukan.".to_string())?;
    let (emp_id, date, status, req_in, req_out) = (sea_int(&r, 0), sea_text(&r, 1), sea_text(&r, 2), sea_opt_text(&r, 3), sea_opt_text(&r, 4));
    if status != "pending" {
        return Err("Koreksi sudah diproses.".to_string());
    }
    let supervisor = q_all(db, "SELECT supervisor_id FROM employees WHERE id = ?1".to_string(), vec![Value::Int(emp_id)], 1, "attendance.decide.sup").await.map_err(|e| format!("gagal memuat supervisor: {e}"))?.into_iter().next().and_then(|x| sea_opt_int(&x, 0));
    let is_supervisor = approver_employee_id == supervisor;
    if !is_supervisor && !is_privileged {
        return Err("Hanya supervisor atau HR yang boleh memutuskan.".to_string());
    }
    let now = now_str();
    exec(db, "UPDATE attendance_corrections SET status = ?1, approved_by = ?2, approved_at = ?3, notes = ?4 WHERE id = ?5".to_string(), vec![sea_text_val(decision), Value::Int(approver_user_id), sea_text_val(&now), sea_opt_str(notes), Value::Int(correction_id)], "attendance.decide").await.map_err(|e| format!("gagal menyimpan keputusan: {e}"))?;
    if decision == "approved" {
        let existing = q_all(db, "SELECT id FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(), vec![Value::Int(emp_id), sea_text_val(&date)], 1, "attendance.decide.check").await.map_err(|e| format!("gagal memeriksa absensi: {e}"))?.into_iter().next().map(|x| sea_int(&x, 0));
        match existing {
            Some(aid) => {
                exec(db, "UPDATE attendances SET clock_in = COALESCE(?1, clock_in), clock_out = COALESCE(?2, clock_out) WHERE id = ?3".to_string(), vec![match &req_in { Some(s) => sea_text_val(s), None => Value::Null }, match &req_out { Some(s) => sea_text_val(s), None => Value::Null }, Value::Int(aid)], "attendance.decide.apply").await.map_err(|e| format!("gagal menerapkan koreksi: {e}"))?;
            }
            None => {
                exec(db, "INSERT INTO attendances (employee_id, date, clock_in, clock_out, status) VALUES (?1, ?2, ?3, ?4, 'present')".to_string(), vec![Value::Int(emp_id), sea_text_val(&date), match &req_in { Some(s) => sea_text_val(s), None => Value::Null }, match &req_out { Some(s) => sea_text_val(s), None => Value::Null }], "attendance.decide.insert").await.map_err(|e| format!("gagal menerapkan koreksi: {e}"))?;
            }
        }
    }
    if let Some(uid) = user_of_employee_sea(db, emp_id).await? {
        notify_sea(
            db,
            uid,
            "attendance_correction",
            if decision == "approved" {
                "Koreksi Absensi Disetujui"
            } else {
                "Koreksi Absensi Ditolak"
            },
            &format!("Pengajuan koreksi absensi tanggal {date} telah {decision}."),
            "/attendance",
        ).await?;
    }
    super::audit::log_sea(db, Some(approver_user_id), decision.to_uppercase().as_str(), "attendance.correction", Some(&correction_id.to_string()), None, None, None).await?;
    Ok(())
}

async fn swap_shift_id_of(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    date: &str,
) -> Result<Option<i64>, String> {
    let rows = q_all(
        db,
        "SELECT shift_id FROM shift_assignments WHERE employee_id = ?1 AND date = ?2 AND shift_id IS NOT NULL".to_string(),
        vec![Value::Int(employee_id), sea_text_val(date)],
        1,
        "attendance.swap.assign",
    )
    .await
    .map_err(|e| format!("gagal memuat penugasan: {e}"))?;
    Ok(rows.first().and_then(|r| sea_opt_int(r, 0)))
}

fn swap_row(r: &[Value]) -> Result<Swap, String> {
    Ok(Swap {
        id: to_dto_int(sea_int(r, 0), "swap.id")?,
        employee_id: to_dto_int(sea_int(r, 1), "swap.emp")?,
        employee_name: sea_text(r, 2),
        target_employee_id: to_dto_int(sea_int(r, 3), "swap.target")?,
        target_name: sea_text(r, 4),
        shift_date: sea_text(r, 5),
        from_shift_name: sea_opt_text(r, 6),
        to_shift_name: sea_opt_text(r, 7),
        reason: sea_text(r, 8),
        status: sea_text(r, 9),
        requested_at: sea_text(r, 10),
        decided_at: sea_opt_text(r, 11),
        notes: sea_opt_text(r, 12),
    })
}

const SWAP_SELECT: &str = "SELECT ss.id, ss.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), ss.target_employee_id, t.first_name || ' ' || COALESCE(t.last_name, ''), ss.shift_date, fs.name, ts.name, ss.reason, ss.status, ss.requested_at, ss.decided_at, ss.notes FROM shift_swaps ss JOIN employees e ON e.id = ss.employee_id JOIN employees t ON t.id = ss.target_employee_id LEFT JOIN shifts fs ON fs.id = ss.from_shift_id LEFT JOIN shifts ts ON ts.id = ss.to_shift_id";

pub async fn swap_request_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &SwapInput,
) -> Result<i32, String> {
    active_employee_sea(db, employee_id).await?;
    let target = input.target_employee_id as i64;
    if target == employee_id {
        return Err("Pilih rekan lain untuk bertukar shift.".to_string());
    }
    active_employee_sea(db, target).await?;
    let date = input.shift_date.trim();
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| "Tanggal harus valid (YYYY-MM-DD).".to_string())?;
    if input.reason.trim().is_empty() {
        return Err("Alasan wajib diisi.".to_string());
    }
    if input.reason.len() > 255 {
        return Err("Alasan maksimal 255 karakter.".to_string());
    }
    let dup = q_all(
        db,
        "SELECT id FROM shift_swaps WHERE employee_id = ?1 AND target_employee_id = ?2 AND shift_date = ?3 AND status = 'pending'".to_string(),
        vec![Value::Int(employee_id), Value::Int(target), sea_text_val(date)],
        1,
        "attendance.swap.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa pengajuan: {e}"))?;
    if !dup.is_empty() {
        return Err("Pengajuan tukar shift pada tanggal itu sudah ada.".to_string());
    }
    let mine = swap_shift_id_of(db, employee_id, date).await?;
    if mine.is_none() {
        return Err("Anda tidak memiliki penugasan shift pada tanggal itu.".to_string());
    }
    let theirs = swap_shift_id_of(db, target, date).await?;
    if theirs.is_none() {
        return Err("Rekan tersebut tidak memiliki penugasan shift pada tanggal itu.".to_string());
    }
    let rid = exec_insert(
        db,
        "INSERT INTO shift_swaps (employee_id, target_employee_id, shift_date, from_shift_id, to_shift_id, reason, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')".to_string(),
        vec![Value::Int(employee_id), Value::Int(target), sea_text_val(date), Value::Int(mine.unwrap()), Value::Int(theirs.unwrap()), sea_text_val(input.reason.trim())],
        "attendance.swap.insert",
    )
    .await
    .map_err(|e| format!("gagal mengajukan tukar shift: {e}"))?;

    if let Some(uid) = user_of_employee_sea(db, target).await? {
        notify_sea(
            db,
            uid,
            "attendance_swap",
            "Pengajuan Tukar Shift",
            "Ada rekan yang mengajukan tukar shift dengan Anda.",
            "/attendance",
        )
        .await?;
    }
    let sup = q_all(
        db,
        "SELECT supervisor_id FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "attendance.swap.sup",
    )
    .await
    .map_err(|e| format!("gagal memuat supervisor: {e}"))?
    .into_iter()
    .next()
    .and_then(|r| sea_opt_int(&r, 0));
    if let Some(sid) = sup {
        if let Some(uid) = user_of_employee_sea(db, sid).await? {
            notify_sea(
                db,
                uid,
                "attendance_swap",
                "Persetujuan Tukar Shift",
                "Ada pengajuan tukar shift yang menunggu persetujuan Anda.",
                "/attendance",
            )
            .await?;
        }
    }
    super::audit::log_sea(db, Some(actor_id), "CREATE", "attendance.swap", Some(&rid.to_string()), None, None, None).await?;
    to_dto_int(rid, "swap.id")
}

pub async fn my_swaps_sea(db: &sea_orm::DatabaseConnection, employee_id: i64) -> Result<Vec<Swap>, String> {
    let rows = q_all(
        db,
        format!("{SWAP_SELECT} WHERE (ss.employee_id = ?1 OR ss.target_employee_id = ?1) ORDER BY ss.id DESC"),
        vec![Value::Int(employee_id)],
        13,
        "attendance.swap.mine",
    )
    .await
    .map_err(|e| format!("gagal membaca pengajuan: {e}"))?;
    rows.iter().map(|r| swap_row(r)).collect()
}

pub async fn pending_swaps_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Swap>, String> {
    let rows = q_all(
        db,
        format!("{SWAP_SELECT} WHERE ss.status = 'pending' ORDER BY ss.id DESC"),
        vec![],
        13,
        "attendance.swap.pending",
    )
    .await
    .map_err(|e| format!("gagal membaca antrean: {e}"))?;
    rows.iter().map(|r| swap_row(r)).collect()
}

pub async fn all_swaps_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Swap>, String> {
    let rows = q_all(
        db,
        format!("{SWAP_SELECT} ORDER BY ss.id DESC"),
        vec![],
        13,
        "attendance.swap.all",
    )
    .await
    .map_err(|e| format!("gagal membaca riwayat: {e}"))?;
    rows.iter().map(|r| swap_row(r)).collect()
}

pub async fn decide_swap_sea(
    db: &sea_orm::DatabaseConnection,
    approver_user_id: i64,
    approver_employee_id: Option<i64>,
    is_privileged: bool,
    swap_id: i64,
    decision: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let rows = q_all(
        db,
        "SELECT employee_id, target_employee_id, shift_date, from_shift_id, to_shift_id, status FROM shift_swaps WHERE id = ?1".to_string(),
        vec![Value::Int(swap_id)],
        6,
        "attendance.swap.decide.read",
    )
    .await
    .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let r = rows.into_iter().next().ok_or_else(|| "Pengajuan tukar shift tidak ditemukan.".to_string())?;
    let (emp_id, target_id, date, from_shift, to_shift, status) = (
        sea_int(&r, 0), sea_int(&r, 1), sea_text(&r, 2),
        sea_opt_int(&r, 3), sea_opt_int(&r, 4), sea_text(&r, 5),
    );
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let supervisor = q_all(
        db,
        "SELECT supervisor_id FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(emp_id)],
        1,
        "attendance.swap.decide.sup",
    )
    .await
    .map_err(|e| format!("gagal memuat supervisor: {e}"))?
    .into_iter()
    .next()
    .and_then(|x| sea_opt_int(&x, 0));
    if approver_employee_id != supervisor && !is_privileged {
        return Err("Hanya supervisor atau HR yang boleh memutuskan.".to_string());
    }
    if decision == "approved" {
        let a = swap_shift_id_of(db, emp_id, &date).await?;
        let b = swap_shift_id_of(db, target_id, &date).await?;
        if a != from_shift || b != to_shift {
            return Err("Penugasan pada tanggal itu sudah berubah.".to_string());
        }
        exec(
            db,
            "UPDATE shift_assignments SET shift_id = ?1 WHERE employee_id = ?2 AND date = ?3 AND shift_id = ?4".to_string(),
            vec![Value::Int(to_shift.unwrap()), Value::Int(emp_id), sea_text_val(&date), Value::Int(from_shift.unwrap())],
            "attendance.swap.apply.a",
        )
        .await
        .map_err(|e| format!("gagal menerapkan tukar: {e}"))?;
        exec(
            db,
            "UPDATE shift_assignments SET shift_id = ?1 WHERE employee_id = ?2 AND date = ?3 AND shift_id = ?4".to_string(),
            vec![Value::Int(from_shift.unwrap()), Value::Int(target_id), sea_text_val(&date), Value::Int(to_shift.unwrap())],
            "attendance.swap.apply.b",
        )
        .await
        .map_err(|e| format!("gagal menerapkan tukar: {e}"))?;
    }
    let now = now_str();
    exec(
        db,
        "UPDATE shift_swaps SET status = ?1, decided_by = ?2, decided_at = ?3, notes = ?4 WHERE id = ?5".to_string(),
        vec![sea_text_val(decision), Value::Int(approver_user_id), sea_text_val(&now), sea_opt_str(notes), Value::Int(swap_id)],
        "attendance.swap.decide",
    )
    .await
    .map_err(|e| format!("gagal menyimpan keputusan: {e}"))?;
    for emp in [emp_id, target_id] {
        if let Some(uid) = user_of_employee_sea(db, emp).await? {
            notify_sea(
                db,
                uid,
                "attendance_swap",
                if decision == "approved" { "Tukar Shift Disetujui" } else { "Tukar Shift Ditolak" },
                &format!("Pengajuan tukar shift tanggal {date} telah {decision}."),
                "/attendance",
            )
            .await?;
        }
    }
    super::audit::log_sea(db, Some(approver_user_id), decision.to_uppercase().as_str(), "attendance.swap", Some(&swap_id.to_string()), None, None, None).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn one(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> i64 {
        q_one(db, sql.to_string(), vec![], 1, label)
            .await
            .expect("one")
            .and_then(|r| value_i64(&r[0]))
            .expect("id")
    }

    async fn opt_text(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> Option<String> {
        q_one(db, sql.to_string(), vec![], 1, label)
            .await
            .expect("q")
            .map(|r| match &r[0] {
                Value::Null => None,
                v => Some(value_to_string(v)),
            })
            .flatten()
    }

    async fn mkemp(db: &sea_orm::DatabaseConnection, number: &str, supervisor: Option<i64>) -> i64 {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, supervisor_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, ?2, '2026-01-01', 'active', 'permanent')".to_string(),
            vec![Value::Text(number.to_string()), match supervisor { Some(s) => Value::Int(s), None => Value::Null }],
            "test.mkemp",
        )
        .await
        .expect("emp");
        one(db, "SELECT last_insert_rowid()", "test.rowid").await
    }

    async fn shift_now(db: &sea_orm::DatabaseConnection, name: &str, back_h: i64, fwd_h: i64) -> i64 {
        let now = Local::now().naive_local();
        let start = (now - chrono::Duration::hours(back_h))
            .format("%H:%M:%S")
            .to_string();
        let end = (now + chrono::Duration::hours(fwd_h))
            .format("%H:%M:%S")
            .to_string();
        shift_save_sea(
            db,
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
        .await
        .expect("shift") as i64
    }

    async fn assign_week(db: &sea_orm::DatabaseConnection, emp: i64, shift: i64) {
        let sched = schedule_save_sea(
            db,
            1,
            None,
            &ScheduleInput {
                name: "Uji".to_string(),
                description: None,
            },
        )
        .await
        .expect("jadwal") as i64;
        let days: Vec<(i64, Option<i64>, bool)> = (0..=6).map(|d| (d, Some(shift), true)).collect();
        schedule_save_days_sea(db, 1, sched, &days).await.expect("hari");
        exec(
            db,
            "INSERT INTO shift_assignments (employee_id, work_schedule_id, start_date) VALUES (?1, ?2, ?3)".to_string(),
            vec![Value::Int(emp), Value::Int(sched), Value::Text(today_str())],
            "test.assign",
        )
        .await
        .expect("assign");
    }

    async fn clock_ok(
        db: &sea_orm::DatabaseConnection,
        files: &Path,
        actor: i64,
        emp: i64,
        lat: Option<f64>,
        lng: Option<f64>,
    ) -> ClockResult {
        let ch = liveness_challenge_sea(db, emp).await.expect("challenge");
        let photo = FileUpload {
            name: "wajah.png".to_string(),
            mime: "image/png".to_string(),
            bytes: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 3],
        };
        clock_in_sea(db, actor, emp, lat, lng, None, files, Some(&photo), Some(&ch.nonce))
            .await
            .expect("in")
    }

    #[tokio::test]
    async fn shift_crud_dan_guard_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let sid = shift_save_sea(
            db,
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
        .await
        .expect("buat");
        assert_eq!(shift_list_sea(db).await.expect("list").len(), 3);
        assert!(shift_save_sea(
            db,
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
        .await
        .is_err());
        // pakai di jadwal lalu hapus harus ditolak
        let sched = schedule_save_sea(
            db,
            1,
            None,
            &ScheduleInput {
                name: "S".to_string(),
                description: None,
            },
        )
        .await
        .expect("jadwal") as i64;
        schedule_save_days_sea(db, 1, sched, &[(1, Some(sid as i64), true)])
            .await
            .expect("hari");
        assert!(shift_delete_sea(db, 1, sid as i64).await.is_err());
        // lepas dari hari lalu hapus berhasil
        schedule_save_days_sea(db, 1, sched, &[(1, None, true)])
            .await
            .expect("lepas");
        shift_delete_sea(db, 1, sid as i64).await.expect("hapus");
        assert_eq!(shift_list_sea(db).await.expect("list2").len(), 2);
    }

    #[tokio::test]
    async fn resolve_override_menang_atas_mingguan_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let emp = one(
            db,
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            "test.emp",
        )
        .await;
        let weekly = shift_now(db, "Mingguan", 1, 8).await;
        assign_week(db, emp, weekly).await;
        let today = today_str();
        let got = resolve_shift_sea(db, emp, &today)
            .await
            .expect("resolve")
            .expect("ada");
        assert_eq!(got.id, weekly);
        // override tanggal spesifik
        let special = shift_now(db, "Khusus", 1, 8).await;
        exec(
            db,
            "INSERT INTO shift_assignments (employee_id, shift_id, date, start_date) VALUES (?1, ?2, ?3, ?4)".to_string(),
            vec![Value::Int(emp), Value::Int(special), Value::Text(today.clone()), Value::Text(today.clone())],
            "test.override",
        )
        .await
        .expect("ovr");
        let got2 = resolve_shift_sea(db, emp, &today)
            .await
            .expect("resolve")
            .expect("ada");
        assert_eq!(got2.id, special);
    }

    #[tokio::test]
    async fn clock_in_out_menghitung_menit_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let emp = mkemp(db, "EMP-T2", None).await;
        // tanpa shift: present
        let r = clock_ok(db, dir.path(), actor, emp, None, None).await;
        assert_eq!(r.status, "present");
        assert!(clock_in_sea(db, actor, emp, None, None, None, dir.path(), None, None).await.is_err());
        let r = clock_out_sea(db, actor, emp, None, None, None).await.expect("out");
        assert_eq!(r.status, "present");
        assert!(clock_out_sea(db, actor, emp, None, None, None).await.is_err());
        // dengan shift (masuk 1 menit lalu -> telat; pulang 2 jam lagi -> awal).
        // bila pulang melewati tengah malam, cukup telat.
        let emp2 = mkemp(db, "EMP-T3", None).await;
        let now2 = Local::now().naive_local();
        let start_ago = (now2 - chrono::Duration::minutes(1))
            .format("%H:%M:%S")
            .to_string();
        let end_fwd = (now2 + chrono::Duration::hours(2))
            .format("%H:%M:%S")
            .to_string();
        let wraps = end_fwd <= start_ago;
        let shift = shift_save_sea(
            db,
            1,
            None,
            &ShiftInput {
                name: "Siang".to_string(),
                start_time: start_ago,
                end_time: end_fwd,
                break_start: None,
                break_end: None,
                grace_period_minutes: 0,
                is_overnight: false,
            },
        )
        .await
        .expect("shift") as i64;
        assign_week(db, emp2, shift).await;
        let r = clock_ok(db, dir.path(), actor, emp2, Some(-6.2), Some(106.8)).await;
        assert_eq!(r.status, "late");
        let att = today_sea(db, emp2).await.expect("today").expect("ada");
        assert!(att.late_minutes >= 1);
        let r = clock_out_sea(db, actor, emp2, None, None, None)
            .await
            .expect("out2");
        assert_eq!(r.status, "late");
        if !wraps {
            let early = one(
                db,
                &format!(
                    "SELECT early_minutes FROM attendances WHERE employee_id = {emp2} AND date = '{}'",
                    today_str()
                ),
                "test.early",
            )
            .await;
            assert!(early > 60);
        }
        // pulang-cepat: masuk dalam toleransi, pulang sebelum shift berakhir.
        // bila jendela melewati tengah malam, cukup verifikasi clock out sukses.
        let emp3 = mkemp(db, "EMP-T3B", None).await;
        let now = Local::now().naive_local();
        let end_dt = now + chrono::Duration::hours(2);
        let wraps = end_dt.date() != now.date();
        let start = (now - chrono::Duration::minutes(10))
            .format("%H:%M:%S")
            .to_string();
        let end = end_dt.format("%H:%M:%S").to_string();
        let sh = shift_save_sea(
            db,
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
        .await
        .expect("shift") as i64;
        assign_week(db, emp3, sh).await;
        let r = clock_ok(db, dir.path(), actor, emp3, None, None).await;
        assert_eq!(r.status, "present");
        let r = clock_out_sea(db, actor, emp3, None, None, None)
            .await
            .expect("out3");
        if !wraps {
            assert_eq!(r.status, "early_checkout");
        }
    }

    #[tokio::test]
    async fn clock_ditolak_libur_dan_nonaktif_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let emp = mkemp(db, "EMP-T4", None).await;
        exec(
            db,
            "INSERT INTO holidays (name, date, type) VALUES ('Uji', date('now','localtime'), 'national')".to_string(),
            vec![],
            "test.holiday",
        )
        .await
        .expect("libur");
        let e = clock_in_sea(db, actor, emp, None, None, None, dir.path(), None, None)
            .await
            .expect_err("libur");
        assert!(e.contains("libur"));
        exec(db, "DELETE FROM holidays".to_string(), vec![], "test.unlibur")
            .await
            .expect("del");
        exec(
            db,
            "UPDATE employees SET employment_status = 'resigned' WHERE id = ?1".to_string(),
            vec![Value::Int(emp)],
            "test.resign",
        )
        .await
        .expect("upd");
        let e = clock_in_sea(db, actor, emp, None, None, None, dir.path(), None, None)
            .await
            .expect_err("nonaktif");
        assert!(e.contains("tidak aktif"));
    }

    #[tokio::test]
    async fn manual_dan_rekap_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let emp = mkemp(db, "EMP-T5", None).await;
        let input = ManualInput {
            employee_id: to_dto_int(emp, "e").unwrap(),
            date: "2026-03-02".to_string(),
            clock_in: Some("08:00".to_string()),
            clock_out: Some("17:00".to_string()),
            status: "present".to_string(),
            notes: None,
        };
        manual_entry_sea(db, actor, &input).await.expect("manual");
        manual_entry_sea(db, actor, &input).await.expect("upsert");
        let hist = history_sea(db, emp, "2026-03").await.expect("hist");
        assert_eq!(hist.len(), 1);
        assert_eq!(hist[0].clock_in.as_deref(), Some("2026-03-02 08:00"));
        assert!(history_sea(db, emp, "bulan-salah").await.is_err());
        let semua = recap_sea(db, "2026-03-02", "").await.expect("rekap");
        assert!(semua.iter().any(|r| r.employee_id as i64 == emp));
        let cari = recap_sea(db, "2026-03-02", "zzz-tidak-ada")
            .await
            .expect("cari");
        assert!(cari.is_empty());
        let bad = ManualInput {
            status: "ngawur".to_string(),
            ..input
        };
        assert!(manual_entry_sea(db, actor, &bad).await.is_err());
    }

    #[tokio::test]
    async fn koreksi_sampai_keputusan_dan_notif_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let sup = one(
            db,
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            "test.sup",
        )
        .await;
        let emp = mkemp(db, "EMP-T6", Some(sup)).await;
        let mut input = CorrectionInput {
            date: "2026-03-03".to_string(),
            requested_clock_in: Some("08:05".to_string()),
            requested_clock_out: Some("17:00".to_string()),
            reason: "Lupa absen masuk".to_string(),
        };
        let cid = request_correction_sea(db, actor, emp, &input)
            .await
            .expect("aju");
        assert_eq!(
            my_corrections_sea(db, emp).await.expect("mine").len(),
            1
        );
        assert_eq!(
            pending_corrections_sea(db).await.expect("pending").len(),
            1
        );
        // notifikasi ke admin (supervisor)
        let notif = one(
            db,
            &format!(
                "SELECT COUNT(*) FROM notifications WHERE user_id = {actor} AND type = 'attendance_correction'"
            ),
            "test.notif",
        )
        .await;
        assert_eq!(notif, 1);
        // bukan supervisor dan bukan privileged ditolak
        let other = mkemp(db, "EMP-T7", None).await;
        let e = decide_correction_sea(db, actor, Some(other), false, cid as i64, "approved", None)
            .await
            .expect_err("ditolak");
        assert!(e.contains("supervisor"));
        // supervisor (admin) menyetujui
        decide_correction_sea(
            db,
            actor,
            Some(sup),
            false,
            cid as i64,
            "approved",
            Some("ok"),
        )
        .await
        .expect("setuju");
        let att = opt_text(
            db,
            &format!(
                "SELECT clock_in FROM attendances WHERE employee_id = {emp} AND date = '2026-03-03'"
            ),
            "test.applied",
        )
        .await;
        assert_eq!(att.as_deref(), Some("2026-03-03 08:05"));
        // sudah diproses tak bisa lagi
        assert!(
            decide_correction_sea(db, actor, Some(sup), false, cid as i64, "rejected", None)
                .await
                .is_err()
        );
        input.reason = String::new();
        assert!(request_correction_sea(db, actor, emp, &input)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn geofence_menolak_di_luar_radius() {
        let dir = tempfile::tempdir().expect("tmp");
        let state = crate::init_state(dir.path().to_path_buf()).expect("init");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        async fn ikat(db: &sea_orm::DatabaseConnection, emp: i64) {
            exec(
                db,
                "UPDATE employees SET work_location_id = 1 WHERE id = ?1".to_string(),
                vec![Value::Int(emp)],
                "test.ikat",
            )
            .await
            .expect("ikat");
        }
        // A terikat lokasi, clock tepat di titik kantor
        let a = mkemp(db, "EMP-GFA", None).await;
        ikat(db, a).await;
        let r = clock_ok(db, dir.path(), actor, a, Some(-6.224), Some(106.809)).await;
        assert_eq!(r.message, "Clock in berhasil.");
        // B terikat lokasi, clock 2.8 km dari kantor ditolak
        let b = mkemp(db, "EMP-GFB", None).await;
        ikat(db, b).await;
        let e = clock_in_sea(db, actor, b, Some(-6.2), Some(106.8), None, dir.path(), None, None)
            .await
            .expect_err("jauh");
        assert!(e.contains("jangkauan"), "pesan: {e}");
        // C terikat lokasi, clock tanpa koordinat ditolak
        let c = mkemp(db, "EMP-GFC", None).await;
        ikat(db, c).await;
        let e = clock_in_sea(db, actor, c, None, None, None, dir.path(), None, None)
            .await
            .expect_err("wajib");
        assert!(e.contains("wajib"), "pesan: {e}");
        // D tanpa lokasi, clock tanpa koordinat lolos
        let d = mkemp(db, "EMP-GFD", None).await;
        clock_ok(db, dir.path(), actor, d, None, None).await;
        // clock out A dari jauh ditolak, lalu dari titik kantor lolos
        let e = clock_out_sea(db, actor, a, Some(-6.2), Some(106.8), None)
            .await
            .expect_err("out jauh");
        assert!(e.contains("jangkauan"), "pesan: {e}");
        let r = clock_out_sea(db, actor, a, Some(-6.224), Some(106.809), None)
            .await
            .expect("out a");
        assert_eq!(r.message, "Clock out berhasil.");
    }

    #[tokio::test]
    async fn liveness_menolak_foto_daur_ulang() {
        let dir = tempfile::tempdir().expect("tmp");
        let state = crate::init_state(dir.path().to_path_buf()).expect("init");
        let db = &state.sea;
        let files = dir.path().join("files");
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let emp = mkemp(db, "EMP-LV", None).await;
        let foto = FileUpload {
            name: "wajah.png".to_string(),
            mime: "image/png".to_string(),
            bytes: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 3],
        };
        let ch = liveness_challenge_sea(db, emp).await.expect("challenge");
        assert!(!ch.nonce.is_empty() && !ch.instruction.is_empty());
        let r = clock_in_sea(db, actor, emp, None, None, None, &files, Some(&foto), Some(&ch.nonce))
            .await
            .expect("in");
        assert_eq!(r.message, "Clock in berhasil.");
        let rel = opt_text(db, &format!("SELECT clock_in_photo FROM attendances WHERE employee_id = {emp} AND date = date('now','localtime')"), "test.foto").await;
        assert!(rel.map(|p| p.starts_with("attendance/")).unwrap_or(false));
        let e = clock_in_sea(db, actor, emp, None, None, None, &files, Some(&foto), Some(&ch.nonce))
            .await
            .expect_err("duplikat");
        assert!(e.contains("Sudah clock in"), "pesan: {e}");
        let emp2 = mkemp(db, "EMP-LV2", None).await;
        let e = clock_in_sea(db, actor, emp2, None, None, None, &files, Some(&foto), Some(&ch.nonce))
            .await
            .expect_err("pakai ulang");
        assert!(e.contains("sudah dipakai"), "pesan: {e}");
        let ch2 = liveness_challenge_sea(db, emp2).await.expect("challenge2");
        exec(db, "UPDATE liveness_challenges SET issued_at = '2000-01-01 00:00:00' WHERE nonce = ?1".to_string(), vec![Value::Text(ch2.nonce.clone())], "test.expire")
            .await
            .expect("upd");
        let e = clock_in_sea(db, actor, emp2, None, None, None, &files, Some(&foto), Some(&ch2.nonce))
            .await
            .expect_err("kedaluwarsa");
        assert!(e.contains("kedaluwarsa"), "pesan: {e}");
        let ch3 = liveness_challenge_sea(db, emp2).await.expect("challenge3");
        let pdf = FileUpload {
            name: "wajah.pdf".to_string(),
            mime: "application/pdf".to_string(),
            bytes: vec![1, 2, 3],
        };
        let e = clock_in_sea(db, actor, emp2, None, None, None, &files, Some(&pdf), Some(&ch3.nonce))
            .await
            .expect_err("mime");
        assert!(e.contains("JPG atau PNG"), "pesan: {e}");
        let e = clock_in_sea(db, actor, emp2, None, None, None, &files, None, Some(&ch3.nonce))
            .await
            .expect_err("tanpa foto");
        assert!(e.contains("Swafoto wajib"), "pesan: {e}");
    }

    #[tokio::test]
    async fn alur_tukar_shift_dan_persetujuan() {
        let dir = tempfile::tempdir().expect("tmp");
        let state = crate::init_state(dir.path().to_path_buf()).expect("init");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let sup = mkemp(db, "SW-SUP", None).await;
        let a = mkemp(db, "SW-A", Some(sup)).await;
        let b = mkemp(db, "SW-B", Some(sup)).await;
        let c = mkemp(db, "SW-C", Some(sup)).await;
        let x = mkemp(db, "SW-X", None).await;
        let s1 = shift_now(db, "SW-Siang", 1, 3).await;
        let s2 = shift_now(db, "SW-Malam", 3, 6).await;
        let date = "2026-04-15";
        for (e, s) in [(a, s1), (b, s2)] {
            exec(
                db,
                "INSERT INTO shift_assignments (employee_id, shift_id, date, start_date) VALUES (?1, ?2, ?3, ?3)".to_string(),
                vec![Value::Int(e), Value::Int(s), Value::Text(date.to_string())],
                "test.swapassign",
            )
            .await
            .expect("assign");
        }
        let inp = |t: i64, d: &str, r: &str| SwapInput {
            target_employee_id: t as i32,
            shift_date: d.to_string(),
            reason: r.to_string(),
        };
        assert!(swap_request_sea(db, actor, a, &inp(a, date, "uji")).await.is_err(), "diri sendiri");
        let e = swap_request_sea(db, actor, a, &inp(b, "2026-13-99", "uji")).await.expect_err("tanggal");
        assert!(e.contains("Tanggal harus valid"), "pesan: {e}");
        let e = swap_request_sea(db, actor, c, &inp(b, date, "uji")).await.expect_err("tanpa penugasan");
        assert!(e.contains("penugasan"), "pesan: {e}");
        let sid = swap_request_sea(db, actor, a, &inp(b, date, "Acara keluarga")).await.expect("ajukan") as i64;
        assert!(pending_swaps_sea(db).await.expect("antrean").iter().any(|s| s.id as i64 == sid && s.status == "pending"));
        let e = swap_request_sea(db, actor, a, &inp(b, date, "lagi")).await.expect_err("ganda");
        assert!(e.contains("sudah ada"), "pesan: {e}");
        assert_eq!(my_swaps_sea(db, a).await.expect("punya saya").len(), 1);
        let e = decide_swap_sea(db, actor, Some(x), false, sid, "approved", None).await.expect_err("bukan berwenang");
        assert!(e.contains("supervisor"), "pesan: {e}");
        let e = decide_swap_sea(db, actor, Some(sup), false, sid, "mungkin", None).await.expect_err("keputusan");
        assert!(e.contains("Keputusan tidak valid"), "pesan: {e}");
        decide_swap_sea(db, actor, Some(sup), false, sid, "rejected", Some("tidak bisa")).await.expect("tolak");
        assert_eq!(opt_text(db, &format!("SELECT status FROM shift_swaps WHERE id = {sid}"), "st").await.as_deref(), Some("rejected"));
        assert_eq!(one(db, &format!("SELECT shift_id FROM shift_assignments WHERE employee_id = {a} AND date = '{date}'"), "g1").await, s1);
        let sid2 = swap_request_sea(db, actor, a, &inp(b, date, "Ronde dua")).await.expect("ajukan lagi") as i64;
        decide_swap_sea(db, actor, Some(sup), false, sid2, "approved", Some("ok")).await.expect("setuju");
        assert_eq!(one(db, &format!("SELECT shift_id FROM shift_assignments WHERE employee_id = {a} AND date = '{date}'"), "g2").await, s2);
        assert_eq!(one(db, &format!("SELECT shift_id FROM shift_assignments WHERE employee_id = {b} AND date = '{date}'"), "g3").await, s1);
        let e = decide_swap_sea(db, actor, Some(sup), false, sid2, "rejected", None).await.expect_err("proses ulang");
        assert!(e.contains("sudah diproses"), "pesan: {e}");
        assert_eq!(all_swaps_sea(db).await.expect("riwayat").len(), 2);
        let row = my_swaps_sea(db, b).await.expect("punya rekan")[0].clone();
        assert_eq!(row.status, "approved");
        assert_eq!(row.notes.as_deref(), Some("ok"));
    }

    #[tokio::test]
    async fn libur_nasional_otomatis_idempoten() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        assert_eq!(holidays_autofill_sea(db, actor, 2027).await.expect("isi"), 5);
        assert_eq!(holidays_autofill_sea(db, actor, 2027).await.expect("isi ulang"), 0);
        let rows = q_all(
            db,
            "SELECT name FROM holidays WHERE date = '2027-08-17'".to_string(),
            vec![],
            1,
            "cek libur",
        )
        .await
        .expect("q");
        assert_eq!(rows.len(), 1);
        assert_eq!(value_to_string(&rows[0][0]), "Hari Kemerdekaan Republik Indonesia");
        let e = holidays_autofill_sea(db, actor, 1900).await.expect_err("tahun");
        assert!(e.contains("Tahun harus"));
    }
}
