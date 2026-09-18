//! Kinerja: periode, KPI, review 4 peran, skor akhir.

use chrono::NaiveDate;

use super::audit;
use crate::to_dto_int;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PerfPeriod {
    pub id: i32,
    pub name: String,
    pub period_type: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct PerfPeriodInput {
    pub name: String,
    pub period_type: String,
    pub start_date: String,
    pub end_date: String,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Kpi {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub department_id: Option<i32>,
    pub department_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct KpiInput {
    pub name: String,
    pub description: Option<String>,
    pub department_id: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReviewRow {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub period_name: String,
    pub status: String,
    pub final_score: Option<f64>,
    pub final_rating: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReviewKpi {
    pub id: i32,
    pub kpi_name: String,
    pub target: f64,
    pub weight: f64,
    pub actual: Option<f64>,
    pub score: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReviewScore {
    pub reviewer_role: String,
    pub reviewer_id: Option<i32>,
    pub comments: Option<String>,
    pub rating: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReviewDetail {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub period_name: String,
    pub status: String,
    pub self_score: Option<f64>,
    pub supervisor_score: Option<f64>,
    pub manager_score: Option<f64>,
    pub hr_score: Option<f64>,
    pub final_score: Option<f64>,
    pub final_rating: Option<i32>,
    pub kpis: Vec<ReviewKpi>,
    pub scores: Vec<ReviewScore>,
}

const PERIOD_TYPES: &[&str] = &["monthly", "quarterly", "semester", "annual"];
const REVIEW_ROLES: &[&str] = &["self", "supervisor", "manager", "hr"];


use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};

fn popt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

fn popt_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse().ok(),
    }
}

fn popt_i(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match value_i64(v) {
        Some(x) => Ok(Some(to_dto_int(x, f)?)),
        None => Ok(None),
    }
}

pub async fn period_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<PerfPeriod>, String> {
    let rows = q_all(
        db,
        "SELECT id, name, type, start_date, end_date, status FROM performance_periods ORDER BY start_date DESC".to_string(),
        vec![],
        6,
        "perf.periods",
    )
    .await
    .map_err(|e| format!("gagal membaca periode: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(PerfPeriod {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "period.id")?,
            name: value_to_string(&r[1]),
            period_type: value_to_string(&r[2]),
            start_date: value_to_string(&r[3]),
            end_date: value_to_string(&r[4]),
            status: value_to_string(&r[5]),
        });
    }
    Ok(out)
}

pub async fn period_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &PerfPeriodInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama periode wajib diisi.".to_string());
    }
    if input.name.len() > 100 {
        return Err("Nama maksimal 100 karakter.".to_string());
    }
    if !PERIOD_TYPES.contains(&input.period_type.as_str()) {
        return Err("Tipe periode tidak valid.".to_string());
    }
    NaiveDate::parse_from_str(input.start_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.end_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if input.end_date.trim() < input.start_date.trim() {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    if input.status != "open" && input.status != "closed" {
        return Err("Status tidak valid.".to_string());
    }
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE performance_periods SET name = ?1, type = ?2, start_date = ?3, end_date = ?4, status = ?5 WHERE id = ?6".to_string(),
            vec![
                Value::Text(input.name.trim().to_string()),
                Value::Text(input.period_type.clone()),
                Value::Text(input.start_date.trim().to_string()),
                Value::Text(input.end_date.trim().to_string()),
                Value::Text(input.status.clone()),
                Value::Int(rid),
            ],
            "perf.periodupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan periode: {e}"))?;
        if n == 0 {
            return Err("Periode tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "performance.period",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "period.id")
    } else {
        let rid = exec_insert(
            db,
            "INSERT INTO performance_periods (name, type, start_date, end_date, status) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
            vec![
                Value::Text(input.name.trim().to_string()),
                Value::Text(input.period_type.clone()),
                Value::Text(input.start_date.trim().to_string()),
                Value::Text(input.end_date.trim().to_string()),
                Value::Text(input.status.clone()),
            ],
            "perf.periodadd",
        )
        .await
        .map_err(|e| format!("gagal menambah periode: {e}"))?;

        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "performance.period",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "period.id")
    }
}

pub async fn period_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM performance_reviews WHERE performance_period_id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "perf.periodrel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa review: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Periode sudah memiliki review.".to_string());
    }
    let d = exec(
        db,
        "DELETE FROM performance_periods WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        "perf.perioddel",
    )
    .await
    .map_err(|e| format!("gagal menghapus periode: {e}"))?;
    if d == 0 {
        return Err("Periode tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "performance.period",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn kpi_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Kpi>, String> {
    let rows = q_all(
        db,
        "SELECT k.id, k.name, k.description, k.department_id, d.name FROM kpis k LEFT JOIN departments d ON d.id = k.department_id WHERE k.deleted_at IS NULL ORDER BY k.name ASC".to_string(),
        vec![],
        5,
        "perf.kpis",
    )
    .await
    .map_err(|e| format!("gagal membaca KPI: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Kpi {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "kpi.id")?,
            name: value_to_string(&r[1]),
            description: popt_text(&r[2]),
            department_id: popt_i(&r[3], "kpi.dept")?,
            department_name: popt_text(&r[4]),
        });
    }
    Ok(out)
}

