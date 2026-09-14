//! Dasbor: ringkasan HR dan ringkasan mandiri karyawan.

use chrono::Local;
use rusqlite::{params, Connection};

use crate::to_dto_int;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct NameCount {
    pub name: String,
    pub count: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Birthday {
    pub name: String,
    pub employee_number: String,
    pub birth_date: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct HolidayLite {
    pub name: String,
    pub date: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct AnnounceLite {
    pub id: i32,
    pub title: String,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct HrDashboard {
    pub total_employees: i32,
    pub new_hires_this_month: i32,
    pub resigned_this_month: i32,
    pub active_contracts: i32,
    pub expiring_contracts: i32,
    pub pending_leave: i32,
    pub pending_overtime: i32,
    pub pending_corrections: i32,
    pub pending_trips: i32,
    pub present_today: i32,
    pub late_today: i32,
    pub birthdays_today: Vec<Birthday>,
    pub upcoming_holidays: Vec<HolidayLite>,
    pub recent_announcements: Vec<AnnounceLite>,
    pub headcount_by_department: Vec<NameCount>,
    pub headcount_by_type: Vec<NameCount>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct LeaveBalanceLite {
    pub leave_type: String,
    pub allocated: f64,
    pub used: f64,
    pub remaining: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct MySummary {
    pub employee_name: String,
    pub employee_number: String,
    pub department: Option<String>,
    pub position: Option<String>,
    pub today_status: Option<String>,
    pub today_clock_in: Option<String>,
    pub pending_leave: i32,
    pub pending_overtime: i32,
    pub pending_corrections: i32,
    pub my_assets: i32,
    pub unread_announcements: i32,
    pub leave_balances: Vec<LeaveBalanceLite>,
}

fn count(conn: &Connection, sql: &str) -> Result<i64, String> {
    conn.query_row(sql, [], |r| r.get(0))
        .map_err(|e| format!("gagal menghitung dasbor: {e}"))
}

fn count_p(conn: &Connection, sql: &str, p: &[&dyn rusqlite::ToSql]) -> Result<i64, String> {
    conn.query_row(sql, p, |r| r.get(0))
        .map_err(|e| format!("gagal menghitung dasbor: {e}"))
}

fn groups(conn: &Connection, sql: &str, label: &str) -> Result<Vec<NameCount>, String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("gagal menyiapkan {label}: {e}"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| format!("gagal membaca {label}: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (name, n) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(NameCount {
            name,
            count: to_dto_int(n, "dashboard.count")?,
        });
    }
    Ok(out)
}

pub fn hr(conn: &Connection) -> Result<HrDashboard, String> {
    let now = Local::now().naive_local();
    let today = now.format("%Y-%m-%d").to_string();
    let month = now.format("%Y-%m").to_string();
    let month_like = format!("{month}%");
    let contract_limit = (now + chrono::Duration::days(60))
        .format("%Y-%m-%d")
        .to_string();

    let birthdays = {
        let mut stmt = conn
            .prepare("SELECT TRIM(first_name || ' ' || COALESCE(last_name,'')), employee_number, birth_date FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation') AND birth_date IS NOT NULL AND strftime('%m-%d', birth_date) = strftime('%m-%d','now','localtime') ORDER BY first_name LIMIT 20")
            .map_err(|e| format!("gagal menyiapkan ulang tahun: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| format!("gagal membaca ulang tahun: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let (name, num, birth) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
            out.push(Birthday {
                name,
                employee_number: num,
                birth_date: birth,
            });
        }
        out
    };
    let holidays = {
        let mut stmt = conn
            .prepare("SELECT name, date FROM holidays WHERE date >= date('now','localtime') ORDER BY date LIMIT 5")
            .map_err(|e| format!("gagal menyiapkan libur: {e}"))?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| format!("gagal membaca libur: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let (name, date) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
            out.push(HolidayLite { name, date });
        }
        out
    };
    let anns = {
        let mut stmt = conn
            .prepare("SELECT id, title, created_at FROM announcements WHERE deleted_at IS NULL AND status = 'published' ORDER BY id DESC LIMIT 5")
            .map_err(|e| format!("gagal menyiapkan pengumuman: {e}"))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("gagal membaca pengumuman: {e}"))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, title, created) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
            out.push(AnnounceLite {
                id: to_dto_int(id, "dashboard.announcement")?,
                title,
                created_at: created,
            });
        }
        out
    };

    Ok(HrDashboard {
        total_employees: to_dto_int(count(conn, "SELECT COUNT(*) FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation')")?, "dashboard.total")?,
        new_hires_this_month: to_dto_int(count_p(conn, "SELECT COUNT(*) FROM employees WHERE deleted_at IS NULL AND join_date LIKE ?1", &[&month_like])?, "dashboard.hires")?,
        resigned_this_month: to_dto_int(count_p(conn, "SELECT COUNT(*) FROM employees WHERE resign_date LIKE ?1", &[&month_like])?, "dashboard.resigned")?,
        active_contracts: to_dto_int(count(conn, "SELECT COUNT(*) FROM employee_contracts WHERE status = 'active' AND deleted_at IS NULL")?, "dashboard.contracts")?,
        expiring_contracts: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM employee_contracts WHERE status = 'active' AND deleted_at IS NULL AND end_date IS NOT NULL AND end_date <= ?1",
                params![contract_limit],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung kontrak: {e}"))?,
            "dashboard.expiring",
        )?,
        pending_leave: to_dto_int(count(conn, "SELECT COUNT(*) FROM leave_requests WHERE status = 'pending'")?, "dashboard.leave")?,
        pending_overtime: to_dto_int(count(conn, "SELECT COUNT(*) FROM overtime_requests WHERE status = 'pending'")?, "dashboard.overtime")?,
        pending_corrections: to_dto_int(count(conn, "SELECT COUNT(*) FROM attendance_corrections WHERE status = 'pending'")?, "dashboard.corrections")?,
        pending_trips: to_dto_int(count(conn, "SELECT COUNT(*) FROM business_trips WHERE status = 'pending'")?, "dashboard.trips")?,
        present_today: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM attendances WHERE date = ?1 AND status IN ('present','late','wfh','business_trip')",
                params![today],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung hadir: {e}"))?,
            "dashboard.present",
        )?,
        late_today: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM attendances WHERE date = ?1 AND status = 'late'",
                params![today],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung telat: {e}"))?,
            "dashboard.late",
        )?,
        birthdays_today: birthdays,
        upcoming_holidays: holidays,
        recent_announcements: anns,
        headcount_by_department: groups(
            conn,
            "SELECT COALESCE(d.name,'(Tanpa departemen)'), COUNT(*) FROM employees e LEFT JOIN departments d ON d.id = e.department_id WHERE e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY d.name ORDER BY COUNT(*) DESC",
            "headcount",
        )?,
        headcount_by_type: groups(
            conn,
            "SELECT employment_type, COUNT(*) FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation') GROUP BY employment_type",
            "tipe",
        )?,
    })
}

