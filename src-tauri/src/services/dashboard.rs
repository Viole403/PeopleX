//! Dasbor: ringkasan HR dan ringkasan mandiri karyawan.

use chrono::Local;

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sea_raw::{exec, q_one, value_i64};

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "test.admin",
        )
        .await
        .expect("admin")
        .and_then(|r| value_i64(&r[0]))
        .expect("admin id")
    }

    #[tokio::test]
    async fn dasbor_hr_memuat_angka_seed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let d = hr_sea(&state.sea).await.expect("hr");
        assert!(d.total_employees >= 1);
        assert!(!d.headcount_by_department.is_empty());
    }

    #[tokio::test]
    async fn workforce_rencana_budget_dan_drilldown_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin_id(db).await;
        let dept = q_one(db, "SELECT id FROM departments LIMIT 1".to_string(), vec![], 1, "t.dept")
            .await
            .expect("dept")
            .expect("ada");
        let did = value_i64(&dept[0]).expect("id");
        headcount_plan_save_sea(db, actor, 2026, did, 10).await.expect("rencana");
        dept_budget_save_sea(db, actor, 2026, did, 1_000_000_000.0).await.expect("budget");
        assert!(headcount_plan_save_sea(db, actor, 1999, did, 5).await.is_err());
        assert!(headcount_plan_save_sea(db, actor, 2026, did, -1).await.is_err());
        let view = workforce_overview_sea(db, 2026).await.expect("view");
        assert!(!view.is_empty());
        let baris = view.iter().find(|v| v.department_id as i64 == did).expect("baris dept");
        assert_eq!(baris.planned, 10);
        assert!((baris.budget - 1_000_000_000.0).abs() < 1.0);
        let drill = drilldown_sea(db).await.expect("drill");
        assert!(!drill.is_empty());
        assert!(drill.iter().all(|c| !c.children.is_empty() || c.headcount >= 0));
    }

    #[tokio::test]
    async fn ringkasan_mandiri_butuh_tautan_karyawan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin = admin_id(db).await;
        let m = mine_sea(db, admin).await.expect("mine");
        assert!(!m.employee_name.is_empty());
        exec(
            db,
            "INSERT INTO users (username, email, password) VALUES ('tanpa-karyawan', 'x@x.id', 'hash')".to_string(),
            vec![],
            "test.mkuser",
        )
        .await
        .expect("insert");
        let uid = q_one(
            db,
            "SELECT id FROM users WHERE username = 'tanpa-karyawan'".to_string(),
            vec![],
            1,
            "test.uid",
        )
        .await
        .expect("uid")
        .and_then(|r| value_i64(&r[0]))
        .expect("uid id");
        let e = mine_sea(db, uid).await.expect_err("tanpa tautan");
        assert_eq!(e, "Akun belum tertaut karyawan.".to_string());
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

    let md = now.format("%m-%d").to_string();
    let brows = q_all(
        db,
        "SELECT TRIM(first_name || ' ' || COALESCE(last_name,'')), employee_number, birth_date FROM employees WHERE deleted_at IS NULL AND employment_status IN ('active','probation') AND birth_date IS NOT NULL ORDER BY first_name".to_string(),
        vec![],
        3,
        "dashboard.birthdays",
    )
    .await
    .map_err(|e| format!("gagal membaca ulang tahun: {e}"))?;
    let mut birthdays = Vec::new();
    for r in &brows {
        let bd = value_to_string(&r[2]);
        if bd.get(5..10) != Some(md.as_str()) {
            continue;
        }
        birthdays.push(Birthday {
            name: value_to_string(&r[0]),
            employee_number: value_to_string(&r[1]),
            birth_date: match &r[2] {
                Value::Null => None,
                _ => Some(value_to_string(&r[2])),
            },
        });
        if birthdays.len() >= 20 {
            break;
        }
    }
    let hrows = q_all(
        db,
        "SELECT name, date FROM holidays WHERE date >= ?1 ORDER BY date LIMIT 5".to_string(),
        vec![Value::Text(today.clone())],
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

// ---------------- Workforce planning + drill-down ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DeptPlan {
    pub department_id: i32,
    pub department_name: String,
    pub planned: i32,
    pub actual: i32,
    pub gap: i32,
    pub budget: f64,
    pub payroll_cost: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DrillNode {
    pub id: String,
    pub label: String,
    pub headcount: i32,
    pub children: Vec<DrillNode>,
}

pub async fn headcount_plan_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    year: i32,
    department_id: i64,
    planned: i32,
) -> Result<(), String> {
    if year < 2000 || year > 2100 {
        return Err("Tahun tidak valid.".to_string());
    }
    if planned < 0 {
        return Err("Rencana tidak boleh negatif.".to_string());
    }
    let ada = q_one(
        db,
        "SELECT id FROM headcount_plans WHERE year = ?1 AND department_id = ?2".to_string(),
        vec![Value::Int(year as i64), Value::Int(department_id)],
        1,
        "wf.cek",
    )
    .await
    .map_err(|e| format!("gagal memeriksa rencana: {e}"))?;
    match ada {
        Some(r) => {
            super::sea_raw::exec(
                db,
                "UPDATE headcount_plans SET planned = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
                vec![Value::Int(planned as i64), Value::Text(Local::now().naive_local().format("%Y-%m-%d %H:%M:%S").to_string()), Value::Int(value_i64(&r[0]).unwrap_or(0))],
                "wf.upd",
            )
            .await?;
        }
        None => {
            super::sea_raw::exec_insert(
                db,
                "INSERT INTO headcount_plans (year, department_id, planned) VALUES (?1, ?2, ?3)".to_string(),
                vec![Value::Int(year as i64), Value::Int(department_id), Value::Int(planned as i64)],
                "wf.ins",
            )
            .await?;
        }
    }
    super::audit::log_sea(db, Some(actor_id), "CREATE", "workforce.plan", None, None, None, None).await?;
    Ok(())
}

