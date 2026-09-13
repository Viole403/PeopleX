//! Penggajian: komponen, periode, generate, alur status, kasbon, slip.

use chrono::{Datelike, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use super::approval;
use super::audit;
use crate::to_dto_int;

// ---------------- PTKP ----------------

fn ptkp(status: &str) -> f64 {
    match status {
        "TK/0" => 54_000_000.0,
        "TK/1" => 58_500_000.0,
        "TK/2" => 63_000_000.0,
        "TK/3" => 67_500_000.0,
        "K/0" => 58_500_000.0,
        "K/1" => 63_000_000.0,
        "K/2" => 67_500_000.0,
        "K/3" => 72_000_000.0,
        _ => 54_000_000.0,
    }
}

/// Estimasi PPh21 bulanan: progresif atas (bruto kena pajak x12 - biaya jabatan - PTKP).
/// BUKAN mesin pajak resmi — selalu dilabeli estimasi di slip.
pub fn pph21_monthly(monthly_taxable_gross: f64, ptkp_status: &str) -> f64 {
    let annual_gross = monthly_taxable_gross * 12.0;
    let job_cost = (annual_gross * 0.05).min(6_000_000.0);
    let taxable = (annual_gross - job_cost - ptkp(ptkp_status)).max(0.0);
    let brackets = [
        (60_000_000.0, 0.05),
        (250_000_000.0, 0.15),
        (500_000_000.0, 0.25),
        (5_000_000_000.0, 0.30),
        (f64::MAX, 0.35),
    ];
    let mut tax = 0.0;
    let mut remaining = taxable;
    let mut lower = 0.0;
    for (upper, rate) in brackets {
        if remaining <= 0.0 {
            break;
        }
        let take = remaining.min(upper - lower);
        tax += take * rate;
        remaining -= take;
        lower = upper;
    }
    (tax / 12.0).round()
}

/// Lembur = basic/173 x multiplier x jam (hanya approved dalam periode).
pub fn overtime_amount(
    conn: &Connection,
    employee_id: i64,
    start: &str,
    end: &str,
    basic: f64,
) -> Result<f64, String> {
    let mut stmt = conn
        .prepare("SELECT duration_minutes, rate_multiplier FROM overtime_requests WHERE employee_id = ?1 AND status = 'approved' AND date BETWEEN ?2 AND ?3")
        .map_err(|e| format!("gagal memuat lembur: {e}"))?;
    let rows = stmt
        .query_map(params![employee_id, start, end], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))
        })
        .map_err(|e| format!("gagal membaca lembur: {e}"))?;
    let hourly = basic / 173.0;
    let mut total = 0.0;
    for row in rows {
        let (mins, mult) = row.map_err(|e| format!("gagal membaca baris lembur: {e}"))?;
        total += hourly * mult * (mins as f64 / 60.0);
    }
    Ok(total.round())
}

/// Potongan absen = basic/22 x hari absent dalam periode.
pub fn absence_deduction(
    conn: &Connection,
    employee_id: i64,
    start: &str,
    end: &str,
    basic: f64,
) -> Result<f64, String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM attendances WHERE employee_id = ?1 AND status = 'absent' AND date BETWEEN ?2 AND ?3",
            params![employee_id, start, end],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung absen: {e}"))?;
    if n == 0 {
        return Ok(0.0);
    }
    Ok(((basic / 22.0) * n as f64).round())
}

/// THR proporsional: gaji pokok x masa kerja dalam bulan (maks 12) / 12.
/// Hanya dihitung bila tanggal hari raya dari settings jatuh dalam periode.
/// Setting kosong berarti THR nonaktif.
fn thr_for_period(
    conn: &Connection,
    basic: f64,
    join_date: &str,
    pstart: &str,
    pend: &str,
) -> Result<f64, String> {
    let holiday: Option<String> = conn
        .query_row(
            "SELECT setting_value FROM system_settings WHERE setting_key = 'thr_holiday_date'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat tanggal hari raya: {e}"))?
        .flatten()
        .map(|s: String| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(h) = holiday else {
        return Ok(0.0);
    };
    if h.as_str() < pstart || h.as_str() > pend {
        return Ok(0.0);
    }
    let (Ok(j), Ok(hd)) = (
        NaiveDate::parse_from_str(join_date.trim(), "%Y-%m-%d"),
        NaiveDate::parse_from_str(&h, "%Y-%m-%d"),
    ) else {
        return Ok(0.0);
    };
    if hd < j {
        return Ok(0.0);
    }
    let months =
        ((hd.year() * 12 + hd.month() as i32) - (j.year() * 12 + j.month() as i32)).clamp(0, 12);
    if months <= 0 {
        return Ok(0.0);
    }
    Ok((basic * months as f64 / 12.0).round())
}

fn setting_pct(conn: &Connection, key: &str, fallback: f64) -> f64 {
    conn.query_row(
        "SELECT setting_value FROM system_settings WHERE setting_key = ?1",
        params![key],
        |r| r.get::<_, Option<String>>(0),
    )
    .optional()
    .unwrap_or(None)
    .flatten()
    .and_then(|v| v.parse::<f64>().ok())
    .unwrap_or(fallback)
}

