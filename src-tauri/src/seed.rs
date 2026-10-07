//! Data awal: company, roles, permissions, admin, master data, dan dataset demo.
//!
//! Idempoten: aman dijalankan berulang, tidak membuat duplikat.

use chrono::{Datelike, Duration, Local, Weekday};
use sea_orm::TransactionTrait;
use crate::services::sea_raw::{exec, exec_insert, q_all, Value};

/// Ringkasan hasil seeding untuk logging.
#[derive(Debug, Default)]
pub struct SeedSummary {
    pub roles: i64,
    pub permissions: i64,
    pub users: i64,
}

type Tx = sea_orm::DatabaseTransaction;

/// Nilai sel generik untuk seed dinamis.
#[derive(Clone, Debug)]
enum SVal {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
}

fn ke_db(v: SVal) -> Value {
    match v {
        SVal::Null => Value::Null,
        SVal::Int(i) => Value::Int(i),
        SVal::Float(f) => Value::Float(f),
        SVal::Text(s) => Value::Text(s),
    }
}

fn dari_db(v: Value) -> SVal {
    match v {
        Value::Null => SVal::Null,
        Value::Int(i) => SVal::Int(i),
        Value::Float(f) => SVal::Float(f),
        Value::Text(s) => SVal::Text(s),
    }
}

fn sval_i64(v: &SVal) -> Option<i64> {
    match v {
        SVal::Int(i) => Some(*i),
        SVal::Text(s) => s.parse().ok(),
        _ => None,
    }
}

async fn tx_q_all(
    tx: &Tx,
    sql: String,
    vals: Vec<SVal>,
    ncols: usize,
    label: &str,
) -> Result<Vec<Vec<SVal>>, String> {
    let rows = q_all(tx, sql, vals.into_iter().map(ke_db).collect(), ncols, label).await?;
    Ok(rows
        .into_iter()
        .map(|r| r.into_iter().map(dari_db).collect())
        .collect())
}

async fn tx_q_one(
    tx: &Tx,
    sql: String,
    vals: Vec<SVal>,
    ncols: usize,
    label: &str,
) -> Result<Option<Vec<SVal>>, String> {
    let mut rows = tx_q_all(tx, sql, vals, ncols, label).await?;
    Ok(rows.pop())
}

async fn tx_exec(
    tx: &Tx,
    sql: String,
    vals: Vec<SVal>,
    label: &str,
) -> Result<u64, String> {
    exec(tx, sql, vals.into_iter().map(ke_db).collect(), label).await
}

