//! Kinerja: periode, KPI, review 4 peran, skor akhir.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

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

// ---------------- Periode ----------------

pub fn period_list(conn: &Connection) -> Result<Vec<PerfPeriod>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, type, start_date, end_date, status FROM performance_periods ORDER BY start_date DESC")
        .map_err(|e| format!("gagal menyiapkan periode: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca periode: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, ptype, start, end, status) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(PerfPeriod {
            id: to_dto_int(id, "period.id")?,
            name,
            period_type: ptype,
            start_date: start,
            end_date: end,
            status,
        });
    }
    Ok(out)
}

pub fn period_save(
    conn: &Connection,
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
        let n = conn.execute(
            "UPDATE performance_periods SET name = ?1, type = ?2, start_date = ?3, end_date = ?4, status = ?5 WHERE id = ?6",
            params![input.name.trim(), input.period_type, input.start_date.trim(), input.end_date.trim(), input.status, rid],
        ).map_err(|e| format!("gagal menyimpan periode: {e}"))?;
        if n == 0 {
            return Err("Periode tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "performance.period",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "period.id")
    } else {
        conn.execute(
            "INSERT INTO performance_periods (name, type, start_date, end_date, status) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![input.name.trim(), input.period_type, input.start_date.trim(), input.end_date.trim(), input.status],
        ).map_err(|e| format!("gagal menambah periode: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "performance.period",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "period.id")
    }
}

pub fn period_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM performance_reviews WHERE performance_period_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa review: {e}"))?;
    if n > 0 {
        return Err("Periode sudah memiliki review.".to_string());
    }
    let d = conn
        .execute("DELETE FROM performance_periods WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus periode: {e}"))?;
    if d == 0 {
        return Err("Periode tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "performance.period",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- KPI ----------------

pub fn kpi_list(conn: &Connection) -> Result<Vec<Kpi>, String> {
    let mut stmt = conn
        .prepare("SELECT k.id, k.name, k.description, k.department_id, d.name FROM kpis k LEFT JOIN departments d ON d.id = k.department_id WHERE k.deleted_at IS NULL ORDER BY k.name ASC")
        .map_err(|e| format!("gagal menyiapkan KPI: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca KPI: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, desc, dept, dept_name) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Kpi {
            id: to_dto_int(id, "kpi.id")?,
            name,
            description: desc,
            department_id: dept.map(|v| to_dto_int(v, "kpi.dept")).transpose()?,
            department_name: dept_name,
        });
    }
    Ok(out)
}

pub fn kpi_save(
    conn: &Connection,
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
        let found: Option<i64> = conn
            .query_row(
                "SELECT id FROM departments WHERE id = ?1 AND deleted_at IS NULL",
                params![dept as i64],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memeriksa departemen: {e}"))?;
        if found.is_none() {
            return Err("Departemen tidak ditemukan.".to_string());
        }
    }
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE kpis SET name = ?1, description = ?2, department_id = ?3 WHERE id = ?4 AND deleted_at IS NULL",
            params![input.name.trim(), input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()), input.department_id.map(|v| v as i64), rid],
        ).map_err(|e| format!("gagal menyimpan KPI: {e}"))?;
        if n == 0 {
            return Err("KPI tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "performance.kpi",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "kpi.id")
    } else {
        conn.execute(
            "INSERT INTO kpis (name, description, department_id) VALUES (?1, ?2, ?3)",
            params![
                input.name.trim(),
                input
                    .description
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty()),
                input.department_id.map(|v| v as i64)
            ],
        )
        .map_err(|e| format!("gagal menambah KPI: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "performance.kpi",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "kpi.id")
    }
}

pub fn kpi_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM employee_kpis WHERE kpi_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if n > 0 {
        return Err("KPI sudah dipakai penilaian.".to_string());
    }
    let d = conn.execute(
        "UPDATE kpis SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus KPI: {e}"))?;
    if d == 0 {
        return Err("KPI tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "performance.kpi",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Review ----------------

pub fn reviews_for_period(conn: &Connection, period_id: i64) -> Result<Vec<ReviewRow>, String> {
    rows_for(conn, "pr.performance_period_id = ?1", period_id)
}

pub fn my_reviews(conn: &Connection, employee_id: i64) -> Result<Vec<ReviewRow>, String> {
    let mut stmt = conn
        .prepare("SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pp.name, pr.status, pr.final_score, pr.final_rating FROM performance_reviews pr INNER JOIN employees e ON e.id = pr.employee_id INNER JOIN performance_periods pp ON pp.id = pr.performance_period_id WHERE pr.employee_id = ?1 ORDER BY pp.start_date DESC")
        .map_err(|e| format!("gagal menyiapkan review saya: {e}"))?;
    collect_review_rows(&mut stmt, employee_id)
}

fn rows_for(conn: &Connection, cond: &str, param: i64) -> Result<Vec<ReviewRow>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pp.name, pr.status, pr.final_score, pr.final_rating FROM performance_reviews pr INNER JOIN employees e ON e.id = pr.employee_id INNER JOIN performance_periods pp ON pp.id = pr.performance_period_id WHERE {cond} ORDER BY e.first_name"))
        .map_err(|e| format!("gagal menyiapkan review: {e}"))?;
    collect_review_rows(&mut stmt, param)
}

fn collect_review_rows(
    stmt: &mut rusqlite::Statement,
    param: i64,
) -> Result<Vec<ReviewRow>, String> {
    let rows = stmt
        .query_map(params![param], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<f64>>(6)?,
                r.get::<_, Option<i64>>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca review: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, number, pname, status, score, rating) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(ReviewRow {
            id: to_dto_int(id, "review.id")?,
            employee_id: to_dto_int(emp, "review.emp")?,
            employee_name: name,
            employee_number: number,
            period_name: pname,
            status,
            final_score: score,
            final_rating: rating.map(|v| to_dto_int(v, "review.rating")).transpose()?,
        });
    }
    Ok(out)
}

pub fn ensure_review(conn: &Connection, period_id: i64, employee_id: i64) -> Result<i32, String> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM performance_reviews WHERE performance_period_id = ?1 AND employee_id = ?2",
            params![period_id, employee_id],
            |r| r.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa review: {e}"))?
    {
        return to_dto_int(id, "review.id");
    }
    conn.execute(
        "INSERT INTO performance_reviews (performance_period_id, employee_id, status) VALUES (?1, ?2, 'draft')",
        params![period_id, employee_id],
    )
    .map_err(|e| format!("gagal membuat review: {e}"))?;
    to_dto_int(conn.last_insert_rowid(), "review.id")
}

pub fn review_detail(conn: &Connection, review_id: i64) -> Result<Option<ReviewDetail>, String> {
    let row: Option<(
        i64, i64, String, String, String, String,
        Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>,
    )> = conn
        .query_row(
            "SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pp.name, pr.status, pr.self_score, pr.supervisor_score, pr.manager_score, pr.hr_score, pr.final_score, pr.final_rating FROM performance_reviews pr INNER JOIN employees e ON e.id = pr.employee_id INNER JOIN performance_periods pp ON pp.id = pr.performance_period_id WHERE pr.id = ?1",
            params![review_id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                    r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat review: {e}"))?;
    let Some((id, emp, name, number, pname, status, s0, s1, s2, s3, fin, rating)) = row else {
        return Ok(None);
    };
    let meta: (i64, i64) = conn
        .query_row(
            "SELECT performance_period_id, employee_id FROM performance_reviews WHERE id = ?1",
            params![review_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("gagal memuat meta: {e}"))?;
    let mut kpis = Vec::new();
    let mut kstmt = conn
        .prepare("SELECT ek.id, k.name, ek.target, ek.weight, ek.actual, ek.score FROM employee_kpis ek INNER JOIN kpis k ON k.id = ek.kpi_id WHERE ek.performance_period_id = ?1 AND ek.employee_id = ?2")
        .map_err(|e| format!("gagal menyiapkan KPI: {e}"))?;
    for krow in kstmt
        .query_map(params![meta.0, meta.1], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, f64>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, Option<f64>>(4)?,
                r.get::<_, Option<f64>>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca KPI: {e}"))?
    {
        let (kid, kname, target, weight, actual, score) =
            krow.map_err(|e| format!("gagal membaca baris KPI: {e}"))?;
        kpis.push(ReviewKpi {
            id: to_dto_int(kid, "rkpi.id")?,
            kpi_name: kname,
            target,
            weight,
            actual,
            score,
        });
    }
    let mut scores = Vec::new();
    let mut dstmt = conn
        .prepare("SELECT reviewer_role, reviewer_id, comments, rating FROM performance_details WHERE performance_review_id = ?1")
        .map_err(|e| format!("gagal menyiapkan skor: {e}"))?;
    for drow in dstmt
        .query_map(params![review_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca skor: {e}"))?
    {
        let (role, rid, comments, rating) =
            drow.map_err(|e| format!("gagal membaca baris skor: {e}"))?;
        scores.push(ReviewScore {
            reviewer_role: role,
            reviewer_id: rid.map(|v| to_dto_int(v, "score.reviewer")).transpose()?,
            comments,
            rating: rating.map(|v| to_dto_int(v, "score.rating")).transpose()?,
        });
    }
    Ok(Some(ReviewDetail {
        id: to_dto_int(id, "review.id")?,
        employee_id: to_dto_int(emp, "review.emp")?,
        employee_name: name,
        employee_number: number,
        period_name: pname,
        status,
        self_score: s0,
        supervisor_score: s1,
        manager_score: s2,
        hr_score: s3,
        final_score: fin,
        final_rating: rating
            .map(|v| to_dto_int(v, "review.frating"))
            .transpose()?,
        kpis,
        scores,
    }))
}

pub fn assign_kpi(
    conn: &Connection,
    actor_id: i64,
    period_id: i64,
    employee_id: i64,
    kpi_id: i64,
    target: f64,
    weight: f64,
) -> Result<i32, String> {
    let kpi: Option<i64> = conn
        .query_row(
            "SELECT id FROM kpis WHERE id = ?1 AND deleted_at IS NULL",
            params![kpi_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa KPI: {e}"))?;
    if kpi.is_none() {
        return Err("KPI tidak ditemukan.".to_string());
    }
    if weight < 0.0 || weight > 100.0 {
        return Err("Bobot 0-100.".to_string());
    }
    conn.execute(
        "INSERT INTO employee_kpis (performance_period_id, employee_id, kpi_id, target, weight) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![period_id, employee_id, kpi_id, target, weight],
    )
    .map_err(|e| format!("gagal menugaskan KPI: {e}"))?;
    let rid = conn.last_insert_rowid();
    ensure_review(conn, period_id, employee_id)?;
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "performance.kpi_assignment",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "assign.id")
}

/// Catat realisasi; skor = min(actual/target, 1.5) x 100.
pub fn submit_actual(
    conn: &Connection,
    actor_id: i64,
    employee_kpi_id: i64,
    actual: f64,
) -> Result<f64, String> {
    let row: Option<(f64, i64, i64)> = conn
        .query_row(
            "SELECT target, performance_period_id, employee_id FROM employee_kpis WHERE id = ?1",
            params![employee_kpi_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat KPI: {e}"))?;
    let Some((target, period_id, emp_id)) = row else {
        return Err("KPI tidak ditemukan.".to_string());
    };
    let ratio = if target > 0.0 {
        (actual / target).min(1.5)
    } else {
        0.0
    };
    let score = (ratio * 100.0 * 100.0).round() / 100.0;
    conn.execute(
        "UPDATE employee_kpis SET actual = ?1, score = ?2 WHERE id = ?3",
        params![actual, score, employee_kpi_id],
    )
    .map_err(|e| format!("gagal menyimpan realisasi: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "performance.kpi_actual",
        Some(&employee_kpi_id.to_string()),
        None,
        None,
        None,
    )?;
    recompute(conn, period_id, emp_id)?;
    Ok(score)
}

fn next_status(role: &str) -> &str {
    match role {
        "self" => "supervisor_review",
        "supervisor" => "manager_review",
        "manager" => "hr_review",
        "hr" => "completed",
        _ => "draft",
    }
}

/// Nilai 0-100 per peran; rating = round(skor/20).
pub fn submit_review(
    conn: &Connection,
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
    let meta: Option<(i64, i64)> = conn
        .query_row(
            "SELECT performance_period_id, employee_id FROM performance_reviews WHERE id = ?1",
            params![review_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat review: {e}"))?;
    let Some((period_id, emp_id)) = meta else {
        return Err("Review tidak ditemukan.".to_string());
    };
    let rating = (score / 20.0).round() as i64;
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM performance_details WHERE performance_review_id = ?1 AND reviewer_role = ?2",
            params![review_id, role],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa skor: {e}"))?;
    match existing {
        Some(did) => {
            conn.execute(
                "UPDATE performance_details SET reviewer_id = ?1, comments = ?2, rating = ?3 WHERE id = ?4",
                params![actor_id, comments.map(str::trim).filter(|s| !s.is_empty()), rating, did],
            )
            .map_err(|e| format!("gagal memperbarui skor: {e}"))?;
        }
        None => {
            conn.execute(
                "INSERT INTO performance_details (performance_review_id, reviewer_role, reviewer_id, comments, rating) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![review_id, role, actor_id, comments.map(str::trim).filter(|s| !s.is_empty()), rating],
            )
            .map_err(|e| format!("gagal menyimpan skor: {e}"))?;
        }
    }
    let column = format!("{role}_score");
    conn.execute(
        &format!("UPDATE performance_reviews SET {column} = ?1, status = ?2 WHERE id = ?3"),
        params![score, next_status(role), review_id],
    )
    .map_err(|e| format!("gagal memperbarui review: {e}"))?;
    recompute(conn, period_id, emp_id)?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "performance.review",
        Some(&review_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

/// Skor akhir = rata-rata (KPI tertimbang, rata-rata reviewer terisi).
fn recompute(conn: &Connection, period_id: i64, employee_id: i64) -> Result<(), String> {
    let mut weight_sum = 0.0;
    let mut kpi_score = 0.0;
    let mut stmt = conn
        .prepare("SELECT score, weight FROM employee_kpis WHERE performance_period_id = ?1 AND employee_id = ?2")
        .map_err(|e| format!("gagal menyiapkan hitung: {e}"))?;
    let rows: Vec<(Option<f64>, f64)> = stmt
        .query_map(params![period_id, employee_id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| format!("gagal membaca KPI: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("gagal membaca baris: {e}"))?;
    for (_, w) in &rows {
        weight_sum += *w;
    }
    if weight_sum > 0.0 {
        for (score, w) in &rows {
            if let Some(s) = score {
                kpi_score += *s * (*w / weight_sum);
            }
        }
    }
    let rev: (Option<f64>, Option<f64>, Option<f64>, Option<f64>) = conn
        .query_row(
            "SELECT self_score, supervisor_score, manager_score, hr_score FROM performance_reviews WHERE performance_period_id = ?1 AND employee_id = ?2",
            params![period_id, employee_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat skor reviewer: {e}"))?
        .unwrap_or((None, None, None, None));
    let filled: Vec<f64> = [rev.0, rev.1, rev.2, rev.3].into_iter().flatten().collect();
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
    conn.execute(
        "UPDATE performance_reviews SET final_score = ?1, final_rating = ?2 WHERE performance_period_id = ?3 AND employee_id = ?4",
        params![final_score, rating, period_id, employee_id],
    )
    .map_err(|e| format!("gagal menyimpan skor akhir: {e}"))?;
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

    fn mkemp(conn: &Connection, number: &str) -> i64 {
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent')",
            params![number],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn admin(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn review_mengalir_dan_skor_akhir_benar() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let emp = mkemp(&conn, "EMP-P1");
        let pid = period_save(
            &conn,
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
        .expect("periode");
        let kid = kpi_save(
            &conn,
            actor,
            None,
            &KpiInput {
                name: "Penjualan".to_string(),
                description: None,
                department_id: None,
            },
        )
        .expect("kpi");
        // bobot 60 + 40
        let ek1 =
            assign_kpi(&conn, actor, pid as i64, emp, kid as i64, 100.0, 60.0).expect("assign1");
        let kid2 = kpi_save(
            &conn,
            actor,
            None,
            &KpiInput {
                name: "Kehadiran".to_string(),
                description: None,
                department_id: None,
            },
        )
        .expect("kpi2");
        assign_kpi(&conn, actor, pid as i64, emp, kid2 as i64, 100.0, 40.0).expect("assign2");
        // realisasi: 200/100 -> cap 150
        assert_eq!(
            submit_actual(&conn, actor, ek1 as i64, 200.0).expect("aktual"),
            150.0
        );
        // review 4 peran: 80, 80, 80, 80 -> rata 80; KPI = 150*0.6 + 80*0.4 = 122? tunggu: KPI kedua belum ada skor
        // KPI1 = 150 (bobot 60), KPI2 tanpa skor diabaikan proporsional? bobot total 100 -> 150*0.6 = 90
        let rid = ensure_review(&conn, pid as i64, emp).expect("review");
        submit_review(&conn, actor, rid as i64, "self", 80.0, None).expect("self");
        submit_review(&conn, actor, rid as i64, "supervisor", 80.0, None).expect("spv");
        submit_review(&conn, actor, rid as i64, "manager", 80.0, None).expect("mgr");
        submit_review(&conn, actor, rid as i64, "hr", 80.0, None).expect("hr");
        let det = review_detail(&conn, rid as i64).expect("det").expect("ada");
        // KPI = 90, reviewer = 80 -> final 85 -> rating 4
        assert_eq!(det.final_score, Some(85.0));
        assert_eq!(det.final_rating, Some(4));
        assert_eq!(det.status, "completed");
        assert!(submit_review(&conn, actor, rid as i64, "bos", 80.0, None).is_err());
        assert!(submit_review(&conn, actor, rid as i64, "self", 150.0, None).is_err());
        // guard periode bertuan
        assert!(period_delete(&conn, actor, pid as i64).is_err());
        // guard KPI terpakai
        assert!(kpi_delete(&conn, actor, kid as i64).is_err());
    }
}