pub fn mine(conn: &Connection, user_id: i64) -> Result<MySummary, String> {
    let emp: Option<i64> = conn
        .query_row(
            "SELECT employee_id FROM users WHERE id = ?1",
            params![user_id],
            |r| r.get::<_, Option<i64>>(0),
        )
        .map_err(|e| format!("gagal memuat akun: {e}"))?;
    let emp = emp.ok_or("Akun belum tertaut karyawan.".to_string())?;
    let (name, num, dept, pos): (String, String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT TRIM(e.first_name || ' ' || COALESCE(e.last_name,'')), e.employee_number, d.name, p.name FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN positions p ON p.id = e.position_id WHERE e.id = ?1",
            params![emp],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    let today = Local::now().naive_local().format("%Y-%m-%d").to_string();
    let (status, clock_in): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT status, clock_in FROM attendances WHERE employee_id = ?1 AND date = ?2",
            params![emp, today],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((None, None));
    let year: i32 = Local::now()
        .naive_local()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2000);
    let mut stmt = conn
        .prepare("SELECT t.name, b.allocated_days, b.used_days FROM leave_balances b INNER JOIN leave_types t ON t.id = b.leave_type_id WHERE b.employee_id = ?1 AND b.year = ?2")
        .map_err(|e| format!("gagal menyiapkan saldo: {e}"))?;
    let rows = stmt
        .query_map(params![emp, year], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, f64>(1)?,
                r.get::<_, f64>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca saldo: {e}"))?;
    let mut balances = Vec::new();
    for row in rows {
        let (t, alloc, used) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        balances.push(LeaveBalanceLite {
            leave_type: t,
            allocated: alloc,
            used,
            remaining: (alloc - used).max(0.0),
        });
    }
    let ann_unread: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM announcements a LEFT JOIN announcement_reads ar ON ar.announcement_id = a.id AND ar.employee_id = ?1 WHERE a.status = 'published' AND a.deleted_at IS NULL AND ar.id IS NULL",
            params![emp],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung pengumuman: {e}"))?;
    Ok(MySummary {
        employee_name: name,
        employee_number: num,
        department: dept,
        position: pos,
        today_status: status,
        today_clock_in: clock_in,
        pending_leave: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM leave_requests WHERE employee_id = ?1 AND status = 'pending'",
                params![emp],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung cuti: {e}"))?,
            "dashboard.myleave",
        )?,
        pending_overtime: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM overtime_requests WHERE employee_id = ?1 AND status = 'pending'",
                params![emp],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung lembur: {e}"))?,
            "dashboard.myot",
        )?,
        pending_corrections: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM attendance_corrections WHERE employee_id = ?1 AND status = 'pending'",
                params![emp],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung koreksi: {e}"))?,
            "dashboard.mycorr",
        )?,
        my_assets: to_dto_int(
            conn.query_row(
                "SELECT COUNT(*) FROM asset_assignments WHERE employee_id = ?1 AND returned_date IS NULL",
                params![emp],
                |r| r.get::<_, i64>(0),
            )
            .map_err(|e| format!("gagal menghitung aset: {e}"))?,
            "dashboard.myassets",
        )?,
        unread_announcements: to_dto_int(ann_unread, "dashboard.myann")?,
        leave_balances: balances,
    })
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
    fn dasbor_hr_memuat_angka_seed() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let d = hr(&conn).expect("hr");
        assert!(d.total_employees >= 1);
        assert!(!d.headcount_by_department.is_empty());
    }

    #[test]
    fn ringkasan_mandiri_butuh_tautan_karyawan() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        // admin seed tertaut karyawan: ringkasan tersedia
        assert!(mine(&conn, admin).is_ok());
        conn.execute(
            "INSERT INTO users (username, email, password) VALUES ('tanpa-karyawan', 'x@x.id', 'hash')",
            [],
        )
        .unwrap();
        let uid: i64 = conn
            .query_row(
                "SELECT id FROM users WHERE username = 'tanpa-karyawan'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(mine(&conn, uid).is_err());
    }

    #[tokio::test]
    async fn dasbor_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let d_sync = serde_json::to_string(&hr(&conn).expect("hs")).unwrap();
        let d_sea = serde_json::to_string(&hr_sea(db).await.expect("hse")).unwrap();
        assert_eq!(d_sync, d_sea);
        let admin: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let m_sync = serde_json::to_string(&mine(&conn, admin).expect("ms")).unwrap();
        let m_sea = serde_json::to_string(&mine_sea(db, admin).await.expect("mse")).unwrap();
        assert_eq!(m_sync, m_sea);
    }
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{q_all, q_one, value_i64, value_to_string, Value};

