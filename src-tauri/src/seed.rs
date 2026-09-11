//! Data awal: company, roles, permissions, admin, dan master data.
//!
//! Idempoten: aman dijalankan berulang, tidak membuat duplikat.

use chrono::Local;
use rusqlite::{params, Connection};

/// Ringkasan hasil seeding untuk logging.
#[derive(Debug, Default)]
pub struct SeedSummary {
    pub roles: i64,
    pub permissions: i64,
    pub users: i64,
}

/// Jalankan seluruh seed dalam satu transaksi.
pub fn seed(conn: &mut Connection) -> Result<SeedSummary, String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("gagal memulai transaksi seed: {e}"))?;
    let company_id = seed_company(&tx)?;
    let (dept_hr, dept_it, dept_fin) = seed_departments(&tx, company_id)?;
    let (lvl_staff, _lvl_spv, lvl_mgr) = seed_job_levels(&tx)?;
    seed_job_grades(&tx)?;
    let pos_hr_manager = seed_positions(&tx, dept_hr, dept_it, dept_fin, lvl_staff, lvl_mgr)?;
    let hq_location = seed_work_location(&tx)?;
    seed_cost_center(&tx, dept_hr)?;
    let role_ids = seed_roles(&tx)?;
    let perm_ids = seed_permissions(&tx)?;
    seed_role_permissions(&tx, &role_ids, &perm_ids)?;
    let admin_employee = seed_admin_employee(
        &tx,
        company_id,
        dept_hr,
        pos_hr_manager,
        lvl_mgr,
        hq_location,
    )?;
    seed_admin_user(&tx, &role_ids, admin_employee)?;
    seed_leave_types(&tx)?;
    seed_permission_types(&tx)?;
    seed_salary_components(&tx)?;
    let schedule_regular = seed_shifts_and_schedule(&tx)?;
    seed_shift_assignment(&tx, admin_employee, schedule_regular)?;
    seed_holidays(&tx)?;
    seed_approval_workflows(&tx, &role_ids)?;
    seed_categories(&tx)?;
    seed_settings(&tx)?;
    tx.commit()
        .map_err(|e| format!("gagal commit transaksi seed: {e}"))?;

    Ok(SeedSummary {
        roles: count(conn, "roles")?,
        permissions: count(conn, "permissions")?,
        users: count(conn, "users")?,
    })
}

fn count(conn: &Connection, table: &str) -> Result<i64, String> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .map_err(|e| format!("gagal menghitung {table}: {e}"))
}

/// Insert bila belum ada (berdasar constraint UNIQUE), kembalikan id.
fn insert_ignore(
    conn: &Connection,
    table: &str,
    columns: &str,
    placeholders: &str,
    p: &[&dyn rusqlite::ToSql],
) -> Result<i64, String> {
    conn.execute(
        &format!("INSERT OR IGNORE INTO {table} ({columns}) VALUES ({placeholders})"),
        p,
    )
    .map_err(|e| format!("gagal insert {table}: {e}"))?;
    Ok(conn.last_insert_rowid())
}

fn find_id(
    conn: &Connection,
    table: &str,
    where_clause: &str,
    p: &[&dyn rusqlite::ToSql],
) -> Result<Option<i64>, String> {
    conn.query_row(
        &format!("SELECT id FROM {table} WHERE {where_clause}"),
        p,
        |r| r.get(0),
    )
    .map(|v| Some(v))
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(format!("gagal mencari {table}: {other}")),
    })
}

fn seed_company(conn: &Connection) -> Result<i64, String> {
    insert_ignore(
        conn,
        "companies",
        "code, name, legal_name, address, city, province, postal_code, phone, email, npwp, established_date",
        "?,?,?,?,?,?,?,?,?,?,?",
        &[
            &"HQ",
            &"PT Contoh Sukses Indonesia",
            &"PT Contoh Sukses Indonesia",
            &"Jl. Pemuda No. 1, Surabaya",
            &"Surabaya",
            &"Jawa Timur",
            &"60271",
            &"+62-31-5551234",
            &"info@contohsukses.co.id",
            &"01.234.567.8-901.000",
            &"2010-01-01",
        ],
    )?;
    find_id(conn, "companies", "code = ?1", &[&"HQ"])?
        .ok_or_else(|| "company HQ tidak ditemukan setelah seed".to_string())
}

