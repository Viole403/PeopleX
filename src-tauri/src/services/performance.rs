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

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

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
        exec(
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
        let rid = prow_id(db, "perf.periodadd").await?;
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
        exec(
            db,
            "INSERT INTO kpis (name, description, department_id) VALUES (?1, ?2, ?3)".to_string(),
            vec![Value::Text(input.name.trim().to_string()), desc, dept],
            "perf.kpiadd",
        )
        .await
        .map_err(|e| format!("gagal menambah KPI: {e}"))?;
        let rid = prow_id(db, "perf.kpiadd").await?;
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
    exec(
        db,
        "INSERT INTO performance_reviews (performance_period_id, employee_id, status) VALUES (?1, ?2, 'draft')".to_string(),
        vec![Value::Int(period_id), Value::Int(employee_id)],
        "perf.reviewadd",
    )
    .await
    .map_err(|e| format!("gagal membuat review: {e}"))?;
    to_dto_int(prow_id(db, "perf.reviewadd").await?, "review.id")
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
    exec(
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
    let rid = prow_id(db, "perf.assign").await?;
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

    #[tokio::test]
    async fn kinerja_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        conn.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type) VALUES ('EMP-SEAP', 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent')",
            [],
        )
        .unwrap();
        let emp: i64 = conn.last_insert_rowid();
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
        assert_eq!(
            submit_actual_sea(db, actor, ek1 as i64, 200.0).await.expect("aktual"),
            150.0
        );
        let rid = ensure_review_sea(db, pid as i64, emp).await.expect("review");
        submit_review_sea(db, actor, rid as i64, "self", 80.0, None).await.expect("self");
        submit_review_sea(db, actor, rid as i64, "supervisor", 80.0, None)
            .await
            .expect("spv");
        submit_review_sea(db, actor, rid as i64, "manager", 80.0, None)
            .await
            .expect("mgr");
        submit_review_sea(db, actor, rid as i64, "hr", 80.0, None).await.expect("hr");
        let d_sync =
            serde_json::to_string(&review_detail(&conn, rid as i64).expect("ds")).unwrap();
        let d_sea =
            serde_json::to_string(&review_detail_sea(db, rid as i64).await.expect("dse"))
                .unwrap();
        assert_eq!(d_sync, d_sea);
        assert!(d_sea.contains("\"final_score\":85.0"));
        assert!(d_sea.contains("\"final_rating\":4"));
        assert!(d_sea.contains("\"status\":\"completed\""));
        let r_sync =
            serde_json::to_string(&reviews_for_period(&conn, pid as i64).expect("rs")).unwrap();
        let r_sea =
            serde_json::to_string(&reviews_for_period_sea(db, pid as i64).await.expect("rse"))
                .unwrap();
        assert_eq!(r_sync, r_sea);
        let e_sea = submit_review_sea(db, actor, rid as i64, "bos", 80.0, None)
            .await
            .expect_err("peran salah");
        let e_sync = submit_review(&conn, actor, rid as i64, "bos", 80.0, None)
            .expect_err("peran salah sync");
        assert_eq!(e_sea, e_sync);
        let k_sync = serde_json::to_string(&kpi_list(&conn).expect("ks")).unwrap();
        let k_sea = serde_json::to_string(&kpi_list_sea(db).await.expect("kse")).unwrap();
        assert_eq!(k_sync, k_sea);
        assert!(period_delete_sea(db, actor, pid as i64).await.is_err());
        assert!(kpi_delete_sea(db, actor, kid as i64).await.is_err());
    }
}
