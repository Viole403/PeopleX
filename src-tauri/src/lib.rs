pub mod db;
pub mod seed;
pub mod services;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

/// State global: koneksi database, direktori data, dan sesi login (user id).
pub struct AppState {
    pub db: db::DbPool,
    pub data_dir: PathBuf,
    pub session: Mutex<Option<i64>>,
}

/// Pengguna yang sedang login.
#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type, Debug)]
pub struct SessionUser {
    pub id: i32,
    pub username: String,
    pub must_change_password: bool,
    pub is_super_admin: bool,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

/// Status database untuk layar diagnosa.
#[derive(serde::Serialize, serde::Deserialize, specta::Type)]
pub struct DbStatus {
    pub ok: bool,
    pub tables: i32,
    pub users: i32,
    pub data_dir: String,
}

/// i64 dari SQLite ke i32 untuk DTO (tolak overflow, jangan silent-truncate).
fn to_dto_int(v: i64, field: &str) -> Result<i32, String> {
    i32::try_from(v).map_err(|_| format!("nilai {field} di luar jangkauan"))
}

#[tauri::command]
#[specta::specta]
fn db_status(state: tauri::State<AppState>) -> Result<DbStatus, String> {
    let conn = state
        .db
        .get()
        .map_err(|e| format!("gagal mengambil koneksi database: {e}"))?;
    let tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung tabel: {e}"))?;
    let users: i64 = conn
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .map_err(|e| format!("gagal menghitung users: {e}"))?;
    Ok(DbStatus {
        ok: true,
        tables: to_dto_int(tables, "tables")?,
        users: to_dto_int(users, "users")?,
        data_dir: state.data_dir.display().to_string(),
    })
}

/// Ambil koneksi dari pool dengan pesan galat seragam.
fn pooled(state: &tauri::State<AppState>) -> Result<db::DbConn, String> {
    state
        .db
        .get()
        .map_err(|e| format!("gagal mengambil koneksi database: {e}"))
}

/// Pengguna aktif dari sesi (izin dimuat ulang tiap panggilan).
fn current_actor(
    state: &tauri::State<AppState>,
    conn: &rusqlite::Connection,
) -> Result<(i64, SessionUser), String> {
    let uid = state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())?
        .ok_or("Belum login.".to_string())?;
    let user = services::auth::load_session_user(conn, uid)?
        .ok_or("Sesi berakhir. Masuk kembali.".to_string())?;
    Ok((uid, user))
}

/// Gerbang izin: super-admin lolos semua; selain itu salah satu izin cukup.
fn require(
    state: &tauri::State<AppState>,
    conn: &rusqlite::Connection,
    any_of: &[&str],
) -> Result<(i64, SessionUser), String> {
    let (uid, user) = current_actor(state, conn)?;
    if user.is_super_admin
        || any_of
            .iter()
            .any(|p| user.permissions.iter().any(|u| u == p))
    {
        Ok((uid, user))
    } else {
        Err("Akses ditolak.".to_string())
    }
}

#[tauri::command]
#[specta::specta]
fn login(
    state: tauri::State<AppState>,
    username: String,
    password: String,
) -> Result<services::auth::LoginOk, String> {
    let conn = pooled(&state)?;
    let ok = services::auth::attempt_login(&conn, &username, &password)?;
    *state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())? = Some(ok.user.id as i64);
    Ok(ok)
}

#[tauri::command]
#[specta::specta]
fn logout(state: tauri::State<AppState>) -> Result<(), String> {
    let uid = state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())?
        .take();
    let conn = pooled(&state)?;
    if let Some(id) = uid {
        services::auth::logout(&conn, id)?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
fn session_state(state: tauri::State<AppState>) -> Result<Option<SessionUser>, String> {
    let conn = pooled(&state)?;
    let uid = state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())?
        .to_owned();
    match uid {
        None => Ok(None),
        Some(id) => match services::auth::load_session_user(&conn, id)? {
            Some(user) => Ok(Some(user)),
            None => {
                *state
                    .session
                    .lock()
                    .map_err(|_| "Sesi terkunci.".to_string())? = None;
                Ok(None)
            }
        },
    }
}

#[tauri::command]
#[specta::specta]
fn change_password(
    state: tauri::State<AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::auth::change_password(&conn, uid, &current_password, &new_password)
}

