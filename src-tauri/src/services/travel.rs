//! Dinas: pengajuan dinamis + biaya + settle. Reimburse: kategori + tahap linier.

use chrono::NaiveDate;
use std::path::Path;

use super::approval;
use super::audit;
use super::employees::FileUpload;
use crate::to_dto_int;

const RECEIPT_MIMES: &[&str] = &["image/jpeg", "image/png", "application/pdf"];
const MAX_RECEIPT_BYTES: usize = 3 * 1024 * 1024;


#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Trip {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub destination: String,
    pub purpose: String,
    pub start_date: String,
    pub end_date: String,
    pub transportation: Option<String>,
    pub hotel: Option<String>,
    pub budget: f64,
    pub status: String,
    pub current_step: i32,
    pub expense_total: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct TripInput {
    pub destination: String,
    pub purpose: String,
    pub start_date: String,
    pub end_date: String,
    pub transportation: Option<String>,
    pub hotel: Option<String>,
    pub budget: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct TripExpense {
    pub id: i32,
    pub category: String,
    pub description: Option<String>,
    pub amount: f64,
    pub receipt_path: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct TripExpenseInput {
    pub category: String,
    pub description: Option<String>,
    pub amount: f64,
}

fn store_receipt(files: &Path, subdir: &str, file: &FileUpload) -> Result<String, String> {
    if !RECEIPT_MIMES.contains(&file.mime.as_str()) {
        return Err("Struk harus JPG, PNG, atau PDF.".to_string());
    }
    if file.bytes.is_empty() {
        return Err("Berkas kosong.".to_string());
    }
    if file.bytes.len() > MAX_RECEIPT_BYTES {
        return Err("Ukuran struk maksimal 3MB.".to_string());
    }
    let dir = files.join(subdir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("gagal membuat folder: {e}"))?;
    let base: String = Path::new(&file.name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("struk")
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S%f").to_string();
    let rel = format!("{subdir}/{stamp}_{base}");
    std::fs::write(files.join(&rel), &file.bytes)
        .map_err(|e| format!("gagal menyimpan struk: {e}"))?;
    Ok(rel)
}


#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReimburseCategory {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub max_amount: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Reimburse {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub category_id: i32,
    pub category_name: String,
    pub amount: f64,
    pub description: Option<String>,
    pub receipt_path: Option<String>,
    pub status: String,
    pub current_step: i32,
    pub paid_at: Option<String>,
    pub supervisor_id: Option<i32>,
    pub manager_id: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ReimburseInput {
    pub category_id: i32,
    pub amount: f64,
    pub description: Option<String>,
}

const REIMBURSE_STAGES: &[&str] = &["pending", "manager_approved", "finance_verified", "paid"];


use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};

fn tropt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

fn tropt_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse().ok(),
    }
}


fn map_trip_row(r: &[Value]) -> Result<Trip, String> {
    Ok(Trip {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "trip.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "trip.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        destination: value_to_string(&r[4]),
        purpose: value_to_string(&r[5]),
        start_date: value_to_string(&r[6]),
        end_date: value_to_string(&r[7]),
        transportation: tropt_text(&r[8]),
        hotel: tropt_text(&r[9]),
        budget: tropt_f64(&r[10]).unwrap_or(0.0),
        status: value_to_string(&r[11]),
        current_step: to_dto_int(value_i64(&r[12]).unwrap_or(0), "trip.step")?,
        expense_total: tropt_f64(&r[13]).unwrap_or(0.0),
    })
}

async fn trip_rows_sea(
    db: &sea_orm::DatabaseConnection,
    extra_where: &str,
    param: Option<i64>,
) -> Result<Vec<Trip>, String> {
    let sql = format!("SELECT bt.id, bt.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, bt.destination, bt.purpose, bt.start_date, bt.end_date, bt.transportation, bt.hotel, bt.budget, bt.status, bt.current_step, COALESCE((SELECT SUM(amount) FROM business_trip_expenses x WHERE x.business_trip_id = bt.id), 0) FROM business_trips bt INNER JOIN employees e ON e.id = bt.employee_id {extra_where} ORDER BY CASE bt.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 WHEN 'rejected' THEN 2 WHEN 'completed' THEN 3 ELSE 4 END, bt.created_at DESC");
    let rows = match param {
        Some(p) => {
            q_all(db, sql, vec![Value::Int(p)], 14, "travel.trips")
                .await
                .map_err(|e| format!("gagal membaca dinas: {e}"))?
        }
        None => q_all(db, sql, vec![], 14, "travel.trips")
            .await
            .map_err(|e| format!("gagal membaca dinas: {e}"))?,
    };
    rows.iter().map(|r| map_trip_row(r)).collect()
}

