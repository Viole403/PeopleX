//! Orientasi karyawan baru: checklist otomatis + progres.

use rusqlite::{params, Connection, OptionalExtension};

use super::audit;
use crate::to_dto_int;

const DEFAULT_TASKS: &[&str] = &[
    "Buat Akun Sistem",
    "Tetapkan ID Karyawan",
    "Tetapkan Departemen & Jabatan",
    "Tetapkan Supervisor",
    "Tetapkan Jadwal Kerja",
    "Setup Payroll (Gaji Pokok)",
    "Pengumpulan Dokumen (KTP, NPWP, dll)",
    "Penyerahan Aset Kerja",
    "Pelatihan Orientasi",
    "Perkenalan Tim & Lingkungan Kerja",
];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct OnboardingTask {
    pub id: i32,
    pub task_name: String,
    pub is_completed: bool,
    pub completed_at: Option<String>,
    pub sort_order: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Onboarding {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub start_date: String,
    pub status: String,
    pub progress_percent: i32,
    pub tasks: Vec<OnboardingTask>,
}

/// Buat paket onboarding (idempoten per karyawan).
pub fn create_for_employee(
    conn: &Connection,
    actor_id: i64,
    employee_id: i64,
    candidate_id: Option<i64>,
    start_date: &str,
) -> Result<i32, String> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM onboarding WHERE employee_id = ?1",
            params![employee_id],
            |r| r.get::<_, i64>(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa onboarding: {e}"))?
    {
        return to_dto_int(id, "onboarding.id");
    }
    conn.execute(
        "INSERT INTO onboarding (employee_id, candidate_id, start_date, status, progress_percent) VALUES (?1, ?2, ?3, 'in_progress', 0)",
        params![employee_id, candidate_id, start_date],
    )
    .map_err(|e| format!("gagal membuat onboarding: {e}"))?;
    let id = conn.last_insert_rowid();
    for (i, task) in DEFAULT_TASKS.iter().enumerate() {
        conn.execute(
            "INSERT INTO onboarding_tasks (onboarding_id, task_name, is_completed, sort_order) VALUES (?1, ?2, 0, ?3)",
            params![id, task, i as i64],
        )
        .map_err(|e| format!("gagal membuat tugas: {e}"))?;
    }
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "onboarding",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(id, "onboarding.id")
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

    #[test]
    fn checklist_10_tugas_sampai_selesai() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let emp: i64 = conn
            .query_row(
                "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let id = create_for_employee(&conn, actor, emp, None, "2026-09-01").expect("buat");
        // idempoten
        assert_eq!(
            create_for_employee(&conn, actor, emp, None, "2026-09-01").expect("lagi"),
            id
        );
        let ob = find(&conn, id as i64).expect("find").expect("ada");
        assert_eq!(ob.tasks.len(), 10);
        assert_eq!(ob.progress_percent, 0);
        for t in &ob.tasks {
            toggle_task(&conn, actor, t.id as i64, true).expect("centang");
        }
        let ob = find(&conn, id as i64).expect("find").expect("ada");
        assert_eq!(ob.progress_percent, 100);
        assert_eq!(ob.status, "completed");
        assert_eq!(
            for_employee(&conn, emp).expect("mine").map(|o| o.id),
            Some(id)
        );
        assert_eq!(list(&conn).expect("list").len(), 1);
        assert!(toggle_task(&conn, actor, 999999, true).is_err());
    }

    #[tokio::test]
    async fn onboarding_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let emp: i64 = conn
            .query_row(
                "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let id = create_for_employee_sea(db, actor, emp, None, "2026-09-01")
            .await
            .expect("buat");
        assert_eq!(
            create_for_employee_sea(db, actor, emp, None, "2026-09-01")
                .await
                .expect("lagi"),
            id
        );
        let o_sync = serde_json::to_string(&find(&conn, id as i64).expect("os")).unwrap();
        let o_sea = serde_json::to_string(&find_sea(db, id as i64).await.expect("ose")).unwrap();
        assert_eq!(o_sync, o_sea);
        let ob = find_sea(db, id as i64).await.expect("find").expect("ada");
        assert_eq!(ob.tasks.len(), 10);
        for t in &ob.tasks {
            toggle_task_sea(db, actor, t.id as i64, true)
                .await
                .expect("centang");
        }
        let done_sync =
            serde_json::to_string(&find(&conn, id as i64).expect("fs")).unwrap();
        let done_sea =
            serde_json::to_string(&find_sea(db, id as i64).await.expect("fse")).unwrap();
        assert_eq!(done_sync, done_sea);
        assert!(done_sea.contains("\"status\":\"completed\""));
        assert!(done_sea.contains("\"progress_percent\":100"));
        let l_sync = serde_json::to_string(&list(&conn).expect("ls")).unwrap();
        let l_sea = serde_json::to_string(&list_sea(db).await.expect("lse")).unwrap();
        assert_eq!(l_sync, l_sea);
        let e_sea = toggle_task_sea(db, actor, 999999, true)
            .await
            .expect_err("tugas hilang");
        let e_sync = toggle_task(&conn, actor, 999999, true).expect_err("tugas hilang sync");
        assert_eq!(e_sea, e_sync);
    }
}

pub fn list(conn: &Connection) -> Result<Vec<Onboarding>, String> {
    let mut stmt = conn
        .prepare("SELECT o.id FROM onboarding o INNER JOIN employees e ON e.id = o.employee_id ORDER BY CASE o.status WHEN 'in_progress' THEN 0 ELSE 1 END, o.start_date DESC")
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, i64>(0))
        .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let id = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        if let Some(o) = find(conn, id)? {
            out.push(o);
        }
    }
    Ok(out)
}

