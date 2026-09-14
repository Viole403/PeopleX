//! Resign 4 tahap: supervisor, HR, finance, selesai + clearance + exit interview.

use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use super::approval;
use super::audit;
use crate::to_dto_int;

const STAGES: &[&str] = &[
    "pending",
    "supervisor_approved",
    "hr_approved",
    "finance_approved",
    "completed",
];

const DEFAULT_CLEARANCE: &[&str] = &[
    "Pengembalian Aset IT",
    "Pengembalian ID Card",
    "Penyelesaian Kasbon/Pinjaman",
    "Serah Terima Pekerjaan",
];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct OffboardingRow {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub resignation_date: String,
    pub last_working_date: String,
    pub status: String,
    pub current_step: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ExitInterview {
    pub feedback: Option<String>,
    pub reason_category: Option<String>,
    pub would_recommend: Option<bool>,
    pub satisfaction_score: Option<i32>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ClearanceItem {
    pub id: i32,
    pub item_name: String,
    pub department: Option<String>,
    pub is_cleared: bool,
    pub cleared_at: Option<String>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct OffboardingDetail {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub supervisor_id: Option<i32>,
    pub resignation_date: String,
    pub last_working_date: String,
    pub reason: Option<String>,
    pub status: String,
    pub current_step: i32,
    pub exit_interview: Option<ExitInterview>,
    pub clearance_items: Vec<ClearanceItem>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct OffboardingCreate {
    pub resignation_date: String,
    pub last_working_date: String,
    pub reason: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ExitInterviewInput {
    pub feedback: Option<String>,
    pub reason_category: Option<String>,
    pub would_recommend: Option<bool>,
    pub satisfaction_score: Option<i32>,
}

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn list(conn: &Connection) -> Result<Vec<OffboardingRow>, String> {
    let mut stmt = conn
        .prepare("SELECT o.id, o.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, o.resignation_date, o.last_working_date, o.status, o.current_step FROM offboarding o INNER JOIN employees e ON e.id = o.employee_id ORDER BY CASE o.status WHEN 'pending' THEN 0 WHEN 'supervisor_approved' THEN 1 WHEN 'hr_approved' THEN 2 WHEN 'finance_approved' THEN 3 WHEN 'completed' THEN 4 ELSE 5 END, o.created_at DESC")
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })
        .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, emp, name, number, resign, last, status, step) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(OffboardingRow {
            id: to_dto_int(id, "off.id")?,
            employee_id: to_dto_int(emp, "off.emp")?,
            employee_name: name,
            employee_number: number,
            resignation_date: resign,
            last_working_date: last,
            status,
            current_step: to_dto_int(step, "off.step")?,
        });
    }
    Ok(out)
}

pub fn my_requests(conn: &Connection, employee_id: i64) -> Result<Vec<OffboardingRow>, String> {
    Ok(list(conn)?
        .into_iter()
        .filter(|r| r.employee_id as i64 == employee_id)
        .collect())
}

pub fn find(conn: &Connection, id: i64) -> Result<Option<OffboardingDetail>, String> {
    let row: Option<(
        i64, i64, String, String, Option<i64>, String, String, Option<String>, String, i64,
    )> = conn
        .query_row(
            "SELECT o.id, o.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, e.supervisor_id, o.resignation_date, o.last_working_date, o.reason, o.status, o.current_step FROM offboarding o INNER JOIN employees e ON e.id = o.employee_id WHERE o.id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                    r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat resign: {e}"))?;
    let Some((oid, emp, name, number, sup, resign, last, reason, status, step)) = row else {
        return Ok(None);
    };
    let exit: Option<(Option<String>, Option<String>, Option<i64>, Option<i64>)> = conn
        .query_row(
            "SELECT feedback, reason_category, would_recommend, satisfaction_score FROM exit_interviews WHERE offboarding_id = ?1",
            params![oid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat exit interview: {e}"))?;
    let mut items = Vec::new();
    let mut istmt = conn
        .prepare("SELECT id, item_name, department, is_cleared, cleared_at, notes FROM clearance_items WHERE offboarding_id = ?1 ORDER BY id")
        .map_err(|e| format!("gagal menyiapkan clearance: {e}"))?;
    for irow in istmt
        .query_map(params![oid], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca clearance: {e}"))?
    {
        let (iid, iname, dept, cleared, at, notes) =
            irow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        items.push(ClearanceItem {
            id: to_dto_int(iid, "clearance.id")?,
            item_name: iname,
            department: dept,
            is_cleared: cleared != 0,
            cleared_at: at,
            notes,
        });
    }
    let opt_i = |v: Option<i64>| v.map(|x| to_dto_int(x, "off.ref")).transpose();
    Ok(Some(OffboardingDetail {
        id: to_dto_int(oid, "off.id")?,
        employee_id: to_dto_int(emp, "off.emp")?,
        employee_name: name,
        employee_number: number,
        supervisor_id: opt_i(sup)?,
        resignation_date: resign,
        last_working_date: last,
        reason,
        status,
        current_step: to_dto_int(step, "off.step")?,
        exit_interview: exit
            .map(|(f, r, w, s)| {
                Ok::<_, String>(ExitInterview {
                    feedback: f,
                    reason_category: r,
                    would_recommend: w.map(|v| v != 0),
                    satisfaction_score: s.map(|v| to_dto_int(v, "off.ref")).transpose()?,
                })
            })
            .transpose()?,
        clearance_items: items,
    }))
}

pub fn create(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    input: &OffboardingCreate,
) -> Result<i32, String> {
    NaiveDate::parse_from_str(input.resignation_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal resign tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.last_working_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal terakhir kerja tidak valid.".to_string())?;
    if input.last_working_date.trim() < input.resignation_date.trim() {
        return Err("Tanggal terakhir kerja sebelum tanggal resign.".to_string());
    }
    if let Some(reason) = input.reason.as_deref() {
        if reason.len() > 255 {
            return Err("Alasan maksimal 255 karakter.".to_string());
        }
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
    let active: Option<i64> = conn
        .query_row(
            "SELECT id FROM offboarding WHERE employee_id = ?1 AND status NOT IN ('completed','rejected')",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa proses berjalan: {e}"))?;
    if active.is_some() {
        return Err("Sudah ada proses resign yang berjalan untuk karyawan ini.".to_string());
    }
    conn.execute(
        "INSERT INTO offboarding (employee_id, resignation_date, last_working_date, reason, status, current_step) VALUES (?1, ?2, ?3, ?4, 'pending', 1)",
        params![
            employee_id,
            input.resignation_date.trim(),
            input.last_working_date.trim(),
            input.reason.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )
    .map_err(|e| format!("gagal mengajukan resign: {e}"))?;
    let rid = conn.last_insert_rowid();
    for item in DEFAULT_CLEARANCE {
        conn.execute(
            "INSERT INTO clearance_items (offboarding_id, item_name, is_cleared) VALUES (?1, ?2, 0)",
            params![rid, item],
        )
        .map_err(|e| format!("gagal membuat clearance: {e}"))?;
    }
    let sup: Option<i64> = conn
        .query_row(
            "SELECT supervisor_id FROM employees WHERE id = ?1",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat supervisor: {e}"))?
        .flatten();
    if let Some(sid) = sup {
        if let Some(uid) = approval::user_of_employee(conn, sid)? {
            let name: String = conn
                .query_row(
                    "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1",
                    params![employee_id],
                    |r| r.get(0),
                )
                .unwrap_or_default();
            approval::notify(
                conn,
                uid,
                "offboarding",
                "Pengajuan Resign Baru",
                &format!("{name} mengajukan resign."),
                "/offboarding",
            )?;
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "offboarding",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "off.id")
}

/// Putuskan tahap berjalan. Tahap pending = supervisor; sisanya izin.
pub fn decide(
    conn: &Connection,
    user_id: i64,
    user_employee_id: Option<i64>,
    privileged: bool,
    id: i64,
    action: &str,
    _notes: Option<&str>,
) -> Result<String, String> {
    if action != "approve" && action != "reject" {
        return Err("Aksi tidak valid.".to_string());
    }
    let det = find(conn, id)?.ok_or("Proses resign tidak ditemukan.".to_string())?;
    if det.status == "completed" || det.status == "rejected" {
        return Err("Proses sudah selesai.".to_string());
    }
    let can = if det.status == "pending" {
        match (user_employee_id, det.supervisor_id) {
            (Some(a), Some(s)) if a as i64 == s as i64 => true,
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
            "UPDATE offboarding SET status = 'rejected' WHERE id = ?1",
            params![id],
        )
        .map_err(|e| format!("gagal menolak: {e}"))?;
        audit::log(
            conn,
            Some(user_id),
            "REJECT",
            "offboarding",
            Some(&id.to_string()),
            None,
            None,
            None,
        )?;
        return Ok("rejected".to_string());
    }
    let idx = STAGES
        .iter()
        .position(|s| *s == det.status)
        .ok_or("Status tidak dikenal.".to_string())?;
    let next = STAGES
        .get(idx + 1)
        .ok_or("Sudah tahap akhir.".to_string())?;
    conn.execute(
        "UPDATE offboarding SET status = ?1, current_step = ?2 WHERE id = ?3",
        params![next, (idx + 2) as i64, id],
    )
    .map_err(|e| format!("gagal maju tahap: {e}"))?;
    if *next == "completed" {
        conn.execute(
            "UPDATE employees SET employment_status = 'resigned', resign_date = ?1 WHERE id = ?2",
            params![det.last_working_date, det.employee_id as i64],
        )
        .map_err(|e| format!("gagal menandai resign: {e}"))?;
        conn.execute(
            "UPDATE users SET status = 'inactive' WHERE employee_id = ?1",
            params![det.employee_id as i64],
        )
        .map_err(|e| format!("gagal menonaktifkan akun: {e}"))?;
    }
    audit::log(
        conn,
        Some(user_id),
        "APPROVE",
        "offboarding",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Tahap menjadi {next}")),
    )?;
    Ok(next.to_string())
}

pub fn save_exit_interview(
    conn: &Connection,
    actor_id: i64,
    offboarding_id: i64,
    input: &ExitInterviewInput,
) -> Result<(), String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM offboarding WHERE id = ?1",
            params![offboarding_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa resign: {e}"))?;
    if exists.is_none() {
        return Err("Proses resign tidak ditemukan.".to_string());
    }
    if let Some(score) = input.satisfaction_score {
        if score < 1 || score > 5 {
            return Err("Skor kepuasan harus 1 sampai 5.".to_string());
        }
    }
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM exit_interviews WHERE offboarding_id = ?1",
            params![offboarding_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa interview: {e}"))?;
    let rec = input.would_recommend.map(|v| if v { 1 } else { 0 });
    match existing {
        Some(eid) => {
            conn.execute(
                "UPDATE exit_interviews SET feedback = ?1, reason_category = ?2, would_recommend = ?3, satisfaction_score = ?4, conducted_by = ?5 WHERE id = ?6",
                params![
                    input.feedback.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    input.reason_category.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    rec,
                    input.satisfaction_score.map(|v| v as i64),
                    actor_id,
                    eid
                ],
            )
            .map_err(|e| format!("gagal menyimpan interview: {e}"))?;
        }
        None => {
            conn.execute(
                "INSERT INTO exit_interviews (offboarding_id, conducted_by, feedback, reason_category, would_recommend, satisfaction_score) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    offboarding_id,
                    actor_id,
                    input.feedback.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    input.reason_category.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                    rec,
                    input.satisfaction_score.map(|v| v as i64),
                ],
            )
            .map_err(|e| format!("gagal menyimpan interview: {e}"))?;
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "offboarding.exit_interview",
        Some(&offboarding_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn toggle_clearance(
    conn: &Connection,
    actor_id: i64,
    item_id: i64,
    cleared: bool,
    notes: Option<&str>,
) -> Result<(), String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM clearance_items WHERE id = ?1",
            params![item_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat item: {e}"))?;
    if exists.is_none() {
        return Err("Item tidak ditemukan.".to_string());
    }
    let now = now_str();
    conn.execute(
        "UPDATE clearance_items SET is_cleared = ?1, cleared_by = ?2, cleared_at = ?3, notes = ?4 WHERE id = ?5",
        params![
            if cleared { 1 } else { 0 },
            if cleared { Some(actor_id) } else { None },
            if cleared { Some(now) } else { None },
            notes.map(str::trim).filter(|s| !s.is_empty()),
            item_id
        ],
    )
    .map_err(|e| format!("gagal mengubah clearance: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "offboarding.clearance",
        Some(&item_id.to_string()),
        None,
        None,
        None,
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
    fn resign_empat_tahap_menonaktifkan_akun() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let admin_emp: i64 = conn
            .query_row(
                "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let (uid, eid) = mkuser(&conn, "resign1", "EMP-R1", Some(admin_emp));
        let id = create(
            &conn,
            uid,
            eid,
            &OffboardingCreate {
                resignation_date: "2026-11-01".to_string(),
                last_working_date: "2026-11-30".to_string(),
                reason: Some("Pindah".to_string()),
            },
        )
        .expect("buat");
        // ganda ditolak
        assert!(create(
            &conn,
            uid,
            eid,
            &OffboardingCreate {
                resignation_date: "2026-12-01".to_string(),
                last_working_date: "2026-12-31".to_string(),
                reason: None,
            },
        )
        .is_err());
        // tanggal terbalik ditolak
        assert!(create(
            &conn,
            uid,
            eid + 999,
            &OffboardingCreate {
                resignation_date: "2026-12-31".to_string(),
                last_working_date: "2026-12-01".to_string(),
                reason: None,
            },
        )
        .is_err());
        let det = find(&conn, id as i64).expect("find").expect("ada");
        assert_eq!(det.clearance_items.len(), 4);
        // orang asing tak bisa approve tahap supervisor
        let (other_uid, _) = mkuser(&conn, "asing1", "EMP-R2", None);
        let e = decide(
            &conn,
            other_uid,
            Some(9999),
            false,
            id as i64,
            "approve",
            None,
        )
        .expect_err("otorisasi");
        assert!(e.contains("berwenang"));
        // supervisor -> hr -> finance (admin privileged)
        assert_eq!(
            decide(
                &conn,
                admin,
                Some(admin_emp),
                false,
                id as i64,
                "approve",
                None
            )
            .expect("s1"),
            "supervisor_approved"
        );
        assert_eq!(
            decide(
                &conn,
                admin,
                Some(admin_emp),
                true,
                id as i64,
                "approve",
                None
            )
            .expect("s2"),
            "hr_approved"
        );
        // clearance + exit interview di tengah jalan
        let det = find(&conn, id as i64).expect("find").expect("ada");
        toggle_clearance(&conn, admin, det.clearance_items[0].id as i64, true, None)
            .expect("clearance");
        save_exit_interview(
            &conn,
            admin,
            id as i64,
            &ExitInterviewInput {
                feedback: Some("Baik".to_string()),
                reason_category: Some("karier".to_string()),
                would_recommend: Some(true),
                satisfaction_score: Some(4),
            },
        )
        .expect("exit");
        let det = find(&conn, id as i64).expect("find").expect("ada");
        assert_eq!(
            det.exit_interview
                .as_ref()
                .and_then(|e| e.satisfaction_score),
            Some(4)
        );
        assert!(save_exit_interview(
            &conn,
            admin,
            id as i64,
            &ExitInterviewInput {
                feedback: None,
                reason_category: None,
                would_recommend: None,
                satisfaction_score: Some(6),
            },
        )
        .is_err());
        assert_eq!(
            decide(
                &conn,
                admin,
                Some(admin_emp),
                true,
                id as i64,
                "approve",
                None
            )
            .expect("s3"),
            "finance_approved"
        );
        assert_eq!(
            decide(
                &conn,
                admin,
                Some(admin_emp),
                true,
                id as i64,
                "approve",
                None
            )
            .expect("s4"),
            "completed"
        );
        let status: String = conn
            .query_row(
                "SELECT employment_status FROM employees WHERE id = ?1",
                params![eid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "resigned");
        let ustatus: String = conn
            .query_row(
                "SELECT status FROM users WHERE id = ?1",
                params![uid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ustatus, "inactive");
        // selesai tak bisa diproses lagi
        assert!(decide(
            &conn,
            admin,
            Some(admin_emp),
            true,
            id as i64,
            "approve",
            None
        )
        .is_err());
        let _ = other_uid;
    }

    #[tokio::test]
    async fn resign_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let admin_emp: i64 = conn
            .query_row(
                "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let (uid, eid) = mkuser(&conn, "searesign", "EMP-SEAR", Some(admin_emp));
        let id = create_sea(
            db,
            uid,
            eid,
            &OffboardingCreate {
                resignation_date: "2026-11-01".to_string(),
                last_working_date: "2026-11-30".to_string(),
                reason: Some("Pindah".to_string()),
            },
        )
        .await
        .expect("buat");
        let e_sea = create_sea(
            db,
            uid,
            eid,
            &OffboardingCreate {
                resignation_date: "2026-12-01".to_string(),
                last_working_date: "2026-12-31".to_string(),
                reason: None,
            },
        )
        .await
        .expect_err("ganda");
        let e_sync = create(
            &conn,
            uid,
            eid,
            &OffboardingCreate {
                resignation_date: "2026-12-01".to_string(),
                last_working_date: "2026-12-31".to_string(),
                reason: None,
            },
        )
        .expect_err("ganda sync");
        assert_eq!(e_sea, e_sync);
        let f_sync = serde_json::to_string(&find(&conn, id as i64).expect("fs")).unwrap();
        let f_sea = serde_json::to_string(&find_sea(db, id as i64).await.expect("fse")).unwrap();
        assert_eq!(f_sync, f_sea);
        assert!(f_sea.contains("\"clearance_items\":[{"));
        assert_eq!(
            decide_sea(db, admin, Some(admin_emp), false, id as i64, "approve", None)
                .await
                .expect("s1"),
            "supervisor_approved"
        );
        assert_eq!(
            decide_sea(db, admin, Some(admin_emp), true, id as i64, "approve", None)
                .await
                .expect("s2"),
            "hr_approved"
        );
        let det = find_sea(db, id as i64).await.expect("find").expect("ada");
        toggle_clearance_sea(db, admin, det.clearance_items[0].id as i64, true, None)
            .await
            .expect("clearance");
        save_exit_interview_sea(
            db,
            admin,
            id as i64,
            &ExitInterviewInput {
                feedback: Some("Baik".to_string()),
                reason_category: Some("karier".to_string()),
                would_recommend: Some(true),
                satisfaction_score: Some(4),
            },
        )
        .await
        .expect("exit");
        let l_sync = serde_json::to_string(&list(&conn).expect("ls")).unwrap();
        let l_sea = serde_json::to_string(&list_sea(db).await.expect("lse")).unwrap();
        assert_eq!(l_sync, l_sea);
        assert_eq!(
            decide_sea(db, admin, Some(admin_emp), true, id as i64, "approve", None)
                .await
                .expect("s3"),
            "finance_approved"
        );
        assert_eq!(
            decide_sea(db, admin, Some(admin_emp), true, id as i64, "approve", None)
                .await
                .expect("s4"),
            "completed"
        );
        let d_sync = serde_json::to_string(&find(&conn, id as i64).expect("ds")).unwrap();
        let d_sea = serde_json::to_string(&find_sea(db, id as i64).await.expect("dse")).unwrap();
        assert_eq!(d_sync, d_sea);
        let status: String = conn
            .query_row(
                "SELECT employment_status FROM employees WHERE id = ?1",
                params![eid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "resigned");
    }
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

fn fopt_i(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match value_i64(v) {
        Some(x) => Ok(Some(to_dto_int(x, f)?)),
        None => Ok(None),
    }
}

fn fopt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

async fn frow_id(db: &sea_orm::DatabaseConnection, label: &str) -> Result<i64, String> {
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

fn map_off_row(r: &[Value]) -> Result<OffboardingRow, String> {
    Ok(OffboardingRow {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "off.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "off.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        resignation_date: value_to_string(&r[4]),
        last_working_date: value_to_string(&r[5]),
        status: value_to_string(&r[6]),
        current_step: to_dto_int(value_i64(&r[7]).unwrap_or(0), "off.step")?,
    })
}

pub async fn list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<OffboardingRow>, String> {
    let rows = q_all(
        db,
        "SELECT o.id, o.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, o.resignation_date, o.last_working_date, o.status, o.current_step FROM offboarding o INNER JOIN employees e ON e.id = o.employee_id ORDER BY CASE o.status WHEN 'pending' THEN 0 WHEN 'supervisor_approved' THEN 1 WHEN 'hr_approved' THEN 2 WHEN 'finance_approved' THEN 3 WHEN 'completed' THEN 4 ELSE 5 END, o.created_at DESC".to_string(),
        vec![],
        8,
        "offboarding.list",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    rows.iter().map(|r| map_off_row(r)).collect()
}

pub async fn my_requests_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<OffboardingRow>, String> {
    Ok(list_sea(db)
        .await?
        .into_iter()
        .filter(|r| r.employee_id as i64 == employee_id)
        .collect())
}

pub async fn find_sea(
    db: &sea_orm::DatabaseConnection,
    id: i64,
) -> Result<Option<OffboardingDetail>, String> {
    let row = q_one(
        db,
        "SELECT o.id, o.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, e.supervisor_id, o.resignation_date, o.last_working_date, o.reason, o.status, o.current_step FROM offboarding o INNER JOIN employees e ON e.id = o.employee_id WHERE o.id = ?1".to_string(),
        vec![Value::Int(id)],
        10,
        "offboarding.find",
    )
    .await
    .map_err(|e| format!("gagal memuat resign: {e}"))?;
    let Some(r) = row else {
        return Ok(None);
    };
    let (oid, emp, sup, step) = (
        value_i64(&r[0]).unwrap_or(0),
        value_i64(&r[1]).unwrap_or(0),
        value_i64(&r[4]),
        value_i64(&r[9]).unwrap_or(0),
    );
    let exit_row = q_one(
        db,
        "SELECT feedback, reason_category, would_recommend, satisfaction_score FROM exit_interviews WHERE offboarding_id = ?1".to_string(),
        vec![Value::Int(oid)],
        4,
        "offboarding.exit",
    )
    .await
    .map_err(|e| format!("gagal memuat exit interview: {e}"))?;
    let mut items = Vec::new();
    let irows = q_all(
        db,
        "SELECT id, item_name, department, is_cleared, cleared_at, notes FROM clearance_items WHERE offboarding_id = ?1 ORDER BY id".to_string(),
        vec![Value::Int(oid)],
        6,
        "offboarding.clearance",
    )
    .await
    .map_err(|e| format!("gagal membaca clearance: {e}"))?;
    for v in &irows {
        items.push(ClearanceItem {
            id: to_dto_int(value_i64(&v[0]).unwrap_or(0), "clearance.id")?,
            item_name: value_to_string(&v[1]),
            department: fopt_text(&v[2]),
            is_cleared: value_i64(&v[3]).unwrap_or(0) != 0,
            cleared_at: fopt_text(&v[4]),
            notes: fopt_text(&v[5]),
        });
    }
    Ok(Some(OffboardingDetail {
        id: to_dto_int(oid, "off.id")?,
        employee_id: to_dto_int(emp, "off.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        supervisor_id: fopt_i(&r[4], "off.ref")?,
        resignation_date: value_to_string(&r[5]),
        last_working_date: value_to_string(&r[6]),
        reason: fopt_text(&r[7]),
        status: value_to_string(&r[8]),
        current_step: to_dto_int(step, "off.step")?,
        exit_interview: exit_row
            .map(|e| {
                Ok::<_, String>(ExitInterview {
                    feedback: fopt_text(&e[0]),
                    reason_category: fopt_text(&e[1]),
                    would_recommend: value_i64(&e[2]).map(|v| v != 0),
                    satisfaction_score: match value_i64(&e[3]) {
                        Some(v) => Some(to_dto_int(v, "off.ref")?),
                        None => None,
                    },
                })
            })
            .transpose()?,
        clearance_items: items,
    }))
}

pub async fn create_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    input: &OffboardingCreate,
) -> Result<i32, String> {
    NaiveDate::parse_from_str(input.resignation_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal resign tidak valid.".to_string())?;
    NaiveDate::parse_from_str(input.last_working_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal terakhir kerja tidak valid.".to_string())?;
    if input.last_working_date.trim() < input.resignation_date.trim() {
        return Err("Tanggal terakhir kerja sebelum tanggal resign.".to_string());
    }
    if let Some(reason) = input.reason.as_deref() {
        if reason.len() > 255 {
            return Err("Alasan maksimal 255 karakter.".to_string());
        }
    }
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "offboarding.empcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let active = q_one(
        db,
        "SELECT id FROM offboarding WHERE employee_id = ?1 AND status NOT IN ('completed','rejected')".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "offboarding.active",
    )
    .await
    .map_err(|e| format!("gagal memeriksa proses berjalan: {e}"))?;
    if active.is_some() {
        return Err("Sudah ada proses resign yang berjalan untuk karyawan ini.".to_string());
    }
    exec(
        db,
        "INSERT INTO offboarding (employee_id, resignation_date, last_working_date, reason, status, current_step) VALUES (?1, ?2, ?3, ?4, 'pending', 1)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(input.resignation_date.trim().to_string()),
            Value::Text(input.last_working_date.trim().to_string()),
            match input.reason.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
        ],
        "offboarding.create",
    )
    .await
    .map_err(|e| format!("gagal mengajukan resign: {e}"))?;
    let rid = frow_id(db, "offboarding.create").await?;
    for item in DEFAULT_CLEARANCE {
        exec(
            db,
            "INSERT INTO clearance_items (offboarding_id, item_name, is_cleared) VALUES (?1, ?2, 0)".to_string(),
            vec![Value::Int(rid), Value::Text(item.to_string())],
            "offboarding.clearadd",
        )
        .await
        .map_err(|e| format!("gagal membuat clearance: {e}"))?;
    }
    let sup_row = q_one(
        db,
        "SELECT supervisor_id FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "offboarding.sup",
    )
    .await
    .map_err(|e| format!("gagal memuat supervisor: {e}"))?;
    if let Some(sid) = sup_row.as_ref().and_then(|r| value_i64(&r[0])) {
        if let Some(uid) = approval::user_of_employee_sea(db, sid).await? {
            let name_row = q_one(
                db,
                "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
                vec![Value::Int(employee_id)],
                1,
                "offboarding.empname",
            )
            .await
            .unwrap_or(None);
            let name = name_row
                .as_ref()
                .map(|r| value_to_string(&r[0]))
                .unwrap_or_default();
            approval::notify_sea(
                db,
                uid,
                "offboarding",
                "Pengajuan Resign Baru",
                &format!("{name} mengajukan resign."),
                "/offboarding",
            )
            .await?;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "offboarding",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "off.id")
}

/// Putuskan tahap berjalan. Tahap pending = supervisor; sisanya izin.
pub async fn decide_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    user_employee_id: Option<i64>,
    privileged: bool,
    id: i64,
    action: &str,
    _notes: Option<&str>,
) -> Result<String, String> {
    if action != "approve" && action != "reject" {
        return Err("Aksi tidak valid.".to_string());
    }
    let det = find_sea(db, id)
        .await?
        .ok_or("Proses resign tidak ditemukan.".to_string())?;
    if det.status == "completed" || det.status == "rejected" {
        return Err("Proses sudah selesai.".to_string());
    }
    let can = if det.status == "pending" {
        match (user_employee_id, det.supervisor_id) {
            (Some(a), Some(s)) if a as i64 == s as i64 => true,
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
            "UPDATE offboarding SET status = 'rejected' WHERE id = ?1".to_string(),
            vec![Value::Int(id)],
            "offboarding.reject",
        )
        .await
        .map_err(|e| format!("gagal menolak: {e}"))?;
        audit::log_sea(
            db,
            Some(user_id),
            "REJECT",
            "offboarding",
            Some(&id.to_string()),
            None,
            None,
            None,
        )
        .await?;
        return Ok("rejected".to_string());
    }
    let idx = STAGES
        .iter()
        .position(|s| *s == det.status)
        .ok_or("Status tidak dikenal.".to_string())?;
    let next = STAGES
        .get(idx + 1)
        .ok_or("Sudah tahap akhir.".to_string())?;
    exec(
        db,
        "UPDATE offboarding SET status = ?1, current_step = ?2 WHERE id = ?3".to_string(),
        vec![
            Value::Text(next.to_string()),
            Value::Int((idx + 2) as i64),
            Value::Int(id),
        ],
        "offboarding.advance",
    )
    .await
    .map_err(|e| format!("gagal maju tahap: {e}"))?;
    if *next == "completed" {
        exec(
            db,
            "UPDATE employees SET employment_status = 'resigned', resign_date = ?1 WHERE id = ?2".to_string(),
            vec![
                Value::Text(det.last_working_date.clone()),
                Value::Int(det.employee_id as i64),
            ],
            "offboarding.resign",
        )
        .await
        .map_err(|e| format!("gagal menandai resign: {e}"))?;
        exec(
            db,
            "UPDATE users SET status = 'inactive' WHERE employee_id = ?1".to_string(),
            vec![Value::Int(det.employee_id as i64)],
            "offboarding.deactivate",
        )
        .await
        .map_err(|e| format!("gagal menonaktifkan akun: {e}"))?;
    }
    audit::log_sea(
        db,
        Some(user_id),
        "APPROVE",
        "offboarding",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Tahap menjadi {next}")),
    )
    .await?;
    Ok(next.to_string())
}

pub async fn save_exit_interview_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    offboarding_id: i64,
    input: &ExitInterviewInput,
) -> Result<(), String> {
    let exists = q_one(
        db,
        "SELECT id FROM offboarding WHERE id = ?1".to_string(),
        vec![Value::Int(offboarding_id)],
        1,
        "offboarding.excheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa resign: {e}"))?;
    if exists.is_none() {
        return Err("Proses resign tidak ditemukan.".to_string());
    }
    if let Some(score) = input.satisfaction_score {
        if score < 1 || score > 5 {
            return Err("Skor kepuasan harus 1 sampai 5.".to_string());
        }
    }
    let existing = q_one(
        db,
        "SELECT id FROM exit_interviews WHERE offboarding_id = ?1".to_string(),
        vec![Value::Int(offboarding_id)],
        1,
        "offboarding.exexisting",
    )
    .await
    .map_err(|e| format!("gagal memeriksa interview: {e}"))?;
    let rec = input.would_recommend.map(|v| if v { 1 } else { 0 });
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    match existing {
        Some(e) => {
            let eid = value_i64(&e[0]).unwrap_or(0);
            exec(
                db,
                "UPDATE exit_interviews SET feedback = ?1, reason_category = ?2, would_recommend = ?3, satisfaction_score = ?4, conducted_by = ?5 WHERE id = ?6".to_string(),
                vec![
                    opt_t(input.feedback.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                    opt_t(input.reason_category.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                    match rec {
                        Some(v) => Value::Int(v),
                        None => Value::Null,
                    },
                    match input.satisfaction_score {
                        Some(v) => Value::Int(v as i64),
                        None => Value::Null,
                    },
                    Value::Int(actor_id),
                    Value::Int(eid),
                ],
                "offboarding.exitupd",
            )
            .await
            .map_err(|e| format!("gagal menyimpan interview: {e}"))?;
        }
        None => {
            exec(
                db,
                "INSERT INTO exit_interviews (offboarding_id, conducted_by, feedback, reason_category, would_recommend, satisfaction_score) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
                vec![
                    Value::Int(offboarding_id),
                    Value::Int(actor_id),
                    opt_t(input.feedback.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                    opt_t(input.reason_category.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                    match rec {
                        Some(v) => Value::Int(v),
                        None => Value::Null,
                    },
                    match input.satisfaction_score {
                        Some(v) => Value::Int(v as i64),
                        None => Value::Null,
                    },
                ],
                "offboarding.exitadd",
            )
            .await
            .map_err(|e| format!("gagal menyimpan interview: {e}"))?;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "offboarding.exit_interview",
        Some(&offboarding_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn toggle_clearance_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    item_id: i64,
    cleared: bool,
    notes: Option<&str>,
) -> Result<(), String> {
    let exists = q_one(
        db,
        "SELECT id FROM clearance_items WHERE id = ?1".to_string(),
        vec![Value::Int(item_id)],
        1,
        "offboarding.itemcheck",
    )
    .await
    .map_err(|e| format!("gagal memuat item: {e}"))?;
    if exists.is_none() {
        return Err("Item tidak ditemukan.".to_string());
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    exec(
        db,
        "UPDATE clearance_items SET is_cleared = ?1, cleared_by = ?2, cleared_at = ?3, notes = ?4 WHERE id = ?5".to_string(),
        vec![
            Value::Int(if cleared { 1 } else { 0 }),
            if cleared {
                Value::Int(actor_id)
            } else {
                Value::Null
            },
            if cleared {
                Value::Text(now)
            } else {
                Value::Null
            },
            match notes.map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Int(item_id),
        ],
        "offboarding.itemupd",
    )
    .await
    .map_err(|e| format!("gagal mengubah clearance: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "offboarding.clearance",
        Some(&item_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}