pub async fn my_trips_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<Trip>, String> {
    trip_rows_sea(db, "WHERE bt.employee_id = ?1", Some(employee_id)).await
}

pub async fn all_trips_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Trip>, String> {
    trip_rows_sea(db, "", None).await
}

pub async fn trip_expenses_sea(
    db: &sea_orm::DatabaseConnection,
    trip_id: i64,
) -> Result<Vec<TripExpense>, String> {
    let rows = q_all(
        db,
        "SELECT id, category, description, amount, receipt_path FROM business_trip_expenses WHERE business_trip_id = ?1 ORDER BY id".to_string(),
        vec![Value::Int(trip_id)],
        5,
        "travel.expenses",
    )
    .await
    .map_err(|e| format!("gagal membaca biaya: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(TripExpense {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "expense.id")?,
            category: value_to_string(&r[1]),
            description: tropt_text(&r[2]),
            amount: tropt_f64(&r[3]).unwrap_or(0.0),
            receipt_path: tropt_text(&r[4]),
        });
    }
    Ok(out)
}

/// Menunggu putusan user: resolve dinamis tiap panggil (tanpa snapshot).
pub async fn pending_for_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Vec<Trip>, String> {
    let mut out = Vec::new();
    for trip in all_trips_sea(db)
        .await?
        .into_iter()
        .filter(|t| t.status == "pending")
    {
        let chain = approval::build_chain_sea(db, "business_trip", trip.employee_id as i64).await?;
        if let Some(step) = chain.get(trip.current_step as usize - 1) {
            if step.approver_id == user_id {
                out.push(trip);
            }
        }
    }
    Ok(out)
}

pub async fn trip_create_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &TripInput,
) -> Result<i32, String> {
    if input.destination.trim().is_empty() {
        return Err("Tujuan wajib diisi.".to_string());
    }
    if input.destination.len() > 150 {
        return Err("Tujuan maksimal 150 karakter.".to_string());
    }
    if input.purpose.trim().is_empty() {
        return Err("Keperluan wajib diisi.".to_string());
    }
    if input.purpose.len() > 255 {
        return Err("Keperluan maksimal 255 karakter.".to_string());
    }
    NaiveDate::parse_from_str(input.start_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.end_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if input.end_date.trim() < input.start_date.trim() {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    if !(input.budget >= 0.0) {
        return Err("Budget minimal 0.".to_string());
    }
    let chain = approval::build_chain_sea(db, "business_trip", employee_id).await?;
    let status = if chain.is_empty() {
        "approved"
    } else {
        "pending"
    };
    let rid = exec_insert(
        db,
        "INSERT INTO business_trips (employee_id, destination, purpose, start_date, end_date, transportation, hotel, budget, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(input.destination.trim().to_string()),
            Value::Text(input.purpose.trim().to_string()),
            Value::Text(input.start_date.trim().to_string()),
            Value::Text(input.end_date.trim().to_string()),
            match input.transportation.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            match input.hotel.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Float(input.budget),
            Value::Text(status.to_string()),
        ],
        "travel.tripadd",
    )
    .await
    .map_err(|e| format!("gagal mengajukan dinas: {e}"))?;

    if let Some(first) = chain.first() {
        let name_row = q_one(
            db,
            "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
            vec![Value::Int(employee_id)],
            1,
            "travel.empname",
        )
        .await
        .unwrap_or(None);
        let name = name_row
            .as_ref()
            .map(|r| value_to_string(&r[0]))
            .unwrap_or_default();
        approval::notify_sea(
            db,
            first.approver_id,
            "business_trip",
            "Pengajuan Dinas Baru",
            &format!("{name} mengajukan perjalanan dinas."),
            "/travel",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "business_trip",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "trip.id")
}

pub async fn trip_decide_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    id: i64,
    decision: &str,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let row = q_one(
        db,
        "SELECT employee_id, status, current_step, destination FROM business_trips WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        4,
        "travel.tripreq",
    )
    .await
    .map_err(|e| format!("gagal memuat dinas: {e}"))?;
    let Some(row) = row else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    let (emp_id, status, step, dest) = (
        value_i64(&row[0]).unwrap_or(0),
        value_to_string(&row[1]),
        value_i64(&row[2]).unwrap_or(0),
        value_to_string(&row[3]),
    );
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let chain = approval::build_chain_sea(db, "business_trip", emp_id).await?;
    let current = chain.get(step as usize - 1);
    match current {
        Some(s) if s.approver_id == user_id => {}
        _ => return Err("Tidak berwenang memutus pengajuan ini.".to_string()),
    }
    if decision == "rejected" {
        exec(
            db,
            "UPDATE business_trips SET status = 'rejected' WHERE id = ?1".to_string(),
            vec![Value::Int(id)],
            "travel.tripreject",
        )
        .await
        .map_err(|e| format!("gagal menolak: {e}"))?;
    } else if chain.get(step as usize).is_some() {
        exec(
            db,
            "UPDATE business_trips SET current_step = current_step + 1 WHERE id = ?1".to_string(),
            vec![Value::Int(id)],
            "travel.tripadv",
        )
        .await
        .map_err(|e| format!("gagal maju tahap: {e}"))?;
        let next = &chain[step as usize];
        approval::notify_sea(
            db,
            next.approver_id,
            "business_trip",
            "Dinas Menunggu Persetujuan",
            "Ada pengajuan dinas menunggu persetujuan Anda.",
            "/travel",
        )
        .await?;
    } else {
        exec(
            db,
            "UPDATE business_trips SET status = 'approved' WHERE id = ?1".to_string(),
            vec![Value::Int(id)],
            "travel.tripapprove",
        )
        .await
        .map_err(|e| format!("gagal menyetujui: {e}"))?;
    }
    if let Some(uid) = approval::user_of_employee_sea(db, emp_id).await? {
        approval::notify_sea(
            db,
            uid,
            "business_trip",
            "Status Perjalanan Dinas",
            &format!("Pengajuan dinas ke {dest} telah diproses."),
            "/travel",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(user_id),
        decision.to_uppercase().as_str(),
        "business_trip",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn trip_add_expense_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    actor_id: i64,
    trip_id: i64,
    input: &TripExpenseInput,
    receipt: Option<&FileUpload>,
) -> Result<i32, String> {
    let exists = q_one(
        db,
        "SELECT id FROM business_trips WHERE id = ?1".to_string(),
        vec![Value::Int(trip_id)],
        1,
        "travel.tripcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa dinas: {e}"))?;
    if exists.is_none() {
        return Err("Dinas tidak ditemukan.".to_string());
    }
    if input.category.trim().is_empty() {
        return Err("Kategori wajib diisi.".to_string());
    }
    if input.category.len() > 100 {
        return Err("Kategori maksimal 100 karakter.".to_string());
    }
    if !(input.amount >= 0.0) {
        return Err("Nominal minimal 0.".to_string());
    }
    let receipt_path = match receipt {
        Some(f) => Some(store_receipt(
            files,
            &format!("trip_expenses/{trip_id}"),
            f,
        )?),
        None => None,
    };
    let rid = exec_insert(
        db,
        "INSERT INTO business_trip_expenses (business_trip_id, category, description, amount, receipt_path) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(trip_id),
            Value::Text(input.category.trim().to_string()),
            match input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Float(input.amount),
            match receipt_path {
                Some(s) => Value::Text(s),
                None => Value::Null,
            },
        ],
        "travel.expadd",
    )
    .await
    .map_err(|e| format!("gagal menambah biaya: {e}"))?;

    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "business_trip.expense",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "expense.id")
}