fn seed_departments(conn: &Connection, company_id: i64) -> Result<(i64, i64, i64), String> {
    let branch_id = insert_ignore(
        conn,
        "branches",
        "company_id, code, name, address, city, phone, is_head_office",
        "?,?,?,?,?,?,1",
        &[
            &company_id,
            &"HO",
            &"Kantor Pusat Surabaya",
            &"Jl. Pemuda No. 1, Surabaya",
            &"Surabaya",
            &"+62-31-5551234",
        ],
    )?;
    let branch_id = find_id(
        conn,
        "branches",
        "company_id = ?1 AND code = 'HO'",
        &[&company_id],
    )?
    .or(Some(branch_id))
    .unwrap_or(branch_id);
    let mut ids = Vec::new();
    for (code, name) in [
        ("HRD", "Human Resources"),
        ("IT", "Information Technology"),
        ("FIN", "Finance & Accounting"),
    ] {
        insert_ignore(
            conn,
            "departments",
            "company_id, branch_id, code, name",
            "?,?,?,?",
            &[&company_id, &branch_id, &code, &name],
        )?;
        let id = find_id(
            conn,
            "departments",
            "company_id = ?1 AND code = ?2",
            &[&company_id, &code],
        )?
        .ok_or_else(|| format!("department {code} tidak ditemukan setelah seed"))?;
        ids.push(id);
    }
    Ok((ids[0], ids[1], ids[2]))
}

fn seed_job_levels(conn: &Connection) -> Result<(i64, i64, i64), String> {
    let mut ids = Vec::new();
    for (code, name, order) in [
        ("STAFF", "Staff", 1),
        ("SPV", "Supervisor", 2),
        ("MGR", "Manager", 3),
        ("DIR", "Director", 4),
    ] {
        insert_ignore(
            conn,
            "job_levels",
            "code, name, level_order",
            "?,?,?",
            &[&code, &name, &order],
        )?;
        ids.push(
            find_id(conn, "job_levels", "code = ?1", &[&code])?
                .ok_or_else(|| format!("job level {code} tidak ditemukan setelah seed"))?,
        );
    }
    Ok((ids[0], ids[1], ids[2]))
}

fn seed_job_grades(conn: &Connection) -> Result<(), String> {
    for order in 1..=5 {
        let code = format!("G{order}");
        let name = format!("Grade {order}");
        insert_ignore(
            conn,
            "job_grades",
            "code, name, grade_order",
            "?,?,?",
            &[&code, &name, &order],
        )?;
    }
    Ok(())
}

fn seed_positions(
    conn: &Connection,
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
            conn,
            "positions",
            "code, name, department_id, job_level_id",
            "?,?,?,?",
            &[&code, &name, &dept, &lvl],
        )?;
        let id = find_id(conn, "positions", "code = ?1", &[&code])?
            .ok_or_else(|| format!("position {code} tidak ditemukan setelah seed"))?;
        if code == "HRM" {
            hr_manager = id;
        }
    }
    Ok(hr_manager)
}

fn seed_work_location(conn: &Connection) -> Result<i64, String> {
    insert_ignore(
        conn,
        "work_locations",
        "name, address, latitude, longitude, radius_meter",
        "?,?,?,?,?",
        &[
            &"Kantor Pusat Surabaya",
            &"Jl. Pemuda No. 1, Surabaya",
            &-6.224_f64,
            &106.809_f64,
            &200_i64,
        ],
    )?;
    find_id(
        conn,
        "work_locations",
        "name = ?1",
        &[&"Kantor Pusat Surabaya"],
    )?
    .ok_or_else(|| "work location tidak ditemukan setelah seed".to_string())
}

fn seed_cost_center(conn: &Connection, dept_hr: i64) -> Result<(), String> {
    insert_ignore(
        conn,
        "cost_centers",
        "code, name, department_id",
        "?,?,?",
        &[&"CC-HRD", &"Cost Center HRD", &dept_hr],
    )?;
    Ok(())
}

fn seed_roles(conn: &Connection) -> Result<std::collections::HashMap<String, i64>, String> {
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
            conn,
            "roles",
            "slug, name, description, is_system",
            "?,?,?,?",
            &[&slug, &name, &desc, &is_system],
        )?;
        let id = find_id(conn, "roles", "slug = ?1", &[&slug])?
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