pub async fn kpi_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &KpiInput,
) -> Result<i32, String> {
    if input.name.trim().is_empty() {
        return Err("Nama KPI wajib diisi.".to_string());
    }
    if input.name.len() > 150 {
        return Err("Nama maksimal 150 karakter.".to_string());
    }
    if let Some(dept) = input.department_id {
        let found = q_one(
            db,
            "SELECT id FROM departments WHERE id = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::Int(dept as i64)],
            1,
            "perf.kpidept",
        )
        .await
        .map_err(|e| format!("gagal memeriksa departemen: {e}"))?;
        if found.is_none() {
            return Err("Departemen tidak ditemukan.".to_string());
        }
    }
    let desc = match input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    let dept = match input.department_id {
        Some(v) => Value::Int(v as i64),
        None => Value::Null,
    };
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE kpis SET name = ?1, description = ?2, department_id = ?3 WHERE id = ?4 AND deleted_at IS NULL".to_string(),
            vec![
                Value::Text(input.name.trim().to_string()),
                desc,
                dept,
                Value::Int(rid),
            ],
            "perf.kpiupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan KPI: {e}"))?;
        if n == 0 {
            return Err("KPI tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "performance.kpi",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "kpi.id")
    } else {
        let rid = exec_insert(
            db,
            "INSERT INTO kpis (name, description, department_id) VALUES (?1, ?2, ?3)".to_string(),
            vec![Value::Text(input.name.trim().to_string()), desc, dept],
            "perf.kpiadd",
        )
        .await
        .map_err(|e| format!("gagal menambah KPI: {e}"))?;

        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "performance.kpi",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "kpi.id")
    }
}

pub async fn kpi_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM employee_kpis WHERE kpi_id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "perf.kpirel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("KPI sudah dipakai penilaian.".to_string());
    }
    let d = exec(
        db,
        "UPDATE kpis SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "perf.kpidel",
    )
    .await
    .map_err(|e| format!("gagal menghapus KPI: {e}"))?;
    if d == 0 {
        return Err("KPI tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "performance.kpi",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

fn map_review_row(r: &[Value]) -> Result<ReviewRow, String> {
    Ok(ReviewRow {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "review.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "review.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        period_name: value_to_string(&r[4]),
        status: value_to_string(&r[5]),
        final_score: popt_f64(&r[6]),
        final_rating: popt_i(&r[7], "review.rating")?,
    })
}

pub async fn reviews_for_period_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
) -> Result<Vec<ReviewRow>, String> {
    let rows = q_all(
        db,
        "SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pp.name, pr.status, pr.final_score, pr.final_rating FROM performance_reviews pr INNER JOIN employees e ON e.id = pr.employee_id INNER JOIN performance_periods pp ON pp.id = pr.performance_period_id WHERE pr.performance_period_id = ?1 ORDER BY e.first_name".to_string(),
        vec![Value::Int(period_id)],
        8,
        "perf.reviews",
    )
    .await
    .map_err(|e| format!("gagal membaca review: {e}"))?;
    rows.iter().map(|r| map_review_row(r)).collect()
}

pub async fn my_reviews_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<ReviewRow>, String> {
    let rows = q_all(
        db,
        "SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pp.name, pr.status, pr.final_score, pr.final_rating FROM performance_reviews pr INNER JOIN employees e ON e.id = pr.employee_id INNER JOIN performance_periods pp ON pp.id = pr.performance_period_id WHERE pr.employee_id = ?1 ORDER BY pp.start_date DESC".to_string(),
        vec![Value::Int(employee_id)],
        8,
        "perf.myreviews",
    )
    .await
    .map_err(|e| format!("gagal membaca review: {e}"))?;
    rows.iter().map(|r| map_review_row(r)).collect()
}

pub async fn ensure_review_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
    employee_id: i64,
) -> Result<i32, String> {
    let row = q_one(
        db,
        "SELECT id FROM performance_reviews WHERE performance_period_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        1,
        "perf.reviewcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa review: {e}"))?;
    if let Some(r) = row {
        return to_dto_int(value_i64(&r[0]).unwrap_or(0), "review.id");
    }
    let rid = exec_insert(
        db,
        "INSERT INTO performance_reviews (performance_period_id, employee_id, status) VALUES (?1, ?2, 'draft')".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        "perf.reviewadd",
    )
    .await
    .map_err(|e| format!("gagal membuat review: {e}"))?;
    to_dto_int(rid, "review.id")
}