pub async fn dept_budget_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    year: i32,
    department_id: i64,
    budget: f64,
) -> Result<(), String> {
    if year < 2000 || year > 2100 {
        return Err("Tahun tidak valid.".to_string());
    }
    if budget < 0.0 {
        return Err("Budget tidak boleh negatif.".to_string());
    }
    let ada = q_one(
        db,
        "SELECT id FROM department_budgets WHERE year = ?1 AND department_id = ?2".to_string(),
        vec![Value::Int(year as i64), Value::Int(department_id)],
        1,
        "wf.bcek",
    )
    .await
    .map_err(|e| format!("gagal memeriksa budget: {e}"))?;
    match ada {
        Some(r) => {
            super::sea_raw::exec(
                db,
                "UPDATE department_budgets SET budget = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
                vec![Value::Float(budget), Value::Text(Local::now().naive_local().format("%Y-%m-%d %H:%M:%S").to_string()), Value::Int(value_i64(&r[0]).unwrap_or(0))],
                "wf.bupd",
            )
            .await?;
        }
        None => {
            super::sea_raw::exec_insert(
                db,
                "INSERT INTO department_budgets (year, department_id, budget) VALUES (?1, ?2, ?3)".to_string(),
                vec![Value::Int(year as i64), Value::Int(department_id), Value::Float(budget)],
                "wf.bins",
            )
            .await?;
        }
    }
    super::audit::log_sea(db, Some(actor_id), "CREATE", "workforce.budget", None, None, None, None).await?;
    Ok(())
}