async fn dcount(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> Result<i64, String> {
    let row = q_one(db, sql.to_string(), vec![], 1, label)
        .await
        .map_err(|e| format!("gagal menghitung dasbor: {e}"))?;
    Ok(row.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0))
}

async fn dcount_p(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
    vals: Vec<Value>,
    label: &str,
) -> Result<i64, String> {
    let row = q_one(db, sql.to_string(), vals, 1, label)
        .await
        .map_err(|e| format!("gagal menghitung dasbor: {e}"))?;
    Ok(row.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0))
}

async fn dgroups(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
    label: &str,
) -> Result<Vec<NameCount>, String> {
    let rows = q_all(db, sql.to_string(), vec![], 2, label)
        .await
        .map_err(|e| format!("gagal membaca {label}: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(NameCount {
            name: value_to_string(&r[0]),
            count: to_dto_int(value_i64(&r[1]).unwrap_or(0), "dashboard.count")?,
        });
    }
    Ok(out)
}

pub async fn hr_sea(db: &sea_orm::DatabaseConnection) -> Result<HrDashboard, String> {
    let now = Local::now().naive_local();
    let today = now.format("%Y-%m-%d").to_string();
    let month = now.format("%Y-%m").to_string();
    let month_like = format!("{month}%");
    let contract_limit = (now + chrono::Duration::days(60))
        .format("%Y-%m-%d")
        .to_string();

    let brows = q_all(
        db,
        "SELECT TRIM(first_name || ' ' || COALESCE(last_name,'')), employee_number, birth_date FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation') AND birth_date IS NOT NULL AND strftime('%m-%d', birth_date) = strftime('%m-%d','now','localtime') ORDER BY first_name LIMIT 20".to_string(),
        vec![],
        3,
        "dashboard.birthdays",
    )
    .await
    .map_err(|e| format!("gagal membaca ulang tahun: {e}"))?;
    let mut birthdays = Vec::new();
    for r in &brows {
        birthdays.push(Birthday {
            name: value_to_string(&r[0]),
            employee_number: value_to_string(&r[1]),
            birth_date: match &r[2] {
                Value::Null => None,
                _ => Some(value_to_string(&r[2])),
            },
        });
    }
    let hrows = q_all(
        db,
        "SELECT name, date FROM holidays WHERE date >= date('now','localtime') ORDER BY date LIMIT 5".to_string(),
        vec![],
        2,
        "dashboard.holidays",
    )
    .await
    .map_err(|e| format!("gagal membaca libur: {e}"))?;
    let mut holidays = Vec::new();
    for r in &hrows {
        holidays.push(HolidayLite {
            name: value_to_string(&r[0]),
            date: value_to_string(&r[1]),
        });
    }
    let arows = q_all(
        db,
        "SELECT id, title, created_at FROM announcements WHERE deleted_at IS NULL AND status = 'published' ORDER BY id DESC LIMIT 5".to_string(),
        vec![],
        3,
        "dashboard.anns",
    )
    .await
    .map_err(|e| format!("gagal membaca pengumuman: {e}"))?;
    let mut anns = Vec::new();
    for r in &arows {
        anns.push(AnnounceLite {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "dashboard.announcement")?,
            title: value_to_string(&r[1]),
            created_at: value_to_string(&r[2]),
        });
    }

    Ok(HrDashboard {
        total_employees: to_dto_int(dcount(db, "SELECT COUNT(*) FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation')", "dashboard.total").await?, "dashboard.total")?,
        new_hires_this_month: to_dto_int(dcount_p(db, "SELECT COUNT(*) FROM employees WHERE deleted_at IS NULL AND join_date LIKE ?1", vec![Value::Text(month_like.clone())], "dashboard.hires").await?, "dashboard.hires")?,
        resigned_this_month: to_dto_int(dcount_p(db, "SELECT COUNT(*) FROM employees WHERE resign_date LIKE ?1", vec![Value::Text(month_like)], "dashboard.resigned").await?, "dashboard.resigned")?,
        active_contracts: to_dto_int(dcount(db, "SELECT COUNT(*) FROM employee_contracts WHERE status = 'active' AND deleted_at IS NULL", "dashboard.contracts").await?, "dashboard.contracts")?,
        expiring_contracts: to_dto_int(dcount_p(db, "SELECT COUNT(*) FROM employee_contracts WHERE status = 'active' AND deleted_at IS NULL AND end_date IS NOT NULL AND end_date <= ?1", vec![Value::Text(contract_limit)], "dashboard.expiring").await?, "dashboard.expiring")?,
        pending_leave: to_dto_int(dcount(db, "SELECT COUNT(*) FROM leave_requests WHERE status = 'pending'", "dashboard.leave").await?, "dashboard.leave")?,
        pending_overtime: to_dto_int(dcount(db, "SELECT COUNT(*) FROM overtime_requests WHERE status = 'pending'", "dashboard.overtime").await?, "dashboard.overtime")?,
        pending_corrections: to_dto_int(dcount(db, "SELECT COUNT(*) FROM attendance_corrections WHERE status = 'pending'", "dashboard.corrections").await?, "dashboard.corrections")?,
        pending_trips: to_dto_int(dcount(db, "SELECT COUNT(*) FROM business_trips WHERE status = 'pending'", "dashboard.trips").await?, "dashboard.trips")?,
        present_today: to_dto_int(dcount_p(db, "SELECT COUNT(*) FROM attendances WHERE date = ?1 AND status IN ('present','late','wfh','business_trip')", vec![Value::Text(today.clone())], "dashboard.present").await?, "dashboard.present")?,
        late_today: to_dto_int(dcount_p(db, "SELECT COUNT(*) FROM attendances WHERE date = ?1 AND status = 'late'", vec![Value::Text(today)], "dashboard.late").await?, "dashboard.late")?,
        birthdays_today: birthdays,
        upcoming_holidays: holidays,
        recent_announcements: anns,
        headcount_by_department: dgroups(
            db,
            "SELECT COALESCE(d.name,'(Tanpa departemen)'), COUNT(*) FROM employees e LEFT JOIN departments d ON d.id = e.department_id WHERE e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY d.name ORDER BY COUNT(*) DESC",
            "headcount",
        )
        .await?,
        headcount_by_type: dgroups(
            db,
            "SELECT employment_type, COUNT(*) FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation') GROUP BY employment_type",
            "tipe",
        )
        .await?,
    })
}

