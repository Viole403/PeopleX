//! Penggajian: komponen, periode, generate, alur status, kasbon, slip.

use chrono::{Datelike, Local, NaiveDate};

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

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

fn pf64(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        Value::Text(s) => s.parse().unwrap_or(0.0),
        Value::Null => 0.0,
    }
}

fn popt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

async fn prow_id(db: &sea_orm::DatabaseConnection, label: &str) -> Result<i64, String> {
    let row = q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        label,
    )
    .await
    .map_err(|e| format!("gagal membaca id baru: {e}"))?;
    Ok(row.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0))
}

// Transaksi generate memakai koneksi SeaORM agar atomic penuh.
type Tx = sea_orm::DatabaseTransaction;

async fn tx_q_all(
    tx: &Tx,
    sql: String,
    vals: Vec<Value>,
    ncols: usize,
    label: &str,
) -> Result<Vec<Vec<Value>>, String> {
    q_all(tx, sql, vals, ncols, label).await
}

async fn tx_q_one(
    tx: &Tx,
    sql: String,
    vals: Vec<Value>,
    ncols: usize,
    label: &str,
) -> Result<Option<Vec<Value>>, String> {
    q_one(tx, sql, vals, ncols, label).await
}

async fn tx_exec(
    tx: &Tx,
    sql: String,
    vals: Vec<Value>,
    label: &str,
) -> Result<u64, String> {
    exec(tx, sql, vals, label).await
}

async fn tx_rowid(tx: &Tx) -> i64 {
    tx_q_one(tx, "SELECT last_insert_rowid()".to_string(), vec![], 1, "payroll.rowid")
        .await
        .ok()
        .flatten()
        .as_ref()
        .and_then(|r| value_i64(&r[0]))
        .unwrap_or(0)
}

async fn overtime_amount_tx(
    tx: &Tx,
    employee_id: i64,
    start: &str,
    end: &str,
    basic: f64,
) -> Result<f64, String> {
    let rows = tx_q_all(
        tx,
        "SELECT duration_minutes, rate_multiplier FROM overtime_requests WHERE employee_id = ?1 AND status = 'approved' AND date BETWEEN ?2 AND ?3".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(start.to_string()),
            Value::Text(end.to_string()),
        ],
        2,
        "payroll.overtime",
    )
    .await
    .map_err(|e| format!("gagal membaca lembur: {e}"))?;
    let hourly = basic / 173.0;
    let mut total = 0.0;
    for r in &rows {
        let (mins, mult) = (value_i64(&r[0]).unwrap_or(0), pf64(&r[1]));
        total += hourly * mult * (mins as f64 / 60.0);
    }
    Ok(total.round())
}

async fn absence_deduction_tx(
    tx: &Tx,
    employee_id: i64,
    start: &str,
    end: &str,
    basic: f64,
) -> Result<f64, String> {
    let row = tx_q_one(
        tx,
        "SELECT COUNT(*) FROM attendances WHERE employee_id = ?1 AND status = 'absent' AND date BETWEEN ?2 AND ?3".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(start.to_string()),
            Value::Text(end.to_string()),
        ],
        1,
        "payroll.absent",
    )
    .await
    .map_err(|e| format!("gagal menghitung absen: {e}"))?;
    let n = row.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0);
    if n == 0 {
        return Ok(0.0);
    }
    Ok(((basic / 22.0) * n as f64).round())
}

async fn setting_tx(tx: &Tx, key: &str) -> Option<String> {
    tx_q_one(
        tx,
        "SELECT setting_value FROM system_settings WHERE setting_key = ?1".to_string(),
        vec![Value::Text(key.to_string())],
        1,
        "payroll.setting",
    )
    .await
    .ok()
    .flatten()
    .as_ref()
    .and_then(|r| popt_text(&r[0]))
}

