//! Rekrutmen: lowongan, kandidat, tahap, interview, assessment, hire.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

use super::approval;
use super::audit;
use super::employees;
use crate::to_dto_int;

pub const STAGES: &[&str] = &[
    "applied",
    "screening",
    "interview",
    "test",
    "hr_interview",
    "offering",
    "hired",
    "rejected",
];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Vacancy {
    pub id: i32,
    pub title: String,
    pub department_id: Option<i32>,
    pub department_name: Option<String>,
    pub position_id: Option<i32>,
    pub position_name: Option<String>,
    pub employment_type: String,
    pub description: Option<String>,
    pub requirements: Option<String>,
    pub quota: i32,
    pub status: String,
    pub posted_date: Option<String>,
    pub closing_date: Option<String>,
    pub candidate_count: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct VacancyInput {
    pub title: String,
    pub department_id: Option<i32>,
    pub position_id: Option<i32>,
    pub employment_type: String,
    pub description: Option<String>,
    pub requirements: Option<String>,
    pub quota: i32,
    pub status: String,
    pub posted_date: Option<String>,
    pub closing_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CandidateRow {
    pub id: i32,
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub stage: String,
    pub rating: Option<f64>,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CandidateDoc {
    pub id: i32,
    pub name: String,
    pub file_path: String,
    pub category: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Interview {
    pub id: i32,
    pub interviewer_id: Option<i32>,
    pub interviewer_name: Option<String>,
    pub schedule_at: String,
    pub location: Option<String>,
    pub interview_type: String,
    pub result: String,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Assessment {
    pub id: i32,
    pub assessment_name: String,
    pub score: Option<f64>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct StageEvent {
    pub stage: String,
    pub notes: Option<String>,
    pub changed_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CandidateDetail {
    pub id: i32,
    pub vacancy_id: i32,
    pub vacancy_title: String,
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub birth_date: Option<String>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub cv_path: Option<String>,
    pub source: Option<String>,
    pub stage: String,
    pub rating: Option<f64>,
    pub notes: Option<String>,
    pub employee_id: Option<i32>,
    pub documents: Vec<CandidateDoc>,
    pub interviews: Vec<Interview>,
    pub assessments: Vec<Assessment>,
    pub stage_history: Vec<StageEvent>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct CandidateInput {
    pub full_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub birth_date: Option<String>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub source: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct InterviewInput {
    pub interviewer_id: Option<i32>,
    pub schedule_at: String,
    pub location: Option<String>,
    pub interview_type: String,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct AssessmentInput {
    pub assessment_name: String,
    pub score: Option<f64>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct HireInput {
    pub join_date: Option<String>,
}

// ---------------- Lowongan ----------------

pub fn vacancy_list(conn: &Connection) -> Result<Vec<Vacancy>, String> {
    let mut stmt = conn
        .prepare("SELECT v.id, v.title, v.department_id, d.name, v.position_id, p.name, v.employment_type, v.description, v.requirements, v.quota, v.status, v.posted_date, v.closing_date, (SELECT COUNT(*) FROM candidates c WHERE c.vacancy_id = v.id AND c.deleted_at IS NULL) FROM vacancies v LEFT JOIN departments d ON d.id = v.department_id LEFT JOIN positions p ON p.id = v.position_id WHERE v.deleted_at IS NULL ORDER BY v.created_at DESC")
        .map_err(|e| format!("gagal menyiapkan lowongan: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, Option<String>>(11)?,
                r.get::<_, Option<String>>(12)?,
                r.get::<_, i64>(13)?,
            ))
        })
        .map_err(|e| format!("gagal membaca lowongan: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            title,
            dept,
            dept_name,
            pos,
            pos_name,
            etype,
            desc,
            req,
            quota,
            status,
            posted,
            closing,
            count,
        ) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        let opt_i = |v: Option<i64>, f: &str| v.map(|x| to_dto_int(x, f)).transpose();
        out.push(Vacancy {
            id: to_dto_int(id, "vacancy.id")?,
            title,
            department_id: opt_i(dept, "vacancy.dept")?,
            department_name: dept_name,
            position_id: opt_i(pos, "vacancy.pos")?,
            position_name: pos_name,
            employment_type: etype,
            description: desc,
            requirements: req,
            quota: to_dto_int(quota, "vacancy.quota")?,
            status,
            posted_date: posted,
            closing_date: closing,
            candidate_count: to_dto_int(count, "vacancy.count")?,
        });
    }
    Ok(out)
}

pub fn vacancy_get(conn: &Connection, id: i64) -> Result<Option<Vacancy>, String> {
    Ok(vacancy_list(conn)?.into_iter().find(|v| v.id as i64 == id))
}

fn validate_vacancy(conn: &Connection, input: &VacancyInput) -> Result<(), String> {
    if input.title.trim().is_empty() {
        return Err("Judul lowongan wajib diisi.".to_string());
    }
    if input.title.len() > 150 {
        return Err("Judul maksimal 150 karakter.".to_string());
    }
    if !["permanent", "contract", "intern", "daily", "freelance"]
        .contains(&input.employment_type.as_str())
    {
        return Err("Jenis kepegawaian tidak valid.".to_string());
    }
    if !["open", "closed", "on_hold"].contains(&input.status.as_str()) {
        return Err("Status tidak valid.".to_string());
    }
    if input.quota < 1 || input.quota > 1000 {
        return Err("Kuota 1-1000.".to_string());
    }
    for (v, label) in [
        (&input.posted_date, "Tanggal pasang"),
        (&input.closing_date, "Tanggal tutup"),
    ] {
        if let Some(s) = v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{label} harus valid (YYYY-MM-DD)."))?;
        }
    }
    for (opt_id, table, label) in [
        (input.department_id, "departments", "Departemen"),
        (input.position_id, "positions", "Jabatan"),
    ] {
        if let Some(v) = opt_id {
            let found: Option<i64> = conn
                .query_row(
                    &format!("SELECT id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
                    params![v as i64],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| format!("gagal memeriksa {label}: {e}"))?;
            if found.is_none() {
                return Err(format!("{label} tidak ditemukan."));
            }
        }
    }
    Ok(())
}

pub fn vacancy_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &VacancyInput,
) -> Result<i32, String> {
    validate_vacancy(conn, input)?;
    let posted = input
        .posted_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let closing = input
        .closing_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE vacancies SET title = ?1, department_id = ?2, position_id = ?3, employment_type = ?4, description = ?5, requirements = ?6, quota = ?7, status = ?8, posted_date = ?9, closing_date = ?10 WHERE id = ?11 AND deleted_at IS NULL",
            params![
                input.title.trim(), input.department_id.map(|v| v as i64),
                input.position_id.map(|v| v as i64), input.employment_type,
                input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.requirements.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.quota as i64, input.status, posted, closing, rid,
            ],
        ).map_err(|e| format!("gagal menyimpan lowongan: {e}"))?;
        if n == 0 {
            return Err("Lowongan tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "recruitment.vacancy",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "vacancy.id")
    } else {
        conn.execute(
            "INSERT INTO vacancies (title, department_id, position_id, employment_type, description, requirements, quota, status, posted_date, closing_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                input.title.trim(), input.department_id.map(|v| v as i64),
                input.position_id.map(|v| v as i64), input.employment_type,
                input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.requirements.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.quota as i64, input.status, posted, closing,
            ],
        ).map_err(|e| format!("gagal menambah lowongan: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "recruitment.vacancy",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "vacancy.id")
    }
}

pub fn vacancy_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM candidates WHERE vacancy_id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa kandidat: {e}"))?;
    if n > 0 {
        return Err("Lowongan masih memiliki kandidat.".to_string());
    }
    let d = conn.execute(
        "UPDATE vacancies SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus lowongan: {e}"))?;
    if d == 0 {
        return Err("Lowongan tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "recruitment.vacancy",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Kandidat ----------------

fn log_stage(
    conn: &Connection,
    candidate_id: i64,
    stage: &str,
    notes: Option<&str>,
    by: Option<i64>,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO recruitment_stages (candidate_id, stage, notes, changed_by, changed_at) VALUES (?1, ?2, ?3, ?4, datetime('now','localtime'))",
        params![candidate_id, stage, notes.map(str::trim).filter(|s| !s.is_empty()), by],
    )
    .map_err(|e| format!("gagal mencatat tahap: {e}"))?;
    Ok(())
}

pub fn candidates_by_vacancy(
    conn: &Connection,
    vacancy_id: i64,
) -> Result<Vec<CandidateRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, full_name, email, phone, stage, rating, created_at FROM candidates WHERE vacancy_id = ?1 AND deleted_at IS NULL ORDER BY created_at DESC")
        .map_err(|e| format!("gagal menyiapkan kandidat: {e}"))?;
    collect_rows(conn, &mut stmt, vacancy_id)
}

fn collect_rows(
    _conn: &Connection,
    stmt: &mut rusqlite::Statement,
    param: i64,
) -> Result<Vec<CandidateRow>, String> {
    let rows = stmt
        .query_map(params![param], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<f64>>(5)?,
                r.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| format!("gagal membaca kandidat: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name, email, phone, stage, rating, created) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(CandidateRow {
            id: to_dto_int(id, "candidate.id")?,
            full_name: name,
            email,
            phone,
            stage,
            rating,
            created_at: created,
        });
    }
    Ok(out)
}

pub fn candidate_detail(conn: &Connection, id: i64) -> Result<Option<CandidateDetail>, String> {
    let row: Option<(
        i64, i64, String, String, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, String,
        Option<f64>, Option<String>, Option<i64>,
    )> = conn
        .query_row(
            "SELECT c.id, c.vacancy_id, v.title, c.full_name, c.email, c.phone, c.birth_date, c.gender, c.address, c.cv_path, c.source, c.stage, c.rating, c.notes, c.employee_id FROM candidates c INNER JOIN vacancies v ON v.id = c.vacancy_id WHERE c.id = ?1 AND c.deleted_at IS NULL",
            params![id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                    r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?,
                    r.get(12)?, r.get(13)?, r.get(14)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    let Some((
        cid,
        vid,
        vtitle,
        name,
        email,
        phone,
        birth,
        gender,
        addr,
        cv,
        source,
        stage,
        rating,
        notes,
        emp,
    )) = row
    else {
        return Ok(None);
    };
    let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "candidate.ref")).transpose();
    let mut docs = Vec::new();
    let mut dstmt = conn
        .prepare("SELECT id, name, file_path, category FROM candidate_documents WHERE candidate_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| format!("gagal menyiapkan dokumen: {e}"))?;
    for drow in dstmt
        .query_map(params![cid], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca dokumen: {e}"))?
    {
        let (did, dname, dpath, dcat) = drow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        docs.push(CandidateDoc {
            id: to_dto_int(did, "doc.id")?,
            name: dname,
            file_path: dpath,
            category: dcat,
        });
    }
    let mut interviews = Vec::new();
    let mut istmt = conn
        .prepare("SELECT i.id, i.interviewer_id, e.first_name || ' ' || COALESCE(e.last_name, ''), i.schedule_at, i.location, i.type, i.result, i.notes FROM interviews i LEFT JOIN employees e ON e.id = i.interviewer_id WHERE i.candidate_id = ?1 ORDER BY i.schedule_at DESC")
        .map_err(|e| format!("gagal menyiapkan interview: {e}"))?;
    for irow in istmt
        .query_map(params![cid], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca interview: {e}"))?
    {
        let (iid, ivr, ivr_name, sched, loc, itype, res, note) =
            irow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        interviews.push(Interview {
            id: to_dto_int(iid, "interview.id")?,
            interviewer_id: opt_i(ivr)?,
            interviewer_name: ivr_name,
            schedule_at: sched,
            location: loc,
            interview_type: itype,
            result: res,
            notes: note,
        });
    }
    let mut assessments = Vec::new();
    let mut astmt = conn
        .prepare("SELECT id, assessment_name, score, notes FROM candidate_assessments WHERE candidate_id = ?1 ORDER BY created_at DESC")
        .map_err(|e| format!("gagal menyiapkan assessment: {e}"))?;
    for arow in astmt
        .query_map(params![cid], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<f64>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca assessment: {e}"))?
    {
        let (aid, aname, score, note) = arow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        assessments.push(Assessment {
            id: to_dto_int(aid, "assessment.id")?,
            assessment_name: aname,
            score,
            notes: note,
        });
    }
    let mut history = Vec::new();
    let mut hstmt = conn
        .prepare("SELECT stage, notes, changed_at FROM recruitment_stages WHERE candidate_id = ?1 ORDER BY changed_at DESC")
        .map_err(|e| format!("gagal menyiapkan riwayat: {e}"))?;
    for hrow in hstmt
        .query_map(params![cid], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca riwayat: {e}"))?
    {
        let (stage, note, at) = hrow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        history.push(StageEvent {
            stage,
            notes: note,
            changed_at: at,
        });
    }
    Ok(Some(CandidateDetail {
        id: to_dto_int(cid, "candidate.id")?,
        vacancy_id: to_dto_int(vid, "candidate.vacancy")?,
        vacancy_title: vtitle,
        full_name: name,
        email,
        phone,
        birth_date: birth,
        gender,
        address: addr,
        cv_path: cv,
        source,
        stage,
        rating,
        notes,
        employee_id: opt_i(emp)?,
        documents: docs,
        interviews,
        assessments,
        stage_history: history,
    }))
}

pub fn candidate_create(
    conn: &Connection,
    files: &Path,
    actor_id: i64,
    vacancy_id: i64,
    input: &CandidateInput,
    cv: Option<&employees::FileUpload>,
) -> Result<i32, String> {
    let vac: Option<i64> = conn
        .query_row(
            "SELECT id FROM vacancies WHERE id = ?1 AND deleted_at IS NULL",
            params![vacancy_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa lowongan: {e}"))?;
    if vac.is_none() {
        return Err("Lowongan tidak ditemukan.".to_string());
    }
    if input.full_name.trim().is_empty() {
        return Err("Nama lengkap wajib diisi.".to_string());
    }
    if input.full_name.len() > 150 {
        return Err("Nama maksimal 150 karakter.".to_string());
    }
    if let Some(email) = input
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !email.contains('@') {
            return Err("Email tidak valid.".to_string());
        }
    }
    if let Some(bd) = input
        .birth_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        NaiveDate::parse_from_str(bd, "%Y-%m-%d")
            .map_err(|_| "Tanggal lahir harus valid (YYYY-MM-DD).".to_string())?;
    }
    if let Some(g) = input
        .gender
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if g != "male" && g != "female" {
            return Err("Jenis kelamin tidak valid.".to_string());
        }
    }
    let cv_path = match cv {
        Some(f) => Some(store_cv(files, f)?),
        None => None,
    };
    conn.execute(
        "INSERT INTO candidates (vacancy_id, full_name, email, phone, birth_date, gender, address, cv_path, source, stage) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'applied')",
        params![
            vacancy_id,
            input.full_name.trim(),
            input.email.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.phone.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.birth_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.gender.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.address.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            cv_path,
            input.source.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )
    .map_err(|e| format!("gagal menambah kandidat: {e}"))?;
    let rid = conn.last_insert_rowid();
    log_stage(conn, rid, "applied", Some("Kandidat mendaftar"), None)?;
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "recruitment.candidate",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "candidate.id")
}

const CV_MIMES: &[&str] = &[
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
];

fn store_cv(files: &Path, file: &employees::FileUpload) -> Result<String, String> {
    if !CV_MIMES.contains(&file.mime.as_str()) {
        return Err("CV harus PDF atau Word.".to_string());
    }
    if file.bytes.is_empty() {
        return Err("Berkas kosong.".to_string());
    }
    if file.bytes.len() > 5 * 1024 * 1024 {
        return Err("Ukuran CV maksimal 5MB.".to_string());
    }
    let dir = files.join("candidates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("gagal membuat folder: {e}"))?;
    let base: String = Path::new(&file.name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("cv")
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
    let rel = format!("candidates/{stamp}_{base}");
    std::fs::write(files.join(&rel), &file.bytes)
        .map_err(|e| format!("gagal menyimpan CV: {e}"))?;
    Ok(rel)
}

pub fn candidate_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n = conn.execute(
        "UPDATE candidates SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus kandidat: {e}"))?;
    if n == 0 {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "recruitment.candidate",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn update_stage(
    conn: &Connection,
    actor_id: i64,
    candidate_id: i64,
    stage: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if !STAGES.contains(&stage) {
        return Err("Tahap tidak valid.".to_string());
    }
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL",
            params![candidate_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    if exists.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    conn.execute(
        "UPDATE candidates SET stage = ?1 WHERE id = ?2",
        params![stage, candidate_id],
    )
    .map_err(|e| format!("gagal mengubah tahap: {e}"))?;
    log_stage(conn, candidate_id, stage, notes, Some(actor_id))?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "recruitment.candidate_stage",
        Some(&candidate_id.to_string()),
        None,
        None,
        Some(&format!("Tahap menjadi {stage}")),
    )?;
    Ok(())
}

pub fn add_interview(
    conn: &Connection,
    actor_id: i64,
    candidate_id: i64,
    input: &InterviewInput,
) -> Result<i32, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL",
            params![candidate_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    if exists.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    if !["hr", "user", "technical"].contains(&input.interview_type.as_str()) {
        return Err("Jenis interview tidak valid.".to_string());
    }
    if let Some(iv) = input.interviewer_id {
        let found: Option<i64> = conn
            .query_row(
                "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
                params![iv as i64],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memeriksa pewawancara: {e}"))?;
        if found.is_none() {
            return Err("Pewawancara tidak ditemukan.".to_string());
        }
    }
    conn.execute(
        "INSERT INTO interviews (candidate_id, interviewer_id, schedule_at, location, type, result, notes) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)",
        params![
            candidate_id,
            input.interviewer_id.map(|v| v as i64),
            input.schedule_at.trim(),
            input.location.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.interview_type,
            input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )
    .map_err(|e| format!("gagal menjadwalkan interview: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "recruitment.interview",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "interview.id")
}

pub fn decide_interview(
    conn: &Connection,
    actor_id: i64,
    interview_id: i64,
    result: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if result != "pass" && result != "fail" {
        return Err("Hasil tidak valid.".to_string());
    }
    let n = conn
        .execute(
            "UPDATE interviews SET result = ?1, notes = ?2 WHERE id = ?3",
            params![
                result,
                notes.map(str::trim).filter(|s| !s.is_empty()),
                interview_id
            ],
        )
        .map_err(|e| format!("gagal menyimpan hasil: {e}"))?;
    if n == 0 {
        return Err("Interview tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "recruitment.interview",
        Some(&interview_id.to_string()),
        None,
        None,
        Some(&format!("Hasil: {result}")),
    )?;
    Ok(())
}

pub fn add_assessment(
    conn: &Connection,
    actor_id: i64,
    candidate_id: i64,
    input: &AssessmentInput,
) -> Result<i32, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL",
            params![candidate_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    if exists.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    if input.assessment_name.trim().is_empty() {
        return Err("Nama assessment wajib diisi.".to_string());
    }
    if input.assessment_name.len() > 150 {
        return Err("Nama assessment maksimal 150 karakter.".to_string());
    }
    conn.execute(
        "INSERT INTO candidate_assessments (candidate_id, assessment_name, score, notes, assessed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            candidate_id,
            input.assessment_name.trim(),
            input.score,
            input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            actor_id,
        ],
    )
    .map_err(|e| format!("gagal menyimpan assessment: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "recruitment.assessment",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "assessment.id")
}

pub fn candidate_document_bytes(
    conn: &Connection,
    files: &Path,
    candidate_id: i64,
    id: i64,
) -> Result<super::employees::DocumentBytes, String> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT file_path, name FROM candidate_documents WHERE id = ?1 AND candidate_id = ?2",
            params![id, candidate_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat dokumen: {e}"))?;
    let Some((rel, name)) = row else {
        return Err("Dokumen tidak ditemukan.".to_string());
    };
    let path = files.join(&rel);
    if !path.starts_with(files) {
        return Err("Path tidak valid.".to_string());
    }
    let bytes = std::fs::read(&path).map_err(|_| "Berkas hilang dari penyimpanan.".to_string())?;
    Ok(super::employees::DocumentBytes {
        mime: "application/octet-stream".to_string(),
        name,
        bytes,
    })
}

/// Terima kandidat: buat karyawan + onboarding. Satu kandidat satu karyawan.
pub fn hire(
    conn: &Connection,
    files: &Path,
    actor_id: i64,
    candidate_id: i64,
    join_date: Option<&str>,
) -> Result<i32, String> {
    let cand: Option<(i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>)> = conn
        .query_row(
            "SELECT vacancy_id, full_name, gender, email, phone, birth_date, address, employee_id FROM candidates WHERE id = ?1 AND deleted_at IS NULL",
            params![candidate_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    let Some((vacancy_id, full_name, gender, email, phone, birth_date, _address, already)) = cand
    else {
        return Err("Kandidat tidak ditemukan.".to_string());
    };
    if already.is_some() {
        return Err("Kandidat ini sudah menjadi karyawan.".to_string());
    }
    let vac: Option<(Option<i64>, Option<i64>, String)> = conn
        .query_row(
            "SELECT department_id, position_id, employment_type FROM vacancies WHERE id = ?1",
            params![vacancy_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat lowongan: {e}"))?;
    let (dept, pos, etype) = vac.unwrap_or((None, None, "contract".to_string()));
    let company: i64 = conn
        .query_row(
            "SELECT id FROM companies WHERE deleted_at IS NULL ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat perusahaan: {e}"))?
        .ok_or("Perusahaan belum ada.".to_string())?;
    let join = join_date
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&chrono::Local::now().format("%Y-%m-%d").to_string())
        .to_string();
    NaiveDate::parse_from_str(&join, "%Y-%m-%d")
        .map_err(|_| "Tanggal masuk tidak valid.".to_string())?;
    let (first, last) = full_name
        .split_once(' ')
        .unwrap_or((full_name.as_str(), ""));
    let input = employees::EmployeeInput {
        first_name: first.to_string(),
        last_name: if last.is_empty() {
            None
        } else {
            Some(last.to_string())
        },
        gender: gender.unwrap_or_else(|| "male".to_string()),
        marital_status: "single".to_string(),
        company_id: to_dto_int(company, "hire.company")?,
        department_id: dept.map(|v| to_dto_int(v, "hire.dept")).transpose()?,
        position_id: pos.map(|v| to_dto_int(v, "hire.pos")).transpose()?,
        employment_type: etype,
        employment_status: "probation".to_string(),
        join_date: join.clone(),
        personal_email: email,
        phone,
        birth_date,
        ..Default::default()
    };
    let employee_id = employees::create(conn, files, actor_id, &input, None)?;
    conn.execute(
        "UPDATE candidates SET stage = 'hired', employee_id = ?1 WHERE id = ?2",
        params![employee_id as i64, candidate_id],
    )
    .map_err(|e| format!("gagal menandai hire: {e}"))?;
    log_stage(
        conn,
        candidate_id,
        "hired",
        Some("Diterima menjadi karyawan"),
        Some(actor_id),
    )?;
    super::onboarding::create_for_employee(conn, actor_id, employee_id as i64, None, &join)?;
    if let Some(uid) = approval::user_of_employee(conn, employee_id as i64)? {
        approval::notify(
            conn,
            uid,
            "recruitment",
            "Selamat Bergabung",
            "Akun karyawan Anda telah dibuat.",
            "/employees",
        )?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "HIRE",
        "recruitment.candidate",
        Some(&candidate_id.to_string()),
        None,
        None,
        Some(&format!("employee {employee_id}")),
    )?;
    Ok(employee_id)
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

fn ropt_i(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match value_i64(v) {
        Some(x) => Ok(Some(to_dto_int(x, f)?)),
        None => Ok(None),
    }
}

fn ropt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

fn ropt_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse().ok(),
    }
}

async fn rrow_id(db: &sea_orm::DatabaseConnection, label: &str) -> Result<i64, String> {
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

pub async fn vacancy_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Vacancy>, String> {
    let rows = q_all(
        db,
        "SELECT v.id, v.title, v.department_id, d.name, v.position_id, p.name, v.employment_type, v.description, v.requirements, v.quota, v.status, v.posted_date, v.closing_date, (SELECT COUNT(*) FROM candidates c WHERE c.vacancy_id = v.id AND c.deleted_at IS NULL) FROM vacancies v LEFT JOIN departments d ON d.id = v.department_id LEFT JOIN positions p ON p.id = v.position_id WHERE v.deleted_at IS NULL ORDER BY v.created_at DESC".to_string(),
        vec![],
        14,
        "recruit.vacancies",
    )
    .await
    .map_err(|e| format!("gagal membaca lowongan: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Vacancy {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "vacancy.id")?,
            title: value_to_string(&r[1]),
            department_id: ropt_i(&r[2], "vacancy.dept")?,
            department_name: ropt_text(&r[3]),
            position_id: ropt_i(&r[4], "vacancy.pos")?,
            position_name: ropt_text(&r[5]),
            employment_type: value_to_string(&r[6]),
            description: ropt_text(&r[7]),
            requirements: ropt_text(&r[8]),
            quota: to_dto_int(value_i64(&r[9]).unwrap_or(0), "vacancy.quota")?,
            status: value_to_string(&r[10]),
            posted_date: ropt_text(&r[11]),
            closing_date: ropt_text(&r[12]),
            candidate_count: to_dto_int(value_i64(&r[13]).unwrap_or(0), "vacancy.count")?,
        });
    }
    Ok(out)
}

pub async fn vacancy_get_sea(
    db: &sea_orm::DatabaseConnection,
    id: i64,
) -> Result<Option<Vacancy>, String> {
    Ok(vacancy_list_sea(db)
        .await?
        .into_iter()
        .find(|v| v.id as i64 == id))
}

async fn validate_vacancy_sea(
    db: &sea_orm::DatabaseConnection,
    input: &VacancyInput,
) -> Result<(), String> {
    if input.title.trim().is_empty() {
        return Err("Judul lowongan wajib diisi.".to_string());
    }
    if input.title.len() > 150 {
        return Err("Judul maksimal 150 karakter.".to_string());
    }
    if !["permanent", "contract", "intern", "daily", "freelance"]
        .contains(&input.employment_type.as_str())
    {
        return Err("Jenis kepegawaian tidak valid.".to_string());
    }
    if !["open", "closed", "on_hold"].contains(&input.status.as_str()) {
        return Err("Status tidak valid.".to_string());
    }
    if input.quota < 1 || input.quota > 1000 {
        return Err("Kuota 1-1000.".to_string());
    }
    for (v, label) in [
        (&input.posted_date, "Tanggal pasang"),
        (&input.closing_date, "Tanggal tutup"),
    ] {
        if let Some(s) = v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{label} harus valid (YYYY-MM-DD)."))?;
        }
    }
    for (opt_id, table, label) in [
        (input.department_id, "departments", "Departemen"),
        (input.position_id, "positions", "Jabatan"),
    ] {
        if let Some(v) = opt_id {
            let found = q_one(
                db,
                format!("SELECT id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
                vec![Value::Int(v as i64)],
                1,
                "recruit.refcheck",
            )
            .await
            .map_err(|e| format!("gagal memeriksa {label}: {e}"))?;
            if found.is_none() {
                return Err(format!("{label} tidak ditemukan."));
            }
        }
    }
    Ok(())
}

pub async fn vacancy_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &VacancyInput,
) -> Result<i32, String> {
    validate_vacancy_sea(db, input).await?;
    let posted = input
        .posted_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let closing = input
        .closing_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let opt_i = |v: Option<i32>| match v {
        Some(x) => Value::Int(x as i64),
        None => Value::Null,
    };
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE vacancies SET title = ?1, department_id = ?2, position_id = ?3, employment_type = ?4, description = ?5, requirements = ?6, quota = ?7, status = ?8, posted_date = ?9, closing_date = ?10 WHERE id = ?11 AND deleted_at IS NULL".to_string(),
            vec![
                Value::Text(input.title.trim().to_string()),
                opt_i(input.department_id),
                opt_i(input.position_id),
                Value::Text(input.employment_type.clone()),
                opt_t(input.description.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.requirements.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                Value::Int(input.quota as i64),
                Value::Text(input.status.clone()),
                opt_t(posted),
                opt_t(closing),
                Value::Int(rid),
            ],
            "recruit.vacupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan lowongan: {e}"))?;
        if n == 0 {
            return Err("Lowongan tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "recruitment.vacancy",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "vacancy.id")
    } else {
        exec(
            db,
            "INSERT INTO vacancies (title, department_id, position_id, employment_type, description, requirements, quota, status, posted_date, closing_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)".to_string(),
            vec![
                Value::Text(input.title.trim().to_string()),
                opt_i(input.department_id),
                opt_i(input.position_id),
                Value::Text(input.employment_type.clone()),
                opt_t(input.description.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.requirements.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                Value::Int(input.quota as i64),
                Value::Text(input.status.clone()),
                opt_t(posted),
                opt_t(closing),
            ],
            "recruit.vacadd",
        )
        .await
        .map_err(|e| format!("gagal menambah lowongan: {e}"))?;
        let rid = rrow_id(db, "recruit.vacadd").await?;
        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "recruitment.vacancy",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "vacancy.id")
    }
}

pub async fn vacancy_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM candidates WHERE vacancy_id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        1,
        "recruit.vacrel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa kandidat: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Lowongan masih memiliki kandidat.".to_string());
    }
    let d = exec(
        db,
        "UPDATE vacancies SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "recruit.vacdel",
    )
    .await
    .map_err(|e| format!("gagal menghapus lowongan: {e}"))?;
    if d == 0 {
        return Err("Lowongan tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "recruitment.vacancy",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

async fn log_stage_sea(
    db: &sea_orm::DatabaseConnection,
    candidate_id: i64,
    stage: &str,
    notes: Option<&str>,
    by: Option<i64>,
) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO recruitment_stages (candidate_id, stage, notes, changed_by, changed_at) VALUES (?1, ?2, ?3, ?4, datetime('now','localtime'))".to_string(),
        vec![
            Value::Int(candidate_id),
            Value::Text(stage.to_string()),
            match notes.map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            match by {
                Some(b) => Value::Int(b),
                None => Value::Null,
            },
        ],
        "recruit.logstage",
    )
    .await
    .map_err(|e| format!("gagal mencatat tahap: {e}"))?;
    Ok(())
}

fn map_candidate_row(r: &[Value]) -> Result<CandidateRow, String> {
    Ok(CandidateRow {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "candidate.id")?,
        full_name: value_to_string(&r[1]),
        email: ropt_text(&r[2]),
        phone: ropt_text(&r[3]),
        stage: value_to_string(&r[4]),
        rating: ropt_f64(&r[5]),
        created_at: value_to_string(&r[6]),
    })
}

pub async fn candidates_by_vacancy_sea(
    db: &sea_orm::DatabaseConnection,
    vacancy_id: i64,
) -> Result<Vec<CandidateRow>, String> {
    let rows = q_all(
        db,
        "SELECT id, full_name, email, phone, stage, rating, created_at FROM candidates WHERE vacancy_id = ?1 AND deleted_at IS NULL ORDER BY created_at DESC".to_string(),
        vec![Value::Int(vacancy_id)],
        7,
        "recruit.cands",
    )
    .await
    .map_err(|e| format!("gagal membaca kandidat: {e}"))?;
    rows.iter().map(|r| map_candidate_row(r)).collect()
}

pub async fn candidate_detail_sea(
    db: &sea_orm::DatabaseConnection,
    id: i64,
) -> Result<Option<CandidateDetail>, String> {
    let row = q_one(
        db,
        "SELECT c.id, c.vacancy_id, v.title, c.full_name, c.email, c.phone, c.birth_date, c.gender, c.address, c.cv_path, c.source, c.stage, c.rating, c.notes, c.employee_id FROM candidates c INNER JOIN vacancies v ON v.id = c.vacancy_id WHERE c.id = ?1 AND c.deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        15,
        "recruit.canddet",
    )
    .await
    .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    let Some(r) = row else {
        return Ok(None);
    };
    let (cid, vid, emp) = (
        value_i64(&r[0]).unwrap_or(0),
        value_i64(&r[1]).unwrap_or(0),
        value_i64(&r[14]),
    );
    let mut docs = Vec::new();
    let drows = q_all(
        db,
        "SELECT id, name, file_path, category FROM candidate_documents WHERE candidate_id = ?1 ORDER BY created_at DESC".to_string(),
        vec![Value::Int(cid)],
        4,
        "recruit.canddocs",
    )
    .await
    .map_err(|e| format!("gagal membaca dokumen: {e}"))?;
    for d in &drows {
        docs.push(CandidateDoc {
            id: to_dto_int(value_i64(&d[0]).unwrap_or(0), "doc.id")?,
            name: value_to_string(&d[1]),
            file_path: value_to_string(&d[2]),
            category: ropt_text(&d[3]),
        });
    }
    let mut interviews = Vec::new();
    let irows = q_all(
        db,
        "SELECT i.id, i.interviewer_id, e.first_name || ' ' || COALESCE(e.last_name, ''), i.schedule_at, i.location, i.type, i.result, i.notes FROM interviews i LEFT JOIN employees e ON e.id = i.interviewer_id WHERE i.candidate_id = ?1 ORDER BY i.schedule_at DESC".to_string(),
        vec![Value::Int(cid)],
        8,
        "recruit.candivs",
    )
    .await
    .map_err(|e| format!("gagal membaca interview: {e}"))?;
    for v in &irows {
        interviews.push(Interview {
            id: to_dto_int(value_i64(&v[0]).unwrap_or(0), "interview.id")?,
            interviewer_id: ropt_i(&v[1], "candidate.ref")?,
            interviewer_name: ropt_text(&v[2]),
            schedule_at: value_to_string(&v[3]),
            location: ropt_text(&v[4]),
            interview_type: value_to_string(&v[5]),
            result: value_to_string(&v[6]),
            notes: ropt_text(&v[7]),
        });
    }
    let mut assessments = Vec::new();
    let arows = q_all(
        db,
        "SELECT id, assessment_name, score, notes FROM candidate_assessments WHERE candidate_id = ?1 ORDER BY created_at DESC".to_string(),
        vec![Value::Int(cid)],
        4,
        "recruit.candass",
    )
    .await
    .map_err(|e| format!("gagal membaca assessment: {e}"))?;
    for a in &arows {
        assessments.push(Assessment {
            id: to_dto_int(value_i64(&a[0]).unwrap_or(0), "assessment.id")?,
            assessment_name: value_to_string(&a[1]),
            score: ropt_f64(&a[2]),
            notes: ropt_text(&a[3]),
        });
    }
    let mut history = Vec::new();
    let hrows = q_all(
        db,
        "SELECT stage, notes, changed_at FROM recruitment_stages WHERE candidate_id = ?1 ORDER BY changed_at DESC".to_string(),
        vec![Value::Int(cid)],
        3,
        "recruit.candhist",
    )
    .await
    .map_err(|e| format!("gagal membaca riwayat: {e}"))?;
    for h in &hrows {
        history.push(StageEvent {
            stage: value_to_string(&h[0]),
            notes: ropt_text(&h[1]),
            changed_at: value_to_string(&h[2]),
        });
    }
    Ok(Some(CandidateDetail {
        id: to_dto_int(cid, "candidate.id")?,
        vacancy_id: to_dto_int(vid, "candidate.vacancy")?,
        vacancy_title: value_to_string(&r[2]),
        full_name: value_to_string(&r[3]),
        email: ropt_text(&r[4]),
        phone: ropt_text(&r[5]),
        birth_date: ropt_text(&r[6]),
        gender: ropt_text(&r[7]),
        address: ropt_text(&r[8]),
        cv_path: ropt_text(&r[9]),
        source: ropt_text(&r[10]),
        stage: value_to_string(&r[11]),
        rating: ropt_f64(&r[12]),
        notes: ropt_text(&r[13]),
        employee_id: match emp {
            Some(x) => Some(to_dto_int(x, "candidate.ref")?),
            None => None,
        },
        documents: docs,
        interviews,
        assessments,
        stage_history: history,
    }))
}

pub async fn candidate_create_sea(
    db: &sea_orm::DatabaseConnection,
    files: &std::path::Path,
    actor_id: i64,
    vacancy_id: i64,
    input: &CandidateInput,
    cv: Option<&employees::FileUpload>,
) -> Result<i32, String> {
    let vac = q_one(
        db,
        "SELECT id FROM vacancies WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(vacancy_id)],
        1,
        "recruit.vaccheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa lowongan: {e}"))?;
    if vac.is_none() {
        return Err("Lowongan tidak ditemukan.".to_string());
    }
    if input.full_name.trim().is_empty() {
        return Err("Nama lengkap wajib diisi.".to_string());
    }
    if input.full_name.len() > 150 {
        return Err("Nama maksimal 150 karakter.".to_string());
    }
    if let Some(email) = input
        .email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !email.contains('@') {
            return Err("Email tidak valid.".to_string());
        }
    }
    if let Some(bd) = input
        .birth_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        NaiveDate::parse_from_str(bd, "%Y-%m-%d")
            .map_err(|_| "Tanggal lahir harus valid (YYYY-MM-DD).".to_string())?;
    }
    if let Some(g) = input
        .gender
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if g != "male" && g != "female" {
            return Err("Jenis kelamin tidak valid.".to_string());
        }
    }
    let cv_path = match cv {
        Some(f) => Some(store_cv(files, f)?),
        None => None,
    };
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    exec(
        db,
        "INSERT INTO candidates (vacancy_id, full_name, email, phone, birth_date, gender, address, cv_path, source, stage) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'applied')".to_string(),
        vec![
            Value::Int(vacancy_id),
            Value::Text(input.full_name.trim().to_string()),
            opt_t(input.email.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.phone.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.birth_date.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.gender.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.address.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(cv_path.as_deref()),
            opt_t(input.source.as_deref().map(str::trim).filter(|s| !s.is_empty())),
        ],
        "recruit.candadd",
    )
    .await
    .map_err(|e| format!("gagal menambah kandidat: {e}"))?;
    let rid = rrow_id(db, "recruit.candadd").await?;
    log_stage_sea(db, rid, "applied", Some("Kandidat mendaftar"), None).await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "recruitment.candidate",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "candidate.id")
}

pub async fn candidate_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let n = exec(
        db,
        "UPDATE candidates SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "recruit.canddel",
    )
    .await
    .map_err(|e| format!("gagal menghapus kandidat: {e}"))?;
    if n == 0 {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "recruitment.candidate",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn update_stage_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    candidate_id: i64,
    stage: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if !STAGES.contains(&stage) {
        return Err("Tahap tidak valid.".to_string());
    }
    let exists = q_one(
        db,
        "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(candidate_id)],
        1,
        "recruit.candcheck",
    )
    .await
    .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    if exists.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    exec(
        db,
        "UPDATE candidates SET stage = ?1 WHERE id = ?2".to_string(),
        vec![Value::Text(stage.to_string()), Value::Int(candidate_id)],
        "recruit.stageupd",
    )
    .await
    .map_err(|e| format!("gagal mengubah tahap: {e}"))?;
    log_stage_sea(db, candidate_id, stage, notes, Some(actor_id)).await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "recruitment.candidate_stage",
        Some(&candidate_id.to_string()),
        None,
        None,
        Some(&format!("Tahap menjadi {stage}")),
    )
    .await?;
    Ok(())
}

pub async fn add_interview_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    candidate_id: i64,
    input: &InterviewInput,
) -> Result<i32, String> {
    let exists = q_one(
        db,
        "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(candidate_id)],
        1,
        "recruit.candcheck",
    )
    .await
    .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    if exists.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    if !["hr", "user", "technical"].contains(&input.interview_type.as_str()) {
        return Err("Jenis interview tidak valid.".to_string());
    }
    if let Some(iv) = input.interviewer_id {
        let found = q_one(
            db,
            "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::Int(iv as i64)],
            1,
            "recruit.ivcheck",
        )
        .await
        .map_err(|e| format!("gagal memeriksa pewawancara: {e}"))?;
        if found.is_none() {
            return Err("Pewawancara tidak ditemukan.".to_string());
        }
    }
    exec(
        db,
        "INSERT INTO interviews (candidate_id, interviewer_id, schedule_at, location, type, result, notes) VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6)".to_string(),
        vec![
            Value::Int(candidate_id),
            match input.interviewer_id {
                Some(v) => Value::Int(v as i64),
                None => Value::Null,
            },
            Value::Text(input.schedule_at.trim().to_string()),
            match input.location.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Text(input.interview_type.clone()),
            match input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
        ],
        "recruit.ivadd",
    )
    .await
    .map_err(|e| format!("gagal menjadwalkan interview: {e}"))?;
    let rid = rrow_id(db, "recruit.ivadd").await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "recruitment.interview",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "interview.id")
}

pub async fn decide_interview_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    interview_id: i64,
    result: &str,
    notes: Option<&str>,
) -> Result<(), String> {
    if result != "pass" && result != "fail" {
        return Err("Hasil tidak valid.".to_string());
    }
    let n = exec(
        db,
        "UPDATE interviews SET result = ?1, notes = ?2 WHERE id = ?3".to_string(),
        vec![
            Value::Text(result.to_string()),
            match notes.map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Int(interview_id),
        ],
        "recruit.ivdecide",
    )
    .await
    .map_err(|e| format!("gagal menyimpan hasil: {e}"))?;
    if n == 0 {
        return Err("Interview tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "recruitment.interview",
        Some(&interview_id.to_string()),
        None,
        None,
        Some(&format!("Hasil: {result}")),
    )
    .await?;
    Ok(())
}

pub async fn add_assessment_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    candidate_id: i64,
    input: &AssessmentInput,
) -> Result<i32, String> {
    let exists = q_one(
        db,
        "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(candidate_id)],
        1,
        "recruit.candcheck",
    )
    .await
    .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    if exists.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    if input.assessment_name.trim().is_empty() {
        return Err("Nama assessment wajib diisi.".to_string());
    }
    if input.assessment_name.len() > 150 {
        return Err("Nama assessment maksimal 150 karakter.".to_string());
    }
    exec(
        db,
        "INSERT INTO candidate_assessments (candidate_id, assessment_name, score, notes, assessed_by) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(candidate_id),
            Value::Text(input.assessment_name.trim().to_string()),
            match input.score {
                Some(s) => Value::Float(s),
                None => Value::Null,
            },
            match input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Int(actor_id),
        ],
        "recruit.assadd",
    )
    .await
    .map_err(|e| format!("gagal menyimpan assessment: {e}"))?;
    let rid = rrow_id(db, "recruit.assadd").await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "recruitment.assessment",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "assessment.id")
}

pub async fn candidate_document_bytes_sea(
    db: &sea_orm::DatabaseConnection,
    files: &std::path::Path,
    candidate_id: i64,
    id: i64,
) -> Result<super::employees::DocumentBytes, String> {
    let row = q_one(
        db,
        "SELECT file_path, name FROM candidate_documents WHERE id = ?1 AND candidate_id = ?2".to_string(),
        vec![Value::Int(id), Value::Int(candidate_id)],
        2,
        "recruit.docbytes",
    )
    .await
    .map_err(|e| format!("gagal memuat dokumen: {e}"))?;
    let Some(row) = row else {
        return Err("Dokumen tidak ditemukan.".to_string());
    };
    let (rel, name) = (value_to_string(&row[0]), value_to_string(&row[1]));
    let path = files.join(&rel);
    if !path.starts_with(files) {
        return Err("Path tidak valid.".to_string());
    }
    let bytes = std::fs::read(&path).map_err(|_| "Berkas hilang dari penyimpanan.".to_string())?;
    Ok(super::employees::DocumentBytes {
        mime: "application/octet-stream".to_string(),
        name,
        bytes,
    })
}

/// Terima kandidat: buat karyawan + onboarding. Satu kandidat satu karyawan.
pub async fn hire_sea(
    db: &sea_orm::DatabaseConnection,
    files: &std::path::Path,
    actor_id: i64,
    candidate_id: i64,
    join_date: Option<&str>,
) -> Result<i32, String> {
    let cand = q_one(
        db,
        "SELECT vacancy_id, full_name, gender, email, phone, birth_date, address, employee_id FROM candidates WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(candidate_id)],
        8,
        "recruit.hirecand",
    )
    .await
    .map_err(|e| format!("gagal memuat kandidat: {e}"))?;
    let Some(cand) = cand else {
        return Err("Kandidat tidak ditemukan.".to_string());
    };
    if value_i64(&cand[7]).is_some() {
        return Err("Kandidat ini sudah menjadi karyawan.".to_string());
    }
    let (vacancy_id, full_name) = (
        value_i64(&cand[0]).unwrap_or(0),
        value_to_string(&cand[1]),
    );
    let vac = q_one(
        db,
        "SELECT department_id, position_id, employment_type FROM vacancies WHERE id = ?1".to_string(),
        vec![Value::Int(vacancy_id)],
        3,
        "recruit.hirevac",
    )
    .await
    .map_err(|e| format!("gagal memuat lowongan: {e}"))?;
    let (dept, pos, etype) = match vac {
        Some(v) => (
            value_i64(&v[0]),
            value_i64(&v[1]),
            {
                let s = value_to_string(&v[2]);
                if s.is_empty() {
                    "contract".to_string()
                } else {
                    s
                }
            },
        ),
        None => (None, None, "contract".to_string()),
    };
    let company_row = q_one(
        db,
        "SELECT id FROM companies WHERE deleted_at IS NULL ORDER BY id LIMIT 1".to_string(),
        vec![],
        1,
        "recruit.hireco",
    )
    .await
    .map_err(|e| format!("gagal memuat perusahaan: {e}"))?;
    let Some(company_row) = company_row else {
        return Err("Perusahaan belum ada.".to_string());
    };
    let company = value_i64(&company_row[0]).unwrap_or(0);
    let join = join_date
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&chrono::Local::now().format("%Y-%m-%d").to_string())
        .to_string();
    NaiveDate::parse_from_str(&join, "%Y-%m-%d")
        .map_err(|_| "Tanggal masuk tidak valid.".to_string())?;
    let (first, last) = full_name
        .split_once(' ')
        .unwrap_or((full_name.as_str(), ""));
    let input = employees::EmployeeInput {
        first_name: first.to_string(),
        last_name: if last.is_empty() {
            None
        } else {
            Some(last.to_string())
        },
        gender: ropt_text(&cand[2]).unwrap_or_else(|| "male".to_string()),
        marital_status: "single".to_string(),
        company_id: to_dto_int(company, "hire.company")?,
        department_id: dept.map(|v| to_dto_int(v, "hire.dept")).transpose()?,
        position_id: pos.map(|v| to_dto_int(v, "hire.pos")).transpose()?,
        employment_type: etype,
        employment_status: "probation".to_string(),
        join_date: join.clone(),
        personal_email: ropt_text(&cand[3]),
        phone: ropt_text(&cand[4]),
        birth_date: ropt_text(&cand[5]),
        ..Default::default()
    };
    let employee_id = employees::create_sea(db, files, actor_id, &input, None).await?;
    exec(
        db,
        "UPDATE candidates SET stage = 'hired', employee_id = ?1 WHERE id = ?2".to_string(),
        vec![Value::Int(employee_id as i64), Value::Int(candidate_id)],
        "recruit.hiremark",
    )
    .await
    .map_err(|e| format!("gagal menandai hire: {e}"))?;
    log_stage_sea(
        db,
        candidate_id,
        "hired",
        Some("Diterima menjadi karyawan"),
        Some(actor_id),
    )
    .await?;
    super::onboarding::create_for_employee_sea(db, actor_id, employee_id as i64, None, &join)
        .await?;
    if let Some(uid) = approval::user_of_employee_sea(db, employee_id as i64).await? {
        approval::notify_sea(
            db,
            uid,
            "recruitment",
            "Selamat Bergabung",
            "Akun karyawan Anda telah dibuat.",
            "/employees",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "HIRE",
        "recruitment.candidate",
        Some(&candidate_id.to_string()),
        None,
        None,
        Some(&format!("employee {employee_id}")),
    )
    .await?;
    Ok(employee_id)
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

    fn admin(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn vacancy(conn: &Connection, actor: i64) -> i32 {
        vacancy_save(
            conn,
            actor,
            None,
            &VacancyInput {
                title: "Staff IT".to_string(),
                department_id: None,
                position_id: None,
                employment_type: "contract".to_string(),
                description: None,
                requirements: None,
                quota: 2,
                status: "open".to_string(),
                posted_date: None,
                closing_date: None,
            },
        )
        .expect("lowongan")
    }

    #[test]
    fn kandidat_mengalir_sampai_hire() {
        let (_d, pool, files) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let vid = vacancy(&conn, actor);
        assert_eq!(vacancy_list(&conn).expect("list").len(), 1);
        let cid = candidate_create(
            &conn,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "Citra Ayu".to_string(),
                email: Some("citra@x.local".to_string()),
                phone: None,
                birth_date: None,
                gender: Some("female".to_string()),
                address: None,
                source: Some("web".to_string()),
            },
            None,
        )
        .expect("kandidat");
        assert!(candidate_create(
            &conn,
            files.path(),
            actor,
            9999,
            &CandidateInput {
                full_name: "X".to_string(),
                ..Default::default()
            },
            None,
        )
        .is_err());
        update_stage(&conn, actor, cid as i64, "screening", None).expect("tahap");
        assert!(update_stage(&conn, actor, cid as i64, "ngawur", None).is_err());
        let iid = add_interview(
            &conn,
            actor,
            cid as i64,
            &InterviewInput {
                interviewer_id: None,
                schedule_at: "2026-10-01 09:00:00".to_string(),
                location: Some("Ruang 1".to_string()),
                interview_type: "hr".to_string(),
                notes: None,
            },
        )
        .expect("interview");
        decide_interview(&conn, actor, iid as i64, "pass", None).expect("lulus");
        assert!(decide_interview(&conn, actor, iid as i64, "bagus", None).is_err());
        add_assessment(
            &conn,
            actor,
            cid as i64,
            &AssessmentInput {
                assessment_name: "Tes logika".to_string(),
                score: Some(85.0),
                notes: None,
            },
        )
        .expect("assessment");
        let det = candidate_detail(&conn, cid as i64)
            .expect("det")
            .expect("ada");
        assert_eq!(det.interviews.len(), 1);
        assert_eq!(det.assessments.len(), 1);
        assert!(det.stage_history.len() >= 2);
        let eid = hire(&conn, files.path(), actor, cid as i64, Some("2026-10-06")).expect("hire");
        let det = candidate_detail(&conn, cid as i64)
            .expect("det")
            .expect("ada");
        assert_eq!(det.stage, "hired");
        assert_eq!(det.employee_id, Some(eid));
        assert!(hire(&conn, files.path(), actor, cid as i64, None).is_err());
        let status: String = conn
            .query_row(
                "SELECT employment_status FROM employees WHERE id = ?1",
                params![eid as i64],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "probation");
        let ob = crate::services::onboarding::for_employee(&conn, eid as i64)
            .expect("ob")
            .expect("ada");
        assert_eq!(ob.tasks.len(), 10);
        candidate_delete(&conn, actor, cid as i64).expect("hapus");
        assert!(candidate_detail(&conn, cid as i64).expect("det").is_none());
    }

    #[test]
    fn lowongan_bertuan_tidak_bisa_dihapus() {
        let (_d, pool, files) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let vid = vacancy(&conn, actor);
        candidate_create(
            &conn,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "Tamu".to_string(),
                ..Default::default()
            },
            None,
        )
        .expect("kandidat");
        assert!(vacancy_delete(&conn, actor, vid as i64).is_err());
    }

    #[tokio::test]
    async fn rekrutmen_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let vid = vacancy_save_sea(
            db,
            actor,
            None,
            &VacancyInput {
                title: "Staff IT".to_string(),
                department_id: None,
                position_id: None,
                employment_type: "contract".to_string(),
                description: None,
                requirements: None,
                quota: 2,
                status: "open".to_string(),
                posted_date: None,
                closing_date: None,
            },
        )
        .await
        .expect("lowongan");
        let v_sync = serde_json::to_string(&vacancy_list(&conn).expect("vs")).unwrap();
        let v_sea = serde_json::to_string(&vacancy_list_sea(db).await.expect("vse")).unwrap();
        assert_eq!(v_sync, v_sea);
        let cid = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "Citra Ayu".to_string(),
                email: Some("citra@x.local".to_string()),
                phone: None,
                birth_date: None,
                gender: Some("female".to_string()),
                address: None,
                source: Some("web".to_string()),
            },
            None,
        )
        .await
        .expect("kandidat");
        update_stage_sea(db, actor, cid as i64, "screening", None)
            .await
            .expect("tahap");
        let e_sea = update_stage_sea(db, actor, cid as i64, "ngawur", None)
            .await
            .expect_err("tahap salah");
        let e_sync = update_stage(&conn, actor, cid as i64, "ngawur", None)
            .expect_err("tahap salah sync");
        assert_eq!(e_sea, e_sync);
        let iid = add_interview_sea(
            db,
            actor,
            cid as i64,
            &InterviewInput {
                interviewer_id: None,
                schedule_at: "2026-10-01 09:00:00".to_string(),
                location: Some("Ruang 1".to_string()),
                interview_type: "hr".to_string(),
                notes: None,
            },
        )
        .await
        .expect("interview");
        decide_interview_sea(db, actor, iid as i64, "pass", None)
            .await
            .expect("lulus");
        add_assessment_sea(
            db,
            actor,
            cid as i64,
            &AssessmentInput {
                assessment_name: "Tes logika".to_string(),
                score: Some(85.0),
                notes: None,
            },
        )
        .await
        .expect("assessment");
        let c_sync =
            serde_json::to_string(&candidates_by_vacancy(&conn, vid as i64).expect("cs"))
                .unwrap();
        let c_sea =
            serde_json::to_string(&candidates_by_vacancy_sea(db, vid as i64).await.expect("cse"))
                .unwrap();
        assert_eq!(c_sync, c_sea);
        let d_sync =
            serde_json::to_string(&candidate_detail(&conn, cid as i64).expect("ds")).unwrap();
        let d_sea =
            serde_json::to_string(&candidate_detail_sea(db, cid as i64).await.expect("dse"))
                .unwrap();
        assert_eq!(d_sync, d_sea);
        let eid = hire_sea(db, files.path(), actor, cid as i64, Some("2026-10-06"))
            .await
            .expect("hire");
        let det = candidate_detail_sea(db, cid as i64)
            .await
            .expect("det")
            .expect("ada");
        assert_eq!(det.stage, "hired");
        assert_eq!(det.employee_id, Some(eid));
        let e2 = hire_sea(db, files.path(), actor, cid as i64, None)
            .await
            .expect_err("hire ganda");
        assert!(e2.contains("sudah menjadi karyawan"));
        let ob = crate::services::onboarding::for_employee_sea(db, eid as i64)
            .await
            .expect("ob")
            .expect("ada");
        assert_eq!(ob.tasks.len(), 10);
        candidate_delete_sea(db, actor, cid as i64).await.expect("hapus");
        assert!(candidate_detail_sea(db, cid as i64)
            .await
            .expect("det")
            .is_none());
    }
}
