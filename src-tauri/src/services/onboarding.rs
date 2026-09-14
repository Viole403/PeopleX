//! Orientasi karyawan baru: checklist otomatis + progres.

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