async fn thr_for_period_tx(
    tx: &Tx,
    basic: f64,
    join_date: &str,
    pstart: &str,
    pend: &str,
) -> Result<f64, String> {
    let h = setting_tx(tx, "thr_holiday_date")
        .await
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let Some(h) = h else {
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

async fn capped_base_tx(tx: &Tx, basic: f64, key: &str) -> Result<f64, String> {
    let cap = setting_tx(tx, key)
        .await
        .and_then(|v| v.parse::<f64>().ok());
    Ok(match cap {
        Some(m) if m > 0.0 => basic.min(m),
        _ => basic,
    })
}

struct MoneyLine {
    name: String,
    component_id: Option<i64>,
    amount: f64,
    loan_id: Option<i64>,
}

async fn save_lines_tx(
    tx: &Tx,
    payroll_id: i64,
    period_id: i64,
    lines: &[MoneyLine],
    line_type: &str,
) -> Result<(), String> {
    for line in lines {
        tx_exec(
            tx,
            "INSERT INTO payroll_details (payroll_id, salary_component_id, component_name, type, amount) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
            vec![
                Value::Int(payroll_id),
                match line.component_id {
                    Some(c) => Value::Int(c),
                    None => Value::Null,
                },
                Value::Text(line.name.clone()),
                Value::Text(line_type.to_string()),
                Value::Float(line.amount),
            ],
            "payroll.saveline",
        )
        .await
        .map_err(|e| format!("gagal menyimpan rincian: {e}"))?;
        if let Some(lid) = line.loan_id {
            tx_exec(
                tx,
                "UPDATE payroll_deductions SET status = 'processed', payroll_period_id = ?1 WHERE id = ?2".to_string(),
                vec![Value::Int(period_id), Value::Int(lid)],
                "payroll.loanproc",
            )
            .await
            .map_err(|e| format!("gagal memproses kasbon: {e}"))?;
        }
    }
    Ok(())
}

async fn generate_one_tx(
    tx: &Tx,
    employee_id: i64,
    period_id: i64,
    pstart: &str,
    pend: &str,
    health_pct: f64,
    emp_pct: f64,
    jp_pct: f64,
) -> Result<(), String> {
    let salary = tx_q_one(
        tx,
        "SELECT id, basic_salary FROM employee_salaries WHERE employee_id = ?1 AND is_active = 1 AND effective_date <= ?2 ORDER BY effective_date DESC LIMIT 1".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(pend.to_string()),
        ],
        2,
        "payroll.salary",
    )
    .await
    .map_err(|e| format!("gagal memuat gaji: {e}"))?;
    let Some(salary) = salary else {
        return Ok(());
    };
    let (salary_id, basic) = (value_i64(&salary[0]).unwrap_or(0), pf64(&salary[1]));
    let ptkp_row = tx_q_one(
        tx,
        "SELECT ptkp_status FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "payroll.ptkp",
    )
    .await
    .map_err(|e| format!("gagal memuat PTKP: {e}"))?;
    let ptkp = ptkp_row.as_ref().and_then(|r| popt_text(&r[0]));
    let join_row = tx_q_one(
        tx,
        "SELECT join_date FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "payroll.join",
    )
    .await
    .map_err(|e| format!("gagal memuat tanggal masuk: {e}"))?;
    let join_date = join_row
        .as_ref()
        .and_then(|r| popt_text(&r[0]))
        .unwrap_or_default();
    let mut incomes = vec![MoneyLine {
        name: "Gaji Pokok".to_string(),
        component_id: None,
        amount: basic,
        loan_id: None,
    }];
    let mut taxable_extra = 0.0;
    let comps = tx_q_all(
        tx,
        "SELECT esc.salary_component_id, sc.name, sc.is_taxable, esc.amount FROM employee_salary_components esc INNER JOIN salary_components sc ON sc.id = esc.salary_component_id WHERE esc.employee_salary_id = ?1 AND sc.type = 'income' AND sc.is_active = 1".to_string(),
        vec![Value::Int(salary_id)],
        4,
        "payroll.comps",
    )
    .await
    .map_err(|e| format!("gagal membaca komponen: {e}"))?;
    for r in &comps {
        let (cid, name, taxable, amount) = (
            value_i64(&r[0]).unwrap_or(0),
            value_to_string(&r[1]),
            value_i64(&r[2]).unwrap_or(0),
            pf64(&r[3]),
        );
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
    let ot = overtime_amount_tx(tx, employee_id, pstart, pend, basic).await?;
    if ot > 0.0 {
        incomes.push(MoneyLine {
            name: "Lembur".to_string(),
            component_id: None,
            amount: ot,
            loan_id: None,
        });
    }
    let thr = thr_for_period_tx(tx, basic, &join_date, pstart, pend).await?;
    if thr > 0.0 {
        incomes.push(MoneyLine {
            name: "THR".to_string(),
            component_id: None,
            amount: thr,
            loan_id: None,
        });
        taxable_extra += thr;
    }
    let absence = absence_deduction_tx(tx, employee_id, pstart, pend, basic).await?;
    if absence > 0.0 {
        deductions.push(MoneyLine {
            name: "Potongan Absensi".to_string(),
            component_id: None,
            amount: absence,
            loan_id: None,
        });
    }
    let bpjs_h = (capped_base_tx(tx, basic, "bpjs_health_max_wage").await? * health_pct).round();
    let bpjs_e = (capped_base_tx(tx, basic, "bpjs_jht_max_wage").await? * emp_pct).round();
    let bpjs_jp = (capped_base_tx(tx, basic, "bpjs_jp_max_wage").await? * jp_pct).round();
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
    let pph = pph21_monthly(basic + taxable_extra + ot, ptkp.as_deref().unwrap_or("TK/0"));
    if pph > 0.0 {
        deductions.push(MoneyLine {
            name: "PPh 21 (estimasi)".to_string(),
            component_id: None,
            amount: pph,
            loan_id: None,
        });
    }
    let loan_rows = tx_q_all(
        tx,
        "SELECT id, description, amount, installment_no, total_installments FROM payroll_deductions WHERE employee_id = ?1 AND status = 'pending' AND (payroll_period_id IS NULL OR payroll_period_id = ?2)".to_string(),
        vec![Value::Int(employee_id), Value::Int(period_id)],
        5,
        "payroll.loans",
    )
    .await
    .map_err(|e| format!("gagal membaca kasbon: {e}"))?;
    for r in &loan_rows {
        let (lid, desc, amount, ino, total) = (
            value_i64(&r[0]).unwrap_or(0),
            value_to_string(&r[1]),
            pf64(&r[2]),
            value_i64(&r[3]),
            value_i64(&r[4]),
        );
        let final_now = match (ino, total) {
            (Some(n), Some(t)) if n < t => {
                tx_exec(
                    tx,
                    "UPDATE payroll_deductions SET installment_no = ?1 WHERE id = ?2".to_string(),
                    vec![Value::Int(n + 1), Value::Int(lid)],
                    "payroll.loanadv",
                )
                .await
                .map_err(|e| format!("gagal maju cicilan: {e}"))?;
                false
            }
            _ => true,
        };
        deductions.push(MoneyLine {
            name: desc,
            component_id: None,
            amount,
            loan_id: if final_now { Some(lid) } else { None },
        });
    }
    let total_income: f64 = incomes.iter().map(|l| l.amount).sum();
    let total_deduction: f64 = deductions.iter().map(|l| l.amount).sum();
    let existing = tx_q_one(
        tx,
        "SELECT id FROM payrolls WHERE payroll_period_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        1,
        "payroll.existing",
    )
    .await
    .map_err(|e| format!("gagal memeriksa payroll: {e}"))?;
    let payroll_id = match existing {
        Some(e) => {
            let pid = value_i64(&e[0]).unwrap_or(0);
            tx_exec(
                tx,
                "UPDATE payrolls SET basic_salary = ?1, total_income = ?2, gross_salary = ?3, total_deduction = ?4, net_salary = ?5, total_overtime_amount = ?6, status = 'review' WHERE id = ?7".to_string(),
                vec![
                    Value::Float(basic),
                    Value::Float(total_income),
                    Value::Float(total_income),
                    Value::Float(total_deduction),
                    Value::Float(total_income - total_deduction),
                    Value::Float(ot),
                    Value::Int(pid),
                ],
                "payroll.upd",
            )
            .await
            .map_err(|e| format!("gagal memperbarui payroll: {e}"))?;
            tx_exec(
                tx,
                "DELETE FROM payroll_details WHERE payroll_id = ?1".to_string(),
                vec![Value::Int(pid)],
                "payroll.resetlines",
            )
            .await
            .map_err(|e| format!("gagal mereset rincian: {e}"))?;
            pid
        }
        None => {
            tx_exec(
                tx,
                "INSERT INTO payrolls (payroll_period_id, employee_id, basic_salary, total_income, gross_salary, total_deduction, net_salary, total_overtime_amount, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'review')".to_string(),
                vec![
                    Value::Int(period_id),
                    Value::Int(employee_id),
                    Value::Float(basic),
                    Value::Float(total_income),
                    Value::Float(total_income),
                    Value::Float(total_deduction),
                    Value::Float(total_income - total_deduction),
                    Value::Float(ot),
                ],
                "payroll.add",
            )
            .await
            .map_err(|e| format!("gagal membuat payroll: {e}"))?;
            tx_rowid(tx).await
        }
    };
    save_lines_tx(tx, payroll_id, period_id, &incomes, "income").await?;
    save_lines_tx(tx, payroll_id, period_id, &deductions, "deduction").await?;
    Ok(())
}

/// Generate: hitung seluruh karyawan aktif dalam satu transaksi, lalu review.
pub async fn generate_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
) -> Result<(), String> {
    if period_status_sea(db, period_id).await? != "draft" {
        return Err("Generate hanya untuk periode draft.".to_string());
    }
    let prow = q_one(
        db,
        "SELECT name, start_date, end_date FROM payroll_periods WHERE id = ?1".to_string(),
        vec![Value::Int(period_id)],
        3,
        "payroll.period",
    )
    .await
    .map_err(|e| format!("gagal memuat periode: {e}"))?;
    let Some(prow) = prow else {
        return Err("gagal memuat periode: periode tidak ditemukan".to_string());
    };
    let (pname, pstart, pend) = (
        value_to_string(&prow[0]),
        value_to_string(&prow[1]),
        value_to_string(&prow[2]),
    );
    let health_pct = setting_pct_sea(db, "bpjs_health_employee_percent", 1.0).await / 100.0;
    let emp_pct = setting_pct_sea(db, "bpjs_employment_employee_percent", 2.0).await / 100.0;
    let jp_pct = setting_pct_sea(db, "bpjs_jp_employee_percent", 1.0).await / 100.0;
    let tx = db
        .begin()
        .await
        .map_err(|e| format!("gagal memulai transaksi: {e}"))?;
    let ids = tx_q_all(
        &tx,
        "SELECT id FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation')".to_string(),
        vec![],
        1,
        "payroll.emps",
    )
    .await;
    let ids = match ids {
        Ok(v) => v,
        Err(e) => {
            tx.rollback().await.ok();
            return Err(format!("gagal membaca karyawan: {e}"));
        }
    };
    let mut failed: Option<String> = None;
    for r in &ids {
        let eid = value_i64(&r[0]).unwrap_or(0);
        if let Err(e) =
            generate_one_tx(&tx, eid, period_id, &pstart, &pend, health_pct, emp_pct, jp_pct)
                .await
        {
            failed = Some(e);
            break;
        }
    }
    if let Some(e) = failed {
        tx.rollback().await.ok();
        return Err(e);
    }
    if let Err(e) = tx_exec(
        &tx,
        "UPDATE payroll_periods SET status = 'review' WHERE id = ?1".to_string(),
        vec![Value::Int(period_id)],
        "payroll.toreview",
    )
    .await
    {
        tx.rollback().await.ok();
        return Err(format!("gagal menandai review: {e}"));
    }
    tx.commit()
        .await
        .map_err(|e| format!("gagal commit generate: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "GENERATE",
        "payroll.period",
        Some(&period_id.to_string()),
        None,
        None,
        Some(&format!("Generate periode {pname}")),
    )
    .await?;
    Ok(())
}

async fn setting_pct_sea(db: &sea_orm::DatabaseConnection, key: &str, fallback: f64) -> f64 {
    q_one(
        db,
        "SELECT setting_value FROM system_settings WHERE setting_key = ?1".to_string(),
        vec![Value::Text(key.to_string())],
        1,
        "payroll.setting",
    )
    .await
    .ok()
    .flatten()
    .as_ref()
    .and_then(|r| popt_text(&r[0]))
    .and_then(|v| v.parse::<f64>().ok())
    .unwrap_or(fallback)
}

pub async fn component_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Component>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name, type, calculation_type, is_taxable, is_active FROM salary_components ORDER BY type ASC, name ASC".to_string(),
        vec![],
        7,
        "payroll.components",
    )
    .await
    .map_err(|e| format!("gagal membaca komponen: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Component {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "component.id")?,
            code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
            component_type: value_to_string(&r[3]),
            calculation_type: value_to_string(&r[4]),
            is_taxable: value_i64(&r[5]).unwrap_or(0) != 0,
            is_active: value_i64(&r[6]).unwrap_or(0) != 0,
        });
    }
    Ok(out)
}

