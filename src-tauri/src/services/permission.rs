//! Izin non-cuti: satu tahap oleh atasan.

use chrono::{Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};

use super::approval;
use super::audit;
use crate::to_dto_int;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PermissionType {
    pub id: i32,
    pub code: String,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PermissionRequest {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub permission_type_id: i32,
    pub permission_type_name: String,
    pub date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub reason: String,
    pub status: String,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct PermissionCreate {
    pub permission_type_id: i32,
    pub date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub reason: String,
}

pub fn types(conn: &Connection) -> Result<Vec<PermissionType>, String> {
    let mut stmt = conn
        .prepare("SELECT id, code, name FROM permission_types ORDER BY name")
        .map_err(|e| format!("gagal menyiapkan tipe izin: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca tipe: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(PermissionType {
            id: to_dto_int(id, "permtype.id")?,
            code,
            name,
        });
    }
    Ok(out)
}

fn map_row(
    id: i64,
    emp: i64,
    name: String,
    number: String,
    tid: i64,
    tname: String,
    date: String,
    start: Option<String>,
    end: Option<String>,
    reason: String,
    status: String,
    created: String,
) -> Result<PermissionRequest, String> {
    Ok(PermissionRequest {
        id: to_dto_int(id, "perm.id")?,
        employee_id: to_dto_int(emp, "perm.emp")?,
        employee_name: name,
        employee_number: number,
        permission_type_id: to_dto_int(tid, "perm.type")?,
        permission_type_name: tname,
        date,
        start_time: start,
        end_time: end,
        reason,
        status,
        created_at: created,
    })
}

const SELECT: &str = "SELECT pr.id, pr.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, pr.permission_type_id, pt.name, pr.date, pr.start_time, pr.end_time, pr.reason, pr.status, pr.created_at FROM permission_requests pr INNER JOIN permission_types pt ON pt.id = pr.permission_type_id INNER JOIN employees e ON e.id = pr.employee_id";

type PermRow = (
    i64,
    i64,
    String,
    String,
    i64,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
);

fn map_perm_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<PermRow> {
    Ok((
        r.get(0)?,
        r.get(1)?,
        r.get(2)?,
        r.get(3)?,
        r.get(4)?,
        r.get(5)?,
        r.get(6)?,
        r.get(7)?,
        r.get(8)?,
        r.get(9)?,
        r.get(10)?,
        r.get(11)?,
    ))
}

fn collect(
    stmt: &mut rusqlite::Statement,
    param: Option<i64>,
    joint: bool,
) -> Result<Vec<PermissionRequest>, String> {
    let rows = if joint {
        stmt.query_map(params![param.unwrap_or(0)], map_perm_row)
            .map_err(|e| format!("gagal membaca daftar: {e}"))?
    } else {
        stmt.query_map([], map_perm_row)
            .map_err(|e| format!("gagal membaca daftar: {e}"))?
    };
    let mut out = Vec::new();
    for row in rows {
        let t = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(map_row(
            t.0, t.1, t.2, t.3, t.4, t.5, t.6, t.7, t.8, t.9, t.10, t.11,
        )?);
    }
    Ok(out)
}

pub fn my_requests(conn: &Connection, employee_id: i64) -> Result<Vec<PermissionRequest>, String> {
    let mut stmt = conn
        .prepare(&format!(
            "{SELECT} WHERE pr.employee_id = ?1 ORDER BY pr.created_at DESC"
        ))
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    collect(&mut stmt, Some(employee_id), true)
}

/// Menunggu putusan user: ia supervisor langsung ATAU pemegang izin.
pub fn pending_for(
    conn: &Connection,
    user_id: i64,
    privileged: bool,
) -> Result<Vec<PermissionRequest>, String> {
    let emp = approval::employee_of_user(conn, user_id)?.unwrap_or(0);
    let mut stmt = conn
        .prepare(&format!("{SELECT} WHERE pr.status = 'pending' AND e.supervisor_id = ?1 ORDER BY pr.created_at ASC"))
        .map_err(|e| format!("gagal menyiapkan antrean: {e}"))?;
    let mut out = collect(&mut stmt, Some(emp), true)?;
    if privileged {
        let mut stmt = conn
            .prepare(&format!("{SELECT} WHERE pr.status = 'pending' AND (e.supervisor_id IS NULL OR e.supervisor_id != ?1) ORDER BY pr.created_at ASC"))
            .map_err(|e| format!("gagal menyiapkan antrean: {e}"))?;
        out.extend(collect(&mut stmt, Some(emp), true)?);
    }
    Ok(out)
}

pub fn all_requests(conn: &Connection) -> Result<Vec<PermissionRequest>, String> {
    let mut stmt = conn
        .prepare(&format!("{SELECT} ORDER BY CASE pr.status WHEN 'pending' THEN 0 WHEN 'approved' THEN 1 ELSE 2 END, pr.created_at DESC"))
        .map_err(|e| format!("gagal menyiapkan semua: {e}"))?;
    collect(&mut stmt, None, false)
}

pub fn create(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    input: &PermissionCreate,
) -> Result<i32, String> {
    let type_ok: Option<i64> = conn
        .query_row(
            "SELECT id FROM permission_types WHERE id = ?1",
            params![input.permission_type_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa tipe: {e}"))?;
    if type_ok.is_none() {
        return Err("Tipe izin tidak ditemukan.".to_string());
    }
    NaiveDate::parse_from_str(input.date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal tidak valid.".to_string())?;
    if input.reason.trim().is_empty() {
        return Err("Alasan wajib diisi.".to_string());
    }
    if input.reason.len() > 255 {
        return Err("Alasan maksimal 255 karakter.".to_string());
    }
    let start = input
        .start_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let end = input
        .end_time
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    conn.execute(
        "INSERT INTO permission_requests (employee_id, permission_type_id, date, start_time, end_time, reason, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')",
        params![employee_id, input.permission_type_id, input.date.trim(), start, end, input.reason.trim()],
    )
    .map_err(|e| format!("gagal mengajukan izin: {e}"))?;
    let rid = conn.last_insert_rowid();
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
                "permission_request",
                "Pengajuan Izin Baru",
                &format!("{name} mengajukan izin."),
                "/leave",
            )?;
        }
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "permission_request",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "perm.id")
}

pub fn decide(
    conn: &Connection,
    approver_user_id: i64,
    approver_employee_id: Option<i64>,
    privileged: bool,
    id: i64,
    decision: &str,
) -> Result<(), String> {
    if decision != "approved" && decision != "rejected" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let req: Option<(Option<i64>, String)> = conn
        .query_row(
            "SELECT e.supervisor_id, pr.status FROM permission_requests pr INNER JOIN employees e ON e.id = pr.employee_id WHERE pr.id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengajuan: {e}"))?;
    let Some((supervisor, status)) = req else {
        return Err("Pengajuan tidak ditemukan.".to_string());
    };
    if status != "pending" {
        return Err("Pengajuan sudah diproses.".to_string());
    }
    let is_supervisor = approver_employee_id == supervisor;
    if !is_supervisor && !privileged {
        return Err("Hanya supervisor atau HR yang boleh memutuskan.".to_string());
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE permission_requests SET status = ?1, approved_by = ?2, approved_at = ?3 WHERE id = ?4",
        params![decision, approver_user_id, now, id],
    )
    .map_err(|e| format!("gagal menyimpan putusan: {e}"))?;
    let emp_id: i64 = conn
        .query_row(
            "SELECT employee_id FROM permission_requests WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if let Some(uid) = approval::user_of_employee(conn, emp_id)? {
        approval::notify(
            conn,
            uid,
            "permission_request",
            if decision == "approved" {
                "Izin Disetujui"
            } else {
                "Izin Ditolak"
            },
            &format!("Pengajuan izin Anda {decision}."),
            "/leave",
        )?;
    }
    audit::log(
        conn,
        Some(approver_user_id),
        decision.to_uppercase().as_str(),
        "permission_request",
        Some(&id.to_string()),
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
    fn izin_satu_tahap_oleh_supervisor() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let (sup_uid, sup_eid) = mkuser(&conn, "spv2", "EMP-P1", None);
        let (uid, eid) = mkuser(&conn, "staff2", "EMP-P2", Some(sup_eid));
        assert!(!types(&conn).expect("tipe").is_empty());
        let tid: i64 = conn
            .query_row(
                "SELECT id FROM permission_types WHERE code = 'PERSONAL'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let id = create(
            &conn,
            uid,
            eid,
            &PermissionCreate {
                permission_type_id: tid as i32,
                date: "2026-05-04".to_string(),
                start_time: Some("10:00".to_string()),
                end_time: Some("12:00".to_string()),
                reason: "Keperluan keluarga".to_string(),
            },
        )
        .expect("buat") as i64;
        assert_eq!(my_requests(&conn, eid).expect("mine").len(), 1);
        assert_eq!(
            pending_for(&conn, sup_uid, false).expect("antrean").len(),
            1
        );
        // rekan kerja ditolak
        let (other_uid, _) = mkuser(&conn, "lain2", "EMP-P3", None);
        let other_emp = approval::employee_of_user(&conn, other_uid)
            .unwrap()
            .unwrap();
        let e = decide(&conn, other_uid, Some(other_emp), false, id, "approved")
            .expect_err("otorisasi");
        assert!(e.contains("supervisor"));
        // supervisor menyetujui langsung final
        decide(&conn, sup_uid, Some(sup_eid), false, id, "approved").expect("setuju");
        let status: String = conn
            .query_row(
                "SELECT status FROM permission_requests WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "approved");
        let _ = sup_uid;
    }

    #[test]
    fn izin_tanpa_supervisor_masuk_antrean_hr() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let (uid, eid) = mkuser(&conn, "solo2", "EMP-P4", None);
        let tid: i64 = conn
            .query_row(
                "SELECT id FROM permission_types WHERE code = 'SICK'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        create(
            &conn,
            uid,
            eid,
            &PermissionCreate {
                permission_type_id: tid as i32,
                date: "2026-05-05".to_string(),
                start_time: None,
                end_time: None,
                reason: "Sakit".to_string(),
            },
        )
        .expect("buat");
        // tanpa supervisor: antrean supervisor kosong, tapi HR privileged melihatnya
        assert!(pending_for(&conn, uid, false).expect("a").is_empty());
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(pending_for(&conn, admin, true).expect("b").len(), 1);
    }
}