pub async fn review_detail_sea(
    db: &sea_orm::DatabaseConnection,
    review_id: i64,
) -> Result<Option<ReviewDetail>, String> {
    let row = q_one(
        db,
        "SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pp.name, pr.status, pr.self_score, pr.supervisor_score, pr.manager_score, pr.hr_score, pr.final_score, pr.final_rating FROM performance_reviews pr INNER JOIN employees e ON e.id = pr.employee_id INNER JOIN performance_periods pp ON pp.id = pr.performance_period_id WHERE pr.id = ?1".to_string(),
        vec![Value::Int(review_id)],
        12,
        "perf.reviewdet",
    )
    .await
    .map_err(|e| format!("gagal memuat review: {e}"))?;
    let Some(r) = row else {
        return Ok(None);
    };
    let (id, emp) = (
        value_i64(&r[0]).unwrap_or(0),
        value_i64(&r[1]).unwrap_or(0),
    );
    let meta = q_one(
        db,
        "SELECT performance_period_id, employee_id FROM performance_reviews WHERE id = ?1".to_string(),
        vec![Value::Int(review_id)],
        2,
        "perf.reviewmeta",
    )
    .await
    .map_err(|e| format!("gagal memuat meta: {e}"))?;
    let Some(meta) = meta else {
        return Ok(None);
    };
    let (mperiod, memp) = (
        value_i64(&meta[0]).unwrap_or(0),
        value_i64(&meta[1]).unwrap_or(0),
    );
    let mut kpis = Vec::new();
    let krows = q_all(
        db,
        "SELECT ek.id, k.name, ek.target, ek.weight, ek.actual, ek.score FROM employee_kpis ek INNER JOIN kpis k ON k.id = ek.kpi_id WHERE ek.performance_period_id = ?1 AND ek.employee_id = ?2".to_string(),
        vec![Value::Int(mperiod), Value::Int(memp)],
        6,
        "perf.reviewkpis",
    )
    .await
    .map_err(|e| format!("gagal membaca KPI: {e}"))?;
    for k in &krows {
        kpis.push(ReviewKpi {
            id: to_dto_int(value_i64(&k[0]).unwrap_or(0), "rkpi.id")?,
            kpi_name: value_to_string(&k[1]),
            target: popt_f64(&k[2]).unwrap_or(0.0),
            weight: popt_f64(&k[3]).unwrap_or(0.0),
            actual: popt_f64(&k[4]),
            score: popt_f64(&k[5]),
        });
    }
    let mut scores = Vec::new();
    let srows = q_all(
        db,
        "SELECT reviewer_role, reviewer_id, comments, rating FROM performance_details WHERE performance_review_id = ?1".to_string(),
        vec![Value::Int(review_id)],
        4,
        "perf.reviewscores",
    )
    .await
    .map_err(|e| format!("gagal membaca skor: {e}"))?;
    for s in &srows {
        scores.push(ReviewScore {
            reviewer_role: value_to_string(&s[0]),
            reviewer_id: popt_i(&s[1], "score.reviewer")?,
            comments: popt_text(&s[2]),
            rating: popt_i(&s[3], "score.rating")?,
        });
    }
    Ok(Some(ReviewDetail {
        id: to_dto_int(id, "review.id")?,
        employee_id: to_dto_int(emp, "review.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        period_name: value_to_string(&r[4]),
        status: value_to_string(&r[5]),
        self_score: popt_f64(&r[6]),
        supervisor_score: popt_f64(&r[7]),
        manager_score: popt_f64(&r[8]),
        hr_score: popt_f64(&r[9]),
        final_score: popt_f64(&r[10]),
        final_rating: popt_i(&r[11], "review.frating")?,
        kpis,
        scores,
    }))
}

pub async fn assign_kpi_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
    employee_id: i64,
    kpi_id: i64,
    target: f64,
    weight: f64,
) -> Result<i32, String> {
    let kpi = q_one(
        db,
        "SELECT id FROM kpis WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(kpi_id)],
        1,
        "perf.kpicheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa KPI: {e}"))?;
    if kpi.is_none() {
        return Err("KPI tidak ditemukan.".to_string());
    }
    if weight < 0.0 || weight > 100.0 {
        return Err("Bobot 0-100.".to_string());
    }
    let rid = exec_insert(
        db,
        "INSERT INTO employee_kpis (performance_period_id, employee_id, kpi_id, target, weight) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(period_id),
            Value::Int(employee_id),
            Value::Int(kpi_id),
            Value::Float(target),
            Value::Float(weight),
        ],
        "perf.assign",
    )
    .await
    .map_err(|e| format!("gagal menugaskan KPI: {e}"))?;

    ensure_review_sea(db, period_id, employee_id).await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "performance.kpi_assignment",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "assign.id")
}

