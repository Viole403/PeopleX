//! Training: katalog, peserta, sertifikasi, skill matrix.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use super::audit;
use crate::to_dto_int;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Training {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub trainer_name: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub location: Option<String>,
    pub cost: f64,
    pub quota: Option<i32>,
    pub status: String,
    pub participant_count: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct TrainingInput {
    pub title: String,
    pub description: Option<String>,
    pub trainer_name: Option<String>,
    pub start_date: String,
    pub end_date: String,
    pub location: Option<String>,
    pub cost: f64,
    pub quota: Option<i32>,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Participant {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub status: String,
    pub quiz_score: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Certification {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub name: String,
    pub issuer: Option<String>,
    pub certificate_number: Option<String>,
    pub issued_date: Option<String>,
    pub expiry_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct CertificationInput {
    pub employee_id: i32,
    pub name: String,
    pub issuer: Option<String>,
    pub certificate_number: Option<String>,
    pub issued_date: Option<String>,
    pub expiry_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct SkillCell {
    pub employee_id: i32,
    pub employee_name: String,
    pub skill_name: String,
    pub level: i32,
}

const TRAINING_STATUS: &[&str] = &["scheduled", "ongoing", "completed", "cancelled"];
const PARTICIPANT_STATUS: &[&str] = &["registered", "attended", "absent", "completed"];

// ---------------- Katalog ----------------

pub fn list(conn: &Connection) -> Result<Vec<Training>, String> {
    let mut stmt = conn
        .prepare("SELECT t.id, t.title, t.description, t.trainer_name, t.start_date, t.end_date, t.location, t.cost, t.quota, t.status, (SELECT COUNT(*) FROM training_participants tp WHERE tp.training_id = t.id) FROM trainings t WHERE t.deleted_at IS NULL ORDER BY t.start_date DESC")
        .map_err(|e| format!("gagal menyiapkan katalog: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, f64>(7)?,
                r.get::<_, Option<i64>>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, i64>(10)?,
            ))
        })
        .map_err(|e| format!("gagal membaca katalog: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, title, desc, trainer, start, end, loc, cost, quota, status, count) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Training {
            id: to_dto_int(id, "training.id")?,
            title,
            description: desc,
            trainer_name: trainer,
            start_date: start,
            end_date: end,
            location: loc,
            cost,
            quota: quota.map(|v| to_dto_int(v, "training.quota")).transpose()?,
            status,
            participant_count: to_dto_int(count, "training.count")?,
        });
    }
    Ok(out)
}

pub fn detail_participants(
    conn: &Connection,
    training_id: i64,
) -> Result<Vec<Participant>, String> {
    let mut stmt = conn
        .prepare("SELECT tp.id, tp.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, tp.status, tp.quiz_score FROM training_participants tp INNER JOIN employees e ON e.id = tp.employee_id WHERE tp.training_id = ?1 ORDER BY e.first_name")
        .map_err(|e| format!("gagal menyiapkan peserta: {e}"))?;
    let rows = stmt
        .query_map(params![training_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<f64>>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca peserta: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, number, status, quiz) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Participant {
            id: to_dto_int(id, "participant.id")?,
            employee_id: to_dto_int(emp, "participant.emp")?,
            employee_name: name,
            employee_number: number,
            status,
            quiz_score: quiz,
        });
    }
    Ok(out)
}

pub fn save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    input: &TrainingInput,
) -> Result<i32, String> {
    if input.title.trim().is_empty() {
        return Err("Judul training wajib diisi.".to_string());
    }
    if input.title.len() > 150 {
        return Err("Judul maksimal 150 karakter.".to_string());
    }
    NaiveDate::parse_from_str(input.start_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.end_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if input.end_date.trim() < input.start_date.trim() {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    if !TRAINING_STATUS.contains(&input.status.as_str()) {
        return Err("Status tidak valid.".to_string());
    }
    if !(input.cost >= 0.0) {
        return Err("Biaya minimal 0.".to_string());
    }
    if let Some(q) = input.quota {
        if q < 1 {
            return Err("Kuota minimal 1.".to_string());
        }
    }
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE trainings SET title = ?1, description = ?2, trainer_name = ?3, start_date = ?4, end_date = ?5, location = ?6, cost = ?7, quota = ?8, status = ?9 WHERE id = ?10 AND deleted_at IS NULL",
            params![
                input.title.trim(),
                input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.trainer_name.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.start_date.trim(), input.end_date.trim(),
                input.location.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.cost, input.quota, input.status, rid,
            ],
        ).map_err(|e| format!("gagal menyimpan training: {e}"))?;
        if n == 0 {
            return Err("Training tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "training",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "training.id")
    } else {
        conn.execute(
            "INSERT INTO trainings (title, description, trainer_name, start_date, end_date, location, cost, quota, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                input.title.trim(),
                input.description.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.trainer_name.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.start_date.trim(), input.end_date.trim(),
                input.location.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.cost, input.quota, input.status,
            ],
        ).map_err(|e| format!("gagal menambah training: {e}"))?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "training",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "training.id")
    }
}