pub async fn mine_sea(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<MySummary, String> {
    let emp_row = q_one(
        db,
        "SELECT employee_id FROM users WHERE id = ?1".to_string(),
        vec![Value::Int(user_id)],
        1,
        "dashboard.emp",
    )
    .await
    .map_err(|e| format!("gagal memuat akun: {e}"))?;
    let emp = emp_row
        .as_ref()
        .and_then(|r| value_i64(&r[0]))
        .ok_or("Akun belum tertaut karyawan.".to_string())?;
    let info = q_one(
        db,
        "SELECT TRIM(e.first_name || ' ' || COALESCE(e.last_name,'')), e.employee_number, d.name, p.name FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN positions p ON p.id = e.position_id WHERE e.id = ?1".to_string(),
        vec![Value::Int(emp)],
        4,
        "dashboard.info",
    )
    .await
    .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    let Some(info) = info else {
        return Err("Akun belum tertaut karyawan.".to_string());
    };
    let today = Local::now().naive_local().format("%Y-%m-%d").to_string();
    let att = q_one(
        db,
        "SELECT status, clock_in FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(),
        vec![Value::Int(emp), Value::Text(today)],
        2,
        "dashboard.att",
    )
    .await
    .unwrap_or(None);
    let (status, clock_in) = match att {
        Some(a) => (
            match &a[0] {
                Value::Null => None,
                _ => Some(value_to_string(&a[0])),
            },
            match &a[1] {
                Value::Null => None,
                _ => Some(value_to_string(&a[1])),
            },
        ),
        None => (None, None),
    };
    let year: i32 = Local::now()
        .naive_local()
        .format("%Y")
        .to_string()
        .parse()
        .unwrap_or(2000);
    let brows = q_all(
        db,
        "SELECT t.name, b.allocated_days, b.used_days FROM leave_balances b INNER JOIN leave_types t ON t.id = b.leave_type_id WHERE b.employee_id = ?1 AND b.year = ?2".to_string(),
        vec![Value::Int(emp), Value::Int(year as i64)],
        3,
        "dashboard.bal",
    )
    .await
    .map_err(|e| format!("gagal membaca saldo: {e}"))?;
    let mut balances = Vec::new();
    for r in &brows {
        let (alloc, used) = (
            match &r[1] {
                Value::Float(f) => *f,
                Value::Int(i) => *i as f64,
                Value::Text(s) => s.parse().unwrap_or(0.0),
                Value::Null => 0.0,
            },
            match &r[2] {
                Value::Float(f) => *f,
                Value::Int(i) => *i as f64,
                Value::Text(s) => s.parse().unwrap_or(0.0),
                Value::Null => 0.0,
            },
        );
        balances.push(LeaveBalanceLite {
            leave_type: value_to_string(&r[0]),
            allocated: alloc,
            used,
            remaining: (alloc - used).max(0.0),
        });
    }
    let ann_unread = dcount_p(
        db,
        "SELECT COUNT(*) FROM announcements a LEFT JOIN announcement_reads ar ON ar.announcement_id = a.id AND ar.employee_id = ?1 WHERE a.status = 'published' AND a.deleted_at IS NULL AND ar.id IS NULL",
        vec![Value::Int(emp)],
        "dashboard.myann",
    )
    .await?;
    Ok(MySummary {
        employee_name: value_to_string(&info[0]),
        employee_number: value_to_string(&info[1]),
        department: match &info[2] {
            Value::Null => None,
            _ => Some(value_to_string(&info[2])),
        },
        position: match &info[3] {
            Value::Null => None,
            _ => Some(value_to_string(&info[3])),
        },
        today_status: status,
        today_clock_in: clock_in,
        pending_leave: to_dto_int(
            dcount_p(
                db,
                "SELECT COUNT(*) FROM leave_requests WHERE employee_id = ?1 AND status = 'pending'",
                vec![Value::Int(emp)],
                "dashboard.myleave",
            )
            .await?,
            "dashboard.myleave",
        )?,
        pending_overtime: to_dto_int(
            dcount_p(
                db,
                "SELECT COUNT(*) FROM overtime_requests WHERE employee_id = ?1 AND status = 'pending'",
                vec![Value::Int(emp)],
                "dashboard.myot",
            )
            .await?,
            "dashboard.myot",
        )?,
        pending_corrections: to_dto_int(
            dcount_p(
                db,
                "SELECT COUNT(*) FROM attendance_corrections WHERE employee_id = ?1 AND status = 'pending'",
                vec![Value::Int(emp)],
                "dashboard.mycorr",
            )
            .await?,
            "dashboard.mycorr",
        )?,
        my_assets: to_dto_int(
            dcount_p(
                db,
                "SELECT COUNT(*) FROM asset_assignments WHERE employee_id = ?1 AND returned_date IS NULL",
                vec![Value::Int(emp)],
                "dashboard.myassets",
            )
            .await?,
            "dashboard.myassets",
        )?,
        unread_announcements: to_dto_int(ann_unread, "dashboard.myann")?,
        leave_balances: balances,
    })
}