pub async fn component_save_sea(
    db: &sea_orm::DatabaseConnection,
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
        let n = exec(
            db,
            "UPDATE salary_components SET code = ?1, name = ?2, type = ?3, calculation_type = ?4, is_taxable = ?5, is_active = ?6 WHERE id = ?7".to_string(),
            vec![
                Value::Text(input.code.trim().to_string()),
                Value::Text(input.name.trim().to_string()),
                Value::Text(input.component_type.clone()),
                Value::Text(input.calculation_type.clone()),
                Value::Int(tax),
                Value::Int(active),
                Value::Int(rid),
            ],
            "payroll.compupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan komponen: {e}"))?;
        if n == 0 {
            return Err("Komponen tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "payroll.component",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "component.id")
    } else {
        let res = exec(
            db,
            "INSERT INTO salary_components (code, name, type, calculation_type, is_taxable, is_active) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
            vec![
                Value::Text(input.code.trim().to_string()),
                Value::Text(input.name.trim().to_string()),
                Value::Text(input.component_type.clone()),
                Value::Text(input.calculation_type.clone()),
                Value::Int(tax),
                Value::Int(active),
            ],
            "payroll.compadd",
        )
        .await;
        let Err(e) = res else {
            let rid = prow_id(db, "payroll.compadd").await?;
            audit::log_sea(
                db,
                Some(actor_id),
                "CREATE",
                "payroll.component",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await?;
            return to_dto_int(rid, "component.id");
        };
        if e.contains("UNIQUE") {
            return Err("Kode komponen sudah dipakai.".to_string());
        }
        return Err(format!("gagal menambah komponen: {e}"));
    }
}

pub async fn component_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM employee_salary_components WHERE salary_component_id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "payroll.comprel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Komponen masih dipakai data gaji.".to_string());
    }
    let d = exec(
        db,
        "DELETE FROM salary_components WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        "payroll.compdel",
    )
    .await
    .map_err(|e| format!("gagal menghapus komponen: {e}"))?;
    if d == 0 {
        return Err("Komponen tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "payroll.component",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn period_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Period>, String> {
    let rows = q_all(
        db,
        "SELECT pp.id, pp.name, pp.start_date, pp.end_date, pp.payment_date, pp.status,
                       (SELECT COUNT(*) FROM payrolls p WHERE p.payroll_period_id = pp.id),
                       COALESCE((SELECT SUM(p.net_salary) FROM payrolls p WHERE p.payroll_period_id = pp.id), 0)
                FROM payroll_periods pp ORDER BY pp.start_date DESC"
            .to_string(),
        vec![],
        8,
        "payroll.periods",
    )
    .await
    .map_err(|e| format!("gagal membaca periode: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let (id, count) = (
            value_i64(&r[0]).unwrap_or(0),
            value_i64(&r[6]).unwrap_or(0),
        );
        out.push(Period {
            id: to_dto_int(id, "period.id")?,
            name: value_to_string(&r[1]),
            start_date: value_to_string(&r[2]),
            end_date: value_to_string(&r[3]),
            payment_date: popt_text(&r[4]),
            status: value_to_string(&r[5]),
            employee_count: to_dto_int(count, "period.count")?,
            total_net: pf64(&r[7]),
        });
    }
    Ok(out)
}

pub async fn period_create_sea(
    db: &sea_orm::DatabaseConnection,
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
    let overlap = q_one(
        db,
        "SELECT id FROM payroll_periods WHERE NOT (end_date < ?1 OR start_date > ?2)".to_string(),
        vec![
            Value::Text(input.start_date.trim().to_string()),
            Value::Text(input.end_date.trim().to_string()),
        ],
        1,
        "payroll.overlap",
    )
    .await
    .map_err(|e| format!("gagal memeriksa tumpang tindih: {e}"))?;
    if overlap.is_some() {
        return Err("Periode bertabrakan dengan periode yang ada.".to_string());
    }
    exec(
        db,
        "INSERT INTO payroll_periods (name, start_date, end_date, payment_date, status, created_by) VALUES (?1, ?2, ?3, ?4, 'draft', ?5)".to_string(),
        vec![
            Value::Text(input.name.trim().to_string()),
            Value::Text(input.start_date.trim().to_string()),
            Value::Text(input.end_date.trim().to_string()),
            match input.payment_date.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Int(created_by),
        ],
        "payroll.periodadd",
    )
    .await
    .map_err(|e| format!("gagal membuat periode: {e}"))?;
    let rid = prow_id(db, "payroll.periodadd").await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "payroll.period",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "period.id")
}