fn seed_permissions(conn: &Connection) -> Result<std::collections::HashMap<String, i64>, String> {
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
        ("system", vec!["manage"]),
        ("audit", vec!["view"]),
    ];
    let mut map = std::collections::HashMap::new();
    for (module, actions) in modules {
        for action in actions {
            let slug = format!("{module}.{action}");
            let display = format!("{} {}", title_case(action), title_case(module));
            insert_ignore(
                conn,
                "permissions",
                "slug, name, module",
                "?,?,?",
                &[&slug.as_str(), &display.as_str(), &module],
            )?;
            let id = find_id(conn, "permissions", "slug = ?1", &[&slug])?
                .ok_or_else(|| format!("permission {slug} tidak ditemukan setelah seed"))?;
            map.insert(slug, id);
        }
    }
    Ok(map)
}

fn grant(conn: &Connection, role_id: i64, perm_id: i64) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO role_permissions (role_id, permission_id) VALUES (?1, ?2)",
        params![role_id, perm_id],
    )
    .map_err(|e| format!("gagal grant permission: {e}"))?;
    Ok(())
}

fn seed_role_permissions(
    conn: &Connection,
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
                grant(conn, *role_id, *perm_id)?;
            }
        }
    }
    Ok(())
}

fn seed_admin_employee(
    conn: &Connection,
    company_id: i64,
    dept_hr: i64,
    pos_hr_manager: i64,
    lvl_mgr: i64,
    hq_location: i64,
) -> Result<i64, String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let branch_id = find_id(
        conn,
        "branches",
        "company_id = ?1 AND code = 'HO'",
        &[&company_id],
    )?
    .ok_or_else(|| "branch HO tidak ditemukan".to_string())?;
    insert_ignore(
        conn,
        "employees",
        "employee_number, nik, first_name, last_name, gender, birth_place, birth_date, marital_status, phone, personal_email, company_id, branch_id, department_id, position_id, job_level_id, work_location_id, join_date, employment_status, employment_type",
        "?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?",
        &[
            &"EMP-0001",
            &"3171000000000001",
            &"Super",
            &"Administrator",
            &"male",
            &"Surabaya",
            &"1990-01-01",
            &"single",
            &"081200000001",
            &"admin.personal@example.com",
            &company_id,
            &branch_id,
            &dept_hr,
            &pos_hr_manager,
            &lvl_mgr,
            &hq_location,
            &today.as_str(),
            &"active",
            &"permanent",
        ],
    )?;
    find_id(conn, "employees", "employee_number = ?1", &[&"EMP-0001"])?
        .ok_or_else(|| "employee EMP-0001 tidak ditemukan setelah seed".to_string())
}

fn seed_admin_user(
    conn: &Connection,
    roles: &std::collections::HashMap<String, i64>,
    admin_employee: i64,
) -> Result<(), String> {
    let hash = bcrypt::hash("Admin@123", bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password admin: {e}"))?;
    // Hash acak tiap run: hanya insert bila username belum ada (jangan overwrite).
    let exists = find_id(conn, "users", "username = ?1", &[&"admin"])?;
    if exists.is_none() {
        conn.execute(
            "INSERT INTO users (employee_id, username, email, password, status, must_change_password) VALUES (?1, ?2, ?3, ?4, 'active', 1)",
            params![admin_employee, "admin", "admin@hris.local", hash],
        )
        .map_err(|e| format!("gagal insert admin: {e}"))?;
    }
    let admin_id = find_id(conn, "users", "username = ?1", &[&"admin"])?
        .ok_or_else(|| "user admin tidak ditemukan setelah seed".to_string())?;
    let super_id = roles
        .get("super-administrator")
        .ok_or_else(|| "role super-administrator belum di-seed".to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES (?1, ?2)",
        params![admin_id, super_id],
    )
    .map_err(|e| format!("gagal assign role admin: {e}"))?;
    Ok(())
}

fn seed_leave_types(conn: &Connection) -> Result<(), String> {
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
            conn,
            "leave_types",
            "code, name, default_days_per_year, is_paid, carry_forward, carry_forward_max_days, requires_attachment",
            "?,?,?,?,?,?,?",
            &[&code, &name, &days, &paid, &carry, &carry_max, &attachment],
        )?;
    }
    Ok(())
}