pub async fn trip_settle_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    trip_id: i64,
) -> Result<(), String> {
    let status_row = q_one(
        db,
        "SELECT status FROM business_trips WHERE id = ?1".to_string(),
        vec![Value::Int(trip_id)],
        1,
        "travel.settlecheck",
    )
    .await
    .map_err(|e| format!("gagal memuat dinas: {e}"))?;
    match status_row.as_ref().map(|r| value_to_string(&r[0])).as_deref() {
        Some("approved") | Some("completed") => {}
        Some(_) | None => {
            return Err("Dinas harus approved atau completed untuk settle.".to_string());
        }
    }
    exec(
        db,
        "UPDATE business_trips SET status = 'settled' WHERE id = ?1".to_string(),
        vec![Value::Int(trip_id)],
        "travel.settle",
    )
    .await
    .map_err(|e| format!("gagal settle: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "business_trip",
        Some(&trip_id.to_string()),
        None,
        None,
        Some("Status menjadi settled"),
    )
    .await?;
    Ok(())
}

pub async fn category_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<ReimburseCategory>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name, max_amount FROM reimbursement_categories ORDER BY name".to_string(),
        vec![],
        4,
        "travel.reimbcats",
    )
    .await
    .map_err(|e| format!("gagal membaca kategori: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(ReimburseCategory {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "reimbcat.id")?,
            code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
            max_amount: tropt_f64(&r[3]),
        });
    }
    Ok(out)
}

