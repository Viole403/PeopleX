//! Dinas: pengajuan dinamis + biaya + settle. Reimburse: kategori + tahap linier.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use super::approval;
use super::audit;
use super::employees::FileUpload;
use crate::to_dto_int;

const RECEIPT_MIMES: &[&str] = &["image/jpeg", "image/png", "application/pdf"];
const MAX_RECEIPT_BYTES: usize = 3 * 1024 * 1024;

// ---------------- Dinas ----------------

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

pub fn my_trips(conn: &Connection, employee_id: i64) -> Result<Vec<Trip>, String> {
    trip_rows(conn, "WHERE bt.employee_id = ?1", Some(employee_id))
}

pub fn all_trips(conn: &Connection) -> Result<Vec<Trip>, String> {
    trip_rows(conn, "", None)
}

fn trip_rows(
    conn: &Connection,
    extra_where: &str,
    param: Option<i64>,
) -> Result<Vec<Trip>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT bt.id, bt.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, bt.destination, bt.purpose, bt.start_date, bt.end_date, bt.transportation, bt.hotel, bt.budget, bt.status, bt.current_step, COALESCE((SELECT SUM(amount) FROM business_trip_expenses x WHERE x.business_trip_id = bt.id), 0) FROM business_trips bt INNER JOIN employees e ON e.id = bt.employee_id {extra_where} ORDER BY CASE bt.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 WHEN 'rejected' THEN 2 WHEN 'completed' THEN 3 ELSE 4 END, bt.created_at DESC"))
        .map_err(|e| format!("gagal menyiapkan dinas: {e}"))?;
    let map_row = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, Option<String>>(9)?,
            r.get::<_, f64>(10)?,
            r.get::<_, String>(11)?,
            r.get::<_, i64>(12)?,
            r.get::<_, f64>(13)?,
        ))
    };
    let rows = match param {
        Some(p) => stmt
            .query_map(params![p], map_row)
            .map_err(|e| format!("gagal membaca dinas: {e}"))?,
        None => stmt
            .query_map([], map_row)
            .map_err(|e| format!("gagal membaca dinas: {e}"))?,
    };
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            emp,
            name,
            number,
            dest,
            purpose,
            start,
            end,
            trans,
            hotel,
            budget,
            status,
            step,
            total,
        ) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Trip {
            id: to_dto_int(id, "trip.id")?,
            employee_id: to_dto_int(emp, "trip.emp")?,
            employee_name: name,
            employee_number: number,
            destination: dest,
            purpose,
            start_date: start,
            end_date: end,
            transportation: trans,
            hotel,
            budget,
            status,
            current_step: to_dto_int(step, "trip.step")?,
            expense_total: total,
        });
    }
    Ok(out)
}

pub fn trip_expenses(conn: &Connection, trip_id: i64) -> Result<Vec<TripExpense>, String> {
    let mut stmt = conn
        .prepare("SELECT id, category, description, amount, receipt_path FROM business_trip_expenses WHERE business_trip_id = ?1 ORDER BY id")
        .map_err(|e| format!("gagal menyiapkan biaya: {e}"))?;
    let rows = stmt
        .query_map(params![trip_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca biaya: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, cat, desc, amount, receipt) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(TripExpense {
            id: to_dto_int(id, "expense.id")?,
            category: cat,
            description: desc,
            amount,
            receipt_path: receipt,
        });
    }
    Ok(out)
}

/// Menunggu putusan user: resolve dinamis tiap panggil (tanpa snapshot).
pub fn pending_for(conn: &Connection, user_id: i64) -> Result<Vec<Trip>, String> {
    let mut out = Vec::new();
    for trip in all_trips(conn)?
        .into_iter()
        .filter(|t| t.status == "pending")
    {
        let chain = approval::build_chain(conn, "business_trip", trip.employee_id as i64)?;
        if let Some(step) = chain.get(trip.current_step as usize - 1) {
            if step.approver_id == user_id {
                out.push(trip);
            }
        }
    }
    Ok(out)
}