/// Dasar upah untuk iuran: gaji pokok dibatasi maksimal bila setting batas > 0.
/// Nilai 0 atau tidak valid berarti tanpa batas.
fn capped_base(conn: &Connection, basic: f64, key: &str) -> Result<f64, String> {
    let cap: Option<f64> = conn
        .query_row(
            "SELECT setting_value FROM system_settings WHERE setting_key = ?1",
            params![key],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat batas upah: {e}"))?
        .flatten()
        .and_then(|v| v.parse::<f64>().ok());
    Ok(match cap {
        Some(m) if m > 0.0 => basic.min(m),
        _ => basic,
    })
}

// ---------------- DTO ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Component {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub component_type: String,
    pub calculation_type: String,
    pub is_taxable: bool,
    pub is_active: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ComponentInput {
    pub code: String,
    pub name: String,
    pub component_type: String,
    pub calculation_type: String,
    pub is_taxable: bool,
    pub is_active: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Period {
    pub id: i32,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub payment_date: Option<String>,
    pub status: String,
    pub employee_count: i32,
    pub total_net: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct PeriodInput {
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub payment_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PayrollRow {
    pub id: i32,
    pub employee_id: i32,
    pub employee_number: String,
    pub name: String,
    pub basic_salary: f64,
    pub total_income: f64,
    pub total_deduction: f64,
    pub net_salary: f64,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PayrollLine {
    pub component_name: String,
    pub line_type: String,
    pub amount: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PayrollDetail {
    pub id: i32,
    pub employee_id: i32,
    pub employee_number: String,
    pub name: String,
    pub department_name: Option<String>,
    pub position_name: Option<String>,
    pub period_name: String,
    pub start_date: String,
    pub end_date: String,
    pub basic_salary: f64,
    pub total_income: f64,
    pub total_deduction: f64,
    pub net_salary: f64,
    pub status: String,
    pub lines: Vec<PayrollLine>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Deduction {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub deduction_type: String,
    pub description: String,
    pub amount: f64,
    pub installment_no: Option<i32>,
    pub total_installments: Option<i32>,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct DeductionInput {
    pub employee_id: i32,
    pub deduction_type: String,
    pub description: String,
    pub amount: f64,
    pub installment_no: Option<i32>,
    pub total_installments: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PayslipInfo {
    pub id: i32,
    pub payroll_id: i32,
    pub payslip_number: String,
    pub period_name: String,
    pub net_salary: f64,
    pub pdf_ready: bool,
}

// ---------------- Komponen ----------------

pub fn component_list(conn: &Connection) -> Result<Vec<Component>, String> {
    let mut stmt = conn
        .prepare("SELECT id, code, name, type, calculation_type, is_taxable, is_active FROM salary_components ORDER BY type ASC, name ASC")
        .map_err(|e| format!("gagal menyiapkan komponen: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
            ))
        })
        .map_err(|e| format!("gagal membaca komponen: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name, ctype, calc, tax, active) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Component {
            id: to_dto_int(id, "component.id")?,
            code,
            name,
            component_type: ctype,
            calculation_type: calc,
            is_taxable: tax != 0,
            is_active: active != 0,
        });
    }
    Ok(out)
}

pub fn component_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &ComponentInput,
) -> Result<i32, String> {
    if input.code.trim().is_empty() || input.name.trim().is_empty() {
        return Err("Kode dan nama komponen wajib diisi.".to_string());
    }
    if input.code.len() > 30 {
        return Err("Kode maksimal 30 karakter.".to_string());
    }
    if !["income", "deduction"].contains(&input.component_type.as_str()) {
        return Err("Jenis komponen tidak valid.".to_string());
    }
    if !["fixed", "percentage", "formula"].contains(&input.calculation_type.as_str()) {
        return Err("Tipe perhitungan tidak valid.".to_string());
    }
    let tax = if input.is_taxable { 1 } else { 0 };
    let active = if input.is_active { 1 } else { 0 };
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE salary_components SET code = ?1, name = ?2, type = ?3, calculation_type = ?4, is_taxable = ?5, is_active = ?6 WHERE id = ?7",
            params![input.code.trim(), input.name.trim(), input.component_type, input.calculation_type, tax, active, rid],
        ).map_err(|e| format!("gagal menyimpan komponen: {e}"))?;
        if n == 0 {
            return Err("Komponen tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "payroll.component",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "component.id")
    } else {
        conn.execute(
            "INSERT INTO salary_components (code, name, type, calculation_type, is_taxable, is_active) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![input.code.trim(), input.name.trim(), input.component_type, input.calculation_type, tax, active],
        ).map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Kode komponen sudah dipakai.".to_string()
            } else {
                format!("gagal menambah komponen: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "payroll.component",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "component.id")
    }
}

pub fn component_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM employee_salary_components WHERE salary_component_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if n > 0 {
        return Err("Komponen masih dipakai data gaji.".to_string());
    }
    let d = conn
        .execute("DELETE FROM salary_components WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus komponen: {e}"))?;
    if d == 0 {
        return Err("Komponen tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "payroll.component",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Periode ----------------

pub fn period_list(conn: &Connection) -> Result<Vec<Period>, String> {
    let mut stmt = conn
        .prepare("SELECT pp.id, pp.name, pp.start_date, pp.end_date, pp.payment_date, pp.status,
                         (SELECT COUNT(*) FROM payrolls p WHERE p.payroll_period_id = pp.id),
                         COALESCE((SELECT SUM(p.net_salary) FROM payrolls p WHERE p.payroll_period_id = pp.id), 0)
                  FROM payroll_periods pp ORDER BY pp.start_date DESC")
        .map_err(|e| format!("gagal menyiapkan periode: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, f64>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca periode: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, start, end, pay, status, count, net) =
            row.map_err(|e| format!("gagal membaca baris periode: {e}"))?;
        out.push(Period {
            id: to_dto_int(id, "period.id")?,
            name,
            start_date: start,
            end_date: end,
            payment_date: pay,
            status,
            employee_count: to_dto_int(count, "period.count")?,
            total_net: net,
        });
    }
    Ok(out)
}

pub fn period_create(
    conn: &Connection,
    actor_id: i64,
    created_by: i64,
    input: &PeriodInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama periode wajib diisi.".to_string());
    }
    if input.name.len() > 100 {
        return Err("Nama periode maksimal 100 karakter.".to_string());
    }
    NaiveDate::parse_from_str(input.start_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.end_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if input.end_date.trim() < input.start_date.trim() {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    if let Some(pay) = input
        .payment_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        NaiveDate::parse_from_str(pay, "%Y-%m-%d")
            .map_err(|_| "Tanggal bayar tidak valid.".to_string())?;
    }
    let overlap: Option<i64> = conn
        .query_row(
            "SELECT id FROM payroll_periods WHERE NOT (end_date < ?1 OR start_date > ?2)",
            params![input.start_date.trim(), input.end_date.trim()],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa tumpang tindih: {e}"))?;
    if overlap.is_some() {
        return Err("Periode bertabrakan dengan periode yang ada.".to_string());
    }
    conn.execute(
        "INSERT INTO payroll_periods (name, start_date, end_date, payment_date, status, created_by) VALUES (?1, ?2, ?3, ?4, 'draft', ?5)",
        params![
            input.name.trim(),
            input.start_date.trim(),
            input.end_date.trim(),
            input.payment_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            created_by,
        ],
    )
    .map_err(|e| format!("gagal membuat periode: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "payroll.period",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "period.id")
}

fn period_status(conn: &Connection, id: i64) -> Result<String, String> {
    conn.query_row(
        "SELECT status FROM payroll_periods WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|e| format!("gagal memuat periode: {e}"))?
    .ok_or("Periode tidak ditemukan.".to_string())
}

/// Generate: hitung seluruh karyawan aktif dalam satu transaksi, lalu review.
pub fn generate(conn: &mut Connection, actor_id: i64, period_id: i64) -> Result<(), String> {
    if period_status(conn, period_id)? != "draft" {
        return Err("Generate hanya untuk periode draft.".to_string());
    }
    let period: (String, String, String) = conn
        .query_row(
            "SELECT name, start_date, end_date FROM payroll_periods WHERE id = ?1",
            params![period_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("gagal memuat periode: {e}"))?;
    let (pname, pstart, pend) = period;
    let health_pct = setting_pct(conn, "bpjs_health_employee_percent", 1.0) / 100.0;
    let emp_pct = setting_pct(conn, "bpjs_employment_employee_percent", 2.0) / 100.0;
    let jp_pct = setting_pct(conn, "bpjs_jp_employee_percent", 1.0) / 100.0;
    let tx = conn
        .transaction()
        .map_err(|e| format!("gagal memulai transaksi: {e}"))?;
    let ids: Vec<i64> = {
        let mut emps = tx
            .prepare("SELECT id FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation')")
            .map_err(|e| format!("gagal menyiapkan karyawan: {e}"))?;
        let mut rows = emps
            .query([])
            .map_err(|e| format!("gagal membaca karyawan: {e}"))?;
        let mut ids = Vec::new();
        while let Some(r) = rows
            .next()
            .map_err(|e| format!("gagal membaca karyawan: {e}"))?
        {
            ids.push(
                r.get(0)
                    .map_err(|e| format!("gagal membaca karyawan: {e}"))?,
            );
        }
        ids
    };
    for eid in ids {
        generate_one(
            &tx, eid, period_id, &pstart, &pend, health_pct, emp_pct, jp_pct,
        )?;
    }
    tx.execute(
        "UPDATE payroll_periods SET status = 'review' WHERE id = ?1",
        params![period_id],
    )
    .map_err(|e| format!("gagal menandai review: {e}"))?;
    tx.commit()
        .map_err(|e| format!("gagal commit generate: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "GENERATE",
        "payroll.period",
        Some(&period_id.to_string()),
        None,
        None,
        Some(&format!("Generate periode {pname}")),
    )?;
    Ok(())
}

struct MoneyLine {
    name: String,
    component_id: Option<i64>,
    amount: f64,
    loan_id: Option<i64>,
}

fn save_lines(
    conn: &Connection,
    payroll_id: i64,
    period_id: i64,
    lines: &[MoneyLine],
    line_type: &str,
) -> Result<(), String> {
    for line in lines {
        conn.execute(
            "INSERT INTO payroll_details (payroll_id, salary_component_id, component_name, type, amount) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![payroll_id, line.component_id, line.name, line_type, line.amount],
        )
        .map_err(|e| format!("gagal menyimpan rincian: {e}"))?;
        if let Some(lid) = line.loan_id {
            conn.execute(
                "UPDATE payroll_deductions SET status = 'processed', payroll_period_id = ?1 WHERE id = ?2",
                params![period_id, lid],
            )
            .map_err(|e| format!("gagal memproses kasbon: {e}"))?;
        }
    }
    Ok(())
}

fn generate_one(
    conn: &Connection,
    employee_id: i64,
    period_id: i64,
    pstart: &str,
    pend: &str,
    health_pct: f64,
    emp_pct: f64,
    jp_pct: f64,
) -> Result<(), String> {
    let salary: Option<(i64, f64)> = conn
        .query_row(
            "SELECT id, basic_salary FROM employee_salaries WHERE employee_id = ?1 AND is_active = 1 AND effective_date <= ?2 ORDER BY effective_date DESC LIMIT 1",
            params![employee_id, pend],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat gaji: {e}"))?;
    let Some((salary_id, basic)) = salary else {
        return Ok(());
    };
    let ptkp: Option<String> = conn
        .query_row(
            "SELECT ptkp_status FROM employees WHERE id = ?1",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat PTKP: {e}"))?
        .flatten();
    let join_date: String = conn
        .query_row(
            "SELECT join_date FROM employees WHERE id = ?1",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat tanggal masuk: {e}"))?
        .flatten()
        .unwrap_or_default();
    let mut incomes = vec![MoneyLine {
        name: "Gaji Pokok".to_string(),
        component_id: None,
        amount: basic,
        loan_id: None,
    }];
    let mut taxable_extra = 0.0;
    let mut stmt = conn
        .prepare("SELECT esc.salary_component_id, sc.name, sc.is_taxable, esc.amount FROM employee_salary_components esc INNER JOIN salary_components sc ON sc.id = esc.salary_component_id WHERE esc.employee_salary_id = ?1 AND sc.type = 'income' AND sc.is_active = 1")
        .map_err(|e| format!("gagal menyiapkan komponen: {e}"))?;
    for row in stmt
        .query_map(params![salary_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, f64>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca komponen: {e}"))?
    {
        let (cid, name, taxable, amount) =
            row.map_err(|e| format!("gagal membaca baris komponen: {e}"))?;
        incomes.push(MoneyLine {
            name,
            component_id: Some(cid),
            amount,
            loan_id: None,
        });
        if taxable != 0 {
            taxable_extra += amount;
        }
    }
    let mut deductions: Vec<MoneyLine> = Vec::new();
    let ot = overtime_amount(conn, employee_id, pstart, pend, basic)?;
    if ot > 0.0 {
        incomes.push(MoneyLine {
            name: "Lembur".to_string(),
            component_id: None,
            amount: ot,
            loan_id: None,
        });
    }
    let thr = thr_for_period(conn, basic, &join_date, pstart, pend)?;
    if thr > 0.0 {
        incomes.push(MoneyLine {
            name: "THR".to_string(),
            component_id: None,
            amount: thr,
            loan_id: None,
        });
        taxable_extra += thr;
    }
    let absence = absence_deduction(conn, employee_id, pstart, pend, basic)?;
    if absence > 0.0 {
        deductions.push(MoneyLine {
            name: "Potongan Absensi".to_string(),
            component_id: None,
            amount: absence,
            loan_id: None,
        });
    }
    let bpjs_h = (capped_base(conn, basic, "bpjs_health_max_wage")? * health_pct).round();
    let bpjs_e = (capped_base(conn, basic, "bpjs_jht_max_wage")? * emp_pct).round();
    let bpjs_jp = (capped_base(conn, basic, "bpjs_jp_max_wage")? * jp_pct).round();
    if bpjs_h > 0.0 {
        deductions.push(MoneyLine {
            name: "BPJS Kesehatan".to_string(),
            component_id: None,
            amount: bpjs_h,
            loan_id: None,
        });
    }
    if bpjs_e > 0.0 {
        deductions.push(MoneyLine {
            name: "BPJS Jaminan Hari Tua".to_string(),
            component_id: None,
            amount: bpjs_e,
            loan_id: None,
        });
    }
    if bpjs_jp > 0.0 {
        deductions.push(MoneyLine {
            name: "BPJS Jaminan Pensiun".to_string(),
            component_id: None,
            amount: bpjs_jp,
            loan_id: None,
        });
    }
    let pph = pph21_monthly(
        basic + taxable_extra + ot,
        ptkp.as_deref().unwrap_or("TK/0"),
    );
    if pph > 0.0 {
        deductions.push(MoneyLine {
            name: "PPh 21 (estimasi)".to_string(),
            component_id: None,
            amount: pph,
            loan_id: None,
        });
    }
    let mut lstmt = conn
        .prepare("SELECT id, description, amount FROM payroll_deductions WHERE employee_id = ?1 AND status = 'pending' AND (payroll_period_id IS NULL OR payroll_period_id = ?2)")
        .map_err(|e| format!("gagal menyiapkan kasbon: {e}"))?;
    for row in lstmt
        .query_map(params![employee_id, period_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca kasbon: {e}"))?
    {
        let (lid, desc, amount) = row.map_err(|e| format!("gagal membaca baris kasbon: {e}"))?;
        deductions.push(MoneyLine {
            name: desc,
            component_id: None,
            amount,
            loan_id: Some(lid),
        });
    }
    let total_income: f64 = incomes.iter().map(|l| l.amount).sum();
    let total_deduction: f64 = deductions.iter().map(|l| l.amount).sum();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM payrolls WHERE payroll_period_id = ?1 AND employee_id = ?2",
            params![period_id, employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa payroll: {e}"))?;
    let payroll_id = match existing {
        Some(pid) => {
            conn.execute(
                "UPDATE payrolls SET basic_salary = ?1, total_income = ?2, gross_salary = ?3, total_deduction = ?4, net_salary = ?5, total_overtime_amount = ?6, status = 'review' WHERE id = ?7",
                params![basic, total_income, total_income, total_deduction, total_income - total_deduction, ot, pid],
            )
            .map_err(|e| format!("gagal memperbarui payroll: {e}"))?;
            conn.execute(
                "DELETE FROM payroll_details WHERE payroll_id = ?1",
                params![pid],
            )
            .map_err(|e| format!("gagal mereset rincian: {e}"))?;
            pid
        }
        None => {
            conn.execute(
                "INSERT INTO payrolls (payroll_period_id, employee_id, basic_salary, total_income, gross_salary, total_deduction, net_salary, total_overtime_amount, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'review')",
                params![period_id, employee_id, basic, total_income, total_income, total_deduction, total_income - total_deduction, ot],
            )
            .map_err(|e| format!("gagal membuat payroll: {e}"))?;
            conn.last_insert_rowid()
        }
    };
    save_lines(conn, payroll_id, period_id, &incomes, "income")?;
    save_lines(conn, payroll_id, period_id, &deductions, "deduction")?;
    Ok(())
}

// ---------------- Alur status ----------------

fn set_period_status(
    conn: &Connection,
    actor_id: i64,
    action: &str,
    period_id: i64,
    expected: &str,
    next: &str,
    err_msg: &str,
) -> Result<(), String> {
    let before = period_status(conn, period_id)?;
    if before != expected {
        return Err(err_msg.to_string());
    }
    conn.execute(
        "UPDATE payroll_periods SET status = ?1 WHERE id = ?2",
        params![next, period_id],
    )
    .map_err(|e| format!("gagal mengubah status periode: {e}"))?;
    conn.execute(
        "UPDATE payrolls SET status = ?1 WHERE payroll_period_id = ?2",
        params![next, period_id],
    )
    .map_err(|e| format!("gagal mengubah status payroll: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        action,
        "payroll.period",
        Some(&period_id.to_string()),
        Some(&before),
        Some(next),
        None,
    )?;
    Ok(())
}

pub fn approve_period(conn: &Connection, actor_id: i64, period_id: i64) -> Result<(), String> {
    set_period_status(
        conn,
        actor_id,
        "APPROVE",
        period_id,
        "review",
        "approved",
        "Periode harus review untuk disetujui.",
    )?;
    let mut stmt = conn
        .prepare("SELECT u.id FROM payrolls p LEFT JOIN users u ON u.employee_id = p.employee_id WHERE p.payroll_period_id = ?1 AND u.id IS NOT NULL")
        .map_err(|e| format!("gagal menyiapkan notifikasi: {e}"))?;
    let rows = stmt
        .query_map(params![period_id], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("gagal membaca user: {e}"))?;
    for row in rows {
        let uid = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        approval::notify(
            conn,
            uid,
            "payroll",
            "Slip Gaji Tersedia",
            "Payroll periode telah disetujui.",
            "/payroll",
        )?;
    }
    Ok(())
}

pub fn mark_paid(conn: &Connection, actor_id: i64, period_id: i64) -> Result<(), String> {
    set_period_status(
        conn,
        actor_id,
        "PAY",
        period_id,
        "approved",
        "paid",
        "Periode harus approved untuk dibayar.",
    )?;
    let mut stmt = conn
        .prepare("SELECT id, employee_id FROM payrolls WHERE payroll_period_id = ?1")
        .map_err(|e| format!("gagal menyiapkan slip: {e}"))?;
    let rows = stmt
        .query_map(params![period_id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("gagal membaca payroll: {e}"))?;
    let now = Local::now();
    for row in rows {
        let (pid, eid) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM payslips WHERE payroll_id = ?1",
                params![pid],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memeriksa slip: {e}"))?;
        if exists.is_none() {
            let number = format!("PS-{}-{:04}-{}", now.format("%Y%m"), eid, pid);
            conn.execute(
                "INSERT INTO payslips (payroll_id, payslip_number, generated_at) VALUES (?1, ?2, ?3)",
                params![pid, number, now.format("%Y-%m-%d %H:%M:%S").to_string()],
            )
            .map_err(|e| format!("gagal membuat slip: {e}"))?;
        }
    }
    Ok(())
}

pub fn lock_period(conn: &Connection, actor_id: i64, period_id: i64) -> Result<(), String> {
    set_period_status(
        conn,
        actor_id,
        "LOCK",
        period_id,
        "paid",
        "locked",
        "Periode harus paid untuk dikunci.",
    )
}

// ---------------- Daftar dan rincian ----------------

pub fn payrolls_for_period(conn: &Connection, period_id: i64) -> Result<Vec<PayrollRow>, String> {
    let mut stmt = conn
        .prepare("SELECT p.id, p.employee_id, e.employee_number, e.first_name || ' ' || COALESCE(e.last_name, ''), p.basic_salary, p.total_income, p.total_deduction, p.net_salary, p.status FROM payrolls p INNER JOIN employees e ON e.id = p.employee_id WHERE p.payroll_period_id = ?1 ORDER BY e.first_name")
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    let rows = stmt
        .query_map(params![period_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, f64>(6)?,
                r.get::<_, f64>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, number, name, basic, income, ded, net, status) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(PayrollRow {
            id: to_dto_int(id, "payroll.id")?,
            employee_id: to_dto_int(emp, "payroll.emp")?,
            employee_number: number,
            name,
            basic_salary: basic,
            total_income: income,
            total_deduction: ded,
            net_salary: net,
            status,
        });
    }
    Ok(out)
}

pub fn payroll_detail(conn: &Connection, payroll_id: i64) -> Result<Option<PayrollDetail>, String> {
    let row: Option<(
        i64, i64, String, String, Option<String>, Option<String>, String, String, String,
        f64, f64, f64, f64, String,
    )> = conn
        .query_row(
            "SELECT p.id, p.employee_id, e.employee_number, e.first_name || ' ' || COALESCE(e.last_name, ''), d.name, ps.name, pp.name, pp.start_date, pp.end_date, p.basic_salary, p.total_income, p.total_deduction, p.net_salary, p.status
             FROM payrolls p
             INNER JOIN employees e ON e.id = p.employee_id
             LEFT JOIN departments d ON d.id = e.department_id
             LEFT JOIN positions ps ON ps.id = e.position_id
             INNER JOIN payroll_periods pp ON pp.id = p.payroll_period_id
             WHERE p.id = ?1",
            params![payroll_id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                    r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?,
                    r.get(12)?, r.get(13)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat payroll: {e}"))?;
    let Some((
        id,
        emp,
        number,
        name,
        dept,
        pos,
        pname,
        start,
        end,
        basic,
        income,
        ded,
        net,
        status,
    )) = row
    else {
        return Ok(None);
    };
    let mut stmt = conn
        .prepare("SELECT component_name, type, amount FROM payroll_details WHERE payroll_id = ?1 ORDER BY type DESC, id ASC")
        .map_err(|e| format!("gagal menyiapkan rincian: {e}"))?;
    let mut lines = Vec::new();
    for line in stmt
        .query_map(params![payroll_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca rincian: {e}"))?
    {
        let (cname, ctype, amount) = line.map_err(|e| format!("gagal membaca baris: {e}"))?;
        lines.push(PayrollLine {
            component_name: cname,
            line_type: ctype,
            amount,
        });
    }
    Ok(Some(PayrollDetail {
        id: to_dto_int(id, "payroll.id")?,
        employee_id: to_dto_int(emp, "payroll.emp")?,
        employee_number: number,
        name,
        department_name: dept,
        position_name: pos,
        period_name: pname,
        start_date: start,
        end_date: end,
        basic_salary: basic,
        total_income: income,
        total_deduction: ded,
        net_salary: net,
        status,
        lines,
    }))
}

pub fn my_payslips(conn: &Connection, employee_id: i64) -> Result<Vec<PayslipInfo>, String> {
    let mut stmt = conn
        .prepare("SELECT ps.id, ps.payroll_id, ps.payslip_number, pp.name, p.net_salary, ps.pdf_path FROM payslips ps INNER JOIN payrolls p ON p.id = ps.payroll_id INNER JOIN payroll_periods pp ON pp.id = p.payroll_period_id WHERE p.employee_id = ?1 ORDER BY pp.start_date DESC")
        .map_err(|e| format!("gagal menyiapkan slip: {e}"))?;
    let rows = stmt
        .query_map(params![employee_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, f64>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca slip: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, pid, number, pname, net, pdf) =
            row.map_err(|e| format!("gagal membaca baris slip: {e}"))?;
        out.push(PayslipInfo {
            id: to_dto_int(id, "payslip.id")?,
            payroll_id: to_dto_int(pid, "payslip.payroll")?,
            payslip_number: number,
            period_name: pname,
            net_salary: net,
            pdf_ready: pdf.map(|s| !s.is_empty()).unwrap_or(false),
        });
    }
    Ok(out)
}

// ---------------- Kasbon ----------------

pub fn deduction_list(conn: &Connection, pending_only: bool) -> Result<Vec<Deduction>, String> {
    let sql = if pending_only {
        "SELECT pd.id, pd.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), pd.type, pd.description, pd.amount, pd.installment_no, pd.total_installments, pd.status FROM payroll_deductions pd INNER JOIN employees e ON e.id = pd.employee_id WHERE pd.status = 'pending' ORDER BY pd.created_at DESC"
    } else {
        "SELECT pd.id, pd.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), pd.type, pd.description, pd.amount, pd.installment_no, pd.total_installments, pd.status FROM payroll_deductions pd INNER JOIN employees e ON e.id = pd.employee_id ORDER BY pd.created_at DESC"
    };
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("gagal menyiapkan kasbon: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, f64>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|e| format!("gagal membaca kasbon: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, dtype, desc, amount, ino, total, status) =
            row.map_err(|e| format!("gagal membaca baris kasbon: {e}"))?;
        out.push(Deduction {
            id: to_dto_int(id, "deduction.id")?,
            employee_id: to_dto_int(emp, "deduction.emp")?,
            employee_name: name,
            deduction_type: dtype,
            description: desc,
            amount,
            installment_no: ino.map(|v| to_dto_int(v, "deduction.ino")).transpose()?,
            total_installments: total
                .map(|v| to_dto_int(v, "deduction.total"))
                .transpose()?,
            status,
        });
    }
    Ok(out)
}

pub fn deduction_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &DeductionInput,
) -> Result<i32, String> {
    if !["loan", "kasbon", "other"].contains(&input.deduction_type.as_str()) {
        return Err("Jenis potongan tidak valid.".to_string());
    }
    if input.description.trim().is_empty() {
        return Err("Keterangan wajib diisi.".to_string());
    }
    if !(input.amount > 0.0) {
        return Err("Nominal harus lebih dari 0.".to_string());
    }
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
    if let Some(rid) = id {
        let status: Option<String> = conn
            .query_row(
                "SELECT status FROM payroll_deductions WHERE id = ?1",
                params![rid],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memuat kasbon: {e}"))?;
        if status.as_deref() != Some("pending") {
            return Err("Hanya kasbon pending yang bisa diubah.".to_string());
        }
        conn.execute(
            "UPDATE payroll_deductions SET type = ?1, description = ?2, amount = ?3, installment_no = ?4, total_installments = ?5 WHERE id = ?6",
            params![input.deduction_type, input.description.trim(), input.amount, input.installment_no, input.total_installments, rid],
        )
        .map_err(|e| format!("gagal menyimpan kasbon: {e}"))?;
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "payroll.deduction",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "deduction.id")
    } else {
        conn.execute(
            "INSERT INTO payroll_deductions (employee_id, type, description, amount, installment_no, total_installments, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')",
            params![input.employee_id as i64, input.deduction_type, input.description.trim(), input.amount, input.installment_no, input.total_installments],
        )
        .map_err(|e| format!("gagal menambah kasbon: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "payroll.deduction",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "deduction.id")
    }
}

pub fn deduction_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM payroll_deductions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat kasbon: {e}"))?;
    if status.as_deref() != Some("pending") {
        return Err("Hanya kasbon pending yang bisa dihapus.".to_string());
    }
    conn.execute("DELETE FROM payroll_deductions WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus kasbon: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "payroll.deduction",
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

    fn live() -> (tempfile::TempDir, crate::db::DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool)
    }

    fn mkemp(conn: &Connection, number: &str, basic: f64, ptkp: &str) -> i64 {
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type, ptkp_status) VALUES (?1, 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent', ?2)",
            params![number, ptkp],
        )
        .unwrap();
        let eid = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, ?2, '2026-01-01', 1)",
            params![eid, basic],
        )
        .unwrap();
        eid
    }

    fn admin(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn pph21_sesuai_tarif_progresif() {
        assert_eq!(pph21_monthly(5_000_000.0, "TK/0"), 12_500.0);
        assert_eq!(pph21_monthly(4_000_000.0, "TK/0"), 0.0);
        assert_eq!(pph21_monthly(5_000_000.0, "XX"), 12_500.0);
        assert_eq!(pph21_monthly(20_000_000.0, "K/1"), 1_637_500.0);
    }

    #[test]
    fn generate_menghitung_lengkap_dan_alur_terkunci() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let eid = mkemp(&conn, "EMP-G1", 5_000_000.0, "TK/0");
        conn.execute(
            "INSERT INTO overtime_requests (employee_id, date, start_time, end_time, duration_minutes, rate_multiplier, status, current_step) VALUES (?1, '2026-02-10', '2026-02-10 18:00:00', '2026-02-10 20:00:00', 120, 1.5, 'approved', 0)",
            params![eid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attendances (employee_id, date, status) VALUES (?1, '2026-02-11', 'absent')",
            params![eid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO payroll_deductions (employee_id, type, description, amount, status) VALUES (?1, 'kasbon', 'Kasbon Feb', 300000, 'pending')",
            params![eid],
        )
        .unwrap();
        let pid = period_create(
            &conn,
            actor,
            actor,
            &PeriodInput {
                name: "Feb 2026".to_string(),
                start_date: "2026-02-01".to_string(),
                end_date: "2026-02-28".to_string(),
                payment_date: Some("2026-03-05".to_string()),
            },
        )
        .expect("periode");
        assert!(period_create(
            &conn,
            actor,
            actor,
            &PeriodInput {
                name: "X".to_string(),
                start_date: "2026-02-15".to_string(),
                end_date: "2026-03-15".to_string(),
                payment_date: None,
            },
        )
        .is_err());
        assert!(approve_period(&conn, actor, pid as i64).is_err());
        drop(conn);
        let mut conn2 = pool.get().expect("get");
        generate(&mut conn2, actor, pid as i64).expect("generate");
        let status: String = conn2
            .query_row(
                "SELECT status FROM payroll_periods WHERE id = ?1",
                params![pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "review");
        let rows = payrolls_for_period(&conn2, pid as i64).expect("rows");
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.total_income, 5_086_705.0);
        assert_eq!(r.net_salary, 4_342_814.0);
        let det = payroll_detail(&conn2, r.id as i64)
            .expect("det")
            .expect("ada");
        assert!(det.lines.iter().any(|l| l.component_name == "Lembur"));
        assert!(det.lines.iter().any(|l| l.component_name == "Kasbon Feb"));
        assert!(det
            .lines
            .iter()
            .any(|l| l.component_name == "PPh 21 (estimasi)"));
        let st: String = conn2
            .query_row(
                "SELECT status FROM payroll_deductions WHERE description = 'Kasbon Feb'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(st, "processed");
        assert!(generate(&mut conn2, actor, pid as i64).is_err());
        approve_period(&conn2, actor, pid as i64).expect("approve");
        assert!(lock_period(&conn2, actor, pid as i64).is_err());
        mark_paid(&conn2, actor, pid as i64).expect("pay");
        let slips = my_payslips(&conn2, eid).expect("slips");
        assert_eq!(slips.len(), 1);
        assert!(slips[0].payslip_number.starts_with("PS-"));
        lock_period(&conn2, actor, pid as i64).expect("lock");
        let status: String = conn2
            .query_row(
                "SELECT status FROM payroll_periods WHERE id = ?1",
                params![pid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "locked");
    }

    #[test]
    fn bpjs_cap_membatasi_dasar_upah() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let eid = mkemp(&conn, "EMP-CAP", 20_000_000.0, "TK/0");
        let pid = period_create(
            &conn,
            actor,
            actor,
            &PeriodInput {
                name: "Cap 2026".to_string(),
                start_date: "2026-03-01".to_string(),
                end_date: "2026-03-31".to_string(),
                payment_date: None,
            },
        )
        .expect("periode");
        conn.execute(
            "UPDATE system_settings SET setting_value = '10000000' WHERE setting_key = 'bpjs_jp_max_wage'",
            [],
        )
        .unwrap();
        drop(conn);
        let mut conn2 = pool.get().expect("get");
        generate(&mut conn2, actor, pid as i64).expect("generate");
        let prid: i64 = conn2
            .query_row(
                "SELECT id FROM payrolls WHERE payroll_period_id = ?1 AND employee_id = ?2",
                params![pid, eid],
                |r| r.get(0),
            )
            .unwrap();
        let det = payroll_detail(&conn2, prid).expect("det").expect("ada");
        let jp = det
            .lines
            .iter()
            .find(|l| l.component_name == "BPJS Jaminan Pensiun")
            .expect("baris JP ada");
        assert_eq!(jp.amount, 100_000.0);
        let jht = det
            .lines
            .iter()
            .find(|l| l.component_name == "BPJS Jaminan Hari Tua")
            .expect("baris JHT ada");
        assert_eq!(jht.amount, 400_000.0);
    }

    #[test]
    fn thr_proporsional_masuk_payroll() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let eid = mkemp(&conn, "EMP-THR", 6_000_000.0, "TK/0");
        let pid = period_create(
            &conn,
            actor,
            actor,
            &PeriodInput {
                name: "Mar 2026".to_string(),
                start_date: "2026-03-01".to_string(),
                end_date: "2026-03-31".to_string(),
                payment_date: None,
            },
        )
        .expect("periode");
        conn.execute(
            "UPDATE system_settings SET setting_value = '2026-03-20' WHERE setting_key = 'thr_holiday_date'",
            [],
        )
        .unwrap();
        drop(conn);
        let mut conn2 = pool.get().expect("get");
        generate(&mut conn2, actor, pid as i64).expect("generate");
        let prid: i64 = conn2
            .query_row(
                "SELECT id FROM payrolls WHERE payroll_period_id = ?1 AND employee_id = ?2",
                params![pid, eid],
                |r| r.get(0),
            )
            .unwrap();
        let det = payroll_detail(&conn2, prid).expect("det").expect("ada");
        let thr = det
            .lines
            .iter()
            .find(|l| l.component_name == "THR")
            .expect("baris THR ada");
        assert_eq!(thr.amount, 1_000_000.0);
    }

    #[test]
    fn komponen_crud_dan_ganda_ditolak() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        assert!(!component_list(&conn).expect("list").is_empty());
        let id = component_save(
            &conn,
            actor,
            None,
            &ComponentInput {
                code: "T-TEST".to_string(),
                name: "Tes".to_string(),
                component_type: "income".to_string(),
                calculation_type: "fixed".to_string(),
                is_taxable: false,
                is_active: true,
            },
        )
        .expect("buat");
        assert!(component_save(
            &conn,
            actor,
            None,
            &ComponentInput {
                code: "T-TEST".to_string(),
                name: "Dobel".to_string(),
                component_type: "income".to_string(),
                calculation_type: "fixed".to_string(),
                is_taxable: false,
                is_active: true,
            },
        )
        .is_err());
        component_delete(&conn, actor, id as i64).expect("hapus");
    }
}