/// Catat realisasi; skor = min(actual/target, 1.5) x 100.
pub async fn submit_actual_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_kpi_id: i64,
    actual: f64,
) -> Result<f64, String> {
    let row = q_one(
        db,
        "SELECT target, performance_period_id, employee_id FROM employee_kpis WHERE id = ?1".to_string(),
        vec![Value::Int(employee_kpi_id)],
        3,
        "perf.actual",
    )
    .await
    .map_err(|e| format!("gagal memuat KPI: {e}"))?;
    let Some(row) = row else {
        return Err("KPI tidak ditemukan.".to_string());
    };
    let (target, period_id, emp_id) = (
        popt_f64(&row[0]).unwrap_or(0.0),
        value_i64(&row[1]).unwrap_or(0),
        value_i64(&row[2]).unwrap_or(0),
    );
    let ratio = if target > 0.0 {
        (actual / target).min(1.5)
    } else {
        0.0
    };
    let score = (ratio * 100.0 * 100.0).round() / 100.0;
    exec(
        db,
        "UPDATE employee_kpis SET actual = ?1, score = ?2 WHERE id = ?3".to_string(),
        vec![
            Value::Float(actual),
            Value::Float(score),
            Value::Int(employee_kpi_id),
        ],
        "perf.actualupd",
    )
    .await
    .map_err(|e| format!("gagal menyimpan realisasi: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "performance.kpi_actual",
        Some(&employee_kpi_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    recompute_sea(db, period_id, emp_id).await?;
    Ok(score)
}

fn next_status_sea(role: &str) -> &str {
    match role {
        "self" => "supervisor_review",
        "supervisor" => "manager_review",
        "manager" => "hr_review",
        "hr" => "completed",
        _ => "draft",
    }
}

/// Nilai 0-100 per peran; rating = round(skor/20).
pub async fn submit_review_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    review_id: i64,
    role: &str,
    score: f64,
    comments: Option<&str>,
) -> Result<(), String> {
    if !REVIEW_ROLES.contains(&role) {
        return Err("Peran reviewer tidak valid.".to_string());
    }
    if !(0.0..=100.0).contains(&score) {
        return Err("Skor 0-100.".to_string());
    }
    let meta = q_one(
        db,
        "SELECT performance_period_id, employee_id FROM performance_reviews WHERE id = ?1".to_string(),
        vec![Value::Int(review_id)],
        2,
        "perf.submitmeta",
    )
    .await
    .map_err(|e| format!("gagal memuat review: {e}"))?;
    let Some(meta) = meta else {
        return Err("Review tidak ditemukan.".to_string());
    };
    let (period_id, emp_id) = (
        value_i64(&meta[0]).unwrap_or(0),
        value_i64(&meta[1]).unwrap_or(0),
    );
    let rating = (score / 20.0).round() as i64;
    let existing = q_one(
        db,
        "SELECT id FROM performance_details WHERE performance_review_id = ?1 AND reviewer_role = ?2".to_string(),
        vec![Value::Int(review_id), Value::Text(role.to_string())],
        1,
        "perf.scorecheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa skor: {e}"))?;
    let comment = match comments.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    match existing {
        Some(e) => {
            let did = value_i64(&e[0]).unwrap_or(0);
            exec(
                db,
                "UPDATE performance_details SET reviewer_id = ?1, comments = ?2, rating = ?3 WHERE id = ?4".to_string(),
                vec![
                    Value::Int(actor_id),
                    comment,
                    Value::Int(rating),
                    Value::Int(did),
                ],
                "perf.scoreupd",
            )
            .await
            .map_err(|e| format!("gagal memperbarui skor: {e}"))?;
        }
        None => {
            exec(
                db,
                "INSERT INTO performance_details (performance_review_id, reviewer_role, reviewer_id, comments, rating) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
                vec![
                    Value::Int(review_id),
                    Value::Text(role.to_string()),
                    Value::Int(actor_id),
                    comment,
                    Value::Int(rating),
                ],
                "perf.scoreadd",
            )
            .await
            .map_err(|e| format!("gagal menyimpan skor: {e}"))?;
        }
    }
    let column = format!("{role}_score");
    exec(
        db,
        format!("UPDATE performance_reviews SET {column} = ?1, status = ?2 WHERE id = ?3"),
        vec![
            Value::Float(score),
            Value::Text(next_status_sea(role).to_string()),
            Value::Int(review_id),
        ],
        "perf.reviewupd",
    )
    .await
    .map_err(|e| format!("gagal memperbarui review: {e}"))?;
    recompute_sea(db, period_id, emp_id).await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "performance.review",
        Some(&review_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

/// Skor akhir = rata-rata (KPI tertimbang, rata-rata reviewer terisi).
async fn recompute_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
    employee_id: i64,
) -> Result<(), String> {
    let rows = q_all(
        db,
        "SELECT score, weight FROM employee_kpis WHERE performance_period_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        2,
        "perf.recompute",
    )
    .await
    .map_err(|e| format!("gagal membaca KPI: {e}"))?;
    let mut weight_sum = 0.0;
    let mut kpi_score = 0.0;
    let mut parsed: Vec<(Option<f64>, f64)> = Vec::new();
    for r in &rows {
        parsed.push((popt_f64(&r[0]), popt_f64(&r[1]).unwrap_or(0.0)));
    }
    for (_, w) in &parsed {
        weight_sum += *w;
    }
    if weight_sum > 0.0 {
        for (score, w) in &parsed {
            if let Some(s) = score {
                kpi_score += *s * (*w / weight_sum);
            }
        }
    }
    let rev = q_one(
        db,
        "SELECT self_score, supervisor_score, manager_score, hr_score FROM performance_reviews WHERE performance_period_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        4,
        "perf.revscores",
    )
    .await
    .map_err(|e| format!("gagal memuat skor reviewer: {e}"))?;
    let filled: Vec<f64> = rev
        .as_ref()
        .map(|r| {
            [popt_f64(&r[0]), popt_f64(&r[1]), popt_f64(&r[2]), popt_f64(&r[3])]
                .into_iter()
                .flatten()
                .collect()
        })
        .unwrap_or_default();
    let avg = if filled.is_empty() {
        0.0
    } else {
        filled.iter().sum::<f64>() / filled.len() as f64
    };
    let final_score = if weight_sum > 0.0 && !filled.is_empty() {
        ((kpi_score + avg) / 2.0 * 100.0).round() / 100.0
    } else {
        ((kpi_score + avg) * 100.0).round() / 100.0
    };
    let rating = if final_score >= 90.0 {
        5
    } else if final_score >= 75.0 {
        4
    } else if final_score >= 60.0 {
        3
    } else if final_score >= 40.0 {
        2
    } else {
        1
    };
    exec(
        db,
        "UPDATE performance_reviews SET final_score = ?1, final_rating = ?2 WHERE performance_period_id = ?3 AND employee_id = ?4".to_string(),
        vec![
            Value::Float(final_score),
            Value::Int(rating),
            Value::Int(period_id),
            Value::Int(employee_id),
        ],
        "perf.final",
    )
    .await
    .map_err(|e| format!("gagal menyimpan skor akhir: {e}"))?;
    Ok(())
}


#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Goal {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub level: String,
    pub title: String,
    pub owner_employee_id: Option<i32>,
    pub department_id: Option<i32>,
    pub target: f64,
    pub actual: Option<f64>,
    pub weight: f64,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct GoalInput {
    pub parent_id: Option<i32>,
    pub level: String,
    pub title: String,
    pub owner_employee_id: Option<i32>,
    pub department_id: Option<i32>,
    pub target: f64,
    pub weight: f64,
}

fn goal_levels() -> &'static [&'static str] {
    &["company", "department", "individual"]
}

fn gopt_i(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match value_i64(v) {
        Some(x) => Ok(Some(to_dto_int(x, f)?)),
        None => Ok(None),
    }
}

fn goal_row(r: &[Value]) -> Result<Goal, String> {
    Ok(Goal {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "goal.id")?,
        parent_id: gopt_i(&r[1], "goal.parent")?,
        level: value_to_string(&r[2]),
        title: value_to_string(&r[3]),
        owner_employee_id: gopt_i(&r[4], "goal.owner")?,
        department_id: gopt_i(&r[5], "goal.dept")?,
        target: match &r[6] {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f64,
            _ => 0.0,
        },
        actual: match &r[7] {
            Value::Null => None,
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        },
        weight: match &r[8] {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f64,
            _ => 0.0,
        },
        status: value_to_string(&r[9]),
    })
}