pub async fn category_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    code: &str,
    name: &str,
    max_amount: Option<f64>,
) -> Result<i32, String> {
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err("Kode dan nama kategori wajib diisi.".to_string());
    }
    if let Some(m) = max_amount {
        if !(m > 0.0) {
            return Err("Batas maksimal harus lebih dari 0.".to_string());
        }
    }
    let max = match max_amount {
        Some(m) => Value::Float(m),
        None => Value::Null,
    };
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE reimbursement_categories SET code = ?1, name = ?2, max_amount = ?3 WHERE id = ?4".to_string(),
            vec![
                Value::Text(code.trim().to_string()),
                Value::Text(name.trim().to_string()),
                max,
                Value::Int(rid),
            ],
            "travel.catupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan kategori: {e}"))?;
        if n == 0 {
            return Err("Kategori tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "reimbursement.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "reimbcat.id")
    } else {
        let rid = exec_insert(
            db,
            "INSERT INTO reimbursement_categories (code, name, max_amount) VALUES (?1, ?2, ?3)".to_string(),
            vec![
                Value::Text(code.trim().to_string()),
                Value::Text(name.trim().to_string()),
                max,
            ],
            "travel.catadd",
        )
        .await
        .map_err(|e| {
            if e.contains("UNIQUE") {
                "Kode kategori sudah dipakai.".to_string()
            } else {
                format!("gagal menambah kategori: {e}")
            }
        })?;
        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "reimbursement.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        return to_dto_int(rid, "reimbcat.id");
    }
}

pub async fn category_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM reimbursements WHERE reimbursement_category_id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "travel.catrel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Kategori sudah dipakai pengajuan.".to_string());
    }
    let d = exec(
        db,
        "DELETE FROM reimbursement_categories WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        "travel.catdel",
    )
    .await
    .map_err(|e| format!("gagal menghapus kategori: {e}"))?;
    if d == 0 {
        return Err("Kategori tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "reimbursement.category",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn my_reimburse_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<Reimburse>, String> {
    reimburse_rows_sea(db, "WHERE r.employee_id = ?1", Some(employee_id)).await
}

pub async fn all_reimburse_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Reimburse>, String> {
    reimburse_rows_sea(db, "", None).await
}

fn map_reimb_row(r: &[Value]) -> Result<Reimburse, String> {
    let opt_i = |v: &Value| match value_i64(v) {
        Some(x) => to_dto_int(x, "reimb.ref").map(Some),
        None => Ok(None),
    };
    Ok(Reimburse {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "reimb.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "reimb.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        category_id: to_dto_int(value_i64(&r[4]).unwrap_or(0), "reimb.cat")?,
        category_name: value_to_string(&r[5]),
        amount: tropt_f64(&r[6]).unwrap_or(0.0),
        description: tropt_text(&r[7]),
        receipt_path: tropt_text(&r[8]),
        status: value_to_string(&r[9]),
        current_step: to_dto_int(value_i64(&r[10]).unwrap_or(0), "reimb.step")?,
        paid_at: tropt_text(&r[11]),
        supervisor_id: opt_i(&r[12])?,
        manager_id: opt_i(&r[13])?,
    })
}

async fn reimburse_rows_sea(
    db: &sea_orm::DatabaseConnection,
    extra_where: &str,
    param: Option<i64>,
) -> Result<Vec<Reimburse>, String> {
    let sql = format!("SELECT r.id, r.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, r.reimbursement_category_id, rc.name, r.amount, r.description, r.receipt_path, r.status, r.current_step, r.paid_at, e.supervisor_id, e.manager_id FROM reimbursements r INNER JOIN reimbursement_categories rc ON rc.id = r.reimbursement_category_id INNER JOIN employees e ON e.id = r.employee_id {extra_where} ORDER BY CASE r.status WHEN 'pending' THEN 0 WHEN 'manager_approved' THEN 1 WHEN 'finance_verified' THEN 2 WHEN 'paid' THEN 3 ELSE 4 END, r.created_at DESC");
    let rows = match param {
        Some(p) => {
            q_all(db, sql, vec![Value::Int(p)], 14, "travel.reimbs")
                .await
                .map_err(|e| format!("gagal membaca reimburse: {e}"))?
        }
        None => q_all(db, sql, vec![], 14, "travel.reimbs")
            .await
            .map_err(|e| format!("gagal membaca reimburse: {e}"))?,
    };
    rows.iter().map(|r| map_reimb_row(r)).collect()
}

pub async fn reimburse_create_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    actor_id: i64,
    employee_id: i64,
    input: &ReimburseInput,
    receipt: Option<&FileUpload>,
) -> Result<i32, String> {
    let cat = q_one(
        db,
        "SELECT max_amount FROM reimbursement_categories WHERE id = ?1".to_string(),
        vec![Value::Int(input.category_id as i64)],
        1,
        "travel.catchk",
    )
    .await
    .map_err(|e| format!("gagal memeriksa kategori: {e}"))?;
    let Some(cat) = cat else {
        return Err("Kategori tidak ditemukan.".to_string());
    };
    let max = tropt_f64(&cat[0]);
    if !(input.amount >= 1.0) {
        return Err("Nominal minimal 1.".to_string());
    }
    if let Some(m) = max {
        if input.amount > m {
            return Err(format!("Melebihi batas kategori ({m})."));
        }
    }
    if let Some(d) = input.description.as_deref() {
        if d.len() > 255 {
            return Err("Keterangan maksimal 255 karakter.".to_string());
        }
    }
    let receipt_path = match receipt {
        Some(f) => Some(store_receipt(files, "reimbursements", f)?),
        None => None,
    };
    let rid = exec_insert(
        db,
        "INSERT INTO reimbursements (employee_id, reimbursement_category_id, amount, description, receipt_path, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 1)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(input.category_id as i64),
            Value::Float(input.amount),
            match input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            match receipt_path {
                Some(s) => Value::Text(s),
                None => Value::Null,
            },
        ],
        "travel.reimbadd",
    )
    .await
    .map_err(|e| format!("gagal mengajukan reimburse: {e}"))?;

    let emp = q_one(
        db,
        "SELECT supervisor_id, manager_id, first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        3,
        "travel.reimbemp",
    )
    .await
    .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    if let Some(emp) = emp {
        let (sup, mgr, name) = (
            value_i64(&emp[0]),
            value_i64(&emp[1]),
            value_to_string(&emp[2]),
        );
        let approver = mgr.or(sup);
        if let Some(aid) = approver {
            if let Some(uid) = approval::user_of_employee_sea(db, aid).await? {
                approval::notify_sea(
                    db,
                    uid,
                    "reimbursement",
                    "Pengajuan Reimbursement Baru",
                    &format!("{name} mengajukan reimbursement."),
                    "/travel",
                )
                .await?;
            }
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "reimbursement",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "reimb.id")
}