#[tauri::command]
#[specta::specta]
fn request_password_reset(
    state: tauri::State<AppState>,
    email: String,
) -> Result<Option<String>, String> {
    let conn = pooled(&state)?;
    services::auth::request_password_reset(&conn, &email)
}

#[tauri::command]
#[specta::specta]
fn reset_password(
    state: tauri::State<AppState>,
    token: String,
    new_password: String,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    services::auth::reset_password(&conn, &token, &new_password)
}

#[tauri::command]
#[specta::specta]
fn list_roles(state: tauri::State<AppState>) -> Result<Vec<services::rbac::Role>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::rbac::list_roles(&conn)
}

#[tauri::command]
#[specta::specta]
fn create_role(
    state: tauri::State<AppState>,
    slug: String,
    name: String,
    description: Option<String>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::rbac::create_role(&conn, uid, &slug, &name, description.as_deref())
}

#[tauri::command]
#[specta::specta]
fn update_role(
    state: tauri::State<AppState>,
    role_id: i32,
    name: String,
    description: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::rbac::update_role(&conn, uid, role_id as i64, &name, description.as_deref())
}

#[tauri::command]
#[specta::specta]
fn delete_role(state: tauri::State<AppState>, role_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::rbac::delete_role(&conn, uid, role_id as i64)
}

#[tauri::command]
#[specta::specta]
fn list_permissions(
    state: tauri::State<AppState>,
) -> Result<Vec<services::rbac::Permission>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::rbac::list_permissions(&conn)
}

#[tauri::command]
#[specta::specta]
fn role_permission_ids(state: tauri::State<AppState>, role_id: i32) -> Result<Vec<i32>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::rbac::role_permission_ids(&conn, role_id as i64)
}

#[tauri::command]
#[specta::specta]
fn sync_role_permissions(
    state: tauri::State<AppState>,
    role_id: i32,
    permission_ids: Vec<i32>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    let ids: Vec<i64> = permission_ids.iter().map(|v| *v as i64).collect();
    services::rbac::sync_role_permissions(&conn, uid, role_id as i64, &ids)
}

#[tauri::command]
#[specta::specta]
fn list_users(state: tauri::State<AppState>) -> Result<Vec<services::rbac::UserRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::rbac::list_users(&conn)
}

#[tauri::command]
#[specta::specta]
fn user_role_ids(state: tauri::State<AppState>, user_id: i32) -> Result<Vec<i32>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::rbac::user_role_ids(&conn, user_id as i64)
}

#[tauri::command]
#[specta::specta]
fn sync_user_roles(
    state: tauri::State<AppState>,
    user_id: i32,
    role_ids: Vec<i32>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    let ids: Vec<i64> = role_ids.iter().map(|v| *v as i64).collect();
    services::rbac::sync_user_roles(&conn, uid, user_id as i64, &ids)
}

#[tauri::command]
#[specta::specta]
fn toggle_user_status(
    state: tauri::State<AppState>,
    user_id: i32,
    status: String,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::rbac::toggle_user_status(&conn, uid, user_id as i64, &status)
}

#[tauri::command]
#[specta::specta]
fn admin_reset_password(
    state: tauri::State<AppState>,
    user_id: i32,
    new_password: String,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::rbac::admin_reset_password(&conn, uid, user_id as i64, &new_password)
}

#[tauri::command]
#[specta::specta]
fn audit_list(
    state: tauri::State<AppState>,
    module: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<services::audit::AuditEntry>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["audit.view", "system.manage"])?;
    services::audit::list(&conn, module.as_deref(), limit.unwrap_or(100) as i64)
}

#[tauri::command]
#[specta::specta]
fn get_settings(state: tauri::State<AppState>) -> Result<Vec<services::settings::Setting>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::settings::all_settings(&conn)
}

#[tauri::command]
#[specta::specta]
fn save_settings(
    state: tauri::State<AppState>,
    items: Vec<(String, String)>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::settings::save_settings(&conn, uid, &items)
}

#[tauri::command]
#[specta::specta]
fn get_company(
    state: tauri::State<AppState>,
) -> Result<Option<services::settings::Company>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::settings::get_company(&conn)
}

