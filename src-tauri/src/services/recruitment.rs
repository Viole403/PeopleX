//! Rekrutmen: lowongan, kandidat, tahap, interview, assessment, hire.

use chrono::NaiveDate;
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

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CareerVacancy {
    pub id: i32,
    pub title: String,
    pub department_name: Option<String>,
    pub position_name: Option<String>,
    pub employment_type: String,
    pub description: Option<String>,
    pub requirements: Option<String>,
    pub quota: i32,
    pub closing_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct CareerApplyInput {
    pub full_name: String,
    pub email: String,
    pub phone: Option<String>,
    pub address: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct OfferLetter {
    pub id: i32,
    pub candidate_id: i32,
    pub title: String,
    pub message: String,
    pub status: String,
    pub recipient_name: String,
    pub signature_name: Option<String>,
    pub signature_hash: Option<String>,
    pub sent_at: Option<String>,
    pub signed_at: Option<String>,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct OfferInput {
    pub candidate_id: i32,
    pub title: String,
    pub message: String,
    pub recipient_name: String,
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

// ---------------- Kandidat ----------------

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

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};

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
        let rid = exec_insert(
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
    let rid = exec_insert(
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

pub async fn career_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<CareerVacancy>, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let rows = q_all(
        db,
        "SELECT v.id, v.title, d.name, p.name, v.employment_type, v.description, v.requirements, v.quota, v.closing_date FROM vacancies v LEFT JOIN departments d ON d.id = v.department_id LEFT JOIN positions p ON p.id = v.position_id WHERE v.deleted_at IS NULL AND v.status = 'open' AND (v.closing_date IS NULL OR v.closing_date >= ?1) ORDER BY v.created_at DESC".to_string(),
        vec![Value::Text(today)],
        9,
        "career.list",
    )
    .await
    .map_err(|e| format!("gagal membaca lowongan publik: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(CareerVacancy {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "career.id")?,
            title: value_to_string(&r[1]),
            department_name: ropt_text(&r[2]),
            position_name: ropt_text(&r[3]),
            employment_type: value_to_string(&r[4]),
            description: ropt_text(&r[5]),
            requirements: ropt_text(&r[6]),
            quota: to_dto_int(value_i64(&r[7]).unwrap_or(0), "career.quota")?,
            closing_date: ropt_text(&r[8]),
        });
    }
    Ok(out)
}

pub async fn career_apply_sea(
    db: &sea_orm::DatabaseConnection,
    files: &std::path::Path,
    vacancy_id: i64,
    input: &CareerApplyInput,
    cv: Option<&employees::FileUpload>,
) -> Result<i32, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let vac = q_one(
        db,
        "SELECT id FROM vacancies WHERE id = ?1 AND deleted_at IS NULL AND status = 'open' AND (closing_date IS NULL OR closing_date >= ?2)"
            .to_string(),
        vec![Value::Int(vacancy_id), Value::Text(today)],
        1,
        "career.vaccheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa lowongan: {e}"))?;
    if vac.is_none() {
        return Err("Lowongan tidak tersedia.".to_string());
    }
    if input.full_name.trim().is_empty() {
        return Err("Nama lengkap wajib diisi.".to_string());
    }
    if input.full_name.len() > 150 {
        return Err("Nama maksimal 150 karakter.".to_string());
    }
    let email = input.email.trim();
    if email.is_empty() {
        return Err("Email wajib diisi.".to_string());
    }
    if !email.contains('@') {
        return Err("Email tidak valid.".to_string());
    }
    let dup = q_one(
        db,
        "SELECT id FROM candidates WHERE vacancy_id = ?1 AND lower(email) = lower(?2) AND deleted_at IS NULL"
            .to_string(),
        vec![Value::Int(vacancy_id), Value::Text(email.to_string())],
        1,
        "career.dupcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa lamaran: {e}"))?;
    if dup.is_some() {
        return Err("Anda sudah melamar lowongan ini.".to_string());
    }
    let cv_path = match cv {
        Some(f) => Some(store_cv(files, f)?),
        None => None,
    };
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    let rid = exec_insert(
        db,
        "INSERT INTO candidates (vacancy_id, full_name, email, phone, address, cv_path, source, stage) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'career-page', 'applied')".to_string(),
        vec![
            Value::Int(vacancy_id),
            Value::Text(input.full_name.trim().to_string()),
            Value::Text(email.to_string()),
            opt_t(input.phone.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.address.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(cv_path.as_deref()),
        ],
        "career.apply",
    )
    .await
    .map_err(|e| format!("gagal mengirim lamaran: {e}"))?;

    log_stage_sea(db, rid, "applied", Some("Lamaran lewat halaman karir"), None).await?;
    audit::log_sea(
        db,
        None,
        "CREATE",
        "career.apply",
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
    let rid = exec_insert(
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
    let rid = exec_insert(
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

pub async fn offer_save_sea(db: &sea_orm::DatabaseConnection, actor:i64, id:Option<i32>, input:&OfferInput)->Result<i32,String>{
    use super::sea_raw::{exec, exec_insert, q_one, value_i64, Value};
    let judul = input.title.trim();
    if judul.is_empty() || judul.chars().count() > 200 { return Err("Judul tawaran wajib diisi sampai 200 karakter.".to_string()); }
    let penerima = input.recipient_name.trim();
    if penerima.is_empty() || penerima.chars().count() > 120 { return Err("Nama penerima wajib diisi sampai 120 karakter.".to_string()); }
    if input.message.chars().count() > 4000 { return Err("Isi tawaran maksimal 4000 karakter.".to_string()); }
    if input.message.trim().is_empty() { return Err("Isi tawaran wajib diisi.".to_string()); }
    let ada = q_one(db, "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL".to_string(), vec![Value::Int(input.candidate_id as i64)], 1, "offer.candidate").await?;
    if ada.is_none() { return Err("Kandidat tidak ditemukan.".to_string()); }
    match id {
        None => {
            let now = now_str();
            let rid = exec_insert(db, "INSERT INTO offer_letters (candidate_id, title, message, status, recipient_name, created_at, updated_at) VALUES (?1, ?2, ?3, 'draft', ?4, ?5, ?5)".to_string(), vec![Value::Int(input.candidate_id as i64), Value::Text(judul.to_string()), Value::Text(input.message.clone()), Value::Text(penerima.to_string()), Value::Text(now)], "offer.save").await?;
            audit::log_sea(db, Some(actor), "CREATE", "recruitment.offer", Some(&rid.to_string()), None, None, None).await?;
            to_dto_int(rid, "id tawaran")
        }
        Some(_existing) => {
            let baris = tawaran_by_id(db, _existing).await?.ok_or("Tawaran tidak ditemukan.")?;
            if baris.status != "draft" { return Err("Hanya tawaran berstatus draf dapat diubah.".to_string()); }
            exec(db, "UPDATE offer_letters SET title = ?1, message = ?2, recipient_name = ?3, updated_at = ?4 WHERE id = ?5".to_string(), vec![Value::Text(judul.to_string()), Value::Text(input.message.clone()), Value::Text(penerima.to_string()), Value::Text(now_str()), Value::Int(_existing as i64)], "offer.update").await?;
            audit::log_sea(db, Some(actor), "UPDATE", "recruitment.offer", Some(&_existing.to_string()), None, None, None).await?;
            Ok(_existing)
        }
    }
}

pub async fn offer_send_sea(db: &sea_orm::DatabaseConnection, actor:i64, id:i32)->Result<(),String>{
    use super::sea_raw::{exec, q_one, Value};
    let baris = tawaran_by_id(db, id).await?.ok_or("Tawaran tidak ditemukan.")?;
    if baris.status != "draft" { return Err("Hanya tawaran berstatus draf dapat dikirim.".to_string()); }
    exec(db, "UPDATE offer_letters SET status = 'sent', sent_at = ?1, updated_at = ?1 WHERE id = ?2".to_string(), vec![Value::Text(now_str()), Value::Int(id as i64)], "offer.send").await?;
    audit::log_sea(db, Some(actor), "UPDATE", "recruitment.offer.send", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn offer_sign_sea(db: &sea_orm::DatabaseConnection, id:i32, signature_name:&str)->Result<(),String>{
    use super::sea_raw::{exec, Value};
    let nama = signature_name.trim();
    if nama.is_empty() || nama.chars().count() > 120 { return Err("Nama tanda tangan wajib diisi sampai 120 karakter.".to_string()); }
    let baris = tawaran_by_id(db, id).await?.ok_or("Tawaran tidak ditemukan.")?;
    if baris.status != "sent" { return Err("Hanya tawaran terkirim dapat ditandatangani.".to_string()); }
    let signed = now_str();
    let hash = sha256_hex(&format!("{}|{}|{}|{}", baris.id, baris.candidate_id, nama, signed));
    exec(db, "UPDATE offer_letters SET status = 'signed', signature_name = ?1, signature_hash = ?2, signed_at = ?3, updated_at = ?3 WHERE id = ?4".to_string(), vec![Value::Text(nama.to_string()), Value::Text(hash), Value::Text(signed), Value::Int(id as i64)], "offer.sign").await?;
    audit::log_sea(db, None, "UPDATE", "recruitment.offer.sign", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn offer_decline_sea(db: &sea_orm::DatabaseConnection, id:i32)->Result<(),String>{
    use super::sea_raw::{exec, Value};
    let baris = tawaran_by_id(db, id).await?.ok_or("Tawaran tidak ditemukan.")?;
    if baris.status != "sent" { return Err("Hanya tawaran terkirim dapat ditolak.".to_string()); }
    exec(db, "UPDATE offer_letters SET status = 'declined', updated_at = ?1 WHERE id = ?2".to_string(), vec![Value::Text(now_str()), Value::Int(id as i64)], "offer.decline").await?;
    Ok(())
}

pub async fn offer_get_sea(db: &sea_orm::DatabaseConnection, id:i32)->Result<Option<OfferLetter>,String>{
    tawaran_by_id(db, id).await
}

pub async fn offer_list_sea(db: &sea_orm::DatabaseConnection, candidate_id:i32)->Result<Vec<OfferLetter>,String>{
    use super::sea_raw::{q_all, Value};
    let rows = q_all(db, format!("{TAWARAN_SELECT} WHERE candidate_id = ?1 ORDER BY id DESC"), vec![Value::Int(candidate_id as i64)], 11, "offer.list").await?;
    Ok(rows.iter().map(tawaran_row).collect())
}

pub async fn offer_verify_sea(db: &sea_orm::DatabaseConnection, id:i32)->Result<bool,String>{
    let baris = tawaran_by_id(db, id).await?.ok_or("Tawaran tidak ditemukan.")?;
    match (&baris.signature_name, &baris.signature_hash, &baris.signed_at) {
        (Some(n), Some(h), Some(t)) => Ok(sha256_hex(&format!("{}|{}|{}|{}", baris.id, baris.candidate_id, n, t)) == *h),
        _ => Err("Tawaran belum ditandatangani.".to_string()),
    }
}

fn now_str()->String{ chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string() }

fn sha256_hex(teks:&str)->String{
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(teks.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

async fn tawaran_by_id(db: &sea_orm::DatabaseConnection, id:i32)->Result<Option<OfferLetter>,String>{
    use super::sea_raw::{q_one, Value};
    let row = q_one(db, format!("{TAWARAN_SELECT} WHERE id = ?1"), vec![Value::Int(id as i64)], 11, "offer.get").await?;
    Ok(row.as_ref().map(tawaran_row))
}

const TAWARAN_SELECT:&str = "SELECT id, candidate_id, title, message, status, recipient_name, signature_name, signature_hash, sent_at, signed_at, created_at FROM offer_letters";

fn tawaran_row(r:&Vec<super::sea_raw::Value>)->OfferLetter{
    use super::sea_raw::{value_i64, value_to_string};
    OfferLetter {
        id: value_i64(&r[0]).unwrap_or_default() as i32,
        candidate_id: value_i64(&r[1]).unwrap_or_default() as i32,
        title: value_to_string(&r[2]),
        message: value_to_string(&r[3]),
        status: value_to_string(&r[4]),
        recipient_name: value_to_string(&r[5]),
        signature_name: ropt_text(&r[6]),
        signature_hash: ropt_text(&r[7]),
        sent_at: ropt_text(&r[8]),
        signed_at: ropt_text(&r[9]),
        created_at: value_to_string(&r[10]),
    }
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct BackgroundCheck {
    pub id: i32,
    pub candidate_id: i32,
    pub kind: String,
    pub status: String,
    pub result: Option<String>,
    pub checked_by: Option<i32>,
    pub checked_at: Option<String>,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct BgInput {
    pub kind: String,
    pub status: String,
    pub result: Option<String>,
}

fn bopt_i(v: &Value) -> Option<i32> {
    match v {
        Value::Int(i) => Some(*i as i32),
        _ => None,
    }
}

fn bopt_t(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn bg_row(r: &[Value]) -> BackgroundCheck {
    BackgroundCheck {
        id: bopt_i(&r[0]).unwrap_or(0),
        candidate_id: bopt_i(&r[1]).unwrap_or(0),
        kind: value_to_string(&r[2]),
        status: value_to_string(&r[3]),
        result: bopt_t(&r[4]),
        checked_by: bopt_i(&r[5]),
        checked_at: bopt_t(&r[6]),
        created_at: value_to_string(&r[7]),
    }
}

pub async fn bg_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    candidate_id: i64,
    id: Option<i64>,
    input: &BgInput,
) -> Result<i32, String> {
    const JENIS: [&str; 4] = ["criminal", "education", "employment", "reference"];
    const STATUS: [&str; 3] = ["pending", "clear", "flagged"];
    if !JENIS.contains(&input.kind.as_str()) {
        return Err("Jenis pemeriksaan tidak valid.".to_string());
    }
    if !STATUS.contains(&input.status.as_str()) {
        return Err("Status pemeriksaan tidak valid.".to_string());
    }
    let ada = q_one(
        db,
        "SELECT id FROM candidates WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(candidate_id)],
        1,
        "bg.cand",
    )
    .await?;
    if ada.is_none() {
        return Err("Kandidat tidak ditemukan.".to_string());
    }
    let sekarang = now_str();
    if let Some(rid) = id {
        let milik = q_one(
            db,
            "SELECT id FROM background_checks WHERE id = ?1 AND candidate_id = ?2".to_string(),
            vec![Value::Int(rid), Value::Int(candidate_id)],
            1,
            "bg.own",
        )
        .await?;
        if milik.is_none() {
            return Err("Pemeriksaan tidak ditemukan.".to_string());
        }
        exec(
            db,
            "UPDATE background_checks SET kind = ?1, status = ?2, result = ?3, checked_by = ?4, checked_at = ?5, updated_at = ?6 WHERE id = ?7".to_string(),
            vec![
                Value::Text(input.kind.clone()),
                Value::Text(input.status.clone()),
                input.result.as_ref().map(|s| Value::Text(s.clone())).unwrap_or(Value::Null),
                Value::Int(actor_id),
                Value::Text(sekarang.clone()),
                Value::Text(sekarang),
                Value::Int(rid),
            ],
            "bg.upd",
        )
        .await?;
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "recruitment.background",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await;
        return to_dto_int(rid, "bg.id");
    }
    let rid = exec_insert(
        db,
        "INSERT INTO background_checks (candidate_id, kind, status, result, checked_by, checked_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)".to_string(),
        vec![
            Value::Int(candidate_id),
            Value::Text(input.kind.clone()),
            Value::Text(input.status.clone()),
            input.result.as_ref().map(|s| Value::Text(s.clone())).unwrap_or(Value::Null),
            Value::Int(actor_id),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang),
        ],
        "bg.ins",
    )
    .await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "recruitment.background",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await;
    to_dto_int(rid, "bg.id")
}

pub async fn bg_list_sea(
    db: &sea_orm::DatabaseConnection,
    candidate_id: i64,
) -> Result<Vec<BackgroundCheck>, String> {
    let rows = q_all(
        db,
        "SELECT id, candidate_id, kind, status, result, checked_by, checked_at, created_at FROM background_checks WHERE candidate_id = ?1 ORDER BY id".to_string(),
        vec![Value::Int(candidate_id)],
        8,
        "bg.list",
    )
    .await?;
    Ok(rows.iter().map(|r| bg_row(r)).collect())
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PipelineRow {
    pub stage: String,
    pub count: i32,
}

pub async fn pipeline_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<PipelineRow>, String> {
    let rows = q_all(
        db,
        "SELECT stage, COUNT(*) FROM candidates WHERE deleted_at IS NULL GROUP BY stage".to_string(),
        vec![],
        2,
        "rec.pipeline",
    )
    .await
    .map_err(|e| format!("gagal membaca pipeline: {e}"))?;
    Ok(STAGES
        .iter()
        .map(|st| PipelineRow {
            stage: st.to_string(),
            count: rows
                .iter()
                .find(|r| value_to_string(&r[0]) == *st)
                .and_then(|r| value_i64(&r[1]))
                .unwrap_or(0) as i32,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn admin(db: &sea_orm::DatabaseConnection) -> i64 {
        let row = q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "test.admin",
        )
        .await
        .expect("admin")
        .expect("baris admin");
        value_i64(&row[0]).expect("id admin")
    }

    async fn text(db: &sea_orm::DatabaseConnection, sql: &str) -> String {
        let row = q_one(db, sql.to_string(), vec![], 1, "test.q")
            .await
            .expect("q")
            .expect("baris");
        value_to_string(&row[0])
    }

    fn vac_input() -> VacancyInput {
        VacancyInput {
            title: "Staff IT".to_string(),
            employment_type: "contract".to_string(),
            quota: 2,
            status: "open".to_string(),
            ..Default::default()
        }
    }

    async fn vacancy(db: &sea_orm::DatabaseConnection, actor: i64) -> i32 {
        vacancy_save_sea(db, actor, None, &vac_input())
            .await
            .expect("lowongan")
    }

    async fn kandidat(
        db: &sea_orm::DatabaseConnection,
        files: &std::path::Path,
        actor: i64,
        vid: i32,
        name: &str,
    ) -> i32 {
        candidate_create_sea(
            db,
            files,
            actor,
            vid as i64,
            &CandidateInput {
                full_name: name.to_string(),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("kandidat")
    }

    #[tokio::test]
    async fn kandidat_mengalir_sampai_hire_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let vid = vacancy(db, actor).await;
        assert_eq!(vacancy_list_sea(db).await.expect("list").len(), 1);
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
        assert!(candidate_create_sea(
            db,
            files.path(),
            actor,
            9999,
            &CandidateInput {
                full_name: "X".to_string(),
                ..Default::default()
            },
            None,
        )
        .await
        .is_err());
        update_stage_sea(db, actor, cid as i64, "screening", None)
            .await
            .expect("tahap");
        let e = update_stage_sea(db, actor, cid as i64, "ngawur", None)
            .await
            .expect_err("tahap salah");
        assert!(e.contains("Tahap tidak valid"));
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
        let e = decide_interview_sea(db, actor, iid as i64, "bagus", None)
            .await
            .expect_err("hasil salah");
        assert!(e.contains("Hasil tidak valid"));
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
        let det = candidate_detail_sea(db, cid as i64)
            .await
            .expect("det")
            .expect("ada");
        assert_eq!(det.interviews.len(), 1);
        assert_eq!(det.interviews[0].result, "pass");
        assert_eq!(det.assessments.len(), 1);
        assert_eq!(det.assessments[0].score, Some(85.0));
        assert!(det.stage_history.len() >= 2);
        let eid = hire_sea(db, files.path(), actor, cid as i64, Some("2026-10-06"))
            .await
            .expect("hire");
        let det = candidate_detail_sea(db, cid as i64)
            .await
            .expect("det")
            .expect("ada");
        assert_eq!(det.stage, "hired");
        assert_eq!(det.employee_id, Some(eid));
        let e = hire_sea(db, files.path(), actor, cid as i64, None)
            .await
            .expect_err("hire ganda");
        assert!(e.contains("sudah menjadi karyawan"));
        assert_eq!(
            text(db, &format!("SELECT employment_status FROM employees WHERE id = {eid}"))
                .await,
            "probation"
        );
        let ob = crate::services::onboarding::for_employee_sea(db, eid as i64)
            .await
            .expect("ob")
            .expect("ada");
        assert_eq!(ob.tasks.len(), 10);
        candidate_delete_sea(db, actor, cid as i64)
            .await
            .expect("hapus");
        assert!(candidate_detail_sea(db, cid as i64)
            .await
            .expect("det")
            .is_none());
    }

    #[tokio::test]
    async fn lowongan_bertuan_tidak_bisa_dihapus_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let vid = vacancy(db, actor).await;
        let cid = kandidat(db, files.path(), actor, vid, "Tamu").await;
        let e = vacancy_delete_sea(db, actor, vid as i64)
            .await
            .expect_err("masih punya kandidat");
        assert!(e.contains("masih memiliki kandidat"));
        candidate_delete_sea(db, actor, cid as i64)
            .await
            .expect("hapus kandidat");
        vacancy_delete_sea(db, actor, vid as i64)
            .await
            .expect("hapus lowongan");
        assert!(vacancy_get_sea(db, vid as i64)
            .await
            .expect("get")
            .is_none());
    }

    #[tokio::test]
    async fn validasi_batas_lowongan_dan_kandidat_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;

        let mut input = vac_input();
        input.title = "   ".to_string();
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("judul kosong");
        assert!(e.contains("Judul lowongan wajib diisi"));
        let mut input = vac_input();
        input.title = "x".repeat(151);
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("judul kepanjangan");
        assert!(e.contains("Judul maksimal 150 karakter"));
        let mut input = vac_input();
        input.employment_type = "magang".to_string();
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("jenis kepegawaian");
        assert!(e.contains("Jenis kepegawaian tidak valid"));
        let mut input = vac_input();
        input.status = "draft".to_string();
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("status");
        assert!(e.contains("Status tidak valid"));
        let mut input = vac_input();
        input.quota = 0;
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("kuota nol");
        assert!(e.contains("Kuota 1-1000"));
        input.quota = 1001;
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("kuota lewat");
        assert!(e.contains("Kuota 1-1000"));
        let mut input = vac_input();
        input.posted_date = Some("2026-02-30".to_string());
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("tanggal pasang");
        assert!(e.contains("Tanggal pasang harus valid"));
        let mut input = vac_input();
        input.closing_date = Some("bukan-tanggal".to_string());
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("tanggal tutup");
        assert!(e.contains("Tanggal tutup harus valid"));
        let mut input = vac_input();
        input.department_id = Some(9999);
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("departemen asing");
        assert!(e.contains("Departemen tidak ditemukan"));
        let mut input = vac_input();
        input.position_id = Some(9999);
        let e = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect_err("jabatan asing");
        assert!(e.contains("Jabatan tidak ditemukan"));
        let mut input = vac_input();
        input.quota = 1000;
        let vid = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect("kuota batas atas");

        let e = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "  ".to_string(),
                ..Default::default()
            },
            None,
        )
        .await
        .expect_err("nama kosong");
        assert!(e.contains("Nama lengkap wajib diisi"));
        let e = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "y".repeat(151),
                ..Default::default()
            },
            None,
        )
        .await
        .expect_err("nama kepanjangan");
        assert!(e.contains("Nama maksimal 150 karakter"));
        let e = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "Sah".to_string(),
                email: Some("tanpa-at".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect_err("email");
        assert!(e.contains("Email tidak valid"));
        let e = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "Sah".to_string(),
                birth_date: Some("2026-13-01".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect_err("tanggal lahir");
        assert!(e.contains("Tanggal lahir harus valid"));
        let e = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "Sah".to_string(),
                gender: Some("other".to_string()),
                ..Default::default()
            },
            None,
        )
        .await
        .expect_err("gender");
        assert!(e.contains("Jenis kelamin tidak valid"));
        let cid = candidate_create_sea(
            db,
            files.path(),
            actor,
            vid as i64,
            &CandidateInput {
                full_name: "z".repeat(150),
                ..Default::default()
            },
            None,
        )
        .await
        .expect("nama batas 150");

        let e = update_stage_sea(db, actor, 9999, "screening", None)
            .await
            .expect_err("kandidat asing");
        assert!(e.contains("Kandidat tidak ditemukan"));
        let e = add_interview_sea(
            db,
            actor,
            cid as i64,
            &InterviewInput {
                interviewer_id: None,
                schedule_at: "2026-10-02 10:00:00".to_string(),
                location: None,
                interview_type: "psikotes".to_string(),
                notes: None,
            },
        )
        .await
        .expect_err("jenis interview");
        assert!(e.contains("Jenis interview tidak valid"));
        let e = add_interview_sea(
            db,
            actor,
            cid as i64,
            &InterviewInput {
                interviewer_id: Some(9999),
                schedule_at: "2026-10-02 10:00:00".to_string(),
                location: None,
                interview_type: "hr".to_string(),
                notes: None,
            },
        )
        .await
        .expect_err("pewawancara asing");
        assert!(e.contains("Pewawancara tidak ditemukan"));
        let e = decide_interview_sea(db, actor, 9999, "pass", None)
            .await
            .expect_err("interview asing");
        assert!(e.contains("Interview tidak ditemukan"));
        let e = add_assessment_sea(
            db,
            actor,
            cid as i64,
            &AssessmentInput {
                assessment_name: " ".to_string(),
                score: None,
                notes: None,
            },
        )
        .await
        .expect_err("nama assessment kosong");
        assert!(e.contains("Nama assessment wajib diisi"));
        let e = add_assessment_sea(
            db,
            actor,
            cid as i64,
            &AssessmentInput {
                assessment_name: "a".repeat(151),
                score: None,
                notes: None,
            },
        )
        .await
        .expect_err("nama assessment panjang");
        assert!(e.contains("Nama assessment maksimal 150"));
        let e = add_assessment_sea(
            db,
            actor,
            9999,
            &AssessmentInput {
                assessment_name: "Tes".to_string(),
                score: None,
                notes: None,
            },
        )
        .await
        .expect_err("kandidat asing");
        assert!(e.contains("Kandidat tidak ditemukan"));
    }

    #[tokio::test]
    async fn halaman_karir_menampilkan_dan_menerima_lamaran() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let buka = vacancy(db, actor).await;

        let mut input = vac_input();
        input.status = "closed".to_string();
        let ditutup = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect("lowongan closed");

        let mut input = vac_input();
        input.title = "Pustakawan".to_string();
        input.closing_date = Some("2020-01-01".to_string());
        let lewat = vacancy_save_sea(db, actor, None, &input)
            .await
            .expect("lowongan lewat");

        let daftar = career_list_sea(db).await.expect("daftar karir");
        assert_eq!(daftar.len(), 1);
        assert_eq!(daftar[0].id, buka);
        assert!(!daftar.iter().any(|v| v.id == ditutup));
        assert!(!daftar.iter().any(|v| v.id == lewat));

        let cv = crate::services::employees::FileUpload {
            name: "cv-sinta.pdf".to_string(),
            mime: "application/pdf".to_string(),
            bytes: b"%PDF-1.4 lamaran karir".to_vec(),
        };
        let rid = career_apply_sea(
            db,
            files.path(),
            buka as i64,
            &CareerApplyInput {
                full_name: "Sinta Dewi".to_string(),
                email: "Sinta@Contoh.ID".to_string(),
                phone: Some("0812".to_string()),
                address: Some("Surabaya".to_string()),
            },
            Some(&cv),
        )
        .await
        .expect("lamar");
        assert!(rid > 0);
        assert_eq!(
            text(db, &format!("SELECT source FROM candidates WHERE id = {rid}")).await,
            "career-page"
        );
        let cv_path = text(db, &format!("SELECT cv_path FROM candidates WHERE id = {rid}")).await;
        assert!(cv_path.starts_with("candidates/"));

        let e = career_apply_sea(
            db,
            files.path(),
            buka as i64,
            &CareerApplyInput {
                full_name: "Sinta Dewi".to_string(),
                email: "sinta@contoh.id".to_string(),
                phone: None,
                address: None,
            },
            None,
        )
        .await
        .expect_err("email duplikat beda kapital");
        assert!(e.contains("sudah melamar"));

        let e = career_apply_sea(
            db,
            files.path(),
            ditutup as i64,
            &CareerApplyInput {
                full_name: "Budi".to_string(),
                email: "budi@contoh.id".to_string(),
                phone: None,
                address: None,
            },
            None,
        )
        .await
        .expect_err("lowongan closed");
        assert!(e.contains("Lowongan tidak tersedia"));

        let e = career_apply_sea(
            db,
            files.path(),
            buka as i64,
            &CareerApplyInput {
                full_name: "Cica".to_string(),
                email: "bukan-email".to_string(),
                phone: None,
                address: None,
            },
            None,
        )
        .await
        .expect_err("email tanpa @");
        assert!(e.contains("Email tidak valid"));
    }

    #[tokio::test]
    async fn tawaran_digital_ditandatangani_dan_diverifikasi() {
        let dir = tempfile::tempdir().expect("dir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let vid = vacancy(db, actor).await;
        let cid = kandidat(db, files.path(), actor, vid, "Rina Kartika").await;
        let input = OfferInput {
            candidate_id: cid,
            title: "Tawaran Kerja Staff IT".to_string(),
            message: "Selamat, kami menawarkan posisi Staff IT.".to_string(),
            recipient_name: "Rina Kartika".to_string(),
        };
        let oid = offer_save_sea(db, actor, None, &input).await.expect("simpan");
        let draf = offer_get_sea(db, oid).await.expect("get").expect("ada");
        assert_eq!(draf.status, "draft");
        assert!(draf.signature_hash.is_none());
        let e = offer_sign_sea(db, oid, "Rina").await.expect_err("ttd sebelum kirim");
        assert!(e.contains("terkirim"));
        offer_send_sea(db, actor, oid).await.expect("kirim");
        let terkirim = offer_get_sea(db, oid).await.expect("get2").expect("ada2");
        assert_eq!(terkirim.status, "sent");
        assert!(terkirim.sent_at.is_some());
        let e = offer_save_sea(db, actor, Some(oid), &input).await.expect_err("ubah terkirim");
        assert!(e.contains("draf"));
        offer_sign_sea(db, oid, "Rina Kartika").await.expect("ttd");
        let bertanda = offer_get_sea(db, oid).await.expect("get3").expect("ada3");
        assert_eq!(bertanda.status, "signed");
        let hash = bertanda.signature_hash.clone().expect("hash");
        assert_eq!(hash.len(), 64);
        assert!(offer_verify_sea(db, oid).await.expect("verifikasi"));
        let e = offer_sign_sea(db, oid, "Rina").await.expect_err("ttd dua kali");
        assert!(e.contains("terkirim"));
        let e = offer_save_sea(db, actor, Some(oid), &input).await.expect_err("ubah bertanda");
        assert!(e.contains("draf"));
        super::super::sea_raw::exec(
            db,
            format!("UPDATE offer_letters SET signature_name = 'Palsu' WHERE id = {oid}"),
            vec![],
            "test.paksa",
        )
        .await
        .expect("ubah paksa");
        assert!(!offer_verify_sea(db, oid).await.expect("verifikasi palsu"));
        assert_eq!(offer_list_sea(db, cid).await.expect("list").len(), 1);
        let cid2 = kandidat(db, files.path(), actor, vid, "Dodi Pratama").await;
        let oid2 = offer_save_sea(db, actor, None, &OfferInput { candidate_id: cid2, ..input.clone() })
            .await
            .expect("draf2");
        offer_send_sea(db, actor, oid2).await.expect("kirim2");
        offer_decline_sea(db, actor, oid2).await.expect("tolak");
        let ditolak = offer_get_sea(db, oid2).await.expect("get4").expect("ada4");
        assert_eq!(ditolak.status, "declined");
        let e = offer_verify_sea(db, oid2).await.expect_err("belum ttd");
        assert!(e.contains("belum"));
        let e = offer_save_sea(db, actor, None, &OfferInput { candidate_id: 999999, ..input.clone() })
            .await
            .expect_err("kandidat asing");
        assert!(e.contains("Kandidat tidak ditemukan"));
    }

    #[tokio::test]
    async fn pemeriksaan_latar_mencatat_hasil() {
        let dir = tempfile::tempdir().expect("dir");
        let files = tempfile::tempdir().expect("files");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin(db).await;
        let vid = vacancy(db, actor).await;
        let cid = kandidat(db, files.path(), actor, vid, "Cek").await as i64;
        let bid = bg_save_sea(
            db,
            actor,
            cid,
            None,
            &BgInput {
                kind: "criminal".to_string(),
                status: "pending".to_string(),
                result: None,
            },
        )
        .await
        .expect("buat");
        let daftar = bg_list_sea(db, cid).await.expect("daftar");
        assert_eq!(daftar.len(), 1);
        assert_eq!(daftar[0].status, "pending");
        bg_save_sea(
            db,
            actor,
            cid,
            Some(bid as i64),
            &BgInput {
                kind: "criminal".to_string(),
                status: "clear".to_string(),
                result: Some("BERSIH".to_string()),
            },
        )
        .await
        .expect("ubah");
        let daftar = bg_list_sea(db, cid).await.expect("daftar2");
        assert_eq!(daftar.len(), 1);
        assert_eq!(daftar[0].status, "clear");
        assert_eq!(daftar[0].result.as_deref(), Some("BERSIH"));
        let err = bg_save_sea(
            db,
            actor,
            cid,
            None,
            &BgInput {
                kind: "hantu".to_string(),
                status: "clear".to_string(),
                result: None,
            },
        )
        .await
        .expect_err("jenis");
        assert!(err.contains("Jenis pemeriksaan tidak valid"));
        let err = bg_save_sea(
            db,
            actor,
            999999,
            None,
            &BgInput {
                kind: "reference".to_string(),
                status: "pending".to_string(),
                result: None,
            },
        )
        .await
        .expect_err("kandidat");
        assert!(err.contains("Kandidat tidak ditemukan"));
    }

    #[tokio::test]
    async fn pipeline_menghitung_setiap_tahap() {
        let dir = tempfile::tempdir().expect("dir");
        let fdir = tempfile::tempdir().expect("fdir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin(db).await;
        let vid = vacancy(db, actor).await;
        let c1 = kandidat(db, fdir.path(), actor, vid, "Satu") as i64;
        let _c2 = kandidat(db, fdir.path(), actor, vid, "Dua");
        let rows = pipeline_sea(db).await.expect("pipeline");
        assert_eq!(rows.len(), 8);
        assert_eq!(rows.iter().find(|r| r.stage == "applied").expect("applied").count, 2);
        assert_eq!(rows.iter().find(|r| r.stage == "screening").expect("scr").count, 0);
        update_stage_sea(db, actor, c1, "screening", None)
            .await
            .expect("tahap");
        let rows2 = pipeline_sea(db).await.expect("pipeline2");
        assert_eq!(rows2.iter().find(|r| r.stage == "applied").expect("a").count, 1);
        assert_eq!(rows2.iter().find(|r| r.stage == "screening").expect("s").count, 1);
    }
}