/// Jalankan seluruh seed dalam satu transaksi.
pub async fn seed(db: &sea_orm::DatabaseConnection) -> Result<SeedSummary, String> {
    let tx = db
        .begin()
        .await
        .map_err(|e| format!("gagal memulai transaksi seed: {e}"))?;
    let inner = async {
        let company_id = seed_company(&tx).await?;
        let (dept_hr, dept_it, dept_fin) = seed_departments(&tx, company_id).await?;
        let (lvl_staff, _lvl_spv, lvl_mgr) = seed_job_levels(&tx).await?;
        seed_job_grades(&tx).await?;
        let pos_hr_manager =
            seed_positions(&tx, dept_hr, dept_it, dept_fin, lvl_staff, lvl_mgr).await?;
        let hq_location = seed_work_location(&tx).await?;
        seed_cost_center(&tx, dept_hr).await?;
        let role_ids = seed_roles(&tx).await?;
        let perm_ids = seed_permissions(&tx).await?;
        seed_role_permissions(&tx, &role_ids, &perm_ids).await?;
        let admin_employee = seed_admin_employee(
            &tx,
            company_id,
            dept_hr,
            pos_hr_manager,
            lvl_mgr,
            hq_location,
        )
        .await?;
        seed_admin_user(&tx, &role_ids, admin_employee).await?;
        seed_leave_types(&tx).await?;
        seed_permission_types(&tx).await?;
        seed_salary_components(&tx).await?;
        let schedule_regular = seed_shifts_and_schedule(&tx).await?;
        seed_shift_assignment(&tx, admin_employee, schedule_regular).await?;
        seed_holidays(&tx).await?;
        seed_approval_workflows(&tx, &role_ids).await?;
        seed_categories(&tx).await?;
        seed_settings(&tx).await?;
        seed_demo(&tx, &role_ids).await?;
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
    tx: &Tx,
    table: &str,
    columns: &str,
    placeholders: &str,
    p: Vec<SVal>,
) -> Result<i64, String> {
    Ok(exec_insert(
        tx,
        format!("INSERT OR IGNORE INTO {table} ({columns}) VALUES ({placeholders})"),
        p.into_iter().map(ke_db).collect(),
        "seed.insert",
    )
    .await
    .map_err(|e| format!("gagal insert {table}: {e}"))
    .unwrap_or(0))
}

async fn find_id(
    tx: &Tx,
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

async fn seed_company(tx: &Tx) -> Result<i64, String> {
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
    tx: &Tx,
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

async fn seed_job_levels(tx: &Tx) -> Result<(i64, i64, i64), String> {
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

async fn seed_job_grades(tx: &Tx) -> Result<(), String> {
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
    tx: &Tx,
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

async fn seed_work_location(tx: &Tx) -> Result<i64, String> {
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

async fn seed_cost_center(tx: &Tx, dept_hr: i64) -> Result<(), String> {
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
    tx: &Tx,
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
            "it-administrator",
            "IT Administrator",
            "Mengelola cadangan dan konfigurasi database",
            0,
        ),
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
    tx: &Tx,
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
        ("pulse", vec!["view", "create", "answer"]),
        ("report", vec!["view", "export"]),
        ("settings", vec!["manage"]),
        ("rbac", vec!["manage"]),
        ("workflow", vec!["manage"]),
        ("backup", vec!["manage"]),
        ("sso", vec!["manage"]),
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

async fn grant(tx: &Tx, role_id: i64, perm_id: i64) -> Result<(), String> {
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
    tx: &Tx,
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
                "pulse.view",
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
                "pulse.view",
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
                "pulse.view",
                "pulse.answer",
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
        (
            "it-administrator",
            [
                "backup.manage",
                "sso.manage",
                "settings.manage",
                "audit.view",
                "report.view",
                "employee.view",
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
    tx: &Tx,
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
    tx: &Tx,
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

async fn seed_leave_types(tx: &Tx) -> Result<(), String> {
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

async fn seed_permission_types(tx: &Tx) -> Result<(), String> {
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

async fn seed_salary_components(tx: &Tx) -> Result<(), String> {
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

async fn seed_shifts_and_schedule(tx: &Tx) -> Result<i64, String> {
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
        let exists = tx_q_one(
            tx,
            "SELECT id FROM shifts WHERE name = ?1".to_string(),
            vec![t(name)],
            1,
            "seed.shiftcheck",
        )
        .await
        .map_err(|e| format!("gagal cek shift: {e}"))?;
        if exists.is_none() {
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
    }
    let regular_id = find_id(tx, "shifts", "name = 'Regular'", vec![])
        .await?
        .ok_or_else(|| "shift Regular tidak ditemukan".to_string())?;
    let sched_exists = tx_q_one(
        tx,
        "SELECT id FROM work_schedules WHERE name = ?1".to_string(),
        vec![t("Senin - Jumat")],
        1,
        "seed.schedcheck",
    )
    .await
    .map_err(|e| format!("gagal cek jadwal: {e}"))?;
    if sched_exists.is_none() {
        insert_ignore(
            tx,
            "work_schedules",
            "name, description",
            "?,?",
            vec![t("Senin - Jumat"), t("Jadwal kerja reguler Senin sampai Jumat")],
        )
        .await?;
    }
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
    tx: &Tx,
    employee_id: i64,
    schedule_id: i64,
) -> Result<(), String> {
    let first_of_month = Local::now().format("%Y-%m-01").to_string();
    tx_exec(
        tx,
        "INSERT INTO shift_assignments (employee_id, work_schedule_id, start_date) SELECT ?1, ?2, ?3 WHERE NOT EXISTS (SELECT 1 FROM shift_assignments WHERE employee_id = ?4 AND work_schedule_id = ?5)".to_string(),
        vec![i(employee_id), i(schedule_id), t(&first_of_month), i(employee_id), i(schedule_id)],
        "seed.assign",
    )
    .await
    .map_err(|e| format!("gagal seed shift assignment: {e}"))?;
    Ok(())
}

async fn seed_holidays(tx: &Tx) -> Result<(), String> {
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
    tx: &Tx,
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

async fn seed_categories(tx: &Tx) -> Result<(), String> {
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

async fn seed_settings(tx: &Tx) -> Result<(), String> {
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

/// Dataset demo: 20 pengguna lintas peran + data tiap modul.
///
/// Idempoten: semua insert memakai INSERT OR IGNORE / cek-eksistensi dulu,
/// dan tanggal transaksi dihitung relatif terhadap hari ini sehingga seed
/// kedua tidak menambah baris baru.
async fn seed_demo(
    tx: &Tx,
    roles: &std::collections::HashMap<String, i64>,
) -> Result<(), String> {
    let today = Local::now().date_naive();
    let fmt = |d: chrono::NaiveDate| d.format("%Y-%m-%d").to_string();
    let today_s = fmt(today);
    let year_now = today.year();
    let first_of_month = today.format("%Y-%m-01").to_string();
    let pay_date = fmt(today + Duration::days(2));

    // Master tambahan untuk demo: posisi per departemen + lokasi kerja kedua.
    let company_id = find_id(tx, "companies", "code = ?1", vec![t("HQ")])
        .await?
        .ok_or_else(|| "company HQ tidak ditemukan".to_string())?;
    async fn dept_id(tx: &Tx, company_id: i64, code: &str) -> Result<i64, String> {
        find_id(
            tx,
            "departments",
            "company_id = ?1 AND code = ?2",
            vec![i(company_id), t(code)],
        )
        .await?
        .ok_or_else(|| format!("department {code} tidak ditemukan"))
    }
    async fn lvl_id(tx: &Tx, code: &str) -> Result<i64, String> {
        find_id(tx, "job_levels", "code = ?1", vec![t(code)])
            .await?
            .ok_or_else(|| format!("job level {code} tidak ditemukan"))
    }
    async fn pos_id(tx: &Tx, code: &str) -> Result<i64, String> {
        find_id(tx, "positions", "code = ?1", vec![t(code)])
            .await?
            .ok_or_else(|| format!("position {code} tidak ditemukan"))
    }
    async fn comp_id(tx: &Tx, code: &str) -> Result<i64, String> {
        find_id(tx, "salary_components", "code = ?1", vec![t(code)])
            .await?
            .ok_or_else(|| format!("salary component {code} tidak ditemukan"))
    }
    async fn lt_id(tx: &Tx, code: &str) -> Result<i64, String> {
        find_id(tx, "leave_types", "code = ?1", vec![t(code)])
            .await?
            .ok_or_else(|| format!("leave type {code} tidak ditemukan"))
    }
    async fn asset_id_of(tx: &Tx, code: &str) -> Result<i64, String> {
        find_id(tx, "assets", "asset_code = ?1", vec![t(code)])
            .await?
            .ok_or_else(|| format!("aset {code} tidak ditemukan"))
    }
    let dept_hr = dept_id(tx, company_id, "HRD").await?;
    let dept_it = dept_id(tx, company_id, "IT").await?;
    let dept_fin = dept_id(tx, company_id, "FIN").await?;
    let lvl_staff = lvl_id(tx, "STAFF").await?;
    let lvl_spv = lvl_id(tx, "SPV").await?;
    let lvl_mgr = lvl_id(tx, "MGR").await?;
    let lvl_dir = lvl_id(tx, "DIR").await?;

    let extra_positions: Vec<(&str, &str, i64, i64)> = vec![
        ("DIRUT", "Direktur Utama", dept_hr, lvl_dir),
        ("HRSPV", "HR Supervisor", dept_hr, lvl_spv),
        ("ITM", "IT Manager", dept_it, lvl_mgr),
        ("ITSPV", "IT Supervisor", dept_it, lvl_spv),
        ("FINM", "Finance Manager", dept_fin, lvl_mgr),
        ("FINSPV", "Finance Supervisor", dept_fin, lvl_spv),
        ("MGR-OPS", "Operational Manager", dept_it, lvl_mgr),
        ("SPV-OPS", "Operational Supervisor", dept_it, lvl_spv),
        ("STF", "Staff Umum", dept_hr, lvl_staff),
    ];
    for (code, name, d, l) in &extra_positions {
        insert_ignore(
            tx,
            "positions",
            "code, name, department_id, job_level_id",
            "?,?,?,?",
            vec![t(code), t(name), i(*d), i(*l)],
        )
        .await?;
    }
    insert_ignore(
        tx,
        "work_locations",
        "name, address, latitude, longitude, radius_meter",
        "?,?,?,?,?",
        vec![
            t("Gudang Sidoarjo"),
            t("Jl. Raya Buduran, Sidoarjo"),
            SVal::Float(-6.224),
            SVal::Float(106.809),
            i(150),
        ],
    )
    .await?;

    // 20 karyawan demo: nomor, nama, gender, dept, posisi, level, status, tipe,
    // peran login. Manager/supervisor dirujuk via supervisor_id/manager_id
    // memakai nomor karyawan agar hirarki konsisten.
    // (nomor, depan, belakang, gender, nik, dept, pos, level,
    //  status, tipe, username, role, sup, mgr)
    struct Demo<'a> {
        no: &'a str,
        first: &'a str,
        last: &'a str,
        gender: &'a str,
        nik: &'a str,
        dept: i64,
        pos: &'a str,
        level: i64,
        status: &'a str,
        etype: &'a str,
        user: &'a str,
        role: &'a str,
        sup: Option<&'a str>,
        mgr: Option<&'a str>,
    }
    let demos = vec![
        Demo { no: "EMP-0002", first: "Budi", last: "Santoso", gender: "male", nik: "3171000000000002", dept: dept_hr, pos: "HRSPV", level: lvl_spv, status: "active", etype: "permanent", user: "hr.admin", role: "hr-administrator", sup: None, mgr: None },
        Demo { no: "EMP-0003", first: "Siti", last: "Rahayu", gender: "female", nik: "3171000000000003", dept: dept_hr, pos: "HRM", level: lvl_mgr, status: "active", etype: "permanent", user: "hr.manager", role: "hr-manager", sup: None, mgr: None },
        Demo { no: "EMP-0004", first: "Agus", last: "Wijaya", gender: "male", nik: "3171000000000004", dept: dept_fin, pos: "FINM", level: lvl_mgr, status: "active", etype: "permanent", user: "finance", role: "finance", sup: None, mgr: None },
        Demo { no: "EMP-0005", first: "Dewi", last: "Lestari", gender: "female", nik: "3171000000000005", dept: dept_it, pos: "ITM", level: lvl_mgr, status: "active", etype: "permanent", user: "manager", role: "manager", sup: None, mgr: None },
        Demo { no: "EMP-0006", first: "Rudi", last: "Hartono", gender: "male", nik: "3171000000000006", dept: dept_it, pos: "ITSPV", level: lvl_spv, status: "active", etype: "permanent", user: "supervisor", role: "supervisor", sup: None, mgr: Some("EMP-0005") },
        Demo { no: "EMP-0007", first: "Maya", last: "Putri", gender: "female", nik: "3171000000000007", dept: dept_it, pos: "ITS", level: lvl_staff, status: "active", etype: "permanent", user: "maya", role: "employee", sup: Some("EMP-0006"), mgr: Some("EMP-0005") },
        Demo { no: "EMP-0008", first: "Joko", last: "Prasetyo", gender: "male", nik: "3171000000000008", dept: dept_it, pos: "ITS", level: lvl_staff, status: "active", etype: "contract", user: "joko", role: "employee", sup: Some("EMP-0006"), mgr: Some("EMP-0005") },
        Demo { no: "EMP-0009", first: "Ayu", last: "Ningsih", gender: "female", nik: "3171000000000009", dept: dept_fin, pos: "FINS", level: lvl_staff, status: "active", etype: "permanent", user: "ayu", role: "employee", sup: None, mgr: Some("EMP-0004") },
        Demo { no: "EMP-0010", first: "Hendra", last: "Gunawan", gender: "male", nik: "3171000000000010", dept: dept_hr, pos: "HRS", level: lvl_staff, status: "probation", etype: "contract", user: "hendra", role: "employee", sup: Some("EMP-0002"), mgr: Some("EMP-0003") },
        Demo { no: "EMP-0011", first: "Rina", last: "Wulandari", gender: "female", nik: "3171000000000011", dept: dept_it, pos: "ITS", level: lvl_staff, status: "active", etype: "permanent", user: "it.admin", role: "it-administrator", sup: None, mgr: Some("EMP-0005") },
        Demo { no: "EMP-0012", first: "Fajar", last: "Nugroho", gender: "male", nik: "3171000000000012", dept: dept_fin, pos: "FINSPV", level: lvl_spv, status: "active", etype: "permanent", user: "auditor", role: "auditor", sup: None, mgr: Some("EMP-0004") },
        Demo { no: "EMP-0013", first: "Lina", last: "Marlina", gender: "female", nik: "3171000000000013", dept: dept_hr, pos: "STF", level: lvl_staff, status: "active", etype: "permanent", user: "lina", role: "employee", sup: Some("EMP-0002"), mgr: Some("EMP-0003") },
        Demo { no: "EMP-0014", first: "Dedi", last: "Kurniawan", gender: "male", nik: "3171000000000014", dept: dept_it, pos: "ITS", level: lvl_staff, status: "active", etype: "contract", user: "dedi", role: "employee", sup: Some("EMP-0006"), mgr: Some("EMP-0005") },
        Demo { no: "EMP-0015", first: "Nina", last: "Kurnia", gender: "female", nik: "3171000000000015", dept: dept_fin, pos: "FINS", level: lvl_staff, status: "probation", etype: "contract", user: "nina", role: "employee", sup: None, mgr: Some("EMP-0004") },
        Demo { no: "EMP-0016", first: "Yoga", last: "Saputra", gender: "male", nik: "3171000000000016", dept: dept_it, pos: "STF", level: lvl_staff, status: "active", etype: "intern", user: "yoga", role: "employee", sup: Some("EMP-0006"), mgr: Some("EMP-0005") },
        Demo { no: "EMP-0017", first: "Fitri", last: "Handayani", gender: "female", nik: "3171000000000017", dept: dept_hr, pos: "HRS", level: lvl_staff, status: "resigned", etype: "contract", user: "fitri", role: "employee", sup: Some("EMP-0002"), mgr: Some("EMP-0003") },
        Demo { no: "EMP-0018", first: "Irfan", last: "Hakim", gender: "male", nik: "3171000000000018", dept: dept_it, pos: "ITS", level: lvl_staff, status: "active", etype: "permanent", user: "irfan", role: "employee", sup: Some("EMP-0006"), mgr: Some("EMP-0005") },
        Demo { no: "EMP-0019", first: "Sari", last: "Dewi", gender: "female", nik: "3171000000000019", dept: dept_fin, pos: "STF", level: lvl_staff, status: "active", etype: "daily", user: "sari", role: "employee", sup: None, mgr: Some("EMP-0004") },
        Demo { no: "EMP-0020", first: "Wahyu", last: "Hidayat", gender: "male", nik: "3171000000000020", dept: dept_hr, pos: "DIRUT", level: lvl_dir, status: "active", etype: "permanent", user: "direktur", role: "manager", sup: None, mgr: None },
        Demo { no: "EMP-0021", first: "Eka", last: "Pratiwi", gender: "female", nik: "3171000000000021", dept: dept_it, pos: "ITS", level: lvl_staff, status: "terminated", etype: "contract", user: "eka", role: "employee", sup: Some("EMP-0006"), mgr: Some("EMP-0005") },
    ];

    let branch_ho = find_id(
        tx,
        "branches",
        "company_id = ?1 AND code = 'HO'",
        vec![i(company_id)],
    )
    .await?
    .ok_or_else(|| "branch HO tidak ditemukan".to_string())?;
    let join_date = fmt(today - Duration::days(400));
    let mut emp_ids: std::collections::HashMap<&str, i64> =
        std::collections::HashMap::new();
    let mut user_ids: std::collections::HashMap<&str, i64> =
        std::collections::HashMap::new();
    for d in &demos {
        insert_ignore(
            tx,
            "employees",
            "employee_number, nik, first_name, last_name, gender, company_id, branch_id, department_id, position_id, job_level_id, join_date, employment_status, employment_type",
            "?,?,?,?,?,?,?,?,?,?,?,?,?",
            vec![
                t(d.no),
                t(d.nik),
                t(d.first),
                t(d.last),
                t(d.gender),
                i(company_id),
                i(branch_ho),
                i(d.dept),
                i(pos_id(tx, d.pos).await?),
                i(d.level),
                t(&join_date),
                t(d.status),
                t(d.etype),
            ],
        )
        .await?;
        let eid = find_id(tx, "employees", "employee_number = ?1", vec![t(d.no)])
            .await?
            .ok_or_else(|| format!("employee {} tidak ditemukan", d.no))?;
        emp_ids.insert(d.no, eid);
    }
    // Tautkan hirarki setelah semua karyawan ada (idempoten via UPDATE
    // hanya bila berbeda, dengan perbandingan aman-NULL).
    for d in &demos {
        let eid = emp_ids[d.no];
        let sup = d.sup.and_then(|s| emp_ids.get(s).copied());
        let mgr = d.mgr.and_then(|s| emp_ids.get(s).copied());
        let cur = tx_q_one(
            tx,
            "SELECT supervisor_id, manager_id FROM employees WHERE id = ?1".to_string(),
            vec![i(eid)],
            2,
            "seed.demohiercheck",
        )
        .await
        .map_err(|e| format!("gagal cek hirarki demo: {e}"))?;
        let (cur_sup, cur_mgr) = match cur.as_ref() {
            Some(r) => (
                match &r[0] {
                    SVal::Int(v) => Some(*v),
                    _ => None,
                },
                match &r[1] {
                    SVal::Int(v) => Some(*v),
                    _ => None,
                },
            ),
            None => (None, None),
        };
        if cur_sup != sup || cur_mgr != mgr {
            tx_exec(
                tx,
                "UPDATE employees SET supervisor_id = ?1, manager_id = ?2 WHERE id = ?3".to_string(),
                vec![
                    sup.map(i).unwrap_or(SVal::Null),
                    mgr.map(i).unwrap_or(SVal::Null),
                    i(eid),
                ],
                "seed.demohier",
            )
            .await
            .map_err(|e| format!("gagal taut hirarki demo: {e}"))?;
        }
    }
    // Pengguna + peran: hash sekali, pakai ulang (bcrypt acak tiap run).
    let demo_hash = bcrypt::hash("Demo@123", bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password demo: {e}"))?;
    for d in &demos {
        let eid = emp_ids[d.no];
        let exists = find_id(tx, "users", "username = ?1", vec![t(d.user)]).await?;
        if exists.is_none() {
            tx_exec(
                tx,
                "INSERT INTO users (employee_id, username, email, password, status, must_change_password) VALUES (?1, ?2, ?3, ?4, 'active', 0)".to_string(),
                vec![
                    i(eid),
                    t(d.user),
                    t(&format!("{}@hris.local", d.user)),
                    t(&demo_hash),
                ],
                "seed.demouser",
            )
            .await
            .map_err(|e| format!("gagal insert user demo: {e}"))?;
        }
        let uid = find_id(tx, "users", "username = ?1", vec![t(d.user)])
            .await?
            .ok_or_else(|| format!("user {} tidak ditemukan", d.user))?;
        user_ids.insert(d.user, uid);
        let rid = roles
            .get(d.role)
            .ok_or_else(|| format!("role {} belum di-seed", d.role))?;
        tx_exec(
            tx,
            "INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?1, ?2)".to_string(),
            vec![i(uid), i(*rid)],
            "seed.demurole",
        )
        .await
        .map_err(|e| format!("gagal assign role demo: {e}"))?;
    }

    let eid = |no: &str| -> Result<i64, String> {
        emp_ids
            .get(no)
            .copied()
            .ok_or_else(|| format!("employee {no} belum di-seed"))
    };
    let uid = |name: &str| -> Result<i64, String> {
        user_ids
            .get(name)
            .copied()
            .ok_or_else(|| format!("user {name} belum di-seed"))
    };

    // Gaji pokok + tunjangan (skala per level) untuk 20 karyawan demo.
    let basic_for = |level: i64| -> f64 {
        if level == lvl_dir {
            30_000_000.0
        } else if level == lvl_mgr {
            15_000_000.0
        } else if level == lvl_spv {
            9_000_000.0
        } else {
            6_000_000.0
        }
    };
    let comp_pos = comp_id(tx, "POS_ALLOW").await?;
    let comp_trans = comp_id(tx, "TRANSPORT").await?;
    let comp_meal = comp_id(tx, "MEAL").await?;
    for d in &demos {
        let e = eid(d.no)?;
        let basic = basic_for(d.level);
        let exists = tx_q_one(
            tx,
            "SELECT id FROM employee_salaries WHERE employee_id = ?1 AND is_active = 1".to_string(),
            vec![i(e)],
            1,
            "seed.demosalcheck",
        )
        .await
        .map_err(|e| format!("gagal cek gaji demo: {e}"))?;
        if exists.is_none() {
            let sid = exec_insert(
                tx,
                "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, ?2, ?3, 1)".to_string(),
                vec![i(e), SVal::Float(basic), t(&first_of_month)]
                    .into_iter()
                    .map(ke_db)
                    .collect(),
                "seed.demosal",
            )
            .await
            .map_err(|e| format!("gagal insert gaji demo: {e}"))?;
            if sid > 0 {
                for (cid, amount) in [
                    (comp_pos, basic * 0.2),
                    (comp_trans, 1_000_000.0),
                    (comp_meal, 750_000.0),
                ] {
                    tx_exec(
                        tx,
                        "INSERT OR IGNORE INTO employee_salary_components (employee_salary_id, salary_component_id, amount) VALUES (?1, ?2, ?3)".to_string(),
                        vec![i(sid), i(cid), SVal::Float(amount)],
                        "seed.demosalcomp",
                    )
                    .await
                    .map_err(|e| format!("gagal insert tunjangan demo: {e}"))?;
                }
            }
        }
    }

    // Saldo cuti tahunan (AL) + sakit (SL) untuk tiap karyawan aktif.
    let lt_al = lt_id(tx, "AL").await?;
    let lt_sl = lt_id(tx, "SL").await?;
    for d in demos.iter().filter(|d| d.status == "active" || d.status == "probation") {
        let e = eid(d.no)?;
        for (lid, alloc) in [(lt_al, 12.0), (lt_sl, 12.0)] {
            tx_exec(
                tx,
                "INSERT OR IGNORE INTO leave_balances (employee_id, leave_type_id, year, allocated_days) VALUES (?1, ?2, ?3, ?4)".to_string(),
                vec![i(e), i(lid), i(year_now as i64), SVal::Float(alloc)],
                "seed.demobal",
            )
            .await
            .map_err(|e| format!("gagal seed saldo cuti demo: {e}"))?;
        }
    }

    // Absensi 5 hari kerja terakhir untuk tiap karyawan aktif (hadir,
    // satu terlambat, satu WFH) memakai tanggal relatif idempoten.
    let reg_shift = find_id(tx, "shifts", "name = 'Regular'", vec![])
        .await?
        .ok_or_else(|| "shift Regular tidak ditemukan".to_string())?;
    let mut workdays: Vec<chrono::NaiveDate> = Vec::new();
    let mut back = today - Duration::days(1);
    while workdays.len() < 5 {
        if !matches!(back.weekday(), Weekday::Sat | Weekday::Sun) {
            workdays.push(back);
        }
        back = back - Duration::days(1);
    }
    for (idx, d) in demos
        .iter()
        .filter(|d| d.status == "active" || d.status == "probation")
        .enumerate()
    {
        let e = eid(d.no)?;
        for (w, day) in workdays.iter().enumerate() {
            let (status, clock_in, late) = if idx % 7 == 3 && w == 0 {
                ("late", "08:45:00", 30)
            } else if idx % 5 == 4 && w == 1 {
                ("wfh", "08:00:00", 0)
            } else {
                ("present", "07:55:00", 0)
            };
            let ds = fmt(*day);
            tx_exec(
                tx,
                "INSERT OR IGNORE INTO attendances (employee_id, date, clock_in, clock_out, shift_id, status, late_minutes, work_minutes) VALUES (?1, ?2, ?3, '17:05:00', ?4, ?5, ?6, 480)".to_string(),
                vec![
                    i(e),
                    t(&ds),
                    t(&format!("{ds} {clock_in}")),
                    i(reg_shift),
                    t(status),
                    i(late),
                ],
                "seed.demoatt",
            )
            .await
            .map_err(|e| format!("gagal seed absensi demo: {e}"))?;
        }
    }

    // Transaksional per modul (tanggal relatif, INSERT OR IGNORE / cek dulu):
    // cuti approved + pending, izin approved, lembur approved, dinas + expense,
    // reimbursement paid + pending, lowongan + kandidat + interview + assessment,
    // onboarding + tasks, kontrak aktif, dokumen, training + peserta + sertifikat,
    // skill, aset + assignment + maintenance, KPI + review + detail,
    // periode + payroll + detail + potongan + slip, pengumuman + target + baca,
    // notifikasi, approval request + history, audit log, offboarding + exit +
    // clearance, koreksi absensi.
    let maya = eid("EMP-0007")?;
    let joko = eid("EMP-0008")?;
    let ayu = eid("EMP-0009")?;
    let hendra = eid("EMP-0010")?;
    let lina = eid("EMP-0013")?;
    let dedi = eid("EMP-0014")?;
    let fitri = eid("EMP-0017")?;
    let irfan = eid("EMP-0018")?;
    let sup_user = uid("supervisor")?;
    let mgr_user = uid("manager")?;
    let admin_user = uid_or_admin(tx, &user_ids, "admin").await?;

    // Cuti: satu approved (potong saldo via used_days), satu pending.
    let leave_start = fmt(today + Duration::days(7));
    let leave_end = fmt(today + Duration::days(8));
    let lr_exists = tx_q_one(
        tx,
        "SELECT id FROM leave_requests WHERE employee_id = ?1 AND start_date = ?2".to_string(),
        vec![i(maya), t(&leave_start)],
        1,
        "seed.demolrcheck",
    )
    .await
    .map_err(|e| format!("gagal cek cuti demo: {e}"))?;
    if lr_exists.is_none() {
        let lr_id = exec_insert(
            tx,
            "INSERT INTO leave_requests (employee_id, leave_type_id, start_date, end_date, total_days, reason, status) VALUES (?1, ?2, ?3, ?4, 2, 'Liburan keluarga', 'approved')".to_string(),
            vec![i(maya), i(lt_al), t(&leave_start), t(&leave_end)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demolr",
        )
        .await
        .map_err(|e| format!("gagal insert cuti demo: {e}"))?;
        if lr_id > 0 {
            tx_exec(
                tx,
                "INSERT INTO leave_approvals (leave_request_id, approver_id, step_order, step_role, status, acted_at) VALUES (?1, ?2, 1, 'supervisor', 'approved', ?3)".to_string(),
                vec![i(lr_id), i(sup_user), t(&today_s)],
                "seed.demolappr",
            )
            .await
            .map_err(|e| format!("gagal insert approval cuti demo: {e}"))?;
            tx_exec(
                tx,
                "UPDATE leave_balances SET used_days = used_days + 2 WHERE employee_id = ?1 AND leave_type_id = ?2 AND year = ?3".to_string(),
                vec![i(maya), i(lt_al), i(year_now as i64)],
                "seed.demobaluse",
            )
            .await
            .map_err(|e| format!("gagal potong saldo cuti demo: {e}"))?;
        }
    }
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO leave_requests (id, employee_id, leave_type_id, start_date, end_date, total_days, reason, status) VALUES (9001, {m}, {lt}, '{s}', '{s}', 1, 'Acara keluarga', 'pending')",
            m = joko.to_string(),
            lt = lt_al.to_string(),
            s = fmt(today + Duration::days(14)),
        ),
        vec![],
        "seed.demolrpend",
    )
    .await
    .map_err(|e| format!("gagal insert cuti pending demo: {e}"))?;

    // Izin WFH approved.
    let perm_type = find_id(tx, "permission_types", "code = ?1", vec![t("WFH")])
        .await?
        .ok_or_else(|| "permission type WFH tidak ditemukan".to_string())?;
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO permission_requests (id, employee_id, permission_type_id, date, start_time, end_time, reason, status, approved_by, approved_at) VALUES (9001, {e}, {p}, '{d}', '08:00', '17:00', 'Fokus pengerjaan laporan', 'approved', {a}, '{d}')",
            e = ayu.to_string(),
            p = perm_type.to_string(),
            d = today_s,
            a = sup_user.to_string(),
        ),
        vec![],
        "seed.demoperm",
    )
    .await
    .map_err(|e| format!("gagal insert izin demo: {e}"))?;

    // Lembur approved + nominal sesuai aturan basic/173 x 1.5.
    let ot_date = fmt(today - Duration::days(3));
    let ot_exists = tx_q_one(
        tx,
        "SELECT id FROM overtime_requests WHERE employee_id = ?1 AND date = ?2".to_string(),
        vec![i(dedi), t(&ot_date)],
        1,
        "seed.demootcheck",
    )
    .await
    .map_err(|e| format!("gagal cek lembur demo: {e}"))?;
    if ot_exists.is_none() {
        let basic = basic_for(lvl_staff);
        let amount = basic / 173.0 * 1.5 * 2.0;
        let ot_id = exec_insert(
            tx,
            "INSERT INTO overtime_requests (employee_id, date, start_time, end_time, duration_minutes, reason, rate_multiplier, amount, status) VALUES (?1, ?2, '18:00', '20:00', 120, 'Deploy rilis mendesak', 1.5, ?3, 'approved')".to_string(),
            vec![i(dedi), t(&ot_date), SVal::Float(amount)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demoot",
        )
        .await
        .map_err(|e| format!("gagal insert lembur demo: {e}"))?;
        if ot_id > 0 {
            tx_exec(
                tx,
                "INSERT INTO overtime_approvals (overtime_request_id, approver_id, step_order, step_role, status, acted_at) VALUES (?1, ?2, 1, 'supervisor', 'approved', ?3)".to_string(),
                vec![i(ot_id), i(sup_user), t(&today_s)],
                "seed.demootappr",
            )
            .await
            .map_err(|e| format!("gagal insert approval lembur demo: {e}"))?;
        }
    }

    // Perjalanan dinas approved + expense.
    let trip_start = fmt(today + Duration::days(10));
    let trip_end = fmt(today + Duration::days(12));
    let trip_exists = tx_q_one(
        tx,
        "SELECT id FROM business_trips WHERE employee_id = ?1 AND start_date = ?2".to_string(),
        vec![i(irfan), t(&trip_start)],
        1,
        "seed.demotripcheck",
    )
    .await
    .map_err(|e| format!("gagal cek dinas demo: {e}"))?;
    if trip_exists.is_none() {
        let trip_id = exec_insert(
            tx,
            "INSERT INTO business_trips (employee_id, destination, purpose, start_date, end_date, transportation, hotel, budget, status) VALUES (?1, 'Jakarta', 'Koordinasi proyek klien', ?2, ?3, 'Pesawat', 'Hotel Contoh', 5000000, 'approved')".to_string(),
            vec![i(irfan), t(&trip_start), t(&trip_end)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demotrip",
        )
        .await
        .map_err(|e| format!("gagal insert dinas demo: {e}"))?;
        if trip_id > 0 {
            for (cat, desc, amount) in [
                ("transport", "Tiket pesawat PP", 2_500_000.0),
                ("akomodasi", "Hotel 2 malam", 1_500_000.0),
            ] {
                tx_exec(
                    tx,
                    "INSERT INTO business_trip_expenses (business_trip_id, category, description, amount) VALUES (?1, ?2, ?3, ?4)".to_string(),
                    vec![i(trip_id), t(cat), t(desc), SVal::Float(amount)],
                    "seed.demotripexp",
                )
                .await
                .map_err(|e| format!("gagal insert expense dinas demo: {e}"))?;
            }
        }
    }

    // Reimbursement: satu paid, satu pending.
    let reimb_cat = find_id(
        tx,
        "reimbursement_categories",
        "code = ?1",
        vec![t("MEDICAL")],
    )
    .await?
    .ok_or_else(|| "kategori reimbursement MEDICAL tidak ditemukan".to_string())?;
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO reimbursements (id, employee_id, reimbursement_category_id, amount, description, status, paid_at) VALUES (9001, {e}, {c}, 750000, 'Klaim rawat jalan', 'paid', '{d}')",
            e = lina.to_string(),
            c = reimb_cat.to_string(),
            d = today_s,
        ),
        vec![],
        "seed.demoreimb",
    )
    .await
    .map_err(|e| format!("gagal insert reimbursement demo: {e}"))?;
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO reimbursements (id, employee_id, reimbursement_category_id, amount, description, status) VALUES (9002, {e}, {c}, 350000, 'Klaim obat', 'pending')",
            e = hendra.to_string(),
            c = reimb_cat.to_string(),
        ),
        vec![],
        "seed.demoreimbpend",
    )
    .await
    .map_err(|e| format!("gagal insert reimbursement pending demo: {e}"))?;

    // Rekrutmen: lowongan + kandidat tiap tahap + interview + assessment.
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO vacancies (id, title, department_id, position_id, employment_type, description, requirements, quota, status, posted_date, closing_date) VALUES (9001, 'Frontend Developer', {it}, (SELECT id FROM positions WHERE code = 'ITS'), 'contract', 'Membangun aplikasi internal', 'Pengalaman React 2 tahun', 2, 'open', '{d}', '{c}')",
            it = dept_it.to_string(),
            d = today_s,
            c = fmt(today + Duration::days(30)),
        ),
        vec![],
        "seed.demovac",
    )
    .await
    .map_err(|e| format!("gagal insert lowongan demo: {e}"))?;
    let cand_stages = [
        (9001, "Andi Pratama", "applied", 0.0),
        (9002, "Bella Kusuma", "screening", 0.0),
        (9003, "Candra Wijaya", "interview", 75.0),
        (9004, "Dinda Puspita", "offering", 85.0),
        (9005, "Eko Saputra", "hired", 90.0),
        (9006, "Farah Azizah", "rejected", 50.0),
    ];
    for (cid, name, stage, rating) in cand_stages {
        tx_exec(
            tx,
            format!(
                "INSERT OR IGNORE INTO candidates (id, vacancy_id, full_name, email, phone, stage, rating, source) VALUES ({cid}, 9001, '{name}', '{mail}', '0812000000{cid}', '{stage}', {rating}, 'Job portal')",
                cid = cid.to_string(),
                name = name,
                mail = format!("kandidat{cid}@example.com"),
                stage = stage,
                rating = rating.to_string(),
            ),
            vec![],
            "seed.democand",
        )
        .await
        .map_err(|e| format!("gagal insert kandidat demo: {e}"))?;
        tx_exec(
            tx,
            format!(
                "INSERT INTO recruitment_stages (candidate_id, stage, notes, changed_at) SELECT {cid}, '{stage}', 'Tahap awal', '{d}' WHERE NOT EXISTS (SELECT 1 FROM recruitment_stages WHERE candidate_id = {cid} AND stage = '{stage}')",
                cid = cid.to_string(),
                stage = stage,
                d = today_s,
            ),
            vec![],
            "seed.democandstage",
        )
        .await
        .map_err(|e| format!("gagal insert tahap kandidat demo: {e}"))?;
    }
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO interviews (id, candidate_id, interviewer_id, schedule_at, location, type, result, notes) VALUES (9001, 9003, {e}, '{d} 10:00:00', 'Ruang Rapat 1', 'user', 'pass', 'Komunikasi baik')",
            e = eid("EMP-0006")?.to_string(),
            d = fmt(today + Duration::days(2)),
        ),
        vec![],
        "seed.demointerview",
    )
    .await
    .map_err(|e| format!("gagal insert interview demo: {e}"))?;
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO candidate_assessments (id, candidate_id, assessment_name, score, notes, assessed_by) VALUES (9001, 9003, 'Tes logika', 80, 'Di atas ambang', {u})",
            u = mgr_user.to_string(),
        ),
        vec![],
        "seed.demoassess",
    )
    .await
    .map_err(|e| format!("gagal insert assessment demo: {e}"))?;

    // Onboarding karyawan probation + 4 tasks (2 selesai).
    let onb_exists = tx_q_one(
        tx,
        "SELECT id FROM onboarding WHERE employee_id = ?1".to_string(),
        vec![i(hendra)],
        1,
        "seed.demoonbcheck",
    )
    .await
    .map_err(|e| format!("gagal cek onboarding demo: {e}"))?;
    if onb_exists.is_none() {
        let onb_id = exec_insert(
            tx,
            "INSERT INTO onboarding (employee_id, start_date, status, progress_percent) VALUES (?1, ?2, 'in_progress', 50)".to_string(),
            vec![i(hendra), t(&join_date)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demoonb",
        )
        .await
        .map_err(|e| format!("gagal insert onboarding demo: {e}"))?;
        if onb_id > 0 {
            for (idx, (task, done)) in [
                ("Serah terima perangkat", true),
                ("Orientasi departemen", true),
                ("Pelatihan sistem HRIS", false),
                ("Penetapan target awal", false),
            ]
            .iter()
            .enumerate()
            {
                let completed = if *done { 1 } else { 0 };
                let at = if *done {
                    format!("'{today_s}'")
                } else {
                    "NULL".to_string()
                };
                tx_exec(
                    tx,
                    format!(
                        "INSERT INTO onboarding_tasks (onboarding_id, task_name, is_completed, completed_at, sort_order) VALUES ({onb}, '{task}', {done}, {at}, {idx})",
                        onb = onb_id.to_string(),
                        task = task,
                        done = completed.to_string(),
                        at = at,
                        idx = ((idx + 1) as i64).to_string(),
                    ),
                    vec![],
                    "seed.demoonbtask",
                )
                .await
                .map_err(|e| format!("gagal insert task onboarding demo: {e}"))?;
            }
        }
    }

    // Kontrak aktif + dokumen untuk karyawan kontrak.
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO employee_documents (id, employee_id, category, name, file_path, mime_type) VALUES (9001, {e}, 'contract', 'Kontrak Joko', '/dokumen/kontrak-joko.pdf', 'application/pdf')",
            e = joko.to_string(),
        ),
        vec![],
        "seed.demodoc",
    )
    .await
    .map_err(|e| format!("gagal insert dokumen demo: {e}"))?;
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO employee_contracts (id, employee_id, contract_number, type, start_date, end_date, status, document_id) VALUES (9001, {e}, 'PKWT/2026/001', 'pkwt', '{s}', '{x}', 'active', 9001)",
            e = joko.to_string(),
            s = fmt(today - Duration::days(60)),
            x = fmt(today + Duration::days(300)),
        ),
        vec![],
        "seed.democontract",
    )
    .await
    .map_err(|e| format!("gagal insert kontrak demo: {e}"))?;

    // Training selesai + peserta + kehadiran + sertifikat + skill.
    let tr_start = fmt(today - Duration::days(20));
    let tr_end = fmt(today - Duration::days(18));
    let tr_exists = tx_q_one(
        tx,
        "SELECT id FROM trainings WHERE title = ?1".to_string(),
        vec![t("Pelatihan Keselamatan Kerja")],
        1,
        "seed.demotrcheck",
    )
    .await
    .map_err(|e| format!("gagal cek training demo: {e}"))?;
    if tr_exists.is_none() {
        let tr_id = exec_insert(
            tx,
            "INSERT INTO trainings (title, description, trainer_name, start_date, end_date, location, cost, quota, status) VALUES ('Pelatihan Keselamatan Kerja', 'Dasar K3 perkantoran', 'Budi Santoso', ?1, ?2, 'Ruang Training', 5000000, 20, 'completed')".to_string(),
            vec![t(&tr_start), t(&tr_end)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demotr",
        )
        .await
        .map_err(|e| format!("gagal insert training demo: {e}"))?;
        if tr_id > 0 {
            for emp_no in ["EMP-0007", "EMP-0008", "EMP-0013"] {
                let e = eid(emp_no)?;
                let part_id = exec_insert(
                    tx,
                    "INSERT OR IGNORE INTO training_participants (training_id, employee_id, status) VALUES (?1, ?2, 'completed')".to_string(),
                    vec![i(tr_id), i(e)].into_iter().map(ke_db).collect(),
                    "seed.demotpart",
                )
                .await
                .map_err(|e| format!("gagal insert peserta training demo: {e}"))?;
                let pid = if part_id > 0 {
                    part_id
                } else {
                    find_id(
                        tx,
                        "training_participants",
                        "training_id = ?1 AND employee_id = ?2",
                        vec![i(tr_id), i(e)],
                    )
                    .await?
                    .unwrap_or(0)
                };
                if pid > 0 {
                    tx_exec(
                        tx,
                        "INSERT OR IGNORE INTO training_attendance (training_participant_id, date, attended) VALUES (?1, ?2, 1)".to_string(),
                        vec![i(pid), t(&tr_start)],
                        "seed.demotatt",
                    )
                    .await
                    .map_err(|e| format!("gagal insert kehadiran training demo: {e}"))?;
                }
            }
            tx_exec(
                tx,
                format!(
                    "INSERT OR IGNORE INTO certifications (employee_id, training_id, name, issuer, certificate_number, issued_date) VALUES ({e}, {tr}, 'Sertifikat K3 Dasar', 'Lembaga K3', 'K3-2026-001', '{d}')",
                    e = maya.to_string(),
                    tr = tr_id.to_string(),
                    d = tr_end,
                ),
                vec![],
                "seed.democert",
            )
            .await
            .map_err(|e| format!("gagal insert sertifikat demo: {e}"))?;
        }
    }
    for (emp_no, skill, level) in [
        ("EMP-0007", "React", 4),
        ("EMP-0008", "Go", 3),
        ("EMP-0011", "Administrasi Jaringan", 5),
    ] {
        tx_exec(
            tx,
            format!(
                "INSERT OR IGNORE INTO employee_skills (employee_id, skill_name, level, assessed_at) VALUES ({e}, '{s}', {l}, '{d}')",
                e = eid(emp_no)?.to_string(),
                s = skill,
                l = level.to_string(),
                d = today_s,
            ),
            vec![],
            "seed.demoskill",
        )
        .await
        .map_err(|e| format!("gagal insert skill demo: {e}"))?;
    }

    // Aset: 3 unit + assignment + maintenance.
    for (code, name, cat, price) in [
        ("AST-LT-001", "Laptop Kerja 1", "LAPTOP", 12_000_000.0),
        ("AST-LT-002", "Laptop Kerja 2", "LAPTOP", 12_000_000.0),
        ("AST-HP-001", "Handphone Operasional", "MOBILE", 4_000_000.0),
    ] {
        let cat_id = find_id(tx, "asset_categories", "code = ?1", vec![t(cat)])
            .await?
            .ok_or_else(|| format!("kategori aset {cat} tidak ditemukan"))?;
        tx_exec(
            tx,
            format!(
                "INSERT OR IGNORE INTO assets (asset_code, name, asset_category_id, purchase_date, purchase_price, condition_status, status) VALUES ('{code}', '{name}', {c}, '{d}', {p}, 'good', 'assigned')",
                code = code,
                name = name,
                c = cat_id.to_string(),
                d = fmt(today - Duration::days(200)),
                p = price.to_string(),
            ),
            vec![],
            "seed.demoasset",
        )
        .await
        .map_err(|e| format!("gagal insert aset demo: {e}"))?;
    }
    for (code, emp_no) in [("AST-LT-001", "EMP-0007"), ("AST-LT-002", "EMP-0008"), ("AST-HP-001", "EMP-0014")] {
        let aid = asset_id_of(tx, code).await?;
        let e = eid(emp_no)?;
        tx_exec(
            tx,
            format!(
                "INSERT INTO asset_assignments (asset_id, employee_id, assigned_date, condition_on_assign) SELECT {a}, {e}, '{d}', 'good' WHERE NOT EXISTS (SELECT 1 FROM asset_assignments WHERE asset_id = {a} AND employee_id = {e} AND returned_date IS NULL)",
                a = aid.to_string(),
                e = e.to_string(),
                d = fmt(today - Duration::days(180)),
            ),
            vec![],
            "seed.demoassign",
        )
        .await
        .map_err(|e| format!("gagal insert assignment aset demo: {e}"))?;
    }
    tx_exec(
        tx,
        format!(
            "INSERT INTO asset_maintenance (asset_id, maintenance_date, description, cost, performed_by) SELECT {a}, '{d}', 'Ganti pasta thermal', 250000, 'IT Support' WHERE NOT EXISTS (SELECT 1 FROM asset_maintenance WHERE asset_id = {a})",
            a = asset_id_of(tx, "AST-LT-001").await?.to_string(),
            d = fmt(today - Duration::days(30)),
        ),
        vec![],
        "seed.demomaint",
    )
    .await
    .map_err(|e| format!("gagal insert maintenance demo: {e}"))?;

    // Kinerja: periode berjalan + KPI + review selesai untuk 3 karyawan.
    let per_start = first_of_month.clone();
    let per_end = fmt(today + Duration::days(30));
    let per_exists = tx_q_one(
        tx,
        "SELECT id FROM performance_periods WHERE name = ?1".to_string(),
        vec![t("Penilaian Bulan Berjalan")],
        1,
        "seed.demopercheck",
    )
    .await
    .map_err(|e| format!("gagal cek periode kinerja demo: {e}"))?;
    if per_exists.is_none() {
        let per_id = exec_insert(
            tx,
            "INSERT INTO performance_periods (name, type, start_date, end_date, status) VALUES ('Penilaian Bulan Berjalan', 'monthly', ?1, ?2, 'open')".to_string(),
            vec![t(&per_start), t(&per_end)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demoper",
        )
        .await
        .map_err(|e| format!("gagal insert periode kinerja demo: {e}"))?;
        if per_id > 0 {
            let kpi_names = ["Ketepatan Waktu", "Kualitas Kerja", "Kerja Sama Tim"];
            let mut kpi_ids = Vec::new();
            for name in kpi_names {
                let kid = exec_insert(
                    tx,
                    "INSERT INTO kpis (name, department_id) VALUES (?1, ?2)".to_string(),
                    vec![t(name), i(dept_it)].into_iter().map(ke_db).collect(),
                    "seed.demokpi",
                )
                .await
                .map_err(|e| format!("gagal insert KPI demo: {e}"))?;
                kpi_ids.push(kid);
            }
            for emp_no in ["EMP-0007", "EMP-0008", "EMP-0014"] {
                let e = eid(emp_no)?;
                for (ki, kid) in kpi_ids.iter().enumerate() {
                    tx_exec(
                        tx,
                        "INSERT INTO employee_kpis (performance_period_id, employee_id, kpi_id, target, weight, actual, score) VALUES (?1, ?2, ?3, 100, 33.33, ?4, ?5)".to_string(),
                        vec![
                            i(per_id),
                            i(e),
                            i(*kid),
                            SVal::Float(85.0 + ki as f64),
                            SVal::Float(85.0 + ki as f64),
                        ],
                        "seed.demoekpi",
                    )
                    .await
                    .map_err(|e| format!("gagal insert KPI karyawan demo: {e}"))?;
                }
                let rev_id = exec_insert(
                    tx,
                    "INSERT INTO performance_reviews (performance_period_id, employee_id, self_score, supervisor_score, manager_score, final_score, final_rating, status) VALUES (?1, ?2, 80, 85, 85, 84, 4, 'completed')".to_string(),
                    vec![i(per_id), i(e)].into_iter().map(ke_db).collect(),
                    "seed.demorev",
                )
                .await
                .map_err(|e| format!("gagal insert review demo: {e}"))?;
                if rev_id > 0 {
                    for (role, score) in [("self", 80), ("supervisor", 85), ("manager", 85)] {
                        tx_exec(
                            tx,
                            "INSERT INTO performance_details (performance_review_id, reviewer_role, comments, rating) VALUES (?1, ?2, 'Kinerja baik', ?3)".to_string(),
                            vec![i(rev_id), t(role), i(score)],
                            "seed.demorevdet",
                        )
                        .await
                        .map_err(|e| format!("gagal insert detail review demo: {e}"))?;
                    }
                }
            }
        }
    }

    // Payroll: periode draft + slip untuk 5 karyawan + potongan kasbon.
    let pp_exists = tx_q_one(
        tx,
        "SELECT id FROM payroll_periods WHERE name = ?1".to_string(),
        vec![t("Payroll Bulan Berjalan")],
        1,
        "seed.demoppcheck",
    )
    .await
    .map_err(|e| format!("gagal cek periode payroll demo: {e}"))?;
    if pp_exists.is_none() {
        let pp_id = exec_insert(
            tx,
            "INSERT INTO payroll_periods (name, start_date, end_date, payment_date, status, created_by) VALUES ('Payroll Bulan Berjalan', ?1, ?2, ?3, 'draft', ?4)".to_string(),
            vec![
                t(&first_of_month),
                t(&today_s),
                t(&pay_date),
                i(admin_user),
            ]
            .into_iter()
            .map(ke_db)
            .collect(),
            "seed.demopp",
        )
        .await
        .map_err(|e| format!("gagal insert periode payroll demo: {e}"))?;
        if pp_id > 0 {
            for emp_no in ["EMP-0007", "EMP-0008", "EMP-0009", "EMP-0013", "EMP-0014"] {
                let e = eid(emp_no)?;
                let sal = tx_q_one(
                    tx,
                    "SELECT basic_salary FROM employee_salaries WHERE employee_id = ?1 AND is_active = 1".to_string(),
                    vec![i(e)],
                    1,
                    "seed.demoppsal",
                )
                .await
                .map_err(|e| format!("gagal baca gaji payroll demo: {e}"))?;
                let basic = sal
                    .as_ref()
                    .and_then(|r| match &r[0] {
                        SVal::Float(f) => Some(*f),
                        SVal::Int(v) => Some(*v as f64),
                        SVal::Text(s) => s.parse().ok(),
                        SVal::Null => None,
                    })
                    .unwrap_or(6_000_000.0);
                let tunj = basic * 0.2 + 1_750_000.0;
                let kotor = basic + tunj;
                let bersih = kotor - kotor * 0.03;
                let pr_id = exec_insert(
                    tx,
                    "INSERT INTO payrolls (payroll_period_id, employee_id, basic_salary, total_income, gross_salary, total_deduction, net_salary, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'draft')".to_string(),
                    vec![
                        i(pp_id),
                        i(e),
                        SVal::Float(basic),
                        SVal::Float(tunj),
                        SVal::Float(kotor),
                        SVal::Float(kotor * 0.03),
                        SVal::Float(bersih),
                    ]
                    .into_iter()
                    .map(ke_db)
                    .collect(),
                    "seed.demopr",
                )
                .await
                .map_err(|e| format!("gagal insert payroll demo: {e}"))?;
                if pr_id > 0 {
                    for (cname, ctype, amount) in [
                        ("Gaji Pokok", "income", basic),
                        ("Tunjangan", "income", tunj),
                        ("BPJS", "deduction", kotor * 0.03),
                    ] {
                        tx_exec(
                            tx,
                            "INSERT INTO payroll_details (payroll_id, component_name, type, amount) VALUES (?1, ?2, ?3, ?4)".to_string(),
                            vec![i(pr_id), t(cname), t(ctype), SVal::Float(amount)],
                            "seed.demoprdet",
                        )
                        .await
                        .map_err(|e| format!("gagal insert detail payroll demo: {e}"))?;
                    }
                    tx_exec(
                        tx,
                        format!(
                            "INSERT OR IGNORE INTO payslips (id, payroll_id, payslip_number, generated_at) VALUES ({id}, {pr}, 'SLIP/{y}/{id}', '{d}')",
                            id = (9000 + pr_id).to_string(),
                            pr = pr_id.to_string(),
                            y = year_now.to_string(),
                            d = today_s,
                        ),
                        vec![],
                        "seed.demoslip",
                    )
                    .await
                    .map_err(|e| format!("gagal insert slip demo: {e}"))?;
                }
            }
            tx_exec(
                tx,
                format!(
                    "INSERT INTO payroll_deductions (employee_id, payroll_period_id, type, description, amount, status) SELECT {e}, {pp}, 'kasbon', 'Kasbon operasional', 500000, 'pending' WHERE NOT EXISTS (SELECT 1 FROM payroll_deductions WHERE employee_id = {e} AND payroll_period_id = {pp})",
                    e = dedi.to_string(),
                    pp = pp_id.to_string(),
                ),
                vec![],
                "seed.demodeduct",
            )
            .await
            .map_err(|e| format!("gagal insert potongan payroll demo: {e}"))?;
        }
    }

    // Pengumuman published + target departemen + 2 baca.
    let ann_exists = tx_q_one(
        tx,
        "SELECT id FROM announcements WHERE title = ?1".to_string(),
        vec![t("Jadwal Libur Bersama")],
        1,
        "seed.demoanncheck",
    )
    .await
    .map_err(|e| format!("gagal cek pengumuman demo: {e}"))?;
    if ann_exists.is_none() {
        let ann_id = exec_insert(
            tx,
            "INSERT INTO announcements (title, content, target_type, publish_at, status, created_by) VALUES ('Jadwal Libur Bersama', 'Perhatikan jadwal libur bulan ini', 'all', ?1, 'published', ?2)".to_string(),
            vec![t(&today_s), i(admin_user)]
                .into_iter()
                .map(ke_db)
                .collect(),
            "seed.demoann",
        )
        .await
        .map_err(|e| format!("gagal insert pengumuman demo: {e}"))?;
        if ann_id > 0 {
            tx_exec(
                tx,
                "INSERT INTO announcement_targets (announcement_id, target_type, target_id) VALUES (?1, 'department', ?2)".to_string(),
                vec![i(ann_id), i(dept_it)],
                "seed.demoanntgt",
            )
            .await
            .map_err(|e| format!("gagal insert target pengumuman demo: {e}"))?;
            for emp_no in ["EMP-0007", "EMP-0008"] {
                tx_exec(
                    tx,
                    "INSERT OR IGNORE INTO announcement_reads (announcement_id, employee_id, read_at) VALUES (?1, ?2, ?3)".to_string(),
                    vec![i(ann_id), i(eid(emp_no)?), t(&today_s)],
                    "seed.demoannread",
                )
                .await
                .map_err(|e| format!("gagal insert baca pengumuman demo: {e}"))?;
            }
        }
    }

    // Notifikasi untuk 3 pengguna.
    for (uname, title) in [
        ("maya", "Cuti disetujui"),
        ("dedi", "Lembur disetujui"),
        ("hr.admin", "1 pengajuan cuti menunggu"),
    ] {
        tx_exec(
            tx,
            format!(
                "INSERT INTO notifications (user_id, type, title, message) SELECT {u}, 'info', '{t}', 'Pemberitahuan otomatis data demo' WHERE NOT EXISTS (SELECT 1 FROM notifications WHERE user_id = {u} AND title = '{t}')",
                u = uid(uname)?.to_string(),
                t = title,
            ),
            vec![],
            "seed.demonotif",
        )
        .await
        .map_err(|e| format!("gagal insert notifikasi demo: {e}"))?;
    }

    // Approval request + history untuk cuti pending Joko.
    let wf_leave = find_id(tx, "approval_workflows", "module = ?1", vec![t("leave")])
        .await?
        .ok_or_else(|| "workflow leave tidak ditemukan".to_string())?;
    tx_exec(
        tx,
        format!(
            "INSERT OR IGNORE INTO approval_requests (id, approval_workflow_id, reference_type, reference_id, requested_by, status) VALUES (9001, {w}, 'leave', 9001, {u}, 'pending')",
            w = wf_leave.to_string(),
            u = uid("joko")?.to_string(),
        ),
        vec![],
        "seed.demoapprreq",
    )
    .await
    .map_err(|e| format!("gagal insert approval request demo: {e}"))?;
    tx_exec(
        tx,
        "INSERT INTO approval_histories (approval_request_id, step_order, action, notes) SELECT 9001, 1, 'approved', 'Setuju supervisor' WHERE NOT EXISTS (SELECT 1 FROM approval_histories WHERE approval_request_id = 9001)".to_string(),
        vec![],
        "seed.demoapprhist",
    )
    .await
    .map_err(|e| format!("gagal insert approval history demo: {e}"))?;

    // Audit log contoh.
    tx_exec(
        tx,
        format!(
            "INSERT INTO audit_logs (user_id, action, module, record_id, description) SELECT {u}, 'create', 'employee', 'EMP-0007', 'Membuat data demo' WHERE NOT EXISTS (SELECT 1 FROM audit_logs WHERE module = 'employee' AND record_id = 'EMP-0007')",
            u = admin_user.to_string(),
        ),
        vec![],
        "seed.demoaudit",
    )
    .await
    .map_err(|e| format!("gagal insert audit demo: {e}"))?;

    // Offboarding resign Fitri + exit interview + clearance.
    let off_exists = tx_q_one(
        tx,
        "SELECT id FROM offboarding WHERE employee_id = ?1".to_string(),
        vec![i(fitri)],
        1,
        "seed.demooffcheck",
    )
    .await
    .map_err(|e| format!("gagal cek offboarding demo: {e}"))?;
    if off_exists.is_none() {
        let off_id = exec_insert(
            tx,
            "INSERT INTO offboarding (employee_id, resignation_date, last_working_date, reason, status) VALUES (?1, ?2, ?3, 'Pindah ke perusahaan lain', 'supervisor_approved')".to_string(),
            vec![
                i(fitri),
                t(&fmt(today - Duration::days(5))),
                t(&fmt(today + Duration::days(25))),
            ]
            .into_iter()
            .map(ke_db)
            .collect(),
            "seed.demooff",
        )
        .await
        .map_err(|e| format!("gagal insert offboarding demo: {e}"))?;
        if off_id > 0 {
            tx_exec(
                tx,
                "INSERT INTO exit_interviews (offboarding_id, conducted_by, feedback, reason_category, would_recommend) VALUES (?1, ?2, 'Lingkungan kerja baik', 'karier', 1)".to_string(),
                vec![i(off_id), i(mgr_user)],
                "seed.demoexit",
            )
            .await
            .map_err(|e| format!("gagal insert exit interview demo: {e}"))?;
            for (item, dept_name, cleared) in [
                ("Pengembalian laptop", "IT", 1),
                ("Penyelesaian kasbon", "Finance", 0),
            ] {
                tx_exec(
                    tx,
                    "INSERT INTO clearance_items (offboarding_id, item_name, department, is_cleared) VALUES (?1, ?2, ?3, ?4)".to_string(),
                    vec![i(off_id), t(item), t(dept_name), i(cleared)],
                    "seed.democlear",
                )
                .await
                .map_err(|e| format!("gagal insert clearance demo: {e}"))?;
            }
        }
    }

    // Koreksi absensi pending untuk Maya.
    tx_exec(
        tx,
        format!(
            "INSERT INTO attendance_corrections (employee_id, date, requested_clock_in, reason, status) SELECT {e}, '{d}', '{d} 08:00:00', 'Lupa clock-in', 'pending' WHERE NOT EXISTS (SELECT 1 FROM attendance_corrections WHERE employee_id = {e} AND date = '{d}')",
            e = maya.to_string(),
            d = fmt(today - Duration::days(2)),
        ),
        vec![],
        "seed.democorr",
    )
    .await
    .map_err(|e| format!("gagal insert koreksi absensi demo: {e}"))?;

    // Shift assignment untuk tiap karyawan demo (jadwal Senin-Jumat).
    let sched = find_id(
        tx,
        "work_schedules",
        "name = ?1",
        vec![t("Senin - Jumat")],
    )
    .await?
    .ok_or_else(|| "jadwal Senin - Jumat tidak ditemukan".to_string())?;
    for d in &demos {
        seed_shift_assignment(tx, emp_ids[d.no], sched).await?;
    }
    Ok(())
}

/// Cari id user demo, fallback ke admin bila belum ada (mis. urutan seed).
async fn uid_or_admin(
    tx: &Tx,
    users: &std::collections::HashMap<&str, i64>,
    name: &str,
) -> Result<i64, String> {
    if let Some(id) = users.get(name) {
        return Ok(*id);
    }
    find_id(tx, "users", "username = ?1", vec![t("admin")])
        .await?
        .ok_or_else(|| "user admin tidak ditemukan".to_string())
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
            "employees",
            "role_permissions",
            "leave_types",
            "salary_components",
            "approval_workflows",
            "approval_steps",
            "system_settings",
            "employee_salaries",
            "leave_balances",
            "attendances",
            "leave_requests",
            "leave_approvals",
            "permission_requests",
            "overtime_requests",
            "overtime_approvals",
            "business_trips",
            "business_trip_expenses",
            "reimbursements",
            "vacancies",
            "candidates",
            "interviews",
            "onboarding",
            "onboarding_tasks",
            "employee_contracts",
            "trainings",
            "training_participants",
            "certifications",
            "employee_skills",
            "assets",
            "asset_assignments",
            "asset_maintenance",
            "performance_periods",
            "performance_reviews",
            "payroll_periods",
            "payrolls",
            "payslips",
            "announcements",
            "announcement_reads",
            "notifications",
            "approval_requests",
            "approval_histories",
            "audit_logs",
            "offboarding",
            "exit_interviews",
            "clearance_items",
            "attendance_corrections",
            "shift_assignments",
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
        assert_eq!(c["roles"], 9);
        assert_eq!(c["permissions"], 92);
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
        assert_eq!(super_perms, 92);
    }

    #[tokio::test]
    async fn seed_demo_mengisi_semua_modul() {
        let (_dir, db) = seeded_db().await;
        let c = counts(&db).await;
        // 20 karyawan demo + 1 admin.
        assert_eq!(c["employees"], 21);
        // 20 user demo + 1 admin.
        assert_eq!(c["users"], 21);
        // Tiap peran terisi minimal satu pengguna.
        for role in [
            "super-administrator",
            "hr-administrator",
            "hr-manager",
            "finance",
            "manager",
            "supervisor",
            "employee",
            "it-administrator",
            "auditor",
        ] {
            let rows = crate::services::sea_raw::q_all(
                &db,
                format!(
                    "SELECT COUNT(*) FROM user_roles ur JOIN roles r ON r.id = ur.role_id WHERE r.slug = '{role}'"
                ),
                vec![],
                1,
                "seed.testrole",
            )
            .await
            .unwrap();
            let n = rows
                .first()
                .and_then(|r| crate::services::sea_raw::value_i64(&r[0]))
                .unwrap_or(0);
            assert!(n >= 1, "role {role} tanpa pengguna demo");
        }
        // Transaksional tiap modul ada isinya.
        for (tbl, min) in [
            ("employee_salaries", 20),
            ("leave_balances", 30),
            ("attendances", 50),
            ("leave_requests", 2),
            ("leave_approvals", 1),
            ("permission_requests", 1),
            ("overtime_requests", 1),
            ("overtime_approvals", 1),
            ("business_trips", 1),
            ("business_trip_expenses", 2),
            ("reimbursements", 2),
            ("vacancies", 1),
            ("candidates", 6),
            ("interviews", 1),
            ("onboarding", 1),
            ("onboarding_tasks", 4),
            ("employee_contracts", 1),
            ("trainings", 1),
            ("training_participants", 3),
            ("certifications", 1),
            ("employee_skills", 3),
            ("assets", 3),
            ("asset_assignments", 3),
            ("asset_maintenance", 1),
            ("performance_periods", 1),
            ("performance_reviews", 3),
            ("payroll_periods", 1),
            ("payrolls", 5),
            ("payslips", 5),
            ("announcements", 1),
            ("announcement_reads", 2),
            ("notifications", 3),
            ("approval_requests", 1),
            ("approval_histories", 1),
            ("audit_logs", 1),
            ("offboarding", 1),
            ("exit_interviews", 1),
            ("clearance_items", 2),
            ("attendance_corrections", 1),
        ] {
            assert!(
                c[tbl] >= min,
                "tabel {tbl} hanya {} baris, minimal {min}",
                c[tbl]
            );
        }
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