/// Putuskan tahap berjalan. Pending = manajer/supervisor; sisanya izin.
pub async fn reimburse_decide_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    user_employee_id: Option<i64>,
    privileged: bool,
    id: i64,
    action: &str,
) -> Result<String, String> {
    if action != "approve" && action != "reject" {
        return Err("Aksi tidak valid.".to_string());
    }
    let row = q_one(
        db,
        "SELECT r.status, e.supervisor_id, e.manager_id, r.employee_id FROM reimbursements r INNER JOIN employees e ON e.id = r.employee_id WHERE r.id = ?1".to_string(),
        vec![Value::Int(id)],
        4,
        "travel.reimbreq",
    )
    .await
    .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some(row) = row else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    let (status, sup, mgr) = (
        value_to_string(&row[0]),
        value_i64(&row[1]),
        value_i64(&row[2]),
    );
    if status == "paid" || status == "rejected" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let can = if status == "pending" {
        let boss = mgr.or(sup);
        match (user_employee_id, boss) {
            (Some(a), Some(b)) if a == b => true,
            _ => privileged,
        }
    } else {
        privileged
    };
    if !can {
        return Err("Tidak berwenang memproses tahap ini.".to_string());
    }
    if action == "reject" {
        exec(
            db,
            "UPDATE reimbursements SET status = 'rejected' WHERE id = ?1".to_string(),
            vec![Value::Int(id)],
            "travel.reimbreject",
        )
        .await
        .map_err(|e| format!("gagal menolak: {e}"))?;
        audit::log_sea(
            db,
            Some(user_id),
            "REJECT",
            "reimbursement",
            Some(&id.to_string()),
            None,
            None,
            None,
        )
        .await?;
        notify_employee_sea(db, id).await?;
        return Ok("rejected".to_string());
    }
    let idx = REIMBURSE_STAGES
        .iter()
        .position(|s| *s == status)
        .ok_or("Status tidak dikenal.".to_string())?;
    let next = REIMBURSE_STAGES
        .get(idx + 1)
        .ok_or("Sudah tahap akhir.".to_string())?;
    if *next == "paid" {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        exec(
            db,
            "UPDATE reimbursements SET status = 'paid', current_step = ?1, paid_at = COALESCE(paid_at, ?2) WHERE id = ?3".to_string(),
            vec![
                Value::Int((idx + 2) as i64),
                Value::Text(now),
                Value::Int(id),
            ],
            "travel.reimbpay",
        )
        .await
        .map_err(|e| format!("gagal membayar: {e}"))?;
    } else {
        exec(
            db,
            "UPDATE reimbursements SET status = ?1, current_step = ?2 WHERE id = ?3".to_string(),
            vec![
                Value::Text(next.to_string()),
                Value::Int((idx + 2) as i64),
                Value::Int(id),
            ],
            "travel.reimbadv",
        )
        .await
        .map_err(|e| format!("gagal maju tahap: {e}"))?;
    }
    audit::log_sea(
        db,
        Some(user_id),
        "APPROVE",
        "reimbursement",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Tahap menjadi {next}")),
    )
    .await?;
    notify_employee_sea(db, id).await?;
    Ok(next.to_string())
}