pub async fn goal_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
    id: Option<i64>,
    input: &GoalInput,
) -> Result<i32, String> {
    if !goal_levels().contains(&input.level.as_str()) {
        return Err("Level goal tidak valid.".to_string());
    }
    if input.title.trim().is_empty() {
        return Err("Judul goal wajib diisi.".to_string());
    }
    if let Some(pid) = input.parent_id {
        let prow = q_one(
            db,
            "SELECT level FROM goals WHERE id = ?1 AND performance_period_id = ?2".to_string(),
            vec![Value::Int(pid as i64), Value::Int(period_id)],
            1,
            "goal.parent",
        )
        .await
        .map_err(|e| format!("gagal memeriksa induk: {e}"))?;
        let Some(prow) = prow else {
            return Err("Goal induk tidak ditemukan.".to_string());
        };
        let plevel = value_to_string(&prow[0]);
        let ok = match input.level.as_str() {
            "department" => plevel == "company",
            "individual" => plevel == "department",
            _ => input.parent_id.is_none(),
        };
        if !ok {
            return Err("Induk tidak sesuai jenjang.".to_string());
        }
    } else if input.level != "company" {
        return Err("Goal non-perusahaan wajib punya induk.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let owner = input.owner_employee_id.map(|v| Value::Int(v as i64)).unwrap_or(Value::Null);
    let dept = input.department_id.map(|v| Value::Int(v as i64)).unwrap_or(Value::Null);
    let parent = input.parent_id.map(|v| Value::Int(v as i64)).unwrap_or(Value::Null);
    let new_id = match id {
        Some(x) => {
            let n = exec(
                db,
                "UPDATE goals SET parent_id = ?1, level = ?2, title = ?3, owner_employee_id = ?4, department_id = ?5, target = ?6, weight = ?7, updated_at = ?8 WHERE id = ?9 AND performance_period_id = ?10".to_string(),
                vec![parent, Value::Text(input.level.clone()), Value::Text(input.title.trim().to_string()), owner, dept, Value::Float(input.target), Value::Float(input.weight), Value::Text(now), Value::Int(x), Value::Int(period_id)],
                "goal.upd",
            )
            .await?;
            if n == 0 {
                return Err("Goal tidak ditemukan.".to_string());
            }
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO goals (performance_period_id, parent_id, level, title, owner_employee_id, department_id, target, weight, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open', ?9, ?9)".to_string(),
                vec![Value::Int(period_id), parent, Value::Text(input.level.clone()), Value::Text(input.title.trim().to_string()), owner, dept, Value::Float(input.target), Value::Float(input.weight), Value::Text(now)],
                "goal.ins",
            )
            .await?
        }
    };
    audit::log_sea(db, Some(actor_id), if id.is_some() { "UPDATE" } else { "CREATE" }, "performance.goal", Some(&new_id.to_string()), None, None, None).await?;
    to_dto_int(new_id, "goal.id")
}

pub async fn goal_tree_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
) -> Result<Vec<Goal>, String> {
    let rows = q_all(
        db,
        "SELECT id, parent_id, level, title, owner_employee_id, department_id, target, actual, weight, status FROM goals WHERE performance_period_id = ?1 ORDER BY id".to_string(),
        vec![Value::Int(period_id)],
        10,
        "goal.tree",
    )
    .await
    .map_err(|e| format!("gagal membaca goal: {e}"))?;
    rows.iter().map(|r| goal_row(r)).collect()
}

pub async fn goal_progress_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    goal_id: i64,
    actual: f64,
) -> Result<(), String> {
    let n = exec(
        db,
        "UPDATE goals SET actual = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
        vec![Value::Float(actual), Value::Text(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()), Value::Int(goal_id)],
        "goal.prog",
    )
    .await?;
    if n == 0 {
        return Err("Goal tidak ditemukan.".to_string());
    }
    audit::log_sea(db, Some(actor_id), "UPDATE", "performance.goal.progress", Some(&goal_id.to_string()), None, None, None).await?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Feedback360 {
    pub id: i32,
    pub employee_id: i32,
    pub reviewer_employee_id: i32,
    pub relation: String,
    pub score: f64,
    pub comments: Option<String>,
}

fn fb_relations() -> &'static [&'static str] {
    &["manager", "peer", "subordinate", "self"]
}

