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
        .prepare("SELECT tp.id, tp.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, tp.status FROM training_participants tp INNER JOIN employees e ON e.id = tp.employee_id WHERE tp.training_id = ?1 ORDER BY e.first_name")
        .map_err(|e| format!("gagal menyiapkan peserta: {e}"))?;
    let rows = stmt
        .query_map(params![training_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca peserta: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, number, status) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Participant {
            id: to_dto_int(id, "participant.id")?,
            employee_id: to_dto_int(emp, "participant.emp")?,
            employee_name: name,
            employee_number: number,
            status,
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
}