fn seed_permission_types(conn: &Connection) -> Result<(), String> {
    for (code, name) in [
        ("LATE", "Terlambat"),
        ("EARLY", "Pulang Cepat"),
        ("PERSONAL", "Keperluan Pribadi"),
        ("SICK", "Sakit"),
        ("WFH", "Work From Home"),
        ("BUSINESS", "Urusan Dinas"),
        ("LEAVING", "Keluar Kantor"),
    ] {
        insert_ignore(
            conn,
            "permission_types",
            "code, name",
            "?,?",
            &[&code, &name],
        )?;
    }
    Ok(())
}

fn seed_salary_components(conn: &Connection) -> Result<(), String> {
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
            conn,
            "salary_components",
            "code, name, type, calculation_type, is_taxable",
            "?,?,?,?,?",
            &[&code, &name, &ctype, &calc, &taxable],
        )?;
    }
    Ok(())
}

fn seed_shifts_and_schedule(conn: &Connection) -> Result<i64, String> {
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
        insert_ignore(
            conn,
            "shifts",
            "name, start_time, end_time, break_start, break_end, grace_period_minutes, is_overnight",
            "?,?,?,?,?,15,?",
            &[&name, &start, &end, &bstart, &bend, &overnight],
        )?;
    }
    let regular_id = find_id(conn, "shifts", "name = 'Regular'", &[])?
        .ok_or_else(|| "shift Regular tidak ditemukan".to_string())?;
    insert_ignore(
        conn,
        "work_schedules",
        "name, description",
        "?,?",
        &[&"Senin - Jumat", &"Jadwal kerja reguler Senin sampai Jumat"],
    )?;
    let schedule_id = find_id(conn, "work_schedules", "name = ?1", &[&"Senin - Jumat"])?
        .ok_or_else(|| "jadwal Senin - Jumat tidak ditemukan".to_string())?;
    for day in 0..=6 {
        let working: i64 = if (1..=5).contains(&day) { 1 } else { 0 };
        let shift: Option<i64> = if working == 1 { Some(regular_id) } else { None };
        conn.execute(
            "INSERT OR IGNORE INTO work_schedule_days (work_schedule_id, day_of_week, shift_id, is_working_day) VALUES (?1, ?2, ?3, ?4)",
            params![schedule_id, day, shift, working],
        )
        .map_err(|e| format!("gagal seed work_schedule_days: {e}"))?;
    }
    Ok(schedule_id)
}

fn seed_shift_assignment(
    conn: &Connection,
    employee_id: i64,
    schedule_id: i64,
) -> Result<(), String> {
    let first_of_month = Local::now().format("%Y-%m-01").to_string();
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM shift_assignments WHERE employee_id = ?1 AND work_schedule_id = ?2",
            params![employee_id, schedule_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(format!("gagal cek shift assignment: {other}")),
        })?;
    if exists.is_none() {
        conn.execute(
            "INSERT INTO shift_assignments (employee_id, work_schedule_id, start_date) VALUES (?1, ?2, ?3)",
            params![employee_id, schedule_id, first_of_month],
        )
        .map_err(|e| format!("gagal seed shift assignment: {e}"))?;
    }
    Ok(())
}

fn seed_holidays(conn: &Connection) -> Result<(), String> {
    let year = Local::now().format("%Y").to_string();
    for (name, suffix) in [
        ("Tahun Baru Masehi", "-01-01"),
        ("Hari Buruh", "-05-01"),
        ("Hari Kemerdekaan RI", "-08-17"),
        ("Hari Raya Natal", "-12-25"),
    ] {
        let date = format!("{year}{suffix}");
        insert_ignore(
            conn,
            "holidays",
            "name, date, type",
            "?,?,?",
            &[&name, &date.as_str(), &"national"],
        )?;
    }
    Ok(())
}