#[tauri::command]
#[specta::specta]
fn save_company(
    state: tauri::State<AppState>,
    input: services::settings::CompanyInput,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::settings::save_company(&conn, uid, &input)
}

#[tauri::command]
#[specta::specta]
fn org_entities(
    state: tauri::State<AppState>,
) -> Result<Vec<services::organization::EntityMeta>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["organization.view", "system.manage"])?;
    Ok(services::organization::entities())
}

#[tauri::command]
#[specta::specta]
fn org_list(
    state: tauri::State<AppState>,
    slug: String,
    search: String,
    page: i32,
    per_page: i32,
) -> Result<services::organization::OrgPage, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["organization.view", "system.manage"])?;
    services::organization::list(&conn, &slug, &search, page, per_page)
}

#[tauri::command]
#[specta::specta]
fn org_get(
    state: tauri::State<AppState>,
    slug: String,
    id: i32,
) -> Result<Option<std::collections::BTreeMap<String, String>>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["organization.view", "system.manage"])?;
    services::organization::get(&conn, &slug, id as i64)
}

#[tauri::command]
#[specta::specta]
fn org_options(
    state: tauri::State<AppState>,
    slug: String,
    field: String,
) -> Result<Vec<services::organization::Opt>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["organization.view", "system.manage"])?;
    services::organization::options(&conn, &slug, &field)
}

#[tauri::command]
#[specta::specta]
fn org_save(
    state: tauri::State<AppState>,
    slug: String,
    id: Option<i32>,
    values: std::collections::BTreeMap<String, String>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &[
            "organization.create",
            "organization.update",
            "system.manage",
        ],
    )?;
    services::organization::save(&conn, uid, &slug, id.map(|v| v as i64), &values)
}

#[tauri::command]
#[specta::specta]
fn org_delete(state: tauri::State<AppState>, slug: String, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["organization.delete", "system.manage"])?;
    services::organization::delete(&conn, uid, &slug, id as i64)
}

#[tauri::command]
#[specta::specta]
fn org_chart(
    state: tauri::State<AppState>,
) -> Result<Vec<services::organization::CompanyNode>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["organization.view", "system.manage"])?;
    services::organization::org_chart(&conn)
}

#[tauri::command]
#[specta::specta]
fn list_workflows(
    state: tauri::State<AppState>,
) -> Result<Vec<services::workflows::Workflow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::workflows::list(&conn)
}

#[tauri::command]
#[specta::specta]
fn add_workflow_step(
    state: tauri::State<AppState>,
    workflow_id: i32,
    approver_type: String,
    role_id: Option<i32>,
    user_id: Option<i32>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::workflows::add_step(
        &conn,
        uid,
        workflow_id as i64,
        &approver_type,
        role_id.map(|v| v as i64),
        user_id.map(|v| v as i64),
    )
}

#[tauri::command]
#[specta::specta]
fn remove_workflow_step(state: tauri::State<AppState>, step_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::workflows::remove_step(&conn, uid, step_id as i64)
}

fn files_dir(state: &tauri::State<AppState>) -> std::path::PathBuf {
    state.data_dir.join("uploads")
}

#[tauri::command]
#[specta::specta]
fn employee_list(
    state: tauri::State<AppState>,
    search: String,
    filters: services::employees::EmployeeFilter,
    page: i32,
    per_page: i32,
) -> Result<services::employees::EmployeePage, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::list(&conn, &search, &filters, page, per_page)
}

#[tauri::command]
#[specta::specta]
fn employee_detail(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::employees::EmployeeDetail>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::detail(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_dropdowns(
    state: tauri::State<AppState>,
) -> Result<services::employees::Dropdowns, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::dropdowns(&conn)
}

#[tauri::command]
#[specta::specta]
fn employee_create(
    state: tauri::State<AppState>,
    input: services::employees::EmployeeInput,
    photo: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.create", "system.manage"])?;
    let dir = files_dir(&state);
    services::employees::create(&conn, &dir, uid, &input, photo.as_ref())
}

#[tauri::command]
#[specta::specta]
fn employee_update(
    state: tauri::State<AppState>,
    id: i32,
    input: services::employees::EmployeeInput,
    resign_date: Option<String>,
    photo: Option<services::employees::FileUpload>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.update", "system.manage"])?;
    let dir = files_dir(&state);
    services::employees::update(
        &conn,
        &dir,
        uid,
        id as i64,
        &input,
        resign_date.as_deref(),
        photo.as_ref(),
    )
}

#[tauri::command]
#[specta::specta]
fn employee_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.delete", "system.manage"])?;
    services::employees::delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_child_types(
    state: tauri::State<AppState>,
) -> Result<Vec<services::employees::ChildMeta>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    Ok(services::employees::child_types())
}