pub async fn workforce_overview_sea(
    db: &sea_orm::DatabaseConnection,
    year: i32,
) -> Result<Vec<DeptPlan>, String> {
    let rows = q_all(
        db,
        "SELECT d.id, d.name, COALESCE(h.planned, 0), COALESCE(b.budget, 0) FROM departments d LEFT JOIN headcount_plans h ON h.department_id = d.id AND h.year = ?1 LEFT JOIN department_budgets b ON b.department_id = d.id AND b.year = ?1 WHERE d.deleted_at IS NULL ORDER BY d.name".to_string(),
        vec![Value::Int(year as i64), Value::Int(year as i64)],
        4,
        "wf.view",
    )
    .await
    .map_err(|e| format!("gagal membaca workforce: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let did = value_i64(&r[0]).unwrap_or(0);
        let actual = dcount_p(
            db,
            "SELECT COUNT(*) FROM employees WHERE department_id = ?1 AND deleted_at IS NULL AND employment_status IN ('active','probation')",
            vec![Value::Int(did)],
            "wf.actual",
        )
        .await?;
        let cost = q_one(
            db,
            "SELECT COALESCE(SUM(s.basic_salary), 0) FROM employee_salaries s INNER JOIN employees e ON e.id = s.employee_id WHERE e.department_id = ?1 AND s.is_active = 1 AND e.deleted_at IS NULL".to_string(),
            vec![Value::Int(did)],
            1,
            "wf.cost",
        )
        .await
        .map_err(|e| format!("gagal menghitung biaya: {e}"))?;
        let planned = value_i64(&r[2]).unwrap_or(0);
        out.push(DeptPlan {
            department_id: to_dto_int(did, "wf.dept")?,
            department_name: value_to_string(&r[1]),
            planned: to_dto_int(planned, "wf.planned")?,
            actual: to_dto_int(actual, "wf.actual")?,
            gap: to_dto_int(planned - actual, "wf.gap").unwrap_or(0),
            budget: match &r[3] {
                Value::Float(v) => *v,
                Value::Int(v) => *v as f64,
                _ => 0.0,
            },
            payroll_cost: cost.as_ref().map(|c| match &c[0] {
                Value::Float(v) => *v,
                Value::Int(v) => *v as f64,
                _ => 0.0,
            }).unwrap_or(0.0),
        });
    }
    Ok(out)
}

pub async fn drilldown_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<DrillNode>, String> {
    let comps = q_all(
        db,
        "SELECT c.id, c.name, COUNT(DISTINCT e.id) FROM companies c LEFT JOIN departments d ON d.company_id = c.id LEFT JOIN employees e ON e.department_id = d.id AND e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY c.id ORDER BY c.name".to_string(),
        vec![],
        3,
        "drill.comp",
    )
    .await
    .map_err(|e| format!("gagal membaca perusahaan: {e}"))?;
    let mut out = Vec::new();
    for c in &comps {
        let cid = value_i64(&c[0]).unwrap_or(0);
        let depts = q_all(
            db,
            "SELECT d.id, d.name, COUNT(e.id) FROM departments d LEFT JOIN employees e ON e.department_id = d.id AND e.deleted_at IS NULL AND e.employment_status IN ('active','probation') WHERE d.company_id = ?1 AND d.deleted_at IS NULL GROUP BY d.id ORDER BY d.name".to_string(),
            vec![Value::Int(cid)],
            3,
            "drill.dept",
        )
        .await
        .map_err(|e| format!("gagal membaca departemen: {e}"))?;
        let mut kids = Vec::new();
        for d in &depts {
            let did = value_i64(&d[0]).unwrap_or(0);
            let teams = q_all(
                db,
                "SELECT COALESCE(p.name, '(Tanpa jabatan)'), COUNT(e.id) FROM employees e LEFT JOIN positions p ON p.id = e.position_id WHERE e.department_id = ?1 AND e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY p.name ORDER BY COUNT(e.id) DESC".to_string(),
                vec![Value::Int(did)],
                2,
                "drill.pos",
            )
            .await
            .map_err(|e| format!("gagal membaca jabatan: {e}"))?;
            kids.push(DrillNode {
                id: format!("d{did}"),
                label: value_to_string(&d[1]),
                headcount: to_dto_int(value_i64(&d[2]).unwrap_or(0), "drill.d")?,
                children: teams.iter().map(|t| DrillNode {
                    id: format!("d{did}-{}", value_to_string(&t[0])),
                    label: value_to_string(&t[0]),
                    headcount: to_dto_int(value_i64(&t[1]).unwrap_or(0), "drill.p").unwrap_or(0),
                    children: vec![],
                }).collect(),
            });
        }
        out.push(DrillNode {
            id: format!("c{cid}"),
            label: value_to_string(&c[1]),
            headcount: to_dto_int(value_i64(&c[2]).unwrap_or(0), "drill.c")?,
            children: kids,
        });
    }
    Ok(out)
}