pub async fn feedback360_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
    employee_id: i64,
    reviewer_employee_id: i64,
    relation: &str,
    score: f64,
    comments: Option<&str>,
) -> Result<i32, String> {
    if !fb_relations().contains(&relation) {
        return Err("Relasi reviewer tidak valid.".to_string());
    }
    if !(0.0..=100.0).contains(&score) {
        return Err("Skor 0-100.".to_string());
    }
    let comment = comments.map(str::trim).filter(|s| !s.is_empty()).map(|s| Value::Text(s.to_string())).unwrap_or(Value::Null);
    let ada = q_one(
        db,
        "SELECT id FROM feedback_360 WHERE performance_period_id = ?1 AND employee_id = ?2 AND reviewer_employee_id = ?3".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id), Value::Int(reviewer_employee_id)],
        1,
        "fb360.cek",
    )
    .await
    .map_err(|e| format!("gagal memeriksa feedback: {e}"))?;
    let rid = match ada {
        Some(r) => {
            let x = value_i64(&r[0]).unwrap_or(0);
            exec(
                db,
                "UPDATE feedback_360 SET relation = ?1, score = ?2, comments = ?3 WHERE id = ?4".to_string(),
                vec![Value::Text(relation.to_string()), Value::Float(score), comment, Value::Int(x)],
                "fb360.upd",
            )
            .await?;
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO feedback_360 (performance_period_id, employee_id, reviewer_employee_id, relation, score, comments) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
                vec![Value::Int(period_id), Value::Int(employee_id), Value::Int(reviewer_employee_id), Value::Text(relation.to_string()), Value::Float(score), comment],
                "fb360.ins",
            )
            .await?
        }
    };
    audit::log_sea(db, Some(actor_id), "CREATE", "performance.fb360", Some(&rid.to_string()), None, None, None).await?;
    to_dto_int(rid, "fb360.id")
}

pub async fn feedback360_list_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
    employee_id: i64,
) -> Result<Vec<Feedback360>, String> {
    let rows = q_all(
        db,
        "SELECT id, employee_id, reviewer_employee_id, relation, score, comments FROM feedback_360 WHERE performance_period_id = ?1 AND employee_id = ?2 ORDER BY id".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        6,
        "fb360.list",
    )
    .await
    .map_err(|e| format!("gagal membaca feedback: {e}"))?;
    rows.iter()
        .map(|r| {
            Ok(Feedback360 {
                id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fb360.id")?,
                employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "fb360.emp")?,
                reviewer_employee_id: to_dto_int(value_i64(&r[2]).unwrap_or(0), "fb360.rev")?,
                relation: value_to_string(&r[3]),
                score: match &r[4] {
                    Value::Float(v) => *v,
                    Value::Int(v) => *v as f64,
                    _ => 0.0,
                },
                comments: match &r[5] {
                    Value::Null => None,
                    v => Some(value_to_string(v)),
                },
            })
        })
        .collect()
}