#[tauri::command]
#[specta::specta]
fn employee_child_list(
    state: tauri::State<AppState>,
    child: String,
    employee_id: i32,
) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::child_list(&conn, &child, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_child_save(
    state: tauri::State<AppState>,
    child: String,
    employee_id: i32,
    id: Option<i32>,
    values: std::collections::BTreeMap<String, String>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["employee.create", "employee.update", "system.manage"],
    )?;
    services::employees::child_save(
        &conn,
        uid,
        &child,
        employee_id as i64,
        id.map(|v| v as i64),
        &values,
    )
}

#[tauri::command]
#[specta::specta]
fn employee_child_delete(
    state: tauri::State<AppState>,
    child: String,
    employee_id: i32,
    id: i32,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.delete", "system.manage"])?;
    services::employees::child_delete(&conn, uid, &child, employee_id as i64, id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_addresses(
    state: tauri::State<AppState>,
    employee_id: i32,
) -> Result<services::employees::Addresses, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::addresses(&conn, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_address_save(
    state: tauri::State<AppState>,
    employee_id: i32,
    address_type: String,
    values: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.update", "system.manage"])?;
    services::employees::save_address(&conn, uid, employee_id as i64, &address_type, &values)
}

#[tauri::command]
#[specta::specta]
fn employee_documents(
    state: tauri::State<AppState>,
    employee_id: i32,
) -> Result<Vec<services::employees::Document>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::documents(&conn, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_document_upload(
    state: tauri::State<AppState>,
    employee_id: i32,
    category: String,
    name: String,
    expiry_date: Option<String>,
    file: services::employees::FileUpload,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["employee.create", "employee.update", "system.manage"],
    )?;
    let dir = files_dir(&state);
    services::employees::upload_document(
        &conn,
        &dir,
        uid,
        employee_id as i64,
        &category,
        &name,
        expiry_date.as_deref(),
        &file,
    )
}

#[tauri::command]
#[specta::specta]
fn employee_document_bytes(
    state: tauri::State<AppState>,
    employee_id: i32,
    id: i32,
) -> Result<services::employees::DocumentBytes, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    let dir = files_dir(&state);
    services::employees::document_bytes(&conn, &dir, employee_id as i64, id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_document_delete(
    state: tauri::State<AppState>,
    employee_id: i32,
    id: i32,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.delete", "system.manage"])?;
    services::employees::child_delete(&conn, uid, "documents", employee_id as i64, id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_salary_current(
    state: tauri::State<AppState>,
    employee_id: i32,
) -> Result<Option<services::employees::SalaryRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::salary_current(&conn, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_salary_history(
    state: tauri::State<AppState>,
    employee_id: i32,
) -> Result<Vec<services::employees::SalaryRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::salary_history(&conn, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_salary_components(
    state: tauri::State<AppState>,
    salary_id: i32,
) -> Result<Vec<services::employees::SalaryComponentRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::salary_components(&conn, salary_id as i64)
}

#[tauri::command]
#[specta::specta]
fn employee_available_components(
    state: tauri::State<AppState>,
) -> Result<Vec<services::employees::SalaryComponentRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["employee.view", "system.manage"])?;
    services::employees::available_components(&conn)
}

#[tauri::command]
#[specta::specta]
fn employee_set_salary(
    state: tauri::State<AppState>,
    employee_id: i32,
    basic_salary: f64,
    effective_date: String,
    components: Vec<(i32, f64)>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["employee.update", "system.manage"])?;
    let comps: Vec<(i64, f64)> = components.iter().map(|(c, a)| (*c as i64, *a)).collect();
    services::employees::set_salary(
        &conn,
        uid,
        employee_id as i64,
        basic_salary,
        &effective_date,
        &comps,
    )
}

/// employee_id dari user login (aksi mandiri). Galat bila akun tak tertaut karyawan.
fn my_employee(conn: &rusqlite::Connection, user_id: i64) -> Result<i64, String> {
    conn.query_row(
        "SELECT employee_id FROM users WHERE id = ?1",
        rusqlite::params![user_id],
        |r| r.get::<_, Option<i64>>(0),
    )
    .map_err(|e| format!("gagal memuat akun: {e}"))?
    .ok_or("Akun belum tertaut karyawan.".to_string())
}

#[tauri::command]
#[specta::specta]
fn shift_list(state: tauri::State<AppState>) -> Result<Vec<services::attendance::Shift>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["attendance.view", "system.manage"])?;
    services::attendance::shift_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn shift_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::attendance::ShiftInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::shift_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn shift_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::shift_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn schedule_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::attendance::Schedule>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["attendance.view", "system.manage"])?;
    services::attendance::schedule_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn schedule_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::attendance::ScheduleInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::schedule_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn schedule_save_days(
    state: tauri::State<AppState>,
    schedule_id: i32,
    days: Vec<(i32, Option<i32>, bool)>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    let mapped: Vec<(i64, Option<i64>, bool)> = days
        .iter()
        .map(|(d, s, w)| (*d as i64, s.map(|v| v as i64), *w))
        .collect();
    services::attendance::schedule_save_days(&conn, uid, schedule_id as i64, &mapped)
}

#[tauri::command]
#[specta::specta]
fn schedule_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::schedule_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn assignment_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::attendance::Assignment>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["attendance.view", "system.manage"])?;
    services::attendance::assignment_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn assignment_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::attendance::AssignmentInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::assignment_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn assignment_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::assignment_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn holiday_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::attendance::Holiday>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["attendance.view", "system.manage"])?;
    services::attendance::holiday_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn holiday_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::attendance::HolidayInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::holiday_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn holiday_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["attendance.update", "system.manage"])?;
    services::attendance::holiday_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn attendance_today(
    state: tauri::State<AppState>,
) -> Result<Option<services::attendance::Attendance>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::attendance::today(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn attendance_clock_in(
    state: tauri::State<AppState>,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<services::attendance::ClockResult, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::attendance::clock_in(
        &conn,
        uid,
        my_employee(&conn, uid)?,
        lat,
        lng,
        Some("desktop"),
    )
}

#[tauri::command]
#[specta::specta]
fn attendance_clock_out(
    state: tauri::State<AppState>,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<services::attendance::ClockResult, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::attendance::clock_out(
        &conn,
        uid,
        my_employee(&conn, uid)?,
        lat,
        lng,
        Some("desktop"),
    )
}

#[tauri::command]
#[specta::specta]
fn attendance_history(
    state: tauri::State<AppState>,
    month: String,
) -> Result<Vec<services::attendance::Attendance>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::attendance::history(&conn, my_employee(&conn, uid)?, &month)
}