pub fn delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM training_participants WHERE training_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa peserta: {e}"))?;
    if n > 0 {
        return Err("Training sudah memiliki peserta.".to_string());
    }
    let d = conn.execute(
        "UPDATE trainings SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus training: {e}"))?;
    if d == 0 {
        return Err("Training tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "training",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Peserta ----------------

pub fn add_participant(
    conn: &Connection,
    actor_id: i64,
    training_id: i64,
    employee_id: i64,
) -> Result<i32, String> {
    let training: Option<i64> = conn
        .query_row(
            "SELECT id FROM trainings WHERE id = ?1 AND deleted_at IS NULL",
            params![training_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa training: {e}"))?;
    if training.is_none() {
        return Err("Training tidak ditemukan.".to_string());
    }
    let emp: Option<i64> = conn
        .query_row(
            "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let dup: Option<i64> = conn
        .query_row(
            "SELECT id FROM training_participants WHERE training_id = ?1 AND employee_id = ?2",
            params![training_id, employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa duplikat: {e}"))?;
    if dup.is_some() {
        return Err("Karyawan sudah terdaftar pada training ini.".to_string());
    }
    conn.execute(
        "INSERT INTO training_participants (training_id, employee_id, status) VALUES (?1, ?2, 'registered')",
        params![training_id, employee_id],
    )
    .map_err(|e| format!("gagal menambah peserta: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "training.participant",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "participant.id")
}

pub fn set_participant_status(
    conn: &Connection,
    actor_id: i64,
    participant_id: i64,
    status: &str,
) -> Result<(), String> {
    if !PARTICIPANT_STATUS.contains(&status) {
        return Err("Status tidak valid.".to_string());
    }
    let row: Option<(i64, i64)> = conn
        .query_row(
            "SELECT training_id, employee_id FROM training_participants WHERE id = ?1",
            params![participant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat peserta: {e}"))?;
    let Some((training_id, _)) = row else {
        return Err("Peserta tidak ditemukan.".to_string());
    };
    conn.execute(
        "UPDATE training_participants SET status = ?1 WHERE id = ?2",
        params![status, participant_id],
    )
    .map_err(|e| format!("gagal memperbarui status: {e}"))?;
    if status == "attended" || status == "completed" {
        let start: Option<String> = conn
            .query_row(
                "SELECT start_date FROM trainings WHERE id = ?1",
                params![training_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memuat training: {e}"))?
            .flatten();
        if let Some(date) = start {
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT id FROM training_attendance WHERE training_participant_id = ?1 AND date = ?2",
                    params![participant_id, date],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| format!("gagal memeriksa kehadiran: {e}"))?;
            if exists.is_none() {
                conn.execute(
                    "INSERT INTO training_attendance (training_participant_id, date, attended) VALUES (?1, ?2, 1)",
                    params![participant_id, date],
                )
                .map_err(|e| format!("gagal mencatat kehadiran: {e}"))?;
            }
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "training.participant",
        Some(&participant_id.to_string()),
        None,
        None,
        Some(&format!("Status menjadi {status}")),
    )?;
    Ok(())
}

// ---------------- Sertifikasi ----------------

pub fn certifications(conn: &Connection) -> Result<Vec<Certification>, String> {
    let mut stmt = conn
        .prepare("SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), c.name, c.issuer, c.certificate_number, c.issued_date, c.expiry_date FROM certifications c INNER JOIN employees e ON e.id = c.employee_id ORDER BY c.issued_date DESC")
        .map_err(|e| format!("gagal menyiapkan sertifikasi: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca sertifikasi: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, cname, issuer, number, issued, expiry) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Certification {
            id: to_dto_int(id, "cert.id")?,
            employee_id: to_dto_int(emp, "cert.emp")?,
            employee_name: name,
            name: cname,
            issuer,
            certificate_number: number,
            issued_date: issued,
            expiry_date: expiry,
        });
    }
    Ok(out)
}

pub fn add_certification(
    conn: &Connection,
    actor_id: i64,
    input: &CertificationInput,
) -> Result<i32, String> {
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
    if input.name.trim().is_empty() {
        return Err("Nama sertifikasi wajib diisi.".to_string());
    }
    if input.name.len() > 150 {
        return Err("Nama maksimal 150 karakter.".to_string());
    }
    for (v, label) in [
        (&input.issued_date, "Tanggal terbit"),
        (&input.expiry_date, "Tanggal kedaluwarsa"),
    ] {
        if let Some(s) = v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{label} harus valid (YYYY-MM-DD)."))?;
        }
    }
    conn.execute(
        "INSERT INTO certifications (employee_id, name, issuer, certificate_number, issued_date, expiry_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            input.employee_id as i64,
            input.name.trim(),
            input.issuer.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.certificate_number.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.issued_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
            input.expiry_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )
    .map_err(|e| format!("gagal menambah sertifikasi: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "training.certification",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "cert.id")
}

// ---------------- Skill matrix ----------------

pub fn skill_matrix(conn: &Connection) -> Result<Vec<SkillCell>, String> {
    let mut stmt = conn
        .prepare("SELECT es.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), es.skill_name, es.level FROM employee_skills es INNER JOIN employees e ON e.id = es.employee_id WHERE e.deleted_at IS NULL ORDER BY e.first_name, es.skill_name")
        .map_err(|e| format!("gagal menyiapkan matrix: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("gagal membaca matrix: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (emp, name, skill, level) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(SkillCell {
            employee_id: to_dto_int(emp, "skill.emp")?,
            employee_name: name,
            skill_name: skill,
            level: to_dto_int(level, "skill.level")?,
        });
    }
    Ok(out)
}

pub fn set_skill(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    skill_name: &str,
    level: i32,
) -> Result<(), String> {
    if level < 1 || level > 5 {
        return Err("Level harus 1-5.".to_string());
    }
    let skill = skill_name.trim();
    if skill.is_empty() {
        return Err("Nama skill wajib diisi.".to_string());
    }
    let emp: Option<i64> = conn
        .query_row(
            "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM employee_skills WHERE employee_id = ?1 AND skill_name = ?2",
            params![employee_id, skill],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa skill: {e}"))?;
    match existing {
        Some(eid) => {
            conn.execute(
                "UPDATE employee_skills SET level = ?1, assessed_at = ?2 WHERE id = ?3",
                params![level as i64, today, eid],
            )
            .map_err(|e| format!("gagal memperbarui skill: {e}"))?;
        }
        None => {
            conn.execute(
                "INSERT INTO employee_skills (employee_id, skill_name, level, assessed_at) VALUES (?1, ?2, ?3, ?4)",
                params![employee_id, skill, level as i64, today],
            )
            .map_err(|e| format!("gagal menambah skill: {e}"))?;
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "training.skill",
        Some(&employee_id.to_string()),
        None,
        None,
        Some(&format!("{skill} level {level}")),
    )?;
    Ok(())
}

pub fn set_quiz_score(
    conn: &Connection,
    actor_id: i64,
    participant_id: i64,
    score: f64,
) -> Result<(), String> {
    if !(0.0..=100.0).contains(&score) {
        return Err("Nilai kuis harus 0 sampai 100.".to_string());
    }
    let n = conn
        .execute(
            "UPDATE training_participants SET quiz_score = ?1 WHERE id = ?2",
            params![score, participant_id],
        )
        .map_err(|e| format!("gagal menyimpan nilai: {e}"))?;
    if n == 0 {
        return Err("Peserta tidak ditemukan.".to_string());
    }
    let pass: f64 = conn
        .query_row(
            "SELECT setting_value FROM system_settings WHERE setting_key = 'training_pass_score'",
            [],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat ambang lulus: {e}"))?
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(70.0);
    if score >= pass {
        let info: Option<(i64, String)> = conn
            .query_row(
                "SELECT tp.employee_id, t.title FROM training_participants tp INNER JOIN trainings t ON t.id = tp.training_id WHERE tp.id = ?1",
                params![participant_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("gagal memuat kelulusan: {e}"))?;
        if let Some((eid, title)) = info {
            let name = format!("Lulus: {title}");
            conn.execute(
                "INSERT INTO certifications (employee_id, name, issued_date) SELECT ?1, ?2, date('now','localtime') WHERE NOT EXISTS (SELECT 1 FROM certifications WHERE employee_id = ?1 AND name = ?2)",
                params![eid, name],
            )
            .map_err(|e| format!("gagal menerbitkan sertifikat: {e}"))?;
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "training.quiz",
        Some(&participant_id.to_string()),
        None,
        None,
        Some(&format!("Nilai kuis {score}")),
    )?;
    Ok(())
}

// ---------------- Materi ----------------

/// Satu materi e-learning milik training.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Material {
    pub id: i32,
    pub training_id: i32,
    pub title: String,
    pub kind: String,
    pub url: Option<String>,
}

pub fn material_list(conn: &Connection, training_id: i64) -> Result<Vec<Material>, String> {
    let mut stmt = conn
        .prepare("SELECT id, training_id, title, kind, url FROM training_materials WHERE training_id = ?1 ORDER BY sort_order ASC, id ASC")
        .map_err(|e| format!("gagal menyiapkan materi: {e}"))?;
    let rows = stmt
        .query_map(params![training_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca materi: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, tid, title, kind, url) =
            row.map_err(|e| format!("gagal membaca baris materi: {e}"))?;
        out.push(Material {
            id: to_dto_int(id, "training.material")?,
            training_id: to_dto_int(tid, "training.id")?,
            title,
            kind,
            url,
        });
    }
    Ok(out)
}

pub fn material_add(
    conn: &Connection,
    actor_id: i64,
    training_id: i64,
    title: &str,
    kind: &str,
    url: Option<&str>,
) -> Result<i32, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM trainings WHERE id = ?1 AND deleted_at IS NULL",
            params![training_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa training: {e}"))?;
    if exists.is_none() {
        return Err("Training tidak ditemukan.".to_string());
    }
    if title.trim().is_empty() {
        return Err("Judul materi wajib diisi.".to_string());
    }
    if title.len() > 150 {
        return Err("Judul materi maksimal 150 karakter.".to_string());
    }
    if !["tautan", "dokumen", "teks"].contains(&kind) {
        return Err("Jenis materi tidak valid.".to_string());
    }
    let clean_url = url.map(str::trim).filter(|s| !s.is_empty());
    conn.execute(
        "INSERT INTO training_materials (training_id, title, kind, url) VALUES (?1, ?2, ?3, ?4)",
        params![training_id, title.trim(), kind, clean_url],
    )
    .map_err(|e| format!("gagal menambah materi: {e}"))?;
    let rid = conn.last_insert_rowid();
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "training.material",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "training.material")
}

pub fn material_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM training_materials WHERE id = ?1", params![id])
        .map_err(|e| format!("gagal menghapus materi: {e}"))?;
    if n == 0 {
        return Err("Materi tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "training.material",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

fn topt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

fn topt_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse().ok(),
    }
}

fn topt_i(v: &Value, f: &str) -> Result<Option<i32>, String> {
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

pub async fn list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Training>, String> {
    let rows = q_all(
        db,
        "SELECT t.id, t.title, t.description, t.trainer_name, t.start_date, t.end_date, t.location, t.cost, t.quota, t.status, (SELECT COUNT(*) FROM training_participants tp WHERE tp.training_id = t.id) FROM trainings t WHERE t.deleted_at IS NULL ORDER BY t.start_date DESC".to_string(),
        vec![],
        11,
        "training.list",
    )
    .await
    .map_err(|e| format!("gagal membaca katalog: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let (id, quota, count) = (
            value_i64(&r[0]).unwrap_or(0),
            value_i64(&r[8]),
            value_i64(&r[10]).unwrap_or(0),
        );
        out.push(Training {
            id: to_dto_int(id, "training.id")?,
            title: value_to_string(&r[1]),
            description: topt_text(&r[2]),
            trainer_name: topt_text(&r[3]),
            start_date: value_to_string(&r[4]),
            end_date: value_to_string(&r[5]),
            location: topt_text(&r[6]),
            cost: topt_f64(&r[7]).unwrap_or(0.0),
            quota: match quota {
                Some(v) => Some(to_dto_int(v, "training.quota")?),
                None => None,
            },
            status: value_to_string(&r[9]),
            participant_count: to_dto_int(count, "training.count")?,
        });
    }
    Ok(out)
}

pub async fn detail_participants_sea(
    db: &sea_orm::DatabaseConnection,
    training_id: i64,
) -> Result<Vec<Participant>, String> {
    let rows = q_all(
        db,
        "SELECT tp.id, tp.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, tp.status, tp.quiz_score FROM training_participants tp INNER JOIN employees e ON e.id = tp.employee_id WHERE tp.training_id = ?1 ORDER BY e.first_name".to_string(),
        vec![Value::Int(training_id)],
        6,
        "training.parts",
    )
    .await
    .map_err(|e| format!("gagal membaca peserta: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Participant {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "participant.id")?,
            employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "participant.emp")?,
            employee_name: value_to_string(&r[2]),
            employee_number: value_to_string(&r[3]),
            status: value_to_string(&r[4]),
            quiz_score: topt_f64(&r[5]),
        });
    }
    Ok(out)
}

pub async fn save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &TrainingInput,
) -> Result<i32, String> {
    if input.title.trim().is_empty() {
        return Err("Judul training wajib diisi.".to_string());
    }
    if input.title.len() > 150 {
        return Err("Judul maksimal 150 karakter.".to_string());
    }
    NaiveDate::parse_from_str(input.start_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal mulai tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.end_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal selesai tidak valid.".to_string())?;
    if input.end_date.trim() < input.start_date.trim() {
        return Err("Tanggal selesai sebelum tanggal mulai.".to_string());
    }
    if !TRAINING_STATUS.contains(&input.status.as_str()) {
        return Err("Status tidak valid.".to_string());
    }
    if !(input.cost >= 0.0) {
        return Err("Biaya minimal 0.".to_string());
    }
    if let Some(q) = input.quota {
        if q < 1 {
            return Err("Kuota minimal 1.".to_string());
        }
    }
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE trainings SET title = ?1, description = ?2, trainer_name = ?3, start_date = ?4, end_date = ?5, location = ?6, cost = ?7, quota = ?8, status = ?9 WHERE id = ?10 AND deleted_at IS NULL".to_string(),
            vec![
                Value::Text(input.title.trim().to_string()),
                opt_t(input.description.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.trainer_name.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                Value::Text(input.start_date.trim().to_string()),
                Value::Text(input.end_date.trim().to_string()),
                opt_t(input.location.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                Value::Float(input.cost),
                match input.quota {
                    Some(v) => Value::Int(v as i64),
                    None => Value::Null,
                },
                Value::Text(input.status.clone()),
                Value::Int(rid),
            ],
            "training.upd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan training: {e}"))?;
        if n == 0 {
            return Err("Training tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "training",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "training.id")
    } else {
        exec(
            db,
            "INSERT INTO trainings (title, description, trainer_name, start_date, end_date, location, cost, quota, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)".to_string(),
            vec![
                Value::Text(input.title.trim().to_string()),
                opt_t(input.description.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.trainer_name.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                Value::Text(input.start_date.trim().to_string()),
                Value::Text(input.end_date.trim().to_string()),
                opt_t(input.location.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                Value::Float(input.cost),
                match input.quota {
                    Some(v) => Value::Int(v as i64),
                    None => Value::Null,
                },
                Value::Text(input.status.clone()),
            ],
            "training.add",
        )
        .await
        .map_err(|e| format!("gagal menambah training: {e}"))?;
        let rid = trow_id(db, "training.add").await?;
        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            "training",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "training.id")
    }
}

pub async fn delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM training_participants WHERE training_id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "training.rel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa peserta: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Training sudah memiliki peserta.".to_string());
    }
    let d = exec(
        db,
        "UPDATE trainings SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "training.del",
    )
    .await
    .map_err(|e| format!("gagal menghapus training: {e}"))?;
    if d == 0 {
        return Err("Training tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "training",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn add_participant_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    training_id: i64,
    employee_id: i64,
) -> Result<i32, String> {
    let training = q_one(
        db,
        "SELECT id FROM trainings WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(training_id)],
        1,
        "training.check",
    )
    .await
    .map_err(|e| format!("gagal memeriksa training: {e}"))?;
    if training.is_none() {
        return Err("Training tidak ditemukan.".to_string());
    }
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "training.empcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let dup = q_one(
        db,
        "SELECT id FROM training_participants WHERE training_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(training_id), Value::Int(employee_id)],
        1,
        "training.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa duplikat: {e}"))?;
    if dup.is_some() {
        return Err("Karyawan sudah terdaftar pada training ini.".to_string());
    }
    exec(
        db,
        "INSERT INTO training_participants (training_id, employee_id, status) VALUES (?1, ?2, 'registered')".to_string(),
        vec![Value::Int(training_id), Value::Int(employee_id)],
        "training.partadd",
    )
    .await
    .map_err(|e| format!("gagal menambah peserta: {e}"))?;
    let rid = trow_id(db, "training.partadd").await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "training.participant",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "participant.id")
}

pub async fn set_participant_status_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    participant_id: i64,
    status: &str,
) -> Result<(), String> {
    if !PARTICIPANT_STATUS.contains(&status) {
        return Err("Status tidak valid.".to_string());
    }
    let row = q_one(
        db,
        "SELECT training_id, employee_id FROM training_participants WHERE id = ?1".to_string(),
        vec![Value::Int(participant_id)],
        2,
        "training.part",
    )
    .await
    .map_err(|e| format!("gagal memuat peserta: {e}"))?;
    let Some(row) = row else {
        return Err("Peserta tidak ditemukan.".to_string());
    };
    let training_id = value_i64(&row[0]).unwrap_or(0);
    exec(
        db,
        "UPDATE training_participants SET status = ?1 WHERE id = ?2".to_string(),
        vec![Value::Text(status.to_string()), Value::Int(participant_id)],
        "training.partupd",
    )
    .await
    .map_err(|e| format!("gagal memperbarui status: {e}"))?;
    if status == "attended" || status == "completed" {
        let start_row = q_one(
            db,
            "SELECT start_date FROM trainings WHERE id = ?1".to_string(),
            vec![Value::Int(training_id)],
            1,
            "training.start",
        )
        .await
        .map_err(|e| format!("gagal memuat training: {e}"))?;
        if let Some(date) = start_row.as_ref().and_then(|r| topt_text(&r[0])) {
            let exists = q_one(
                db,
                "SELECT id FROM training_attendance WHERE training_participant_id = ?1 AND date = ?2".to_string(),
                vec![Value::Int(participant_id), Value::Text(date.clone())],
                1,
                "training.attex",
            )
            .await
            .map_err(|e| format!("gagal memeriksa kehadiran: {e}"))?;
            if exists.is_none() {
                exec(
                    db,
                    "INSERT INTO training_attendance (training_participant_id, date, attended) VALUES (?1, ?2, 1)".to_string(),
                    vec![Value::Int(participant_id), Value::Text(date)],
                    "training.attadd",
                )
                .await
                .map_err(|e| format!("gagal mencatat kehadiran: {e}"))?;
            }
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "training.participant",
        Some(&participant_id.to_string()),
        None,
        None,
        Some(&format!("Status menjadi {status}")),
    )
    .await?;
    Ok(())
}

pub async fn certifications_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Certification>, String> {
    let rows = q_all(
        db,
        "SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), c.name, c.issuer, c.certificate_number, c.issued_date, c.expiry_date FROM certifications c INNER JOIN employees e ON e.id = c.employee_id ORDER BY c.issued_date DESC".to_string(),
        vec![],
        8,
        "training.certs",
    )
    .await
    .map_err(|e| format!("gagal membaca sertifikasi: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Certification {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "cert.id")?,
            employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "cert.emp")?,
            employee_name: value_to_string(&r[2]),
            name: value_to_string(&r[3]),
            issuer: topt_text(&r[4]),
            certificate_number: topt_text(&r[5]),
            issued_date: topt_text(&r[6]),
            expiry_date: topt_text(&r[7]),
        });
    }
    Ok(out)
}

pub async fn add_certification_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    input: &CertificationInput,
) -> Result<i32, String> {
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(input.employee_id as i64)],
        1,
        "training.certemp",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    if input.name.trim().is_empty() {
        return Err("Nama sertifikasi wajib diisi.".to_string());
    }
    if input.name.len() > 150 {
        return Err("Nama maksimal 150 karakter.".to_string());
    }
    for (v, label) in [
        (&input.issued_date, "Tanggal terbit"),
        (&input.expiry_date, "Tanggal kedaluwarsa"),
    ] {
        if let Some(s) = v.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{label} harus valid (YYYY-MM-DD)."))?;
        }
    }
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    exec(
        db,
        "INSERT INTO certifications (employee_id, name, issuer, certificate_number, issued_date, expiry_date) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
        vec![
            Value::Int(input.employee_id as i64),
            Value::Text(input.name.trim().to_string()),
            opt_t(input.issuer.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.certificate_number.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.issued_date.as_deref().map(str::trim).filter(|s| !s.is_empty())),
            opt_t(input.expiry_date.as_deref().map(str::trim).filter(|s| !s.is_empty())),
        ],
        "training.certadd",
    )
    .await
    .map_err(|e| format!("gagal menambah sertifikasi: {e}"))?;
    let rid = trow_id(db, "training.certadd").await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "training.certification",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "cert.id")
}

pub async fn skill_matrix_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<SkillCell>, String> {
    let rows = q_all(
        db,
        "SELECT es.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), es.skill_name, es.level FROM employee_skills es INNER JOIN employees e ON e.id = es.employee_id WHERE e.deleted_at IS NULL ORDER BY e.first_name, es.skill_name".to_string(),
        vec![],
        4,
        "training.matrix",
    )
    .await
    .map_err(|e| format!("gagal membaca matrix: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(SkillCell {
            employee_id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "skill.emp")?,
            employee_name: value_to_string(&r[1]),
            skill_name: value_to_string(&r[2]),
            level: to_dto_int(value_i64(&r[3]).unwrap_or(0), "skill.level")?,
        });
    }
    Ok(out)
}

pub async fn set_skill_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    skill_name: &str,
    level: i32,
) -> Result<(), String> {
    if level < 1 || level > 5 {
        return Err("Level harus 1-5.".to_string());
    }
    let skill = skill_name.trim();
    if skill.is_empty() {
        return Err("Nama skill wajib diisi.".to_string());
    }
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "training.skillemp",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let existing = q_one(
        db,
        "SELECT id FROM employee_skills WHERE employee_id = ?1 AND skill_name = ?2".to_string(),
        vec![Value::Int(employee_id), Value::Text(skill.to_string())],
        1,
        "training.skillex",
    )
    .await
    .map_err(|e| format!("gagal memeriksa skill: {e}"))?;
    match existing {
        Some(e) => {
            let eid = value_i64(&e[0]).unwrap_or(0);
            exec(
                db,
                "UPDATE employee_skills SET level = ?1, assessed_at = ?2 WHERE id = ?3".to_string(),
                vec![
                    Value::Int(level as i64),
                    Value::Text(today),
                    Value::Int(eid),
                ],
                "training.skillupd",
            )
            .await
            .map_err(|e| format!("gagal memperbarui skill: {e}"))?;
        }
        None => {
            exec(
                db,
                "INSERT INTO employee_skills (employee_id, skill_name, level, assessed_at) VALUES (?1, ?2, ?3, ?4)".to_string(),
                vec![
                    Value::Int(employee_id),
                    Value::Text(skill.to_string()),
                    Value::Int(level as i64),
                    Value::Text(today),
                ],
                "training.skilladd",
            )
            .await
            .map_err(|e| format!("gagal menambah skill: {e}"))?;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "training.skill",
        Some(&employee_id.to_string()),
        None,
        None,
        Some(&format!("{skill} level {level}")),
    )
    .await?;
    Ok(())
}

pub async fn set_quiz_score_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    participant_id: i64,
    score: f64,
) -> Result<(), String> {
    if !(0.0..=100.0).contains(&score) {
        return Err("Nilai kuis harus 0 sampai 100.".to_string());
    }
    let n = exec(
        db,
        "UPDATE training_participants SET quiz_score = ?1 WHERE id = ?2".to_string(),
        vec![Value::Float(score), Value::Int(participant_id)],
        "training.quiz",
    )
    .await
    .map_err(|e| format!("gagal menyimpan nilai: {e}"))?;
    if n == 0 {
        return Err("Peserta tidak ditemukan.".to_string());
    }
    let pass_row = q_one(
        db,
        "SELECT setting_value FROM system_settings WHERE setting_key = 'training_pass_score'".to_string(),
        vec![],
        1,
        "training.pass",
    )
    .await
    .map_err(|e| format!("gagal memuat ambang lulus: {e}"))?;
    let pass: f64 = pass_row
        .as_ref()
        .and_then(|r| topt_text(&r[0]))
        .and_then(|v| v.parse().ok())
        .unwrap_or(70.0);
    if score >= pass {
        let info = q_one(
            db,
            "SELECT tp.employee_id, t.title FROM training_participants tp INNER JOIN trainings t ON t.id = tp.training_id WHERE tp.id = ?1".to_string(),
            vec![Value::Int(participant_id)],
            2,
            "training.passinfo",
        )
        .await
        .map_err(|e| format!("gagal memuat kelulusan: {e}"))?;
        if let Some(info) = info {
            let (eid, title) = (value_i64(&info[0]).unwrap_or(0), value_to_string(&info[1]));
            let name = format!("Lulus: {title}");
            exec(
                db,
                "INSERT INTO certifications (employee_id, name, issued_date) SELECT ?1, ?2, date('now','localtime') WHERE NOT EXISTS (SELECT 1 FROM certifications WHERE employee_id = ?1 AND name = ?2)".to_string(),
                vec![Value::Int(eid), Value::Text(name)],
                "training.autocert",
            )
            .await
            .map_err(|e| format!("gagal menerbitkan sertifikat: {e}"))?;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "training.quiz",
        Some(&participant_id.to_string()),
        None,
        None,
        Some(&format!("Nilai kuis {score}")),
    )
    .await?;
    Ok(())
}

pub async fn material_list_sea(
    db: &sea_orm::DatabaseConnection,
    training_id: i64,
) -> Result<Vec<Material>, String> {
    let rows = q_all(
        db,
        "SELECT id, training_id, title, kind, url FROM training_materials WHERE training_id = ?1 ORDER BY sort_order ASC, id ASC".to_string(),
        vec![Value::Int(training_id)],
        5,
        "training.mats",
    )
    .await
    .map_err(|e| format!("gagal membaca materi: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Material {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "training.material")?,
            training_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "training.id")?,
            title: value_to_string(&r[2]),
            kind: value_to_string(&r[3]),
            url: topt_text(&r[4]),
        });
    }
    Ok(out)
}

pub async fn material_add_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    training_id: i64,
    title: &str,
    kind: &str,
    url: Option<&str>,
) -> Result<i32, String> {
    let exists = q_one(
        db,
        "SELECT id FROM trainings WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(training_id)],
        1,
        "training.matcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa training: {e}"))?;
    if exists.is_none() {
        return Err("Training tidak ditemukan.".to_string());
    }
    if title.trim().is_empty() {
        return Err("Judul materi wajib diisi.".to_string());
    }
    if title.len() > 150 {
        return Err("Judul materi maksimal 150 karakter.".to_string());
    }
    if !["tautan", "dokumen", "teks"].contains(&kind) {
        return Err("Jenis materi tidak valid.".to_string());
    }
    let clean_url = url.map(str::trim).filter(|s| !s.is_empty());
    exec(
        db,
        "INSERT INTO training_materials (training_id, title, kind, url) VALUES (?1, ?2, ?3, ?4)".to_string(),
        vec![
            Value::Int(training_id),
            Value::Text(title.trim().to_string()),
            Value::Text(kind.to_string()),
            match clean_url {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
        ],
        "training.matadd",
    )
    .await
    .map_err(|e| format!("gagal menambah materi: {e}"))?;
    let rid = trow_id(db, "training.matadd").await?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "training.material",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "training.material")
}

pub async fn material_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let n = exec(
        db,
        "DELETE FROM training_materials WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        "training.matdel",
    )
    .await
    .map_err(|e| format!("gagal menghapus materi: {e}"))?;
    if n == 0 {
        return Err("Materi tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "training.material",
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
    use crate::{db, seed};

    fn live() -> (tempfile::TempDir, crate::db::DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool)
    }

    fn admin(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn emp(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn materi_tersimpan_per_training() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        conn.execute(
            "INSERT INTO trainings (title, start_date, end_date, status) VALUES ('Dasar K3', '2026-01-01', '2026-01-02', 'scheduled')",
            [],
        )
        .unwrap();
        let tid = conn.last_insert_rowid();
        assert!(material_add(&conn, actor, 999999, "X", "tautan", None).is_err());
        assert!(material_add(&conn, actor, tid, "  ", "tautan", None).is_err());
        assert!(material_add(&conn, actor, tid, "Modul 1", "video", None).is_err());
        let mid = material_add(
            &conn,
            actor,
            tid,
            "Modul 1",
            "tautan",
            Some("https://x.local/1"),
        )
        .expect("tambah") as i64;
        let items = material_list(&conn, tid).expect("daftar");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Modul 1");
        material_delete(&conn, actor, mid).expect("hapus");
        assert!(material_list(&conn, tid).expect("daftar").is_empty());
        assert!(material_delete(&conn, actor, mid).is_err());
    }

    #[test]
    fn nilai_kuis_tersimpan_per_peserta() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        conn.execute(
            "INSERT INTO trainings (title, start_date, end_date, status) VALUES ('Kuis K3', '2026-01-01', '2026-01-02', 'scheduled')",
            [],
        )
        .unwrap();
        let tid = conn.last_insert_rowid();
        let pid = add_participant(&conn, actor, tid, emp(&conn)).expect("peserta") as i64;
        assert!(set_quiz_score(&conn, actor, pid, 101.0).is_err());
        assert!(set_quiz_score(&conn, actor, pid, -1.0).is_err());
        assert!(set_quiz_score(&conn, actor, 999999, 80.0).is_err());
        set_quiz_score(&conn, actor, pid, 85.5).expect("simpan");
        let items = detail_participants(&conn, tid).expect("daftar");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quiz_score, Some(85.5));
    }

    #[test]
    fn sertifikat_terbit_otomatis_saat_lulus() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        conn.execute(
            "INSERT INTO trainings (title, start_date, end_date, status) VALUES ('Kuis Lulus', '2026-01-01', '2026-01-02', 'scheduled')",
            [],
        )
        .unwrap();
        let tid = conn.last_insert_rowid();
        let pid = add_participant(&conn, actor, tid, emp(&conn)).expect("peserta") as i64;
        let certs = || -> i64 {
            conn.query_row(
                "SELECT COUNT(*) FROM certifications WHERE name = 'Lulus: Kuis Lulus'",
                [],
                |r| r.get(0),
            )
            .unwrap()
        };
        set_quiz_score(&conn, actor, pid, 60.0).expect("simpan");
        assert_eq!(certs(), 0);
        set_quiz_score(&conn, actor, pid, 85.0).expect("lulus");
        assert_eq!(certs(), 1);
        set_quiz_score(&conn, actor, pid, 90.0).expect("ulang");
        assert_eq!(certs(), 1);
    }

    #[test]
    fn katalog_peserta_sertifikasi_skill() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let eid = emp(&conn);
        let tid = save(
            &conn,
            actor,
            None,
            &TrainingInput {
                title: "K3 Dasar".to_string(),
                description: None,
                trainer_name: Some("Budi".to_string()),
                start_date: "2026-10-01".to_string(),
                end_date: "2026-10-02".to_string(),
                location: Some("Ruang A".to_string()),
                cost: 500000.0,
                quota: Some(20),
                status: "scheduled".to_string(),
            },
        )
        .expect("buat");
        assert!(save(
            &conn,
            actor,
            None,
            &TrainingInput {
                title: "X".to_string(),
                description: None,
                trainer_name: None,
                start_date: "2026-10-05".to_string(),
                end_date: "2026-10-01".to_string(),
                location: None,
                cost: 0.0,
                quota: None,
                status: "scheduled".to_string(),
            },
        )
        .is_err());
        let pid = add_participant(&conn, actor, tid as i64, eid).expect("peserta");
        assert!(add_participant(&conn, actor, tid as i64, eid).is_err());
        set_participant_status(&conn, actor, pid as i64, "completed").expect("status");
        assert!(set_participant_status(&conn, actor, pid as i64, "ngawur").is_err());
        let att: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM training_attendance WHERE training_participant_id = ?1",
                params![pid as i64],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(att, 1);
        assert!(delete(&conn, actor, tid as i64).is_err());
        add_certification(
            &conn,
            actor,
            &CertificationInput {
                employee_id: to_dto_int(eid, "e").unwrap(),
                name: "K3 Umum".to_string(),
                issuer: Some("BNPB".to_string()),
                certificate_number: None,
                issued_date: Some("2026-10-03".to_string()),
                expiry_date: None,
            },
        )
        .expect("sertifikasi");
        assert_eq!(certifications(&conn).expect("list").len(), 1);
        set_skill(&conn, actor, eid, "Forklift", 3).expect("skill");
        set_skill(&conn, actor, eid, "Forklift", 4).expect("naik");
        assert!(set_skill(&conn, actor, eid, "Forklift", 9).is_err());
        let matrix = skill_matrix(&conn).expect("matrix");
        assert_eq!(matrix.len(), 1);
        assert_eq!(matrix[0].level, 4);
        assert_eq!(matrix[0].skill_name, "Forklift");
    }

    #[tokio::test]
    async fn training_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let actor: i64 = conn
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
        let tid = save_sea(
            db,
            actor,
            None,
            &TrainingInput {
                title: "K3 Dasar".to_string(),
                description: None,
                trainer_name: Some("Budi".to_string()),
                start_date: "2026-10-01".to_string(),
                end_date: "2026-10-02".to_string(),
                location: Some("Ruang A".to_string()),
                cost: 500000.0,
                quota: Some(20),
                status: "scheduled".to_string(),
            },
        )
        .await
        .expect("buat");
        let l_sync = serde_json::to_string(&list(&conn).expect("ls")).unwrap();
        let l_sea = serde_json::to_string(&list_sea(db).await.expect("lse")).unwrap();
        assert_eq!(l_sync, l_sea);
        let pid = add_participant_sea(db, actor, tid as i64, eid)
            .await
            .expect("peserta");
        let e_sea = add_participant_sea(db, actor, tid as i64, eid)
            .await
            .expect_err("ganda");
        let e_sync = add_participant(&conn, actor, tid as i64, eid)
            .expect_err("ganda sync");
        assert_eq!(e_sea, e_sync);
        set_participant_status_sea(db, actor, pid as i64, "completed")
            .await
            .expect("status");
        let p_sync =
            serde_json::to_string(&detail_participants(&conn, tid as i64).expect("ps")).unwrap();
        let p_sea =
            serde_json::to_string(&detail_participants_sea(db, tid as i64).await.expect("pse"))
                .unwrap();
        assert_eq!(p_sync, p_sea);
        set_quiz_score_sea(db, actor, pid as i64, 85.5).await.expect("kuis");
        let certs = certifications_sea(db).await.expect("certs");
        assert!(certs.iter().any(|c| c.name == "Lulus: K3 Dasar"));
        add_certification_sea(
            db,
            actor,
            &CertificationInput {
                employee_id: to_dto_int(eid, "e").unwrap(),
                name: "K3 Umum".to_string(),
                issuer: Some("BNPB".to_string()),
                certificate_number: None,
                issued_date: Some("2026-10-03".to_string()),
                expiry_date: None,
            },
        )
        .await
        .expect("sertifikasi");
        set_skill_sea(db, actor, eid, "Forklift", 3).await.expect("skill");
        set_skill_sea(db, actor, eid, "Forklift", 4).await.expect("naik");
        let m_sync = serde_json::to_string(&skill_matrix(&conn).expect("ms")).unwrap();
        let m_sea = serde_json::to_string(&skill_matrix_sea(db).await.expect("mse")).unwrap();
        assert_eq!(m_sync, m_sea);
        assert!(m_sea.contains("\"level\":4"));
        let mid = material_add_sea(db, actor, tid as i64, "Modul 1", "tautan", Some("https://x.local/1"))
            .await
            .expect("materi");
        let t_sync =
            serde_json::to_string(&material_list(&conn, tid as i64).expect("ts")).unwrap();
        let t_sea =
            serde_json::to_string(&material_list_sea(db, tid as i64).await.expect("tse"))
                .unwrap();
        assert_eq!(t_sync, t_sea);
        material_delete_sea(db, actor, mid as i64).await.expect("hapus materi");
        assert!(delete_sea(db, actor, tid as i64).await.is_err());
    }
}