async fn period_status_sea(db: &sea_orm::DatabaseConnection, id: i64) -> Result<String, String> {
    let row = q_one(
        db,
        "SELECT status FROM payroll_periods WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "payroll.pstatus",
    )
    .await
    .map_err(|e| format!("gagal memuat periode: {e}"))?;
    row.as_ref()
        .map(|r| value_to_string(&r[0]))
        .filter(|s| !s.is_empty())
        .ok_or("Periode tidak ditemukan.".to_string())
}

async fn set_period_status_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    action: &str,
    period_id: i64,
    expected: &str,
    next: &str,
    err_msg: &str,
) -> Result<(), String> {
    let before = period_status_sea(db, period_id).await?;
    if before != expected {
        return Err(err_msg.to_string());
    }
    exec(
        db,
        "UPDATE payroll_periods SET status = ?1 WHERE id = ?2".to_string(),
        vec![Value::Text(next.to_string()), Value::Int(period_id)],
        "payroll.setperiod",
    )
    .await
    .map_err(|e| format!("gagal mengubah status periode: {e}"))?;
    exec(
        db,
        "UPDATE payrolls SET status = ?1 WHERE payroll_period_id = ?2".to_string(),
        vec![Value::Text(next.to_string()), Value::Int(period_id)],
        "payroll.setrows",
    )
    .await
    .map_err(|e| format!("gagal mengubah status payroll: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        action,
        "payroll.period",
        Some(&period_id.to_string()),
        Some(&before),
        Some(next),
        None,
    )
    .await?;
    Ok(())
}

pub async fn approve_period_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
) -> Result<(), String> {
    set_period_status_sea(
        db,
        actor_id,
        "APPROVE",
        period_id,
        "review",
        "approved",
        "Periode harus review untuk disetujui.",
    )
    .await?;
    let rows = q_all(
        db,
        "SELECT u.id FROM payrolls p LEFT JOIN users u ON u.employee_id = p.employee_id WHERE p.payroll_period_id = ?1 AND u.id IS NOT NULL".to_string(),
        vec![Value::Int(period_id)],
        1,
        "payroll.notifusers",
    )
    .await
    .map_err(|e| format!("gagal membaca user: {e}"))?;
    for r in &rows {
        let uid = value_i64(&r[0]).unwrap_or(0);
        approval::notify_sea(
            db,
            uid,
            "payroll",
            "Slip Gaji Tersedia",
            "Payroll periode telah disetujui.",
            "/payroll",
        )
        .await?;
    }
    Ok(())
}