#[tauri::command]
#[specta::specta]
fn attendance_recap(
    state: tauri::State<AppState>,
    date: String,
    search: String,
) -> Result<Vec<services::attendance::RecapRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["attendance.view", "system.manage"])?;
    services::attendance::recap(&conn, &date, &search)
}

#[tauri::command]
#[specta::specta]
fn attendance_manual(
    state: tauri::State<AppState>,
    input: services::attendance::ManualInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["attendance.create", "attendance.correct", "system.manage"],
    )?;
    services::attendance::manual_entry(&conn, uid, &input)
}

#[tauri::command]
#[specta::specta]
fn attendance_my_corrections(
    state: tauri::State<AppState>,
) -> Result<Vec<services::attendance::Correction>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::attendance::my_corrections(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn attendance_pending_corrections(
    state: tauri::State<AppState>,
) -> Result<Vec<services::attendance::Correction>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["attendance.approve", "system.manage"])?;
    services::attendance::pending_corrections(&conn)
}

#[tauri::command]
#[specta::specta]
fn attendance_request_correction(
    state: tauri::State<AppState>,
    input: services::attendance::CorrectionInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::attendance::request_correction(&conn, uid, my_employee(&conn, uid)?, &input)
}

#[tauri::command]
#[specta::specta]
fn attendance_decide_correction(
    state: tauri::State<AppState>,
    id: i32,
    decision: String,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, user) = require(&state, &conn, &["attendance.approve", "system.manage"])?;
    let actor_emp = my_employee(&conn, uid).ok();
    let privileged = user.is_super_admin
        || user
            .permissions
            .iter()
            .any(|p| p == "attendance.approve" || p == "system.manage");
    services::attendance::decide_correction(
        &conn,
        uid,
        actor_emp,
        privileged,
        id as i64,
        &decision,
        notes.as_deref(),
    )
}