pub fn find(conn: &Connection, id: i64) -> Result<Option<Onboarding>, String> {
    let row: Option<(i64, i64, String, String, String, String, i64)> = conn
        .query_row(
            "SELECT o.id, o.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, o.start_date, o.status, o.progress_percent FROM onboarding o INNER JOIN employees e ON e.id = o.employee_id WHERE o.id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat onboarding: {e}"))?;
    let Some((oid, emp, name, number, start, status, progress)) = row else {
        return Ok(None);
    };
    let mut tasks = Vec::new();
    let mut tstmt = conn
        .prepare("SELECT id, task_name, is_completed, completed_at, sort_order FROM onboarding_tasks WHERE onboarding_id = ?1 ORDER BY sort_order")
        .map_err(|e| format!("gagal menyiapkan tugas: {e}"))?;
    for trow in tstmt
        .query_map(params![oid], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca tugas: {e}"))?
    {
        let (tid, tname, done, at, order) =
            trow.map_err(|e| format!("gagal membaca baris tugas: {e}"))?;
        tasks.push(OnboardingTask {
            id: to_dto_int(tid, "task.id")?,
            task_name: tname,
            is_completed: done != 0,
            completed_at: at,
            sort_order: to_dto_int(order, "task.order")?,
        });
    }
    Ok(Some(Onboarding {
        id: to_dto_int(oid, "onboarding.id")?,
        employee_id: to_dto_int(emp, "onboarding.emp")?,
        employee_name: name,
        employee_number: number,
        start_date: start,
        status,
        progress_percent: to_dto_int(progress, "onboarding.progress")?,
        tasks,
    }))
}

pub fn for_employee(conn: &Connection, employee_id: i64) -> Result<Option<Onboarding>, String> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM onboarding WHERE employee_id = ?1",
            params![employee_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal mencari onboarding: {e}"))?;
    match id {
        Some(v) => find(conn, v),
        None => Ok(None),
    }
}