pub async fn mark_paid_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
) -> Result<(), String> {
    set_period_status_sea(
        db,
        actor_id,
        "PAY",
        period_id,
        "approved",
        "paid",
        "Periode harus approved untuk dibayar.",
    )
    .await?;
    let rows = q_all(
        db,
        "SELECT id, employee_id FROM payrolls WHERE payroll_period_id = ?1".to_string(),
        vec![Value::Int(period_id)],
        2,
        "payroll.slips",
    )
    .await
    .map_err(|e| format!("gagal membaca payroll: {e}"))?;
    let now = Local::now();
    for r in &rows {
        let (pid, eid) = (value_i64(&r[0]).unwrap_or(0), value_i64(&r[1]).unwrap_or(0));
        let exists = q_one(
            db,
            "SELECT id FROM payslips WHERE payroll_id = ?1".to_string(),
            vec![Value::Int(pid)],
            1,
            "payroll.slipex",
        )
        .await
        .map_err(|e| format!("gagal memeriksa slip: {e}"))?;
        if exists.is_none() {
            let number = format!("PS-{}-{:04}-{}", now.format("%Y%m"), eid, pid);
            exec(
                db,
                "INSERT INTO payslips (payroll_id, payslip_number, generated_at) VALUES (?1, ?2, ?3)".to_string(),
                vec![
                    Value::Int(pid),
                    Value::Text(number),
                    Value::Text(now.format("%Y-%m-%d %H:%M:%S").to_string()),
                ],
                "payroll.slipadd",
            )
            .await
            .map_err(|e| format!("gagal membuat slip: {e}"))?;
        }
    }
    Ok(())
}

pub async fn lock_period_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
) -> Result<(), String> {
    set_period_status_sea(
        db,
        actor_id,
        "LOCK",
        period_id,
        "paid",
        "locked",
        "Periode harus paid untuk dikunci.",
    )
    .await
}

pub async fn payrolls_for_period_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
) -> Result<Vec<PayrollRow>, String> {
    let rows = q_all(
        db,
        "SELECT p.id, p.employee_id, e.employee_number, e.first_name || ' ' || COALESCE(e.last_name, ''), p.basic_salary, p.total_income, p.total_deduction, p.net_salary, p.status FROM payrolls p INNER JOIN employees e ON e.id = p.employee_id WHERE p.payroll_period_id = ?1 ORDER BY e.first_name".to_string(),
        vec![Value::Int(period_id)],
        9,
        "payroll.rows",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(PayrollRow {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "payroll.id")?,
            employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "payroll.emp")?,
            employee_number: value_to_string(&r[2]),
            name: value_to_string(&r[3]),
            basic_salary: pf64(&r[4]),
            total_income: pf64(&r[5]),
            total_deduction: pf64(&r[6]),
            net_salary: pf64(&r[7]),
            status: value_to_string(&r[8]),
        });
    }
    Ok(out)
}

pub async fn payroll_detail_sea(
    db: &sea_orm::DatabaseConnection,
    payroll_id: i64,
) -> Result<Option<PayrollDetail>, String> {
    let row = q_one(
        db,
        "SELECT p.id, p.employee_id, e.employee_number, e.first_name || ' ' || COALESCE(e.last_name, ''), d.name, ps.name, pp.name, pp.start_date, pp.end_date, p.basic_salary, p.total_income, p.total_deduction, p.net_salary, p.status
             FROM payrolls p
             INNER JOIN employees e ON e.id = p.employee_id
             LEFT JOIN departments d ON d.id = e.department_id
             LEFT JOIN positions ps ON ps.id = e.position_id
             INNER JOIN payroll_periods pp ON pp.id = p.payroll_period_id
             WHERE p.id = ?1"
            .to_string(),
        vec![Value::Int(payroll_id)],
        14,
        "payroll.detail",
    )
    .await
    .map_err(|e| format!("gagal memuat payroll: {e}"))?;
    let Some(r) = row else {
        return Ok(None);
    };
    let lines_rows = q_all(
        db,
        "SELECT component_name, type, amount FROM payroll_details WHERE payroll_id = ?1 ORDER BY type DESC, id ASC".to_string(),
        vec![Value::Int(payroll_id)],
        3,
        "payroll.lines",
    )
    .await
    .map_err(|e| format!("gagal membaca rincian: {e}"))?;
    let mut lines = Vec::new();
    for l in &lines_rows {
        lines.push(PayrollLine {
            component_name: value_to_string(&l[0]),
            line_type: value_to_string(&l[1]),
            amount: pf64(&l[2]),
        });
    }
    Ok(Some(PayrollDetail {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "payroll.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "payroll.emp")?,
        employee_number: value_to_string(&r[2]),
        name: value_to_string(&r[3]),
        department_name: popt_text(&r[4]),
        position_name: popt_text(&r[5]),
        period_name: value_to_string(&r[6]),
        start_date: value_to_string(&r[7]),
        end_date: value_to_string(&r[8]),
        basic_salary: pf64(&r[9]),
        total_income: pf64(&r[10]),
        total_deduction: pf64(&r[11]),
        net_salary: pf64(&r[12]),
        status: value_to_string(&r[13]),
        lines,
    }))
}

