//! Data awal: company, roles, permissions, admin, dan master data.
//!
//! Idempoten: aman dijalankan berulang, tidak membuat duplikat.

use chrono::Local;
use sea_orm::sqlx::{AssertSqlSafe, Row};

/// Ringkasan hasil seeding untuk logging.
#[derive(Debug, Default)]
pub struct SeedSummary {
    pub roles: i64,
    pub permissions: i64,
    pub users: i64,
}

type Tx<'a> = sea_orm::sqlx::Transaction<'a, sea_orm::sqlx::Sqlite>;

/// Nilai sel generik untuk seed dinamis.
#[derive(Clone, Debug)]
enum SVal {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
}

fn tx_cell(row: &sea_orm::sqlx::sqlite::SqliteRow, i: usize) -> SVal {
    if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(i) {
        return SVal::Int(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(i) {
        return SVal::Float(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<String>, _>(i) {
        return SVal::Text(v);
    }
    SVal::Null
}

fn sval_i64(v: &SVal) -> Option<i64> {
    match v {
        SVal::Int(i) => Some(*i),
        SVal::Text(s) => s.parse().ok(),
        _ => None,
    }
}

async fn tx_q_all(
    tx: &mut Tx<'_>,
    sql: String,
    vals: Vec<SVal>,
    ncols: usize,
    label: &str,
) -> Result<Vec<Vec<SVal>>, String> {
    let mut q = sea_orm::sqlx::query(AssertSqlSafe(sql));
    for v in vals {
        q = match v {
            SVal::Null => q.bind(None::<String>),
            SVal::Int(i) => q.bind(i),
            SVal::Float(f) => q.bind(f),
            SVal::Text(s) => q.bind(s),
        };
    }
    let rows = q
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| format!("gagal {label}: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        let mut v = Vec::with_capacity(ncols);
        for i in 0..ncols {
            v.push(tx_cell(&r, i));
        }
        out.push(v);
    }
    Ok(out)
}

async fn tx_q_one(
    tx: &mut Tx<'_>,
    sql: String,
    vals: Vec<SVal>,
    ncols: usize,
    label: &str,
) -> Result<Option<Vec<SVal>>, String> {
    let mut rows = tx_q_all(tx, sql, vals, ncols, label).await?;
    Ok(rows.pop())
}

async fn tx_exec(
    tx: &mut Tx<'_>,
    sql: String,
    vals: Vec<SVal>,
    label: &str,
) -> Result<u64, String> {
    let mut q = sea_orm::sqlx::query(AssertSqlSafe(sql));
    for v in vals {
        q = match v {
            SVal::Null => q.bind(None::<String>),
            SVal::Int(i) => q.bind(i),
            SVal::Float(f) => q.bind(f),
            SVal::Text(s) => q.bind(s),
        };
    }
    q.execute(&mut **tx)
        .await
        .map_err(|e| format!("gagal {label}: {e}"))
        .map(|r| r.rows_affected())
}

async fn tx_rowid(tx: &mut Tx<'_>) -> i64 {
    tx_q_one(
        tx,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        "seed.rowid",
    )
    .await
    .ok()
    .flatten()
    .as_ref()
    .and_then(|r| sval_i64(&r[0]))
    .unwrap_or(0)
}

/// Jalankan seluruh seed dalam satu transaksi.
pub async fn seed(db: &sea_orm::DatabaseConnection) -> Result<SeedSummary, String> {
    let pool = db.get_sqlite_connection_pool();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("gagal memulai transaksi seed: {e}"))?;
    let inner = async {
        let company_id = seed_company(&mut tx).await?;
        let (dept_hr, dept_it, dept_fin) = seed_departments(&mut tx, company_id).await?;
        let (lvl_staff, _lvl_spv, lvl_mgr) = seed_job_levels(&mut tx).await?;
        seed_job_grades(&mut tx).await?;
        let pos_hr_manager =
            seed_positions(&mut tx, dept_hr, dept_it, dept_fin, lvl_staff, lvl_mgr).await?;
        let hq_location = seed_work_location(&mut tx).await?;
        seed_cost_center(&mut tx, dept_hr).await?;
        let role_ids = seed_roles(&mut tx).await?;
        let perm_ids = seed_permissions(&mut tx).await?;
        seed_role_permissions(&mut tx, &role_ids, &perm_ids).await?;
        let admin_employee = seed_admin_employee(
            &mut tx,
            company_id,
            dept_hr,
            pos_hr_manager,
            lvl_mgr,
            hq_location,
        )
        .await?;
        seed_admin_user(&mut tx, &role_ids, admin_employee).await?;
        seed_leave_types(&mut tx).await?;
        seed_permission_types(&mut tx).await?;
        seed_salary_components(&mut tx).await?;
        let schedule_regular = seed_shifts_and_schedule(&mut tx).await?;
        seed_shift_assignment(&mut tx, admin_employee, schedule_regular).await?;
        seed_holidays(&mut tx).await?;
        seed_approval_workflows(&mut tx, &role_ids).await?;
        seed_categories(&mut tx).await?;
        seed_settings(&mut tx).await?;
        Ok::<(), String>(())
    }
    .await;
    if let Err(e) = inner {
        tx.rollback().await.ok();
        return Err(e);
    }
    tx.commit()
        .await
        .map_err(|e| format!("gagal commit transaksi seed: {e}"))?;

    Ok(SeedSummary {
        roles: count(db, "roles").await?,
        permissions: count(db, "permissions").await?,
        users: count(db, "users").await?,
    })
}

async fn count(db: &sea_orm::DatabaseConnection, table: &str) -> Result<i64, String> {
    let rows = crate::services::sea_raw::q_all(
        db,
        format!("SELECT COUNT(*) FROM {table}"),
        vec![],
        1,
        "seed.count",
    )
    .await
    .map_err(|e| format!("gagal menghitung {table}: {e}"))?;
    Ok(rows
        .first()
        .and_then(|r| crate::services::sea_raw::value_i64(&r[0]))
        .unwrap_or(0))
}

/// Insert bila belum ada (berdasar constraint UNIQUE), kembalikan id.
async fn insert_ignore(
    tx: &mut Tx<'_>,
    table: &str,
    columns: &str,
    placeholders: &str,
    p: Vec<SVal>,
) -> Result<i64, String> {
    tx_exec(
        tx,
        format!("INSERT OR IGNORE INTO {table} ({columns}) VALUES ({placeholders})"),
        p,
        "seed.insert",
    )
    .await
    .map_err(|e| format!("gagal insert {table}: {e}"))?;
    Ok(tx_rowid(tx).await)
}

async fn find_id(
    tx: &mut Tx<'_>,
    table: &str,
    where_clause: &str,
    p: Vec<SVal>,
) -> Result<Option<i64>, String> {
    let row = tx_q_one(
        tx,
        format!("SELECT id FROM {table} WHERE {where_clause}"),
        p,
        1,
        "seed.find",
    )
    .await
    .map_err(|e| format!("gagal mencari {table}: {e}"))?;
    Ok(row.as_ref().and_then(|r| sval_i64(&r[0])))
}

fn t(s: &str) -> SVal {
    SVal::Text(s.to_string())
}

fn i(v: i64) -> SVal {
    SVal::Int(v)
}

async fn seed_company(tx: &mut Tx<'_>) -> Result<i64, String> {
    insert_ignore(
        tx,
        "companies",
        "code, name, legal_name, address, city, province, postal_code, phone, email, npwp, established_date",
        "?,?,?,?,?,?,?,?,?,?,?",
        vec![
            t("HQ"),
            t("PT Contoh Sukses Indonesia"),
            t("PT Contoh Sukses Indonesia"),
            t("Jl. Pemuda No. 1, Surabaya"),
            t("Surabaya"),
            t("Jawa Timur"),
            t("60271"),
            t("+62-31-5551234"),
            t("info@contohsukses.co.id"),
            t("01.234.567.8-901.000"),
            t("2010-01-01"),
        ],
    )
    .await?;
    find_id(tx, "companies", "code = ?1", vec![t("HQ")])
        .await?
        .ok_or_else(|| "company HQ tidak ditemukan setelah seed".to_string())
}

async fn seed_departments(
    tx: &mut Tx<'_>,
    company_id: i64,
) -> Result<(i64, i64, i64), String> {
    let branch_id = insert_ignore(
        tx,
        "branches",
        "company_id, code, name, address, city, phone, is_head_office",
        "?,?,?,?,?,?,1",
        vec![
            i(company_id),
            t("HO"),
            t("Kantor Pusat Surabaya"),
            t("Jl. Pemuda No. 1, Surabaya"),
            t("Surabaya"),
            t("+62-31-5551234"),
        ],
    )
    .await?;
    let branch_id = find_id(
        tx,
        "branches",
        "company_id = ?1 AND code = 'HO'",
        vec![i(company_id)],
    )
    .await?
    .or(Some(branch_id))
    .unwrap_or(branch_id);
    let mut ids = Vec::new();
    for (code, name) in [
        ("HRD", "Human Resources"),
        ("IT", "Information Technology"),
        ("FIN", "Finance & Accounting"),
    ] {
        insert_ignore(
            tx,
            "departments",
            "company_id, branch_id, code, name",
            "?,?,?,?",
            vec![i(company_id), i(branch_id), t(code), t(name)],
        )
        .await?;
        let id = find_id(
            tx,
            "departments",
            "company_id = ?1 AND code = ?2",
            vec![i(company_id), t(code)],
        )
        .await?
        .ok_or_else(|| format!("department {code} tidak ditemukan setelah seed"))?;
        ids.push(id);
    }
    Ok((ids[0], ids[1], ids[2]))
}

async fn seed_job_levels(tx: &mut Tx<'_>) -> Result<(i64, i64, i64), String> {
    let mut ids = Vec::new();
    for (code, name, order) in [
        ("STAFF", "Staff", 1),
        ("SPV", "Supervisor", 2),
        ("MGR", "Manager", 3),
        ("DIR", "Director", 4),
    ] {
        insert_ignore(
            tx,
            "job_levels",
            "code, name, level_order",
            "?,?,?",
            vec![t(code), t(name), i(order)],
        )
        .await?;
        ids.push(
            find_id(tx, "job_levels", "code = ?1", vec![t(code)])
                .await?
                .ok_or_else(|| format!("job level {code} tidak ditemukan setelah seed"))?,
        );
    }
    Ok((ids[0], ids[1], ids[2]))
}

async fn seed_job_grades(tx: &mut Tx<'_>) -> Result<(), String> {
    for order in 1..=5 {
        let code = format!("G{order}");
        let name = format!("Grade {order}");
        insert_ignore(
            tx,
            "job_grades",
            "code, name, grade_order",
            "?,?,?",
            vec![t(&code), t(&name), i(order)],
        )
        .await?;
    }
    Ok(())
}

async fn seed_positions(
    tx: &mut Tx<'_>,
    dept_hr: i64,
    dept_it: i64,
    dept_fin: i64,
    lvl_staff: i64,
    lvl_mgr: i64,
) -> Result<i64, String> {
    let rows: Vec<(&str, &str, i64, i64)> = vec![
        ("HRM", "HR Manager", dept_hr, lvl_mgr),
        ("HRS", "HR Staff", dept_hr, lvl_staff),
        ("ITS", "IT Staff", dept_it, lvl_staff),
        ("FINS", "Finance Staff", dept_fin, lvl_staff),
    ];
    let mut hr_manager = 0;
    for (code, name, dept, lvl) in rows {
        insert_ignore(
            tx,
            "positions",
            "code, name, department_id, job_level_id",
            "?,?,?,?",
            vec![t(code), t(name), i(dept), i(lvl)],
        )
        .await?;
        let id = find_id(tx, "positions", "code = ?1", vec![t(code)])
            .await?
            .ok_or_else(|| format!("position {code} tidak ditemukan setelah seed"))?;
        if code == "HRM" {
            hr_manager = id;
        }
    }
    Ok(hr_manager)
}

async fn seed_work_location(tx: &mut Tx<'_>) -> Result<i64, String> {
    insert_ignore(
        tx,
        "work_locations",
        "name, address, latitude, longitude, radius_meter",
        "?,?,?,?,?",
        vec![
            t("Kantor Pusat Surabaya"),
            t("Jl. Pemuda No. 1, Surabaya"),
            SVal::Float(-6.224),
            SVal::Float(106.809),
            i(200),
        ],
    )
    .await?;
    find_id(
        tx,
        "work_locations",
        "name = ?1",
        vec![t("Kantor Pusat Surabaya")],
    )
    .await?
    .ok_or_else(|| "work location tidak ditemukan setelah seed".to_string())
}

async fn seed_cost_center(tx: &mut Tx<'_>, dept_hr: i64) -> Result<(), String> {
    insert_ignore(
        tx,
        "cost_centers",
        "code, name, department_id",
        "?,?,?",
        vec![t("CC-HRD"), t("Cost Center HRD"), i(dept_hr)],
    )
    .await?;
    Ok(())
}

async fn seed_roles(
    tx: &mut Tx<'_>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let roles = [
        (
            "super-administrator",
            "Super Administrator",
            "Akses penuh ke seluruh sistem",
            1,
        ),
        (
            "hr-administrator",
            "HR Administrator",
            "Mengelola seluruh operasional HR",
            0,
        ),
        (
            "hr-manager",
            "HR Manager",
            "Menyetujui proses HR tingkat manajerial",
            0,
        ),
        (
            "finance",
            "Finance",
            "Mengelola payroll dan reimbursement",
            0,
        ),
        (
            "manager",
            "Manager",
            "Menyetujui pengajuan bawahan tingkat manajer",
            0,
        ),
        (
            "supervisor",
            "Supervisor",
            "Menyetujui pengajuan bawahan langsung",
            0,
        ),
        ("employee", "Employee", "Pengguna karyawan standar", 0),
        (
            "auditor",
            "Auditor",
            "Akses baca untuk audit dan kepatuhan",
            0,
        ),
    ];
    let mut map = std::collections::HashMap::new();
    for (slug, name, desc, is_system) in roles {
        insert_ignore(
            tx,
            "roles",
            "slug, name, description, is_system",
            "?,?,?,?",
            vec![t(slug), t(name), t(desc), i(is_system)],
        )
        .await?;
        let id = find_id(tx, "roles", "slug = ?1", vec![t(slug)])
            .await?
            .ok_or_else(|| format!("role {slug} tidak ditemukan setelah seed"))?;
        map.insert(slug.to_string(), id);
    }
    Ok(map)
}

fn title_case(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

async fn seed_permissions(
    tx: &mut Tx<'_>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let modules: Vec<(&str, Vec<&str>)> = vec![
        (
            "employee",
            vec!["view", "create", "update", "delete", "export"],
        ),
        ("organization", vec!["view", "create", "update", "delete"]),
        (
            "attendance",
            vec!["view", "create", "update", "delete", "correct", "approve"],
        ),
        (
            "leave",
            vec!["view", "create", "update", "delete", "approve"],
        ),
        (
            "permission",
            vec!["view", "create", "update", "delete", "approve"],
        ),
        (
            "overtime",
            vec!["view", "create", "update", "delete", "approve"],
        ),
        (
            "payroll",
            vec!["view", "create", "update", "generate", "approve", "delete"],
        ),
        ("contract", vec!["view", "create", "update", "delete"]),
        ("document", vec!["view", "create", "update", "delete"]),
        ("recruitment", vec!["view", "create", "update", "delete"]),
        ("onboarding", vec!["view", "create", "update"]),
        ("offboarding", vec!["view", "create", "update", "approve"]),
        (
            "performance",
            vec!["view", "create", "update", "delete", "review"],
        ),
        ("training", vec!["view", "create", "update", "delete"]),
        ("asset", vec!["view", "create", "update", "delete"]),
        ("business_trip", vec!["view", "create", "update", "approve"]),
        ("reimbursement", vec!["view", "create", "update", "approve"]),
        ("announcement", vec!["view", "create", "update", "delete"]),
        ("report", vec!["view", "export"]),
        ("settings", vec!["manage"]),
        ("rbac", vec!["manage"]),
        ("workflow", vec!["manage"]),
        ("system", vec!["manage"]),
        ("audit", vec!["view"]),
    ];
    let mut map = std::collections::HashMap::new();
    for (module, actions) in modules {
        for action in actions {
            let slug = format!("{module}.{action}");
            let display = format!("{} {}", title_case(action), title_case(module));
            insert_ignore(
                tx,
                "permissions",
                "slug, name, module",
                "?,?,?",
                vec![t(&slug), t(&display), t(module)],
            )
            .await?;
            let id = find_id(tx, "permissions", "slug = ?1", vec![t(&slug)])
                .await?
                .ok_or_else(|| format!("permission {slug} tidak ditemukan setelah seed"))?;
            map.insert(slug, id);
        }
    }
    Ok(map)
}

async fn grant(tx: &mut Tx<'_>, role_id: i64, perm_id: i64) -> Result<(), String> {
    tx_exec(
        tx,
        "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES (?1, ?2)".to_string(),
        vec![i(role_id), i(perm_id)],
        "seed.grant",
    )
    .await
    .map_err(|e| format!("gagal grant permission: {e}"))?;
    Ok(())
}

async fn seed_role_permissions(
    tx: &mut Tx<'_>,
    roles: &std::collections::HashMap<String, i64>,
    perms: &std::collections::HashMap<String, i64>,
) -> Result<(), String> {
    let all: Vec<String> = perms.keys().cloned().collect();
    let without_system: Vec<String> = all
        .iter()
        .filter(|p| !p.starts_with("system."))
        .cloned()
        .collect();
    let by_prefix = |prefixes: &[&str]| -> Vec<String> {
        all.iter()
            .filter(|p| prefixes.iter().any(|pre| p.starts_with(pre)))
            .cloned()
            .collect()
    };
    let lists: Vec<(&str, Vec<String>)> = vec![
        ("super-administrator", all.clone()),
        ("hr-administrator", without_system.clone()),
        (
            "hr-manager",
            without_system
                .iter()
                .filter(|p| p.as_str() != "payroll.delete")
                .cloned()
                .collect(),
        ),
        ("finance", {
            let mut v = by_prefix(&["payroll.", "reimbursement.", "business_trip."]);
            v.extend(
                ["employee.view", "report.view", "report.export"]
                    .iter()
                    .map(|s| s.to_string()),
            );
            v
        }),
        (
            "manager",
            [
                "employee.view",
                "attendance.view",
                "attendance.approve",
                "leave.view",
                "leave.approve",
                "permission.view",
                "permission.approve",
                "overtime.view",
                "overtime.approve",
                "performance.view",
                "performance.review",
                "business_trip.view",
                "business_trip.approve",
                "reimbursement.view",
                "reimbursement.approve",
                "report.view",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        (
            "supervisor",
            [
                "employee.view",
                "attendance.view",
                "attendance.approve",
                "leave.view",
                "leave.approve",
                "permission.view",
                "permission.approve",
                "overtime.view",
                "overtime.approve",
                "performance.view",
                "performance.review",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        (
            "employee",
            [
                "employee.view",
                "attendance.view",
                "attendance.create",
                "leave.view",
                "leave.create",
                "permission.view",
                "permission.create",
                "overtime.view",
                "overtime.create",
                "training.view",
                "asset.view",
                "business_trip.view",
                "business_trip.create",
                "reimbursement.view",
                "reimbursement.create",
                "announcement.view",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
        (
            "auditor",
            [
                "audit.view",
                "report.view",
                "report.export",
                "employee.view",
                "payroll.view",
                "attendance.view",
                "leave.view",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ),
    ];
    for (role_slug, slugs) in lists {
        let role_id = roles
            .get(role_slug)
            .ok_or_else(|| format!("role {role_slug} belum di-seed"))?;
        for slug in slugs {
            if let Some(perm_id) = perms.get(&slug) {
                grant(tx, *role_id, *perm_id).await?;
            }
        }
    }
    Ok(())
}

async fn seed_admin_employee(
    tx: &mut Tx<'_>,
    company_id: i64,
    dept_hr: i64,
    pos_hr_manager: i64,
    lvl_mgr: i64,
    hq_location: i64,
) -> Result<i64, String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let branch_id = find_id(
        tx,
        "branches",
        "company_id = ?1 AND code = 'HO'",
        vec![i(company_id)],
    )
    .await?
    .ok_or_else(|| "branch HO tidak ditemukan".to_string())?;
    insert_ignore(
        tx,
        "employees",
        "employee_number, nik, first_name, last_name, gender, birth_place, birth_date, marital_status, phone, personal_email, company_id, branch_id, department_id, position_id, job_level_id, work_location_id, join_date, employment_status, employment_type",
        "?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?",
        vec![
            t("EMP-0001"),
            t("3171000000000001"),
            t("Super"),
            t("Administrator"),
            t("male"),
            t("Surabaya"),
            t("1990-01-01"),
            t("single"),
            t("081200000001"),
            t("admin.personal@example.com"),
            i(company_id),
            i(branch_id),
            i(dept_hr),
            i(pos_hr_manager),
            i(lvl_mgr),
            i(hq_location),
            t(&today),
            t("active"),
            t("permanent"),
        ],
    )
    .await?;
    find_id(tx, "employees", "employee_number = ?1", vec![t("EMP-0001")])
        .await?
        .ok_or_else(|| "employee EMP-0001 tidak ditemukan setelah seed".to_string())
}

async fn seed_admin_user(
    tx: &mut Tx<'_>,
    roles: &std::collections::HashMap<String, i64>,
    admin_employee: i64,
) -> Result<(), String> {
    let hash = bcrypt::hash("Admin@123", bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password admin: {e}"))?;
    // Hash acak tiap run: hanya insert bila username belum ada (jangan overwrite).
    let exists = find_id(tx, "users", "username = ?1", vec![t("admin")]).await?;
    if exists.is_none() {
        tx_exec(
            tx,
            "INSERT INTO users (employee_id, username, email, password, status, must_change_password) VALUES (?1, ?2, ?3, ?4, 'active', 1)".to_string(),
            vec![
                i(admin_employee),
                t("admin"),
                t("admin@hris.local"),
                t(&hash),
            ],
            "seed.admin",
        )
        .await
        .map_err(|e| format!("gagal insert admin: {e}"))?;
    }
    let admin_id = find_id(tx, "users", "username = ?1", vec![t("admin")])
        .await?
        .ok_or_else(|| "user admin tidak ditemukan setelah seed".to_string())?;
    let super_id = roles
        .get("super-administrator")
        .ok_or_else(|| "role super-administrator belum di-seed".to_string())?;
    tx_exec(
        tx,
        "INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?1, ?2)".to_string(),
        vec![i(admin_id), i(*super_id)],
        "seed.adminrole",
    )
    .await
    .map_err(|e| format!("gagal assign role admin: {e}"))?;
    Ok(())
}

async fn seed_leave_types(tx: &mut Tx<'_>) -> Result<(), String> {
    let rows: Vec<(&str, &str, i64, i64, i64, i64, i64)> = vec![
        ("AL", "Annual Leave", 12, 1, 1, 6, 0),
        ("SL", "Sick Leave", 12, 1, 0, 0, 1),
        ("ML", "Marriage Leave", 3, 1, 0, 0, 1),
        ("MTL", "Maternity Leave", 90, 1, 0, 0, 1),
        ("BL", "Bereavement Leave", 2, 1, 0, 0, 0),
        ("SPL", "Special Leave", 3, 1, 0, 0, 0),
        ("UL", "Unpaid Leave", 0, 0, 0, 0, 0),
        ("CL", "Company Leave", 0, 1, 0, 0, 0),
    ];
    for (code, name, days, paid, carry, carry_max, attachment) in rows {
        insert_ignore(
            tx,
            "leave_types",
            "code, name, default_days_per_year, is_paid, carry_forward, carry_forward_max_days, requires_attachment",
            "?,?,?,?,?,?,?",
            vec![
                t(code),
                t(name),
                i(days),
                i(paid),
                i(carry),
                i(carry_max),
                i(attachment),
            ],
        )
        .await?;
    }
    Ok(())
}

async fn seed_permission_types(tx: &mut Tx<'_>) -> Result<(), String> {
    for (code, name) in [
        ("LATE", "Terlambat"),
        ("EARLY", "Pulang Cepat"),
        ("PERSONAL", "Keperluan Pribadi"),
        ("SICK", "Sakit"),
        ("WFH", "Work From Home"),
        ("BUSINESS", "Urusan Dinas"),
        ("LEAVING", "Keluar Kantor"),
    ] {
        insert_ignore(tx, "permission_types", "code, name", "?,?", vec![
            t(code),
            t(name),
        ])
        .await?;
    }
    Ok(())
}

async fn seed_salary_components(tx: &mut Tx<'_>) -> Result<(), String> {
    let rows: Vec<(&str, &str, &str, &str, i64)> = vec![
        ("BASIC", "Gaji Pokok", "income", "fixed", 1),
        ("POS_ALLOW", "Tunjangan Jabatan", "income", "fixed", 1),
        ("TRANSPORT", "Tunjangan Transport", "income", "fixed", 0),
        ("MEAL", "Tunjangan Makan", "income", "fixed", 0),
        ("COMM", "Tunjangan Komunikasi", "income", "fixed", 0),
        ("ATTEND_ALLOW", "Tunjangan Kehadiran", "income", "fixed", 0),
        ("OVERTIME", "Lembur", "income", "formula", 1),
        ("BONUS", "Bonus", "income", "fixed", 1),
        ("INCENTIVE", "Insentif", "income", "fixed", 1),
        ("THR", "Tunjangan Hari Raya", "income", "fixed", 1),
        (
            "BPJS_HEALTH",
            "BPJS Kesehatan",
            "deduction",
            "percentage",
            0,
        ),
        (
            "BPJS_EMP",
            "BPJS Ketenagakerjaan",
            "deduction",
            "percentage",
            0,
        ),
        ("PPH21", "PPh 21", "deduction", "formula", 0),
        ("LOAN", "Cicilan Pinjaman", "deduction", "fixed", 0),
        ("KASBON", "Kasbon", "deduction", "fixed", 0),
        ("ABSENCE", "Potongan Absensi", "deduction", "formula", 0),
    ];
    for (code, name, ctype, calc, taxable) in rows {
        insert_ignore(
            tx,
            "salary_components",
            "code, name, type, calculation_type, is_taxable",
            "?,?,?,?,?",
            vec![t(code), t(name), t(ctype), t(calc), i(taxable)],
        )
        .await?;
    }
    Ok(())
}

async fn seed_shifts_and_schedule(tx: &mut Tx<'_>) -> Result<i64, String> {
    for (name, start, end, bstart, bend, overnight) in [
        (
            "Regular",
            "08:00:00",
            "17:00:00",
            Some("12:00:00"),
            Some("13:00:00"),
            0,
        ),
        ("Night", "22:00:00", "06:00:00", None, None, 1),
    ] {
        let opt_t = |v: Option<&str>| match v {
            Some(s) => t(s),
            None => SVal::Null,
        };
        insert_ignore(
            tx,
            "shifts",
            "name, start_time, end_time, break_start, break_end, grace_period_minutes, is_overnight",
            "?,?,?,?,?,15,?",
            vec![
                t(name),
                t(start),
                t(end),
                opt_t(bstart),
                opt_t(bend),
                i(overnight),
            ],
        )
        .await?;
    }
    let regular_id = find_id(tx, "shifts", "name = 'Regular'", vec![])
        .await?
        .ok_or_else(|| "shift Regular tidak ditemukan".to_string())?;
    insert_ignore(
        tx,
        "work_schedules",
        "name, description",
        "?,?",
        vec![t("Senin - Jumat"), t("Jadwal kerja reguler Senin sampai Jumat")],
    )
    .await?;
    let schedule_id = find_id(tx, "work_schedules", "name = ?1", vec![t("Senin - Jumat")])
        .await?
        .ok_or_else(|| "jadwal Senin - Jumat tidak ditemukan".to_string())?;
    for day in 0..=6 {
        let working: i64 = if (1..=5).contains(&day) { 1 } else { 0 };
        let shift: SVal = if working == 1 {
            i(regular_id)
        } else {
            SVal::Null
        };
        tx_exec(
            tx,
            "INSERT OR IGNORE INTO work_schedule_days (work_schedule_id, day_of_week, shift_id, is_working_day) VALUES (?1, ?2, ?3, ?4)".to_string(),
            vec![i(schedule_id), i(day), shift, i(working)],
            "seed.schedday",
        )
        .await
        .map_err(|e| format!("gagal seed work_schedule_days: {e}"))?;
    }
    Ok(schedule_id)
}

async fn seed_shift_assignment(
    tx: &mut Tx<'_>,
    employee_id: i64,
    schedule_id: i64,
) -> Result<(), String> {
    let first_of_month = Local::now().format("%Y-%m-01").to_string();
    let exists = tx_q_one(
        tx,
        "SELECT id FROM shift_assignments WHERE employee_id = ?1 AND work_schedule_id = ?2".to_string(),
        vec![i(employee_id), i(schedule_id)],
        1,
        "seed.assigncheck",
    )
    .await
    .map_err(|e| format!("gagal cek shift assignment: {e}"))?;
    if exists.is_none() {
        tx_exec(
            tx,
            "INSERT INTO shift_assignments (employee_id, work_schedule_id, start_date) VALUES (?1, ?2, ?3)".to_string(),
            vec![i(employee_id), i(schedule_id), t(&first_of_month)],
            "seed.assign",
        )
        .await
        .map_err(|e| format!("gagal seed shift assignment: {e}"))?;
    }
    Ok(())
}

async fn seed_holidays(tx: &mut Tx<'_>) -> Result<(), String> {
    let year = Local::now().format("%Y").to_string();
    for (name, suffix) in [
        ("Tahun Baru Masehi", "-01-01"),
        ("Hari Buruh", "-05-01"),
        ("Hari Kemerdekaan RI", "-08-17"),
        ("Hari Raya Natal", "-12-25"),
    ] {
        let date = format!("{year}{suffix}");
        insert_ignore(
            tx,
            "holidays",
            "name, date, type",
            "?,?,?",
            vec![t(name), t(&date), t("national")],
        )
        .await?;
    }
    Ok(())
}

async fn seed_approval_workflows(
    tx: &mut Tx<'_>,
    roles: &std::collections::HashMap<String, i64>,
) -> Result<(), String> {
    let workflows: Vec<(&str, &str, Vec<&str>)> = vec![
        (
            "leave",
            "Alur Persetujuan Cuti",
            vec!["supervisor", "manager", "role:hr-administrator"],
        ),
        (
            "overtime",
            "Alur Persetujuan Lembur",
            vec!["supervisor", "manager"],
        ),
        (
            "reimbursement",
            "Alur Persetujuan Reimbursement",
            vec!["manager", "role:finance"],
        ),
        (
            "business_trip",
            "Alur Persetujuan Perjalanan Dinas",
            vec!["supervisor", "manager"],
        ),
        (
            "offboarding",
            "Alur Persetujuan Resign",
            vec!["supervisor", "role:hr-administrator", "role:finance"],
        ),
    ];
    for (module, name, steps) in workflows {
        insert_ignore(
            tx,
            "approval_workflows",
            "module, name, is_active",
            "?,?,1",
            vec![t(module), t(name)],
        )
        .await?;
        let wf_id = find_id(tx, "approval_workflows", "module = ?1", vec![t(module)])
            .await?
            .ok_or_else(|| format!("workflow {module} tidak ditemukan"))?;
        for (idx, step) in steps.iter().enumerate() {
            let order = (idx + 1) as i64;
            if let Some(role_slug) = step.strip_prefix("role:") {
                let role_id = roles
                    .get(role_slug)
                    .ok_or_else(|| format!("role {role_slug} belum di-seed"))?;
                tx_exec(
                    tx,
                    "INSERT OR IGNORE INTO approval_steps (approval_workflow_id, step_order, approver_type, role_id) VALUES (?1, ?2, 'role', ?3)".to_string(),
                    vec![i(wf_id), i(order), i(*role_id)],
                    "seed.step",
                )
                .await
                .map_err(|e| format!("gagal seed approval step: {e}"))?;
            } else {
                tx_exec(
                    tx,
                    "INSERT OR IGNORE INTO approval_steps (approval_workflow_id, step_order, approver_type) VALUES (?1, ?2, ?3)".to_string(),
                    vec![i(wf_id), i(order), t(step)],
                    "seed.step",
                )
                .await
                .map_err(|e| format!("gagal seed approval step: {e}"))?;
            }
        }
    }
    Ok(())
}

async fn seed_categories(tx: &mut Tx<'_>) -> Result<(), String> {
    for (code, name) in [
        ("LAPTOP", "Laptop"),
        ("MOBILE", "Handphone"),
        ("FURNITURE", "Furniture"),
        ("VEHICLE", "Kendaraan"),
    ] {
        insert_ignore(tx, "asset_categories", "code, name", "?,?", vec![
            t(code),
            t(name),
        ])
        .await?;
    }
    let rows: Vec<(&str, &str, Option<f64>)> = vec![
        ("TRANSPORT", "Transportasi", Some(1_000_000.0)),
        ("MEDICAL", "Kesehatan", Some(2_000_000.0)),
        ("MEAL", "Makan", Some(500_000.0)),
        ("OTHER", "Lainnya", None),
    ];
    for (code, name, max) in rows {
        insert_ignore(
            tx,
            "reimbursement_categories",
            "code, name, max_amount",
            "?,?,?",
            vec![
                t(code),
                t(name),
                match max {
                    Some(m) => SVal::Float(m),
                    None => SVal::Null,
                },
            ],
        )
        .await?;
    }
    Ok(())
}

async fn seed_settings(tx: &mut Tx<'_>) -> Result<(), String> {
    let settings: Vec<(&str, &str)> = vec![
        ("company_name", "PT Contoh Sukses Indonesia"),
        ("company_email", "info@contohsukses.co.id"),
        ("company_phone", "+62-31-5551234"),
        ("work_start_time", "08:00"),
        ("work_end_time", "17:00"),
        ("attendance_grace_minutes", "15"),
        ("annual_leave_default_days", "12"),
        ("overtime_rate_multiplier", "1.5"),
        ("bpjs_health_employee_percent", "1"),
        ("bpjs_employment_employee_percent", "2"),
        ("bpjs_jp_employee_percent", "1"),
        ("bpjs_health_max_wage", "0"),
        ("bpjs_jht_max_wage", "0"),
        ("bpjs_jp_max_wage", "0"),
        ("thr_holiday_date", ""),
        ("backup_schedule", "off"),
        ("backup_last_at", ""),
        ("training_pass_score", "70"),
    ];
    for (key, value) in settings {
        insert_ignore(
            tx,
            "system_settings",
            "setting_key, setting_value, setting_group",
            "?,?,?",
            vec![t(key), t(value), t("general")],
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connect_sea, migrate_sea};

    async fn seeded_db() -> (tempfile::TempDir, sea_orm::DatabaseConnection) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db = connect_sea(&crate::config::AppConfig::sqlite_file("seed.db"), dir.path())
            .await
            .expect("connect");
        migrate_sea(&db).await.expect("migrate");
        seed(&db).await.expect("seed pertama");
        (dir, db)
    }

    async fn counts(
        db: &sea_orm::DatabaseConnection,
    ) -> std::collections::HashMap<String, i64> {
        let mut m = std::collections::HashMap::new();
        for tbl in [
            "roles",
            "permissions",
            "users",
            "role_permissions",
            "leave_types",
            "salary_components",
            "approval_workflows",
            "approval_steps",
            "system_settings",
        ] {
            let rows = crate::services::sea_raw::q_all(
                db,
                format!("SELECT COUNT(*) FROM {tbl}"),
                vec![],
                1,
                "seed.testcount",
            )
            .await
            .unwrap();
            m.insert(
                tbl.to_string(),
                rows.first()
                    .and_then(|r| crate::services::sea_raw::value_i64(&r[0]))
                    .unwrap_or(0),
            );
        }
        m
    }

    #[tokio::test]
    async fn seed_idempoten_dijalankan_dua_kali() {
        let (_dir, db) = seeded_db().await;
        let before = counts(&db).await;
        seed(&db).await.expect("seed kedua");
        let after = counts(&db).await;
        assert_eq!(before, after, "seed kedua tidak boleh menambah baris");
    }

    #[tokio::test]
    async fn seed_mengisi_master_data_kunci() {
        let (_dir, db) = seeded_db().await;
        let c = counts(&db).await;
        assert_eq!(c["roles"], 8);
        assert_eq!(c["permissions"], 87);
        assert_eq!(c["leave_types"], 8);
        assert_eq!(c["salary_components"], 16);
        assert_eq!(c["approval_workflows"], 5);
        assert_eq!(c["system_settings"], 18);
        let rows = crate::services::sea_raw::q_all(
            &db,
            "SELECT COUNT(*) FROM role_permissions rp JOIN roles r ON r.id = rp.role_id WHERE r.slug = 'super-administrator'".to_string(),
            vec![],
            1,
            "seed.testsuper",
        )
        .await
        .unwrap();
        let super_perms = rows
            .first()
            .and_then(|r| crate::services::sea_raw::value_i64(&r[0]))
            .unwrap_or(0);
        assert_eq!(super_perms, 87);
    }

    #[tokio::test]
    async fn admin_bisa_login_dengan_kredensial_awal() {
        let (_dir, db) = seeded_db().await;
        let rows = crate::services::sea_raw::q_all(
            &db,
            "SELECT password, must_change_password FROM users WHERE username = 'admin'".to_string(),
            vec![],
            2,
            "seed.testadmin",
        )
        .await
        .unwrap();
        let row = rows.first().expect("admin ada");
        let hash = crate::services::sea_raw::value_to_string(&row[0]);
        let must_change = crate::services::sea_raw::value_i64(&row[1]).unwrap_or(0);
        assert!(bcrypt::verify("Admin@123", &hash).unwrap());
        assert_eq!(must_change, 1);
    }
}