pub fn trip_create(
    conn: &Connection,
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
    let chain = approval::build_chain(conn, "business_trip", employee_id)?;
    let status = if chain.is_empty() {
        "approved"
    } else {
        "pending"
    };
    conn.execute(
        "INSERT INTO business_trips (employee_id, destination, purpose, start_date, end_date, transportation, hotel, budget, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)",
        params![
            employee_id,
            input.destination.trim(),
            input.purpose.trim(),
            input.start_date.trim(),
            input.end_date.trim(),
            input.transportation.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.hotel.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.budget,
            status,
        ],
    )
    .map_err(|e| format!("gagal mengajukan dinas: {e}"))?;
    let rid = conn.last_insert_rowid();
    if let Some(first) = chain.first() {
        let name: String = conn
            .query_row(
                "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1",
                params![employee_id],
                |r| r.get(0),
            )
            .unwrap_or_default();
        approval::notify(
            conn,
            first.approver_id,
            "business_trip",
            "Pengajuan Dinas Baru",
            &format!("{name} mengajukan perjalanan dinas."),
            "/travel",
        )?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "business_trip",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "trip.id")
}

pub fn trip_decide(conn: &Connection, user_id: i64, id: i64, decision: &str) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let row: Option<(i64, String, i64, String)> = conn
        .query_row(
            "SELECT employee_id, status, current_step, destination FROM business_trips WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat dinas: {e}"))?;
    let Some((emp_id, status, step, dest)) = row else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let chain = approval::build_chain(conn, "business_trip", emp_id)?;
    let current = chain.get(step as usize - 1);
    match current {
        Some(s) if s.approver_id == user_id => {}
        _ => return Err("Tidak berwenang memutus pengajuan ini.".to_string()),
    }
    if decision == "rejected" {
        conn.execute(
            "UPDATE business_trips SET status = 'rejected' WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("gagal menolak: {e}"))?;
    } else if chain.get(step as usize).is_some() {
        conn.execute(
            "UPDATE business_trips SET current_step = current_step + 1 WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("gagal maju tahap: {e}"))?;
        let next = &chain[step as usize];
        approval::notify(
            conn,
            next.approver_id,
            "business_trip",
            "Dinas Menunggu Persetujuan",
            "Ada pengajuan dinas menunggu persetujuan Anda.",
            "/travel",
        )?;
    } else {
        conn.execute(
            "UPDATE business_trips SET status = 'approved' WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("gagal menyetujui: {e}"))?;
    }
    if let Some(uid) = approval::user_of_employee(conn, emp_id)? {
        approval::notify(
            conn,
            uid,
            "business_trip",
            "Status Perjalanan Dinas",
            &format!("Pengajuan dinas ke {dest} telah diproses."),
            "/travel",
        )?;
    }
    audit::log(
        conn,
        Some(user_id),
        decision.to_uppercase().as_str(),
        "business_trip",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn trip_add_expense(
    conn: &Connection,
    files: &Path,
    actor_id: i64,
    trip_id: i64,
    input: &TripExpenseInput,
    receipt: Option<&FileUpload>,
) -> Result<i32, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM business_trips WHERE id = ?1",
            params![trip_id],
            |r| r.get(0),
        )
        .optional()
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
    conn.execute(
        "INSERT INTO business_trip_expenses (business_trip_id, category, description, amount, receipt_path) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            trip_id,
            input.category.trim(),
            input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.amount,
            receipt_path,
        ],
    )
    .map_err(|e| format!("gagal menambah biaya: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "business_trip.expense",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "expense.id")
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

pub fn trip_settle(conn: &Connection, actor_id: i64, trip_id: i64) -> Result<(), String> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM business_trips WHERE id = ?1",
            params![trip_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat dinas: {e}"))?;
    match status.as_deref() {
        Some("approved") | Some("completed") => {}
        Some(_) | None => {
            return Err("Dinas harus approved atau completed untuk settle.".to_string());
        }
    }
    conn.execute(
        "UPDATE business_trips SET status = 'settled' WHERE id = ?1",
        params![trip_id],
    )
    .map_err(|e| format!("gagal settle: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "business_trip",
        Some(&trip_id.to_string()),
        None,
        None,
        Some("Status menjadi settled"),
    )?;
    Ok(())
}

// ---------------- Reimburse ----------------

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

pub fn category_list(conn: &Connection) -> Result<Vec<ReimburseCategory>, String> {
    let mut stmt = conn
        .prepare("SELECT id, code, name, max_amount FROM reimbursement_categories ORDER BY name")
        .map_err(|e| format!("gagal menyiapkan kategori: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<f64>>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca kategori: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name, max) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(ReimburseCategory {
            id: to_dto_int(id, "reimbcat.id")?,
            code,
            name,
            max_amount: max,
        });
    }
    Ok(out)
}

pub fn category_save(
    conn: &Connection,
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
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE reimbursement_categories SET code = ?1, name = ?2, max_amount = ?3 WHERE id = ?4",
            params![code.trim(), name.trim(), max_amount, rid],
        ).map_err(|e| format!("gagal menyimpan kategori: {e}"))?;
        if n == 0 {
            return Err("Kategori tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "reimbursement.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "reimbcat.id")
    } else {
        conn.execute(
            "INSERT INTO reimbursement_categories (code, name, max_amount) VALUES (?1, ?2, ?3)",
            params![code.trim(), name.trim(), max_amount],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Kode kategori sudah dipakai.".to_string()
            } else {
                format!("gagal menambah kategori: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "reimbursement.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "reimbcat.id")
    }
}

pub fn category_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM reimbursements WHERE reimbursement_category_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if n > 0 {
        return Err("Kategori sudah dipakai pengajuan.".to_string());
    }
    let d = conn
        .execute(
            "DELETE FROM reimbursement_categories WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("gagal menghapus kategori: {e}"))?;
    if d == 0 {
        return Err("Kategori tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "reimbursement.category",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn my_reimburse(conn: &Connection, employee_id: i64) -> Result<Vec<Reimburse>, String> {
    reimburse_rows(conn, "WHERE r.employee_id = ?1", Some(employee_id))
}

pub fn all_reimburse(conn: &Connection) -> Result<Vec<Reimburse>, String> {
    reimburse_rows(conn, "", None)
}

fn reimburse_rows(
    conn: &Connection,
    extra_where: &str,
    param: Option<i64>,
) -> Result<Vec<Reimburse>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT r.id, r.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, r.reimbursement_category_id, rc.name, r.amount, r.description, r.receipt_path, r.status, r.current_step, r.paid_at, e.supervisor_id, e.manager_id FROM reimbursements r INNER JOIN reimbursement_categories rc ON rc.id = r.reimbursement_category_id INNER JOIN employees e ON e.id = r.employee_id {extra_where} ORDER BY CASE r.status WHEN 'pending' THEN 0 WHEN 'manager_approved' THEN 1 WHEN 'finance_verified' THEN 2 WHEN 'paid' THEN 3 ELSE 4 END, r.created_at DESC"))
        .map_err(|e| format!("gagal menyiapkan reimburse: {e}"))?;
    let map_row = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, f64>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, Option<String>>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, i64>(10)?,
            r.get::<_, Option<String>>(11)?,
            r.get::<_, Option<i64>>(12)?,
            r.get::<_, Option<i64>>(13)?,
        ))
    };
    let rows = match param {
        Some(p) => stmt
            .query_map(params![p], map_row)
            .map_err(|e| format!("gagal membaca reimburse: {e}"))?,
        None => stmt
            .query_map([], map_row)
            .map_err(|e| format!("gagal membaca reimburse: {e}"))?,
    };
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            emp,
            name,
            number,
            cat,
            cat_name,
            amount,
            desc,
            receipt,
            status,
            step,
            paid_at,
            sup,
            mgr,
        ) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "reimb.ref")).transpose();
        out.push(Reimburse {
            id: to_dto_int(id, "reimb.id")?,
            employee_id: to_dto_int(emp, "reimb.emp")?,
            employee_name: name,
            employee_number: number,
            category_id: to_dto_int(cat, "reimb.cat")?,
            category_name: cat_name,
            amount,
            description: desc,
            receipt_path: receipt,
            status,
            current_step: to_dto_int(step, "reimb.step")?,
            paid_at,
            supervisor_id: opt_i(sup)?,
            manager_id: opt_i(mgr)?,
        });
    }
    Ok(out)
}