pub async fn my_payslips_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<PayslipInfo>, String> {
    let rows = q_all(
        db,
        "SELECT ps.id, ps.payroll_id, ps.payslip_number, pp.name, p.net_salary, ps.pdf_path FROM payslips ps INNER JOIN payrolls p ON p.id = ps.payroll_id INNER JOIN payroll_periods pp ON pp.id = p.payroll_period_id WHERE p.employee_id = ?1 ORDER BY pp.start_date DESC".to_string(),
        vec![Value::Int(employee_id)],
        6,
        "payroll.myslips",
    )
    .await
    .map_err(|e| format!("gagal membaca slip: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(PayslipInfo {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "payslip.id")?,
            payroll_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "payslip.payroll")?,
            payslip_number: value_to_string(&r[2]),
            period_name: value_to_string(&r[3]),
            net_salary: pf64(&r[4]),
            pdf_ready: popt_text(&r[5]).map(|s| !s.is_empty()).unwrap_or(false),
        });
    }
    Ok(out)
}

pub async fn deduction_list_sea(
    db: &sea_orm::DatabaseConnection,
    pending_only: bool,
) -> Result<Vec<Deduction>, String> {
    let sql = if pending_only {
        "SELECT pd.id, pd.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), pd.type, pd.description, pd.amount, pd.installment_no, pd.total_installments, pd.status FROM payroll_deductions pd INNER JOIN employees e ON e.id = pd.employee_id WHERE pd.status = 'pending' ORDER BY pd.created_at DESC"
    } else {
        "SELECT pd.id, pd.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), pd.type, pd.description, pd.amount, pd.installment_no, pd.total_installments, pd.status FROM payroll_deductions pd INNER JOIN employees e ON e.id = pd.employee_id ORDER BY pd.created_at DESC"
    };
    let rows = q_all(db, sql.to_string(), vec![], 9, "payroll.deductions")
        .await
        .map_err(|e| format!("gagal membaca kasbon: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Deduction {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "deduction.id")?,
            employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "deduction.emp")?,
            employee_name: value_to_string(&r[2]),
            deduction_type: value_to_string(&r[3]),
            description: value_to_string(&r[4]),
            amount: pf64(&r[5]),
            installment_no: match value_i64(&r[6]) {
                Some(v) => Some(to_dto_int(v, "deduction.ino")?),
                None => None,
            },
            total_installments: match value_i64(&r[7]) {
                Some(v) => Some(to_dto_int(v, "deduction.total")?),
                None => None,
            },
            status: value_to_string(&r[8]),
        });
    }
    Ok(out)
}

pub async fn deduction_save_sea(
    db: &sea_orm::DatabaseConnection,
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
    match (input.installment_no, input.total_installments) {
        (None, None) => {}
        (Some(_), None) => {
            return Err("Nomor cicilan butuh total tenor.".to_string());
        }
        (_, Some(t)) if t < 1 => {
            return Err("Tenor minimal 1 cicilan.".to_string());
        }
        (None, Some(_)) => {}
        (Some(n), Some(t)) if n < 1 || n > t => {
            return Err("Nomor cicilan harus 1 sampai total tenor.".to_string());
        }
        (Some(_), Some(_)) => {}
    }
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(input.employee_id as i64)],
        1,
        "payroll.dedemp",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let opt_int = |v: Option<i32>| match v {
        Some(x) => Value::Int(x as i64),
        None => Value::Null,
    };
    if let Some(rid) = id {
        let st = q_one(
            db,
            "SELECT status FROM payroll_deductions WHERE id = ?1".to_string(),
            vec![Value::Int(rid)],
            1,
            "payroll.dedst",
        )
        .await
        .map_err(|e| format!("gagal memuat kasbon: {e}"))?;
        if st.as_ref().map(|r| value_to_string(&r[0])).as_deref() != Some("pending") {
            return Err("Hanya kasbon pending yang bisa diubah.".to_string());
        }
        exec(
            db,
            "UPDATE payroll_deductions SET type = ?1, description = ?2, amount = ?3, installment_no = ?4, total_installments = ?5 WHERE id = ?6".to_string(),
            vec![
                Value::Text(input.deduction_type.clone()),
                Value::Text(input.description.trim().to_string()),
                Value::Float(input.amount),
                opt_int(input.installment_no),
                opt_int(input.total_installments),
                Value::Int(rid),
            ],
            "payroll.dedupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan kasbon: {e}"))?;
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "payroll.deduction",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "deduction.id")
    } else {
        exec(
            db,
            "INSERT INTO payroll_deductions (employee_id, type, description, amount, installment_no, total_installments, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')".to_string(),
            vec![
                Value::Int(input.employee_id as i64),
                Value::Text(input.deduction_type.clone()),
                Value::Text(input.description.trim().to_string()),
                Value::Float(input.amount),
                opt_int(input.installment_no),
                opt_int(input.total_installments),
            ],
            "payroll.dedadd",
        )
        .await
        .map_err(|e| format!("gagal menambah kasbon: {e}"))?;
        let rid = prow_id(db, "payroll.dedadd").await?;
        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "payroll.deduction",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "deduction.id")
    }
}