pub async fn calibrate_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    period_id: i64,
    employee_id: i64,
    final_score: f64,
    notes: Option<&str>,
) -> Result<(), String> {
    if !(0.0..=100.0).contains(&final_score) {
        return Err("Skor 0-100.".to_string());
    }
    let awal = q_one(
        db,
        "SELECT final_score FROM performance_reviews WHERE performance_period_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        1,
        "cal.awal",
    )
    .await
    .map_err(|e| format!("gagal membaca skor awal: {e}"))?;
    let Some(awal) = awal else {
        return Err("Review karyawan tidak ditemukan.".to_string());
    };
    let initial = match &awal[0] {
        Value::Null => return Err("Skor awal belum ada.".to_string()),
        Value::Float(v) => *v,
        Value::Int(v) => *v as f64,
        _ => 0.0,
    };
    let note = notes.map(str::trim).filter(|s| !s.is_empty()).map(|s| Value::Text(s.to_string())).unwrap_or(Value::Null);
    let ada = q_one(
        db,
        "SELECT id FROM calibrations WHERE performance_period_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        1,
        "cal.cek",
    )
    .await
    .map_err(|e| format!("gagal memeriksa kalibrasi: {e}"))?;
    match ada {
        Some(r) => {
            exec(
                db,
                "UPDATE calibrations SET initial_score = ?1, final_score = ?2, decided_by = ?3, notes = ?4 WHERE id = ?5".to_string(),
                vec![Value::Float(initial), Value::Float(final_score), Value::Int(actor_id), note, Value::Int(value_i64(&r[0]).unwrap_or(0))],
                "cal.upd",
            )
            .await?;
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO calibrations (performance_period_id, employee_id, initial_score, final_score, decided_by, notes) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
                vec![Value::Int(period_id), Value::Int(employee_id), Value::Float(initial), Value::Float(final_score), Value::Int(actor_id), note],
                "cal.ins",
            )
            .await?;
        }
    }
    let rating = if final_score >= 90.0 { 5 } else if final_score >= 75.0 { 4 } else if final_score >= 60.0 { 3 } else if final_score >= 40.0 { 2 } else { 1 };
    exec(
        db,
        "UPDATE performance_reviews SET final_score = ?1, final_rating = ?2 WHERE performance_period_id = ?3 AND employee_id = ?4".to_string(),
        vec![Value::Float(final_score), Value::Int(rating), Value::Int(period_id), Value::Int(employee_id)],
        "cal.apply",
    )
    .await?;
    audit::log_sea(db, Some(actor_id), "UPDATE", "performance.calibration", Some(&employee_id.to_string()), None, None, None).await?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Succession {
    pub id: i32,
    pub position_id: i32,
    pub position_name: Option<String>,
    pub successor_employee_id: i32,
    pub successor_name: String,
    pub readiness: String,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct SuccessionInput {
    pub position_id: i32,
    pub successor_employee_id: i32,
    pub readiness: String,
    pub notes: Option<String>,
}

pub async fn succession_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    input: &SuccessionInput,
) -> Result<i32, String> {
    if !["ready", "developing", "not_ready"].contains(&input.readiness.as_str()) {
        return Err("Kesiapan tidak valid.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let note = input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| Value::Text(s.to_string())).unwrap_or(Value::Null);
    let ada = q_one(
        db,
        "SELECT id FROM successions WHERE position_id = ?1 AND successor_employee_id = ?2".to_string(),
        vec![Value::Int(input.position_id as i64), Value::Int(input.successor_employee_id as i64)],
        1,
        "suc.cek",
    )
    .await
    .map_err(|e| format!("gagal memeriksa succession: {e}"))?;
    let rid = match ada {
        Some(r) => {
            let x = value_i64(&r[0]).unwrap_or(0);
            exec(
                db,
                "UPDATE successions SET readiness = ?1, notes = ?2, updated_at = ?3 WHERE id = ?4".to_string(),
                vec![Value::Text(input.readiness.clone()), note, Value::Text(now), Value::Int(x)],
                "suc.upd",
            )
            .await?;
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO successions (position_id, successor_employee_id, readiness, notes, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)".to_string(),
                vec![Value::Int(input.position_id as i64), Value::Int(input.successor_employee_id as i64), Value::Text(input.readiness.clone()), note, Value::Text(now)],
                "suc.ins",
            )
            .await?
        }
    };
    audit::log_sea(db, Some(actor_id), "CREATE", "performance.succession", Some(&rid.to_string()), None, None, None).await?;
    to_dto_int(rid, "suc.id")
}

pub async fn succession_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Succession>, String> {
    let rows = q_all(
        db,
        "SELECT s.id, s.position_id, p.name, s.successor_employee_id, TRIM(e.first_name || ' ' || COALESCE(e.last_name, '')), s.readiness, s.notes FROM successions s LEFT JOIN positions p ON p.id = s.position_id INNER JOIN employees e ON e.id = s.successor_employee_id ORDER BY s.id".to_string(),
        vec![],
        7,
        "suc.list",
    )
    .await
    .map_err(|e| format!("gagal membaca succession: {e}"))?;
    rows.iter()
        .map(|r| {
            Ok(Succession {
                id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "suc.id")?,
                position_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "suc.pos")?,
                position_name: match &r[2] {
                    Value::Null => None,
                    v => Some(value_to_string(v)),
                },
                successor_employee_id: to_dto_int(value_i64(&r[3]).unwrap_or(0), "suc.peg")?,
                successor_name: value_to_string(&r[4]),
                readiness: value_to_string(&r[5]),
                notes: match &r[6] {
                    Value::Null => None,
                    v => Some(value_to_string(v)),
                },
            })
        })
        .collect()
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

    async fn mkemp(db: &sea_orm::DatabaseConnection, number: &str) -> i64 {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent')".to_string(),
            vec![Value::Text(number.to_string())],
            "test.mkemp",
        )
        .await
        .expect("emp");
        one(db, "SELECT last_insert_rowid()", "test.rowid").await
    }

    #[tokio::test]
    async fn kinerja_sea_alur_penuh_menuju_skor_akhir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let emp = mkemp(db, "EMP-P1").await;
        let pid = period_save_sea(
            db,
            actor,
            None,
            &PerfPeriodInput {
                name: "Q3 2026".to_string(),
                period_type: "quarterly".to_string(),
                start_date: "2026-07-01".to_string(),
                end_date: "2026-09-30".to_string(),
                status: "open".to_string(),
            },
        )
        .await
        .expect("periode");
        let kid = kpi_save_sea(
            db,
            actor,
            None,
            &KpiInput {
                name: "Penjualan".to_string(),
                description: None,
                department_id: None,
            },
        )
        .await
        .expect("kpi");
        let ek1 = assign_kpi_sea(db, actor, pid as i64, emp, kid as i64, 100.0, 60.0)
            .await
            .expect("assign1");
        let kid2 = kpi_save_sea(
            db,
            actor,
            None,
            &KpiInput {
                name: "Kehadiran".to_string(),
                description: None,
                department_id: None,
            },
        )
        .await
        .expect("kpi2");
        assign_kpi_sea(db, actor, pid as i64, emp, kid2 as i64, 100.0, 40.0)
            .await
            .expect("assign2");
        // realisasi 200 dari target 100 dipotong ke cap 150%
        assert_eq!(
            submit_actual_sea(db, actor, ek1 as i64, 200.0)
                .await
                .expect("aktual"),
            150.0
        );
        let rid = ensure_review_sea(db, pid as i64, emp).await.expect("review");
        for role in ["self", "supervisor", "manager", "hr"] {
            submit_review_sea(db, actor, rid as i64, role, 80.0, None)
                .await
                .expect(role);
        }
        let det = review_detail_sea(db, rid as i64)
            .await
            .expect("det")
            .expect("ada");
        // KPI = 150 x 0.6 = 90; rata-rata reviewer = 80 -> final 85 -> rating 4
        assert_eq!(det.final_score, Some(85.0));
        assert_eq!(det.final_rating, Some(4));
        assert_eq!(det.status, "completed");
        assert_eq!(det.kpis.len(), 2);
        assert_eq!(det.scores.len(), 4);
        assert!(det.scores.iter().all(|s| s.rating == Some(4)));
        let rows = reviews_for_period_sea(db, pid as i64)
            .await
            .expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].final_score, Some(85.0));
        assert_eq!(rows[0].final_rating, Some(4));
        let mine = my_reviews_sea(db, emp).await.expect("mine");
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].id, rows[0].id);
        assert!(period_list_sea(db)
            .await
            .expect("plist")
            .iter()
            .any(|p| p.id == pid));
        assert!(kpi_list_sea(db)
            .await
            .expect("klist")
            .iter()
            .any(|k| k.id == kid));
        assert!(submit_review_sea(db, actor, rid as i64, "bos", 80.0, None)
            .await
            .is_err());
        assert!(submit_review_sea(db, actor, rid as i64, "self", 150.0, None)
            .await
            .is_err());
        assert!(period_delete_sea(db, actor, pid as i64).await.is_err());
        assert!(kpi_delete_sea(db, actor, kid as i64).await.is_err());
    }

    #[tokio::test]
    async fn okr_360_kalibrasi_succession_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await;
        let emp = mkemp(db, "EMP-OKR").await;
        let peer = mkemp(db, "EMP-PEER").await;
        let pid = period_save_sea(
            db,
            actor,
            None,
            &PerfPeriodInput {
                name: "2026".to_string(),
                period_type: "annual".to_string(),
                start_date: "2026-01-01".to_string(),
                end_date: "2026-12-31".to_string(),
                status: "open".to_string(),
            },
        )
        .await
        .expect("periode") as i64;
        let g1 = goal_save_sea(
            db,
            actor,
            pid,
            None,
            &GoalInput {
                parent_id: None,
                level: "company".to_string(),
                title: "Tumbuh 20%".to_string(),
                owner_employee_id: None,
                department_id: None,
                target: 20.0,
                weight: 100.0,
            },
        )
        .await
        .expect("goal1");
        let g2 = goal_save_sea(
            db,
            actor,
            pid,
            None,
            &GoalInput {
                parent_id: Some(g1),
                level: "department".to_string(),
                title: "Rekrut 10 orang".to_string(),
                owner_employee_id: None,
                department_id: None,
                target: 10.0,
                weight: 50.0,
            },
        )
        .await
        .expect("goal2");
        assert!(goal_save_sea(
            db,
            actor,
            pid,
            None,
            &GoalInput {
                parent_id: Some(g2),
                level: "company".to_string(),
                title: "Salah jenjang".to_string(),
                owner_employee_id: None,
                department_id: None,
                target: 1.0,
                weight: 1.0,
            },
        )
        .await
        .is_err());
        assert!(goal_save_sea(
            db,
            actor,
            pid,
            None,
            &GoalInput {
                parent_id: None,
                level: "individual".to_string(),
                title: "Tanpa induk".to_string(),
                owner_employee_id: Some(emp as i32),
                department_id: None,
                target: 1.0,
                weight: 1.0,
            },
        )
        .await
        .is_err());
        let g3 = goal_save_sea(
            db,
            actor,
            pid,
            None,
            &GoalInput {
                parent_id: Some(g2),
                level: "individual".to_string(),
                title: "Onboarding tepat waktu".to_string(),
                owner_employee_id: Some(emp as i32),
                department_id: None,
                target: 100.0,
                weight: 100.0,
            },
        )
        .await
        .expect("goal3");
        goal_progress_sea(db, actor, g3 as i64, 80.0).await.expect("progress");
        let tree = goal_tree_sea(db, pid).await.expect("tree");
        assert_eq!(tree.len(), 3);
        let f1 = feedback360_save_sea(db, actor, pid, emp, peer, "peer", 85.0, None)
            .await
            .expect("fb");
        assert!(f1 > 0);
        assert!(feedback360_save_sea(db, actor, pid, emp, peer, "bos", 80.0, None)
            .await
            .is_err());
        let fbs = feedback360_list_sea(db, pid, emp).await.expect("list fb");
        assert_eq!(fbs.len(), 1);
        assert_eq!(fbs[0].score, 85.0);
        let rid = ensure_review_sea(db, pid, emp).await.expect("review");
        submit_review_sea(db, actor, rid as i64, "self", 70.0, None).await.expect("nilai");
        calibrate_sea(db, actor, pid, emp, 90.0, Some("Kalibrasi lintas dept")).await.expect("kalibrasi");
        let det = review_detail_sea(db, rid as i64).await.expect("det").expect("ada");
        assert_eq!(det.final_score, Some(90.0));
        assert_eq!(det.final_rating, Some(5));
        assert!(calibrate_sea(db, actor, pid, 999999, 80.0, None).await.is_err());
        let pos = one(db, "SELECT id FROM positions LIMIT 1", "t.pos").await;
        let sid = succession_save_sea(
            db,
            actor,
            &SuccessionInput {
                position_id: pos as i32,
                successor_employee_id: emp as i32,
                readiness: "developing".to_string(),
                notes: None,
            },
        )
        .await
        .expect("succession");
        assert!(sid > 0);
        assert!(succession_save_sea(
            db,
            actor,
            &SuccessionInput {
                position_id: pos as i32,
                successor_employee_id: emp as i32,
                readiness: "salah".to_string(),
                notes: None,
            },
        )
        .await
        .is_err());
        let sucs = succession_list_sea(db).await.expect("list suc");
        assert_eq!(sucs.len(), 1);
        assert_eq!(sucs[0].readiness, "developing");
    }
}