async fn notify_employee_sea(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(), String> {
    let emp = q_one(
        db,
        "SELECT employee_id FROM reimbursements WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "travel.reimbemp2",
    )
    .await
    .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    if let Some(eid) = emp.as_ref().and_then(|r| value_i64(&r[0])) {
        if let Some(uid) = approval::user_of_employee_sea(db, eid).await? {
            approval::notify_sea(
                db,
                uid,
                "reimbursement",
                "Status Reimbursement",
                "Pengajuan reimbursement Anda diproses.",
                "/travel",
            )
            .await?;
        }
    }
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

    async fn one_val(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> Option<Value> {
        q_one(db, sql.to_string(), vec![], 1, label)
            .await
            .expect("one")
            .map(|r| r[0].clone())
    }

    async fn status_of(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> String {
        match one_val(db, sql, label).await {
            Some(Value::Null) => String::new(),
            Some(v) => value_to_string(&v),
            None => panic!("baris tidak ada"),
        }
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
        let eid = one(db, "SELECT last_insert_rowid()", "test.rowid").await;
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
        let uid = one(db, "SELECT last_insert_rowid()", "test.rowid").await;
        (uid, eid)
    }

    #[tokio::test]
    async fn dinas_sea_ajukan_setujui_biaya_lalu_settle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin_uid = one(db, "SELECT id FROM users WHERE username = 'admin'", "t.admin").await;
        let (sup_uid, sup_eid) = mkuser(db, "dns-spv", "EMP-D1", None).await;
        let (uid, eid) = mkuser(db, "dns-staff", "EMP-D2", Some(sup_eid)).await;
        let ev = trip_create_sea(db, uid, eid, &TripInput::default())
            .await
            .expect_err("validasi");
        assert!(ev.contains("Tujuan wajib diisi"));
        assert!(trip_create_sea(
            db,
            uid,
            eid,
            &TripInput {
                destination: "Bandung".to_string(),
                purpose: "Survey".to_string(),
                start_date: "2026-10-07".to_string(),
                end_date: "2026-10-05".to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("rentang tanggal")
        .contains("sebelum"));
        let tid = trip_create_sea(
            db,
            uid,
            eid,
            &TripInput {
                destination: "Bandung".to_string(),
                purpose: "Survey".to_string(),
                start_date: "2026-10-05".to_string(),
                end_date: "2026-10-07".to_string(),
                transportation: Some("Kereta".to_string()),
                hotel: None,
                budget: 2_000_000.0,
            },
        )
        .await
        .expect("buat");
        assert_eq!(
            status_of(
                db,
                &format!("SELECT status FROM business_trips WHERE id = {tid}"),
                "t.st"
            )
            .await,
            "pending"
        );
        let pend = pending_for_sea(db, sup_uid).await.expect("antre");
        assert!(pend.iter().any(|t| t.id == tid));
        assert!(pending_for_sea(db, uid)
            .await
            .expect("antre2")
            .iter()
            .all(|t| t.id != tid));
        let e = trip_decide_sea(db, uid, tid as i64, "approved")
            .await
            .expect_err("otorisasi");
        assert!(e.contains("berwenang"));
        trip_decide_sea(db, sup_uid, tid as i64, "approved")
            .await
            .expect("setuju");
        assert_eq!(
            status_of(
                db,
                &format!("SELECT status FROM business_trips WHERE id = {tid}"),
                "t.st2"
            )
            .await,
            "approved"
        );
        assert!(trip_decide_sea(db, sup_uid, tid as i64, "approved")
            .await
            .expect_err("putus ganda")
            .contains("sudah diproses"));
        assert!(pending_for_sea(db, sup_uid)
            .await
            .expect("antre3")
            .iter()
            .all(|t| t.id != tid));
        let receipt = FileUpload {
            name: "tiket 1.png".to_string(),
            mime: "image/png".to_string(),
            bytes: vec![1, 2, 3],
        };
        trip_add_expense_sea(
            db,
            files.path(),
            uid,
            tid as i64,
            &TripExpenseInput {
                category: "Tiket".to_string(),
                description: None,
                amount: 500_000.0,
            },
            Some(&receipt),
        )
        .await
        .expect("biaya");
        trip_add_expense_sea(
            db,
            files.path(),
            uid,
            tid as i64,
            &TripExpenseInput {
                category: "Hotel".to_string(),
                description: Some("2 malam".to_string()),
                amount: 150_000.0,
            },
            None,
        )
        .await
        .expect("biaya2");
        assert!(trip_add_expense_sea(
            db,
            files.path(),
            uid,
            999_999,
            &TripExpenseInput {
                category: "Lain".to_string(),
                description: None,
                amount: 10_000.0,
            },
            None,
        )
        .await
        .expect_err("dinas hilang")
        .contains("tidak ditemukan"));
        let exps = trip_expenses_sea(db, tid as i64).await.expect("exp");
        assert_eq!(exps.len(), 2);
        assert_eq!(exps[0].amount, 500_000.0);
        let rp = exps[0].receipt_path.as_deref().expect("path struk");
        assert!(files.path().join(rp).exists());
        assert!(exps[1].receipt_path.is_none());
        assert_eq!(exps[1].description.as_deref(), Some("2 malam"));
        let trip = my_trips_sea(db, eid)
            .await
            .expect("mine")
            .into_iter()
            .find(|t| t.id == tid)
            .expect("ada");
        assert_eq!(trip.employee_number, "EMP-D2");
        assert_eq!(trip.transportation.as_deref(), Some("Kereta"));
        assert_eq!(trip.current_step, 1);
        assert_eq!(trip.expense_total, 650_000.0);
        assert!(all_trips_sea(db)
            .await
            .expect("all")
            .iter()
            .any(|t| t.id == tid));
        trip_settle_sea(db, admin_uid, tid as i64)
            .await
            .expect("settle");
        assert_eq!(
            status_of(
                db,
                &format!("SELECT status FROM business_trips WHERE id = {tid}"),
                "t.st3"
            )
            .await,
            "settled"
        );
        assert!(trip_settle_sea(db, admin_uid, tid as i64)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn reimburse_sea_rantai_manager_finance_paid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin_uid = one(db, "SELECT id FROM users WHERE username = 'admin'", "t.admin").await;
        let admin_emp = one(
            db,
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            "t.adminemp",
        )
        .await;
        let (mgr_uid, mgr_eid) = mkuser(db, "rm-mgr", "EMP-R1", None).await;
        let (uid, eid) = mkuser(db, "rm-staff", "EMP-R2", Some(mgr_eid)).await;
        let cat = one(
            db,
            "SELECT id FROM reimbursement_categories WHERE code = 'TRANSPORT'",
            "t.cat",
        )
        .await;
        let e = reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 5_000_000.0,
                description: Some("X".to_string()),
            },
            None,
        )
        .await
        .expect_err("batas");
        assert!(e.contains("Melebihi batas kategori"));
        assert!(reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: 999_999,
                amount: 100_000.0,
                description: None,
            },
            None,
        )
        .await
        .expect_err("kategori hilang")
        .contains("tidak ditemukan"));
        assert!(reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 0.5,
                description: None,
            },
            None,
        )
            .await
            .is_err());
        let bad = FileUpload {
            name: "x.exe".to_string(),
            mime: "application/x-ms".to_string(),
            bytes: vec![1],
        };
        assert!(reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 100_000.0,
                description: None,
            },
            Some(&bad),
        )
        .await
        .expect_err("jenis struk")
        .contains("Struk"));
        let rid = reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 500_000.0,
                description: Some("Bensin".to_string()),
            },
            None,
        )
        .await
        .expect("buat");
        assert_eq!(
            status_of(
                db,
                &format!("SELECT status FROM reimbursements WHERE id = {rid}"),
                "t.rst"
            )
            .await,
            "pending"
        );
        assert!(reimburse_decide_sea(db, admin_uid, Some(admin_emp), false, rid as i64, "approve")
            .await
            .expect_err("bukan atasan")
            .contains("berwenang"));
        assert!(reimburse_decide_sea(db, admin_uid, Some(admin_emp), true, rid as i64, "setuju")
            .await
            .is_err());
        assert_eq!(
            reimburse_decide_sea(db, mgr_uid, Some(mgr_eid), false, rid as i64, "approve")
                .await
                .expect("t1"),
            "manager_approved"
        );
        assert_eq!(
            reimburse_decide_sea(db, admin_uid, Some(admin_emp), true, rid as i64, "approve")
                .await
                .expect("t2"),
            "finance_verified"
        );
        assert_eq!(
            reimburse_decide_sea(db, admin_uid, Some(admin_emp), true, rid as i64, "approve")
                .await
                .expect("t3"),
            "paid"
        );
        let row = q_one(
            db,
            format!("SELECT COALESCE(paid_at, ''), current_step FROM reimbursements WHERE id = {rid}"),
            vec![],
            2,
            "t.paid",
        )
        .await
        .expect("bayar")
        .expect("baris");
        assert!(!value_to_string(&row[0]).is_empty());
        assert_eq!(value_i64(&row[1]), Some(4));
        assert!(reimburse_decide_sea(db, admin_uid, Some(admin_emp), true, rid as i64, "approve")
            .await
            .expect_err("sudah final")
            .contains("sudah diproses"));
        let rid2 = reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 100_000.0,
                description: None,
            },
            None,
        )
        .await
        .expect("buat2");
        assert_eq!(
            reimburse_decide_sea(db, mgr_uid, Some(mgr_eid), false, rid2 as i64, "reject")
                .await
                .expect("reject"),
            "rejected"
        );
        assert!(reimburse_decide_sea(db, mgr_uid, Some(mgr_eid), false, rid2 as i64, "approve")
            .await
            .expect_err("telah ditolak")
            .contains("sudah diproses"));
        let mine = my_reimburse_sea(db, eid).await.expect("mine");
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].id, rid);
        assert_eq!(mine[0].status, "paid");
        assert_eq!(mine[0].amount, 500_000.0);
        assert_eq!(mine[0].category_name, "Transportasi");
        assert_eq!(mine[0].employee_number, "EMP-R2");
        assert!(mine[0].paid_at.is_some());
        assert_eq!(mine[1].id, rid2);
        assert_eq!(mine[1].status, "rejected");
        assert!(all_reimburse_sea(db)
            .await
            .expect("all")
            .iter()
            .any(|r| r.id == rid));
        let notif = one(
            db,
            &format!("SELECT COUNT(*) FROM notifications WHERE user_id = {uid} AND type = 'reimbursement'"),
            "t.notif",
        )
        .await;
        assert!(notif >= 4);
    }

    #[tokio::test]
    async fn kategori_sea_crud_dan_guard_pemakaian() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin_uid = one(db, "SELECT id FROM users WHERE username = 'admin'", "t.admin").await;
        assert!(category_save_sea(db, admin_uid, None, " ", "Nama", None)
            .await
            .is_err());
        assert!(category_save_sea(db, admin_uid, None, "NOL", "Nol", Some(0.0))
            .await
            .expect_err("batas nol")
            .contains("Batas maksimal"));
        let cid = category_save_sea(db, admin_uid, None, "OBS", "Observasi", Some(250_000.0))
            .await
            .expect("create");
        let got = category_list_sea(db)
            .await
            .expect("list")
            .into_iter()
            .find(|c| c.id == cid)
            .expect("ada");
        assert_eq!(got.code, "OBS");
        assert_eq!(got.max_amount, Some(250_000.0));
        let e = category_save_sea(db, admin_uid, None, "OBS", "Kembar", None)
            .await
            .expect_err("duplikat");
        assert!(e.contains("sudah dipakai"));
        category_save_sea(db, admin_uid, Some(cid as i64), "OBS", "Revisi", None)
            .await
            .expect("update");
        let got = category_list_sea(db)
            .await
            .expect("list2")
            .into_iter()
            .find(|c| c.id == cid)
            .expect("ada");
        assert_eq!(got.name, "Revisi");
        assert!(got.max_amount.is_none());
        assert!(category_save_sea(db, admin_uid, Some(999_999), "X", "Y", None)
            .await
            .expect_err("id hilang")
            .contains("tidak ditemukan"));
        let (mgr_uid, mgr_eid) = mkuser(db, "ct-mgr", "EMP-C1", None).await;
        let (uid, eid) = mkuser(db, "ct-staff", "EMP-C2", Some(mgr_eid)).await;
        let rid = reimburse_create_sea(
            db,
            files.path(),
            uid,
            eid,
            &ReimburseInput {
                category_id: cid as i32,
                amount: 100.0,
                description: None,
            },
            None,
        )
        .await
        .expect("pakai");
        assert_eq!(
            reimburse_decide_sea(db, mgr_uid, Some(mgr_eid), false, rid as i64, "reject")
                .await
                .expect("rej"),
            "rejected"
        );
        assert!(category_delete_sea(db, admin_uid, cid as i64)
            .await
            .expect_err("terpakai")
            .contains("sudah dipakai"));
        let cid2 = category_save_sea(db, admin_uid, None, "LAIN", "Lainnya", None)
            .await
            .expect("create2");
        category_delete_sea(db, admin_uid, cid2 as i64)
            .await
            .expect("hapus");
        assert!(!category_list_sea(db)
            .await
            .expect("list3")
            .iter()
            .any(|c| c.id == cid2));
        assert!(category_delete_sea(db, admin_uid, 999_999)
            .await
            .expect_err("tak ada")
            .contains("tidak ditemukan"));
    }
}