pub async fn deduction_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let st = q_one(
        db,
        "SELECT status FROM payroll_deductions WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "payroll.dedst",
    )
    .await
    .map_err(|e| format!("gagal memuat kasbon: {e}"))?;
    if st.as_ref().map(|r| value_to_string(&r[0])).as_deref() != Some("pending") {
        return Err("Hanya kasbon pending yang bisa dihapus.".to_string());
    }
    exec(
        db,
        "DELETE FROM payroll_deductions WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        "payroll.deddel",
    )
    .await
    .map_err(|e| format!("gagal menghapus kasbon: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "payroll.deduction",
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

    async fn admin(db: &sea_orm::DatabaseConnection) -> i64 {
        one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await
    }

    async fn mkemp(db: &sea_orm::DatabaseConnection, number: &str, basic: f64, ptkp_status: &str) -> i64 {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type, ptkp_status) VALUES (?1, 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent', ?2)".to_string(),
            vec![Value::Text(number.to_string()), Value::Text(ptkp_status.to_string())],
            "test.mkemp",
        )
        .await
        .expect("emp");
        let eid = one(db, "SELECT last_insert_rowid()", "test.rowid").await;
        exec(
            db,
            "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, ?2, '2026-01-01', 1)".to_string(),
            vec![Value::Int(eid), Value::Float(basic)],
            "test.mksal",
        )
        .await
        .expect("sal");
        eid
    }

    fn period_input(name: &str, start: &str, end: &str) -> PeriodInput {
        PeriodInput {
            name: name.to_string(),
            start_date: start.to_string(),
            end_date: end.to_string(),
            payment_date: None,
        }
    }

    #[test]
    fn pph21_sesuai_tarif_progresif() {
        assert_eq!(pph21_monthly(5_000_000.0, "TK/0"), 12_500.0);
        assert_eq!(pph21_monthly(4_000_000.0, "TK/0"), 0.0);
        assert_eq!(pph21_monthly(5_000_000.0, "XX"), 12_500.0);
        assert_eq!(pph21_monthly(20_000_000.0, "K/1"), 1_637_500.0);
    }

    #[tokio::test]
    async fn generate_sea_menghitung_lengkap_dan_alur_terkunci() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let eid = mkemp(db, "EMP-G1", 5_000_000.0, "TK/0").await;
        exec(
            db,
            "INSERT INTO overtime_requests (employee_id, date, start_time, end_time, duration_minutes, rate_multiplier, status, current_step) VALUES (?1, '2026-02-10', '2026-02-10 18:00:00', '2026-02-10 20:00:00', 120, 1.5, 'approved', 0)".to_string(),
            vec![Value::Int(eid)],
            "test.ot",
        )
        .await
        .expect("ot");
        exec(
            db,
            "INSERT INTO attendances (employee_id, date, status) VALUES (?1, '2026-02-11', 'absent')".to_string(),
            vec![Value::Int(eid)],
            "test.absen",
        )
        .await
        .expect("absen");
        exec(
            db,
            "INSERT INTO payroll_deductions (employee_id, type, description, amount, status) VALUES (?1, 'kasbon', 'Kasbon Feb', 300000, 'pending')".to_string(),
            vec![Value::Int(eid)],
            "test.kasbon",
        )
        .await
        .expect("kasbon");
        let pid = period_create_sea(
            db,
            actor,
            actor,
            &PeriodInput {
                name: "Feb 2026".to_string(),
                start_date: "2026-02-01".to_string(),
                end_date: "2026-02-28".to_string(),
                payment_date: Some("2026-03-05".to_string()),
            },
        )
        .await
        .expect("periode");
        assert!(period_create_sea(
            db,
            actor,
            actor,
            &period_input("X", "2026-02-15", "2026-03-15"),
        )
        .await
        .is_err());
        assert!(approve_period_sea(db, actor, pid as i64).await.is_err());
        generate_sea(db, actor, pid as i64).await.expect("generate");
        assert_eq!(
            text(db, &format!("SELECT status FROM payroll_periods WHERE id = {pid}"), "test.pstatus").await,
            "review"
        );
        let rows = payrolls_for_period_sea(db, pid as i64).await.expect("rows");
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.total_income, 5_086_705.0);
        assert_eq!(r.net_salary, 4_342_814.0);
        let det = payroll_detail_sea(db, r.id as i64).await.expect("det").expect("ada");
        assert!(det.lines.iter().any(|l| l.component_name == "Lembur"));
        assert!(det.lines.iter().any(|l| l.component_name == "Kasbon Feb"));
        assert!(det.lines.iter().any(|l| l.component_name == "PPh 21 (estimasi)"));
        assert_eq!(
            text(db, "SELECT status FROM payroll_deductions WHERE description = 'Kasbon Feb'", "test.kstatus").await,
            "processed"
        );
        assert!(generate_sea(db, actor, pid as i64).await.is_err());
        approve_period_sea(db, actor, pid as i64).await.expect("approve");
        assert!(lock_period_sea(db, actor, pid as i64).await.is_err());
        mark_paid_sea(db, actor, pid as i64).await.expect("pay");
        let slips = my_payslips_sea(db, eid).await.expect("slips");
        assert_eq!(slips.len(), 1);
        assert!(slips[0].payslip_number.starts_with("PS-"));
        assert_eq!(slips[0].net_salary, 4_342_814.0);
        lock_period_sea(db, actor, pid as i64).await.expect("lock");
        assert_eq!(
            text(db, &format!("SELECT status FROM payroll_periods WHERE id = {pid}"), "test.pstatus").await,
            "locked"
        );
    }

    #[tokio::test]
    async fn bpjs_cap_sea_membatasi_dasar_upah() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let eid = mkemp(db, "EMP-CAP", 20_000_000.0, "TK/0").await;
        let pid = period_create_sea(db, actor, actor, &period_input("Cap 2026", "2026-03-01", "2026-03-31"))
            .await
            .expect("periode");
        exec(
            db,
            "UPDATE system_settings SET setting_value = '10000000' WHERE setting_key = 'bpjs_jp_max_wage'".to_string(),
            vec![],
            "test.cap",
        )
        .await
        .expect("cap");
        generate_sea(db, actor, pid as i64).await.expect("generate");
        let prid = one(
            db,
            &format!("SELECT id FROM payrolls WHERE payroll_period_id = {pid} AND employee_id = {eid}"),
            "test.prid",
        )
        .await;
        let det = payroll_detail_sea(db, prid).await.expect("det").expect("ada");
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

    #[tokio::test]
    async fn thr_sea_proporsional_masuk_payroll() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let eid = mkemp(db, "EMP-THR", 6_000_000.0, "TK/0").await;
        let pid = period_create_sea(db, actor, actor, &period_input("Mar 2026", "2026-03-01", "2026-03-31"))
            .await
            .expect("periode");
        exec(
            db,
            "UPDATE system_settings SET setting_value = '2026-03-20' WHERE setting_key = 'thr_holiday_date'".to_string(),
            vec![],
            "test.holiday",
        )
        .await
        .expect("holiday");
        generate_sea(db, actor, pid as i64).await.expect("generate");
        let prid = one(
            db,
            &format!("SELECT id FROM payrolls WHERE payroll_period_id = {pid} AND employee_id = {eid}"),
            "test.prid",
        )
        .await;
        let det = payroll_detail_sea(db, prid).await.expect("det").expect("ada");
        let thr = det.lines.iter().find(|l| l.component_name == "THR").expect("baris THR ada");
        assert_eq!(thr.amount, 1_000_000.0);
    }

    #[tokio::test]
    async fn tenor_kasbon_sea_divalidasi_saat_simpan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let eid = mkemp(db, "EMP-TENOR", 5_000_000.0, "TK/0").await;
        let base = DeductionInput {
            employee_id: eid as i32,
            deduction_type: "kasbon".to_string(),
            description: "Kasbon tenor".to_string(),
            amount: 900000.0,
            installment_no: None,
            total_installments: None,
        };
        assert!(deduction_save_sea(db, actor, None, &base).await.is_ok());
        let no_total = DeductionInput {
            installment_no: Some(1),
            ..base.clone()
        };
        assert!(deduction_save_sea(db, actor, None, &no_total).await.is_err());
        let zero = DeductionInput {
            total_installments: Some(0),
            ..base.clone()
        };
        assert!(deduction_save_sea(db, actor, None, &zero).await.is_err());
        let over = DeductionInput {
            installment_no: Some(4),
            total_installments: Some(3),
            ..base.clone()
        };
        assert!(deduction_save_sea(db, actor, None, &over).await.is_err());
        let valid = DeductionInput {
            installment_no: Some(1),
            total_installments: Some(3),
            ..base
        };
        assert!(deduction_save_sea(db, actor, None, &valid).await.is_ok());
    }

    #[tokio::test]
    async fn kasbon_sea_bertahap_terproses_di_cicilan_terakhir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let eid = mkemp(db, "EMP-CICIL", 5_000_000.0, "TK/0").await;
        deduction_save_sea(
            db,
            actor,
            None,
            &DeductionInput {
                employee_id: eid as i32,
                deduction_type: "kasbon".to_string(),
                description: "Kasbon 3x".to_string(),
                amount: 300000.0,
                installment_no: Some(1),
                total_installments: Some(3),
            },
        )
        .await
        .expect("simpan tenor");
        let mut pids = Vec::new();
        for (name, start, end) in [
            ("Feb 2026", "2026-02-01", "2026-02-28"),
            ("Mar 2026", "2026-03-01", "2026-03-31"),
            ("Apr 2026", "2026-04-01", "2026-04-30"),
        ] {
            pids.push(
                period_create_sea(db, actor, actor, &period_input(name, start, end))
                    .await
                    .expect("periode") as i64,
            );
        }
        for (i, pid) in pids.iter().enumerate() {
            generate_sea(db, actor, *pid).await.expect("generate");
            let prid = one(
                db,
                &format!("SELECT id FROM payrolls WHERE payroll_period_id = {pid} AND employee_id = {eid}"),
                "test.prid",
            )
            .await;
            let det = payroll_detail_sea(db, prid).await.expect("det").expect("ada");
            let line = det
                .lines
                .iter()
                .find(|l| l.component_name == "Kasbon 3x")
                .expect("baris kasbon ada");
            assert_eq!(line.amount, 300000.0);
            let st = q_one(
                db,
                "SELECT status, installment_no FROM payroll_deductions WHERE description = 'Kasbon 3x'".to_string(),
                vec![],
                2,
                "test.kstatus",
            )
            .await
            .expect("st")
            .expect("ada");
            let status = value_to_string(&st[0]);
            let no = value_i64(&st[1]);
            if i < 2 {
                assert_eq!(status, "pending");
                assert_eq!(no, Some(i as i64 + 2));
            } else {
                assert_eq!(status, "processed");
                assert_eq!(no, Some(3));
            }
        }
    }

    #[tokio::test]
    async fn komponen_crud_sea_dan_ganda_ditolak() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        assert!(!component_list_sea(db).await.expect("list").is_empty());
        let comp = ComponentInput {
            code: "T-TEST".to_string(),
            name: "Tes".to_string(),
            component_type: "income".to_string(),
            calculation_type: "fixed".to_string(),
            is_taxable: false,
            is_active: true,
        };
        let id = component_save_sea(db, actor, None, &comp).await.expect("buat");
        let dup = component_save_sea(
            db,
            actor,
            None,
            &ComponentInput {
                code: "T-TEST".to_string(),
                name: "Dobel".to_string(),
                ..comp.clone()
            },
        )
        .await
        .expect_err("ganda");
        assert_eq!(dup, "Kode komponen sudah dipakai.");
        component_delete_sea(db, actor, id as i64).await.expect("hapus");
        assert!(component_delete_sea(db, actor, id as i64).await.is_err());
    }
}