pub fn toggle_task(
    conn: &Connection,
    actor_id: i64,
    task_id: i64,
    completed: bool,
) -> Result<(i32, String), String> {
    let task: Option<(i64, i64)> = conn
        .query_row(
            "SELECT onboarding_id, is_completed FROM onboarding_tasks WHERE id = ?1",
            params![task_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat tugas: {e}"))?;
    let Some((oid, _)) = task else {
        return Err("Tugas tidak ditemukan.".to_string());
    };
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "UPDATE onboarding_tasks SET is_completed = ?1, completed_at = ?2, completed_by = ?3 WHERE id = ?4",
        params![
            if completed { 1 } else { 0 },
            if completed { Some(now.clone()) } else { None },
            if completed { Some(actor_id) } else { None },
            task_id
        ],
    )
    .map_err(|e| format!("gagal mengubah tugas: {e}"))?;
    let (total, done): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(is_completed), 0) FROM onboarding_tasks WHERE onboarding_id = ?1",
            params![oid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| format!("gagal menghitung progres: {e}"))?;
    let progress = if total > 0 { (done * 100) / total } else { 0 };
    let status = if progress >= 100 {
        "completed"
    } else {
        "in_progress"
    };
    conn.execute(
        "UPDATE onboarding SET progress_percent = ?1, status = ?2 WHERE id = ?3",
        params![progress, status, oid],
    )
    .map_err(|e| format!("gagal memperbarui progres: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "onboarding.task",
        Some(&task_id.to_string()),
        None,
        None,
        Some(&format!(
            "Tugas {}",
            if completed { "selesai" } else { "dibuka" }
        )),
    )?;
    Ok((
        to_dto_int(progress, "onboarding.progress")?,
        status.to_string(),
    ))
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

/// Buat paket onboarding (idempoten per karyawan).
pub async fn create_for_employee_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    candidate_id: Option<i64>,
    start_date: &str,
) -> Result<i32, String> {
    let row = q_one(
        db,
        "SELECT id FROM onboarding WHERE employee_id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "onboarding.check",
    )
    .await
    .map_err(|e| format!("gagal memeriksa onboarding: {e}"))?;
    if let Some(r) = row {
        return to_dto_int(value_i64(&r[0]).unwrap_or(0), "onboarding.id");
    }
    exec(
        db,
        "INSERT INTO onboarding (employee_id, candidate_id, start_date, status, progress_percent) VALUES (?1, ?2, ?3, 'in_progress', 0)".to_string(),
        vec![
            Value::Int(employee_id),
            match candidate_id {
                Some(c) => Value::Int(c),
                None => Value::Null,
            },
            Value::Text(start_date.to_string()),
        ],
        "onboarding.create",
    )
    .await
    .map_err(|e| format!("gagal membuat onboarding: {e}"))?;
    let id_row = q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        "onboarding.create",
    )
    .await
    .map_err(|e| format!("gagal membaca id baru: {e}"))?;
    let id = id_row
        .as_ref()
        .and_then(|r| value_i64(&r[0]))
        .unwrap_or(0);
    for (i, task) in DEFAULT_TASKS.iter().enumerate() {
        exec(
            db,
            "INSERT INTO onboarding_tasks (onboarding_id, task_name, is_completed, sort_order) VALUES (?1, ?2, 0, ?3)".to_string(),
            vec![
                Value::Int(id),
                Value::Text(task.to_string()),
                Value::Int(i as i64),
            ],
            "onboarding.taskadd",
        )
        .await
        .map_err(|e| format!("gagal membuat tugas: {e}"))?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "onboarding",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(id, "onboarding.id")
}

pub async fn find_sea(
    db: &sea_orm::DatabaseConnection,
    id: i64,
) -> Result<Option<Onboarding>, String> {
    let row = q_one(
        db,
        "SELECT o.id, o.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, o.start_date, o.status, o.progress_percent FROM onboarding o INNER JOIN employees e ON e.id = o.employee_id WHERE o.id = ?1".to_string(),
        vec![Value::Int(id)],
        7,
        "onboarding.find",
    )
    .await
    .map_err(|e| format!("gagal memuat onboarding: {e}"))?;
    let Some(r) = row else {
        return Ok(None);
    };
    let (oid, emp, progress) = (
        value_i64(&r[0]).unwrap_or(0),
        value_i64(&r[1]).unwrap_or(0),
        value_i64(&r[6]).unwrap_or(0),
    );
    let trows = q_all(
        db,
        "SELECT id, task_name, is_completed, completed_at, sort_order FROM onboarding_tasks WHERE onboarding_id = ?1 ORDER BY sort_order".to_string(),
        vec![Value::Int(oid)],
        5,
        "onboarding.tasks",
    )
    .await
    .map_err(|e| format!("gagal membaca tugas: {e}"))?;
    let mut tasks = Vec::new();
    for t in &trows {
        let (tid, done, order) = (
            value_i64(&t[0]).unwrap_or(0),
            value_i64(&t[2]).unwrap_or(0),
            value_i64(&t[4]).unwrap_or(0),
        );
        tasks.push(OnboardingTask {
            id: to_dto_int(tid, "task.id")?,
            task_name: value_to_string(&t[1]),
            is_completed: done != 0,
            completed_at: match &t[3] {
                Value::Null => None,
                _ => Some(value_to_string(&t[3])),
            },
            sort_order: to_dto_int(order, "task.order")?,
        });
    }
    Ok(Some(Onboarding {
        id: to_dto_int(oid, "onboarding.id")?,
        employee_id: to_dto_int(emp, "onboarding.emp")?,
        employee_name: value_to_string(&r[2]),
        employee_number: value_to_string(&r[3]),
        start_date: value_to_string(&r[4]),
        status: value_to_string(&r[5]),
        progress_percent: to_dto_int(progress, "onboarding.progress")?,
        tasks,
    }))
}

pub async fn for_employee_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Option<Onboarding>, String> {
    let row = q_one(
        db,
        "SELECT id FROM onboarding WHERE employee_id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "onboarding.byemp",
    )
    .await
    .map_err(|e| format!("gagal mencari onboarding: {e}"))?;
    match row {
        Some(r) => find_sea(db, value_i64(&r[0]).unwrap_or(0)).await,
        None => Ok(None),
    }
}

pub async fn list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Onboarding>, String> {
    let rows = q_all(
        db,
        "SELECT o.id FROM onboarding o INNER JOIN employees e ON e.id = o.employee_id ORDER BY CASE o.status WHEN 'in_progress' THEN 0 ELSE 1 END, o.start_date DESC".to_string(),
        vec![],
        1,
        "onboarding.list",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        if let Some(o) = find_sea(db, value_i64(&r[0]).unwrap_or(0)).await? {
            out.push(o);
        }
    }
    Ok(out)
}

pub async fn toggle_task_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    task_id: i64,
    completed: bool,
) -> Result<(i32, String), String> {
    let task = q_one(
        db,
        "SELECT onboarding_id, is_completed FROM onboarding_tasks WHERE id = ?1".to_string(),
        vec![Value::Int(task_id)],
        2,
        "onboarding.task",
    )
    .await
    .map_err(|e| format!("gagal memuat tugas: {e}"))?;
    let Some(task) = task else {
        return Err("Tugas tidak ditemukan.".to_string());
    };
    let oid = value_i64(&task[0]).unwrap_or(0);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    exec(
        db,
        "UPDATE onboarding_tasks SET is_completed = ?1, completed_at = ?2, completed_by = ?3 WHERE id = ?4".to_string(),
        vec![
            Value::Int(if completed { 1 } else { 0 }),
            if completed {
                Value::Text(now.clone())
            } else {
                Value::Null
            },
            if completed {
                Value::Int(actor_id)
            } else {
                Value::Null
            },
            Value::Int(task_id),
        ],
        "onboarding.toggle",
    )
    .await
    .map_err(|e| format!("gagal mengubah tugas: {e}"))?;
    let cnt = q_one(
        db,
        "SELECT COUNT(*), COALESCE(SUM(is_completed), 0) FROM onboarding_tasks WHERE onboarding_id = ?1".to_string(),
        vec![Value::Int(oid)],
        2,
        "onboarding.count",
    )
    .await
    .map_err(|e| format!("gagal menghitung progres: {e}"))?;
    let (total, done) = cnt
        .as_ref()
        .map(|r| (value_i64(&r[0]).unwrap_or(0), value_i64(&r[1]).unwrap_or(0)))
        .unwrap_or((0, 0));
    let progress = if total > 0 { (done * 100) / total } else { 0 };
    let status = if progress >= 100 {
        "completed"
    } else {
        "in_progress"
    };
    exec(
        db,
        "UPDATE onboarding SET progress_percent = ?1, status = ?2 WHERE id = ?3".to_string(),
        vec![
            Value::Int(progress),
            Value::Text(status.to_string()),
            Value::Int(oid),
        ],
        "onboarding.progress",
    )
    .await
    .map_err(|e| format!("gagal memperbarui progres: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "onboarding.task",
        Some(&task_id.to_string()),
        None,
        None,
        Some(&format!(
            "Tugas {}",
            if completed { "selesai" } else { "dibuka" }
        )),
    )
    .await?;
    Ok((
        to_dto_int(progress, "onboarding.progress")?,
        status.to_string(),
    ))
}