fn init_state(data_dir: PathBuf) -> Result<AppState, String> {
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("gagal membuat direktori data: {e}"))?;
    let pool = db::init_pool(&data_dir.join("peoplex.db"))?;
    {
        let mut conn = pool
            .get()
            .map_err(|e| format!("gagal mengambil koneksi database: {e}"))?;
        db::migrate(&mut conn)?;
        seed::seed(&mut conn)?;
    }
    Ok(AppState {
        db: pool,
        data_dir,
        session: Mutex::new(None),
    })
}

/// Builder specta: satu-satunya daftar command yang diekspos ke frontend.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new().commands(tauri_specta::collect_commands![
        db_status,
        login,
        logout,
        session_state,
        change_password,
        request_password_reset,
        reset_password,
        list_roles,
        create_role,
        update_role,
        delete_role,
        list_permissions,
        role_permission_ids,
        sync_role_permissions,
        list_users,
        user_role_ids,
        sync_user_roles,
        toggle_user_status,
        admin_reset_password,
        audit_list,
        get_settings,
        save_settings,
        get_company,
        save_company,
        org_entities,
        org_list,
        org_get,
        org_options,
        org_save,
        org_delete,
        org_chart,
        list_workflows,
        add_workflow_step,
        remove_workflow_step,
        employee_list,
        employee_detail,
        employee_dropdowns,
        employee_create,
        employee_update,
        employee_delete,
        employee_child_types,
        employee_child_list,
        employee_child_save,
        employee_child_delete,
        employee_addresses,
        employee_address_save,
        employee_documents,
        employee_document_upload,
        employee_document_bytes,
        employee_document_delete,
        employee_salary_current,
        employee_salary_history,
        employee_salary_components,
        employee_available_components,
        employee_set_salary,
        shift_list,
        shift_save,
        shift_delete,
        schedule_list,
        schedule_save,
        schedule_save_days,
        schedule_delete,
        assignment_list,
        assignment_save,
        assignment_delete,
        holiday_list,
        holiday_save,
        holiday_delete,
        attendance_today,
        attendance_clock_in,
        attendance_clock_out,
        attendance_history,
        attendance_recap,
        attendance_manual,
        attendance_my_corrections,
        attendance_pending_corrections,
        attendance_request_correction,
        attendance_decide_correction
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();
    #[cfg(debug_assertions)]
    builder
        .export(
            specta_typescript::Typescript::default(),
            "../src/bindings.ts",
        )
        .expect("gagal export TypeScript bindings");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|e| format!("gagal resolve direktori data: {e}"))?;
            let state = init_state(data_dir)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_state_menyiapkan_db_lengkap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = init_state(dir.path().to_path_buf()).expect("init_state");
        assert!(dir.path().join("peoplex.db").exists());
        assert_eq!(state.data_dir, dir.path());
        assert!(state.session.lock().unwrap().is_none());
        let conn = state.db.get().expect("get");
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let admin: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM users WHERE username = 'admin'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 87);
        assert_eq!(admin, 1);
    }

    #[test]
    fn specta_export_memuat_db_status() {
        let dir = tempfile::tempdir().expect("tempdir");
        let out = dir.path().join("bindings.ts");
        specta_builder()
            .export(specta_typescript::Typescript::default(), &out)
            .expect("export");
        let ts = std::fs::read_to_string(&out).expect("read");
        assert!(
            ts.contains("dbStatus"),
            "bindings harus memuat command dbStatus"
        );
        assert!(
            ts.contains("DbStatus"),
            "bindings harus memuat tipe DbStatus"
        );
    }

    #[test]
    #[ignore]
    fn export_bindings_ke_src() {
        specta_builder()
            .export(
                specta_typescript::Typescript::default(),
                "../src/bindings.ts",
            )
            .expect("export");
    }
}