fn seed_approval_workflows(
    conn: &Connection,
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
            conn,
            "approval_workflows",
            "module, name, is_active",
            "?,?,1",
            &[&module, &name],
        )?;
        let wf_id = find_id(conn, "approval_workflows", "module = ?1", &[&module])?
            .ok_or_else(|| format!("workflow {module} tidak ditemukan"))?;
        for (idx, step) in steps.iter().enumerate() {
            let order = (idx + 1) as i64;
            if let Some(role_slug) = step.strip_prefix("role:") {
                let role_id = roles
                    .get(role_slug)
                    .ok_or_else(|| format!("role {role_slug} belum di-seed"))?;
                conn.execute(
                    "INSERT OR IGNORE INTO approval_steps (approval_workflow_id, step_order, approver_type, role_id) VALUES (?1, ?2, 'role', ?3)",
                    params![wf_id, order, role_id],
                )
                .map_err(|e| format!("gagal seed approval step: {e}"))?;
            } else {
                conn.execute(
                    "INSERT OR IGNORE INTO approval_steps (approval_workflow_id, step_order, approver_type) VALUES (?1, ?2, ?3)",
                    params![wf_id, order, step],
                )
                .map_err(|e| format!("gagal seed approval step: {e}"))?;
            }
        }
    }
    Ok(())
}

fn seed_categories(conn: &Connection) -> Result<(), String> {
    for (code, name) in [
        ("LAPTOP", "Laptop"),
        ("MOBILE", "Handphone"),
        ("FURNITURE", "Furniture"),
        ("VEHICLE", "Kendaraan"),
    ] {
        insert_ignore(
            conn,
            "asset_categories",
            "code, name",
            "?,?",
            &[&code, &name],
        )?;
    }
    let rows: Vec<(&str, &str, Option<f64>)> = vec![
        ("TRANSPORT", "Transportasi", Some(1_000_000.0)),
        ("MEDICAL", "Kesehatan", Some(2_000_000.0)),
        ("MEAL", "Makan", Some(500_000.0)),
        ("OTHER", "Lainnya", None),
    ];
    for (code, name, max) in rows {
        insert_ignore(
            conn,
            "reimbursement_categories",
            "code, name, max_amount",
            "?,?,?",
            &[&code, &name, &max],
        )?;
    }
    Ok(())
}

fn seed_settings(conn: &Connection) -> Result<(), String> {
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
    ];
    for (key, value) in settings {
        insert_ignore(
            conn,
            "system_settings",
            "setting_key, setting_value, setting_group",
            "?,?,?",
            &[&key, &value, &"general"],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{init_pool, migrate};

    fn seeded_pool() -> (tempfile::TempDir, crate::db::DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = init_pool(&dir.path().join("seed.db")).expect("init_pool");
        {
            let mut conn = pool.get().expect("get");
            migrate(&mut conn).expect("migrate");
            seed(&mut conn).expect("seed pertama");
        }
        (dir, pool)
    }

    fn counts(conn: &Connection) -> std::collections::HashMap<String, i64> {
        let mut m = std::collections::HashMap::new();
        for t in [
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
            let n: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |r| r.get(0))
                .unwrap();
            m.insert(t.to_string(), n);
        }
        m
    }

    #[test]
    fn seed_idempoten_dijalankan_dua_kali() {
        let (_dir, pool) = seeded_pool();
        let mut conn = pool.get().expect("get");
        let before = counts(&conn);
        seed(&mut conn).expect("seed kedua");
        let after = counts(&conn);
        assert_eq!(before, after, "seed kedua tidak boleh menambah baris");
    }

    #[test]
    fn seed_mengisi_master_data_kunci() {
        let (_dir, pool) = seeded_pool();
        let conn = pool.get().expect("get");
        let c = counts(&conn);
        assert_eq!(c["roles"], 8);
        assert_eq!(c["permissions"], 84);
        assert_eq!(c["leave_types"], 8);
        assert_eq!(c["salary_components"], 16);
        assert_eq!(c["approval_workflows"], 5);
        assert_eq!(c["system_settings"], 10);
        let super_perms: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM role_permissions rp JOIN roles r ON r.id = rp.role_id WHERE r.slug = 'super-administrator'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(super_perms, 84);
    }

    #[test]
    fn admin_bisa_login_dengan_kredensial_awal() {
        let (_dir, pool) = seeded_pool();
        let conn = pool.get().expect("get");
        let (hash, must_change): (String, i64) = conn
            .query_row(
                "SELECT password, must_change_password FROM users WHERE username = 'admin'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(bcrypt::verify("Admin@123", &hash).unwrap());
        assert_eq!(must_change, 1);
    }
}