pub fn reimburse_create(
    conn: &Connection,
    files: &Path,
    actor_id: i64,
    employee_id: i64,
    input: &ReimburseInput,
    receipt: Option<&FileUpload>,
) -> Result<i32, String> {
    let cat: Option<(Option<f64>,)> = conn
        .query_row(
            "SELECT max_amount FROM reimbursement_categories WHERE id = ?1",
            params![input.category_id],
            |r| Ok((r.get(0)?,)),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa kategori: {e}"))?;
    let Some((max,)) = cat else {
        return Err("Kategori tidak ditemukan.".to_string());
    };
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
    conn.execute(
        "INSERT INTO reimbursements (employee_id, reimbursement_category_id, amount, description, receipt_path, status, current_step) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', 1)",
        params![
            employee_id,
            input.category_id,
            input.amount,
            input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            receipt_path,
        ],
    )
    .map_err(|e| format!("gagal mengajukan reimburse: {e}"))?;
    let rid = conn.last_insert_rowid();
    let emp: Option<(Option<i64>, Option<i64>, String)> = conn
        .query_row(
            "SELECT supervisor_id, manager_id, first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1",
            params![employee_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    if let Some((sup, mgr, name)) = emp {
        let approver = mgr.or(sup);
        if let Some(aid) = approver {
            if let Some(uid) = approval::user_of_employee(conn, aid)? {
                approval::notify(
                    conn,
                    uid,
                    "reimbursement",
                    "Pengajuan Reimbursement Baru",
                    &format!("{name} mengajukan reimbursement."),
                    "/travel",
                )?;
            }
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "reimbursement",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "reimb.id")
}

/// Putuskan tahap berjalan. Pending = manajer/supervisor; sisanya izin.
pub fn reimburse_decide(
    conn: &Connection,
    user_id: i64,
    user_employee_id: Option<i64>,
    privileged: bool,
    id: i64,
    action: &str,
) -> Result<String, String> {
    if action != "approve" && action != "reject" {
        return Err("Aksi tidak valid.".to_string());
    }
    let row: Option<(String, Option<i64>, Option<i64>, i64)> = conn
        .query_row(
            "SELECT r.status, e.supervisor_id, e.manager_id, r.employee_id FROM reimbursements r INNER JOIN employees e ON e.id = r.employee_id WHERE r.id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some((status, sup, mgr, _emp)) = row else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
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
        conn.execute(
            "UPDATE reimbursements SET status = 'rejected' WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("gagal menolak: {e}"))?;
        audit::log(
            conn,
            Some(user_id),
            "REJECT",
            "reimbursement",
            Some(&id.to_string()),
            None,
            None,
            None,
        )?;
        notify_employee(conn, id)?;
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
        conn.execute(
            "UPDATE reimbursements SET status = 'paid', current_step = ?1, paid_at = COALESCE(paid_at, ?2) WHERE id = ?3",
            params![(idx + 2) as i64, now, id],
        )
        .map_err(|e| format!("gagal membayar: {e}"))?;
    } else {
        conn.execute(
            "UPDATE reimbursements SET status = ?1, current_step = ?2 WHERE id = ?3",
            params![next, (idx + 2) as i64, id],
        )
        .map_err(|e| format!("gagal maju tahap: {e}"))?;
    }
    audit::log(
        conn,
        Some(user_id),
        "APPROVE",
        "reimbursement",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Tahap menjadi {next}")),
    )?;
    notify_employee(conn, id)?;
    Ok(next.to_string())
}

fn notify_employee(conn: &Connection, id: i64) -> Result<(), String> {
    let emp: Option<i64> = conn
        .query_row(
            "SELECT employee_id FROM reimbursements WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    if let Some(eid) = emp {
        if let Some(uid) = approval::user_of_employee(conn, eid)? {
            approval::notify(
                conn,
                uid,
                "reimbursement",
                "Status Reimbursement",
                "Pengajuan reimbursement Anda diproses.",
                "/travel",
            )?;
        }
    }
    Ok(())
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

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

fn tropt_i(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match value_i64(v) {
        Some(x) => Ok(Some(to_dto_int(x, f)?)),
        None => Ok(None),
    }
}

async fn trow_id(db: &sea_orm::DatabaseConnection, label: &str) -> Result<i64, String> {
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
    exec(
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
    let rid = trow_id(db, "travel.tripadd").await?;
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
    exec(
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
    let rid = trow_id(db, "travel.expadd").await?;
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
        let res = exec(
            db,
            "INSERT INTO reimbursement_categories (code, name, max_amount) VALUES (?1, ?2, ?3)".to_string(),
            vec![
                Value::Text(code.trim().to_string()),
                Value::Text(name.trim().to_string()),
                max,
            ],
            "travel.catadd",
        )
        .await;
        let Err(e) = res else {
            let rid = trow_id(db, "travel.catadd").await?;
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
        };
        if e.contains("UNIQUE") {
            return Err("Kode kategori sudah dipakai.".to_string());
        }
        return Err(format!("gagal menambah kategori: {e}"));
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
    exec(
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
    let rid = trow_id(db, "travel.reimbadd").await?;
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

    fn admin(conn: &Connection) -> (i64, i64) {
        let uid: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let eid: i64 = conn
            .query_row(
                "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        (uid, eid)
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

    #[test]
    fn dinas_dua_tahap_lalu_settle() {
        let (_d, pool, files) = live();
        let conn = pool.get().expect("get");
        let (admin_uid, admin_eid) = admin(&conn);
        let (sup_uid, sup_eid) = mkuser(&conn, "dns-spv", "EMP-D1", None);
        let (uid, eid) = mkuser(&conn, "dns-staff", "EMP-D2", Some(sup_eid));
        let tid = trip_create(
            &conn,
            uid,
            eid,
            &TripInput {
                destination: "Bandung".to_string(),
                purpose: "Survey".to_string(),
                start_date: "2026-10-05".to_string(),
                end_date: "2026-10-07".to_string(),
                transportation: Some("Kereta".to_string()),
                hotel: None,
                budget: 2000000.0,
            },
        )
        .expect("buat");
        assert!(trip_decide(&conn, uid, tid as i64, "approved").is_err());
        trip_decide(&conn, sup_uid, tid as i64, "approved").expect("setuju");
        let status: String = conn
            .query_row(
                "SELECT status FROM business_trips WHERE id = ?1",
                params![tid as i64],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "approved");
        trip_add_expense(
            &conn,
            files.path(),
            uid,
            tid as i64,
            &TripExpenseInput {
                category: "Tiket".to_string(),
                description: None,
                amount: 500000.0,
            },
            None,
        )
        .expect("biaya");
        assert_eq!(trip_expenses(&conn, tid as i64).expect("exp").len(), 1);
        trip_settle(&conn, admin_uid, tid as i64).expect("settle");
        let _ = (admin_eid, sup_eid);
    }

    #[test]
    fn reimburse_tiga_tahap_dan_batas_kategori() {
        let (_d, pool, files) = live();
        let conn = pool.get().expect("get");
        let (admin_uid, admin_eid) = admin(&conn);
        let (mgr_uid, mgr_eid) = mkuser(&conn, "rm-mgr", "EMP-R1", None);
        let (uid, eid) = mkuser(&conn, "rm-staff", "EMP-R2", Some(mgr_eid));
        let cat: i64 = conn
            .query_row(
                "SELECT id FROM reimbursement_categories WHERE code = 'TRANSPORT'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let e = reimburse_create(
            &conn,
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
        .expect_err("batas");
        assert!(e.contains("batas"));
        let rid = reimburse_create(
            &conn,
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
        .expect("buat");
        assert_eq!(
            reimburse_decide(&conn, mgr_uid, Some(mgr_eid), false, rid as i64, "approve")
                .expect("t1"),
            "manager_approved"
        );
        assert_eq!(
            reimburse_decide(
                &conn,
                admin_uid,
                Some(admin_eid),
                true,
                rid as i64,
                "approve"
            )
            .expect("t2"),
            "finance_verified"
        );
        assert_eq!(
            reimburse_decide(
                &conn,
                admin_uid,
                Some(admin_eid),
                true,
                rid as i64,
                "approve"
            )
            .expect("t3"),
            "paid"
        );
        let paid_at: Option<String> = conn
            .query_row(
                "SELECT paid_at FROM reimbursements WHERE id = ?1",
                params![rid as i64],
                |r| r.get(0),
            )
            .unwrap();
        assert!(paid_at.is_some());
        let bad = FileUpload {
            name: "x.exe".to_string(),
            mime: "application/x-ms".to_string(),
            bytes: vec![1],
        };
        assert!(reimburse_create(
            &conn,
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
        .is_err());
        let _ = mgr_uid;
    }

    #[tokio::test]
    async fn dinas_reimburse_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let (admin_uid, _admin_eid): (i64, i64) = {
            let uid: i64 = conn
                .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                    r.get(0)
                })
                .unwrap();
            let eid: i64 = conn
                .query_row(
                    "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            (uid, eid)
        };
        let (sup_uid, sup_eid) = mkuser(&conn, "sead-spv", "EMP-SEAD1", None);
        let (uid, eid) = mkuser(&conn, "sead-staff", "EMP-SEAD2", Some(sup_eid));
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
                budget: 2000000.0,
            },
        )
        .await
        .expect("buat");
        let e_sea = trip_decide_sea(db, uid, tid as i64, "approved")
            .await
            .expect_err("otorisasi");
        let e_sync = trip_decide(&conn, uid, tid as i64, "approved")
            .expect_err("otorisasi sync");
        assert_eq!(e_sea, e_sync);
        trip_decide_sea(db, sup_uid, tid as i64, "approved")
            .await
            .expect("setuju");
        let m_sync = serde_json::to_string(&my_trips(&conn, eid).expect("ms")).unwrap();
        let m_sea = serde_json::to_string(&my_trips_sea(db, eid).await.expect("mse")).unwrap();
        assert_eq!(m_sync, m_sea);
        assert!(m_sea.contains("\"status\":\"approved\""));
        trip_add_expense_sea(
            db,
            files.path(),
            uid,
            tid as i64,
            &TripExpenseInput {
                category: "Tiket".to_string(),
                description: None,
                amount: 500000.0,
            },
            None,
        )
        .await
        .expect("biaya");
        let x_sync =
            serde_json::to_string(&trip_expenses(&conn, tid as i64).expect("xs")).unwrap();
        let x_sea =
            serde_json::to_string(&trip_expenses_sea(db, tid as i64).await.expect("xse")).unwrap();
        assert_eq!(x_sync, x_sea);
        trip_settle_sea(db, admin_uid, tid as i64).await.expect("settle");
        let (mgr_uid, mgr_eid) = mkuser(&conn, "sear-mgr", "EMP-SEAR1", None);
        let (uid2, eid2) = mkuser(&conn, "sear-staff", "EMP-SEAR2", Some(mgr_eid));
        let cat: i64 = conn
            .query_row(
                "SELECT id FROM reimbursement_categories WHERE code = 'TRANSPORT'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let eb_sea = reimburse_create_sea(
            db,
            files.path(),
            uid2,
            eid2,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 5_000_000.0,
                description: Some("X".to_string()),
            },
            None,
        )
        .await
        .expect_err("batas");
        let eb_sync = reimburse_create(
            &conn,
            files.path(),
            uid2,
            eid2,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 5_000_000.0,
                description: Some("X".to_string()),
            },
            None,
        )
        .expect_err("batas sync");
        assert_eq!(eb_sea, eb_sync);
        let rid = reimburse_create_sea(
            db,
            files.path(),
            uid2,
            eid2,
            &ReimburseInput {
                category_id: cat as i32,
                amount: 500_000.0,
                description: Some("Bensin".to_string()),
            },
            None,
        )
        .await
        .expect("buat");
        let g_sync = serde_json::to_string(&my_reimburse(&conn, eid2).expect("gs")).unwrap();
        let g_sea = serde_json::to_string(&my_reimburse_sea(db, eid2).await.expect("gse")).unwrap();
        assert_eq!(g_sync, g_sea);
        assert_eq!(
            reimburse_decide_sea(db, mgr_uid, Some(mgr_eid), false, rid as i64, "approve")
                .await
                .expect("t1"),
            "manager_approved"
        );
        assert_eq!(
            reimburse_decide_sea(db, admin_uid, None, true, rid as i64, "approve")
                .await
                .expect("t2"),
            "finance_verified"
        );
        assert_eq!(
            reimburse_decide_sea(db, admin_uid, None, true, rid as i64, "approve")
                .await
                .expect("t3"),
            "paid"
        );
        let a_sync = serde_json::to_string(&all_reimburse(&conn).expect("as")).unwrap();
        let a_sea = serde_json::to_string(&all_reimburse_sea(db).await.expect("ase")).unwrap();
        assert_eq!(a_sync, a_sea);
        assert!(a_sea.contains("\"status\":\"paid\""));
        let _ = mgr_uid;
    }
}
