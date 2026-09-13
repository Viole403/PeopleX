pub mod db;
pub mod seed;
pub mod services;

use rusqlite::OptionalExtension;
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

/// Cek permission literal untuk wrapper yang juga memakai otorisasi dinamis.
fn privileged(user: &SessionUser, perm: &str) -> bool {
    user.is_super_admin
        || user
            .permissions
            .iter()
            .any(|p| p == perm || p == "system.manage")
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
    if !ok.mfa_required {
        *state
            .session
            .lock()
            .map_err(|_| "Sesi terkunci.".to_string())? = Some(ok.user.id as i64);
    }
    Ok(ok)
}

#[tauri::command]
#[specta::specta]
fn mfa_challenge(
    state: tauri::State<AppState>,
    user_id: i32,
    code: String,
) -> Result<services::auth::LoginOk, String> {
    let conn = pooled(&state)?;
    let user = services::auth::verify_mfa(&conn, user_id as i64, &code)?;
    *state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())? = Some(user.id as i64);
    Ok(services::auth::LoginOk {
        must_change_password: user.must_change_password,
        mfa_required: false,
        user,
    })
}

#[tauri::command]
#[specta::specta]
fn mfa_setup(state: tauri::State<AppState>) -> Result<services::auth::MfaSetup, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::auth::mfa_setup(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn mfa_enable(state: tauri::State<AppState>, code: String) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::auth::mfa_enable(&conn, uid, &code)
}

#[tauri::command]
#[specta::specta]
fn mfa_disable(state: tauri::State<AppState>, password: String) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::auth::mfa_disable(&conn, uid, &password)
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
    require(&state, &conn, &["rbac.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
    services::rbac::update_role(&conn, uid, role_id as i64, &name, description.as_deref())
}

#[tauri::command]
#[specta::specta]
fn delete_role(state: tauri::State<AppState>, role_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
    services::rbac::delete_role(&conn, uid, role_id as i64)
}

#[tauri::command]
#[specta::specta]
fn list_permissions(
    state: tauri::State<AppState>,
) -> Result<Vec<services::rbac::Permission>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["rbac.manage"])?;
    services::rbac::list_permissions(&conn)
}

#[tauri::command]
#[specta::specta]
fn role_permission_ids(state: tauri::State<AppState>, role_id: i32) -> Result<Vec<i32>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["rbac.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
    let ids: Vec<i64> = permission_ids.iter().map(|v| *v as i64).collect();
    services::rbac::sync_role_permissions(&conn, uid, role_id as i64, &ids)
}

#[tauri::command]
#[specta::specta]
fn list_users(state: tauri::State<AppState>) -> Result<Vec<services::rbac::UserRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["rbac.manage"])?;
    services::rbac::list_users(&conn)
}

#[tauri::command]
#[specta::specta]
fn user_role_ids(state: tauri::State<AppState>, user_id: i32) -> Result<Vec<i32>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["rbac.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["rbac.manage"])?;
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
    require(&state, &conn, &["settings.manage"])?;
    services::settings::all_settings(&conn)
}

#[tauri::command]
#[specta::specta]
fn save_settings(
    state: tauri::State<AppState>,
    items: Vec<(String, String)>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["settings.manage"])?;
    services::settings::save_settings(&conn, uid, &items)
}

#[tauri::command]
#[specta::specta]
fn get_company(
    state: tauri::State<AppState>,
) -> Result<Option<services::settings::Company>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["settings.manage"])?;
    services::settings::get_company(&conn)
}

#[tauri::command]
#[specta::specta]
fn save_company(
    state: tauri::State<AppState>,
    input: services::settings::CompanyInput,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["settings.manage"])?;
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
    require(&state, &conn, &["workflow.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["workflow.manage"])?;
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
    let (uid, _) = require(&state, &conn, &["workflow.manage"])?;
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
fn contract_expiring(
    state: tauri::State<AppState>,
    days: i32,
) -> Result<Vec<services::employees::ContractAlert>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["contract.view", "system.manage"])?;
    services::employees::expiring_contracts(&conn, days as i64)
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
    let privileged = privileged(&user, "attendance.approve");
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

#[tauri::command]
#[specta::specta]
fn leave_balances(
    state: tauri::State<AppState>,
    year: i32,
) -> Result<Vec<services::leave::Balance>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::leave::balances(&conn, my_employee(&conn, uid)?, year)
}

#[tauri::command]
#[specta::specta]
fn leave_types(state: tauri::State<AppState>) -> Result<Vec<services::leave::LeaveType>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["leave.view", "system.manage"])?;
    services::leave::type_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn leave_type_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::leave::LeaveTypeInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["leave.update", "system.manage"])?;
    services::leave::type_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn leave_type_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["leave.update", "system.manage"])?;
    services::leave::type_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn leave_my(state: tauri::State<AppState>) -> Result<Vec<services::leave::LeaveRequest>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::leave::my_requests(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn leave_pending(
    state: tauri::State<AppState>,
) -> Result<Vec<services::leave::LeaveRequest>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::leave::pending_for(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn leave_all(state: tauri::State<AppState>) -> Result<Vec<services::leave::LeaveRequest>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["leave.view", "system.manage"])?;
    services::leave::all_requests(&conn)
}

#[tauri::command]
#[specta::specta]
fn leave_create(
    state: tauri::State<AppState>,
    input: services::leave::LeaveCreate,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::leave::create(&conn, uid, my_employee(&conn, uid)?, &input)
}

#[tauri::command]
#[specta::specta]
fn leave_decide(
    state: tauri::State<AppState>,
    id: i32,
    decision: String,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::leave::decide(&conn, uid, id as i64, &decision, notes.as_deref())
}

#[tauri::command]
#[specta::specta]
fn leave_cancel(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::leave::cancel(&conn, uid, my_employee(&conn, uid)?, id as i64)
}

#[tauri::command]
#[specta::specta]
fn leave_calendar(
    state: tauri::State<AppState>,
    month: String,
) -> Result<Vec<services::leave::CalendarDay>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["leave.view", "system.manage"])?;
    services::leave::calendar(&conn, &month)
}

#[tauri::command]
#[specta::specta]
fn overtime_my(state: tauri::State<AppState>) -> Result<Vec<services::overtime::Overtime>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::overtime::my_requests(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn overtime_pending(
    state: tauri::State<AppState>,
) -> Result<Vec<services::overtime::Overtime>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::overtime::pending_for(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn overtime_all(
    state: tauri::State<AppState>,
) -> Result<Vec<services::overtime::Overtime>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["overtime.view", "system.manage"])?;
    services::overtime::all_requests(&conn)
}

#[tauri::command]
#[specta::specta]
fn overtime_create(
    state: tauri::State<AppState>,
    input: services::overtime::OvertimeCreate,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::overtime::create(&conn, uid, my_employee(&conn, uid)?, &input)
}

#[tauri::command]
#[specta::specta]
fn overtime_decide(
    state: tauri::State<AppState>,
    id: i32,
    decision: String,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::overtime::decide(&conn, uid, id as i64, &decision, notes.as_deref())
}

#[tauri::command]
#[specta::specta]
fn permission_types(
    state: tauri::State<AppState>,
) -> Result<Vec<services::permission::PermissionType>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["permission.view", "system.manage"])?;
    services::permission::types(&conn)
}

#[tauri::command]
#[specta::specta]
fn permission_my(
    state: tauri::State<AppState>,
) -> Result<Vec<services::permission::PermissionRequest>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::permission::my_requests(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn permission_pending(
    state: tauri::State<AppState>,
) -> Result<Vec<services::permission::PermissionRequest>, String> {
    let conn = pooled(&state)?;
    let (uid, user) = current_actor(&state, &conn)?;
    let privileged = privileged(&user, "permission.approve");
    services::permission::pending_for(&conn, uid, privileged)
}

#[tauri::command]
#[specta::specta]
fn permission_all(
    state: tauri::State<AppState>,
) -> Result<Vec<services::permission::PermissionRequest>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["permission.view", "system.manage"])?;
    services::permission::all_requests(&conn)
}

#[tauri::command]
#[specta::specta]
fn permission_create(
    state: tauri::State<AppState>,
    input: services::permission::PermissionCreate,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::permission::create(&conn, uid, my_employee(&conn, uid)?, &input)
}

#[tauri::command]
#[specta::specta]
fn permission_decide(
    state: tauri::State<AppState>,
    id: i32,
    decision: String,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, user) = current_actor(&state, &conn)?;
    let actor_emp = my_employee(&conn, uid).ok();
    let privileged = privileged(&user, "permission.approve");
    services::permission::decide(&conn, uid, actor_emp, privileged, id as i64, &decision)
}

#[tauri::command]
#[specta::specta]
fn payroll_periods(
    state: tauri::State<AppState>,
) -> Result<Vec<services::payroll::Period>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["payroll.view", "system.manage"])?;
    services::payroll::period_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn payroll_period_create(
    state: tauri::State<AppState>,
    input: services::payroll::PeriodInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.create", "system.manage"])?;
    services::payroll::period_create(&conn, uid, uid, &input)
}

#[tauri::command]
#[specta::specta]
fn payroll_rows(
    state: tauri::State<AppState>,
    period_id: i32,
) -> Result<Vec<services::payroll::PayrollRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["payroll.view", "system.manage"])?;
    services::payroll::payrolls_for_period(&conn, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_detail(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::payroll::PayrollDetail>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["payroll.view", "system.manage"])?;
    services::payroll::payroll_detail(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_generate(state: tauri::State<AppState>, period_id: i32) -> Result<(), String> {
    let mut conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.generate", "system.manage"])?;
    services::payroll::generate(&mut conn, uid, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_approve(state: tauri::State<AppState>, period_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.approve", "system.manage"])?;
    services::payroll::approve_period(&conn, uid, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_pay(state: tauri::State<AppState>, period_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.approve", "system.manage"])?;
    services::payroll::mark_paid(&conn, uid, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_lock(state: tauri::State<AppState>, period_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.approve", "system.manage"])?;
    services::payroll::lock_period(&conn, uid, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_my_slips(
    state: tauri::State<AppState>,
) -> Result<Vec<services::payroll::PayslipInfo>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::payroll::my_payslips(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn payroll_payslip_render(
    state: tauri::State<AppState>,
    payroll_id: i32,
) -> Result<String, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["payroll.view", "system.manage"])?;
    let dir = files_dir(&state);
    services::payslip::render(&conn, &dir, payroll_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_payslip_file(
    state: tauri::State<AppState>,
    payroll_id: i32,
) -> Result<services::payslip::PayslipFile, String> {
    let conn = pooled(&state)?;
    let (uid, user) = current_actor(&state, &conn)?;
    let owner: Option<i64> = conn
        .query_row(
            "SELECT p.employee_id FROM payrolls p WHERE p.id = ?1",
            rusqlite::params![payroll_id as i64],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat payroll: {e}"))?;
    let mine = my_employee(&conn, uid).ok();
    let allowed = privileged(&user, "payroll.view") || (mine.is_some() && owner == mine);
    if !allowed {
        return Err("Akses ditolak.".to_string());
    }
    let dir = files_dir(&state);
    services::payslip::read_file(&conn, &dir, payroll_id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_components(
    state: tauri::State<AppState>,
) -> Result<Vec<services::payroll::Component>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["payroll.view", "system.manage"])?;
    services::payroll::component_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn payroll_component_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::payroll::ComponentInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["payroll.create", "payroll.update", "system.manage"],
    )?;
    services::payroll::component_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn payroll_component_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.delete", "system.manage"])?;
    services::payroll::component_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn payroll_deductions(
    state: tauri::State<AppState>,
    pending_only: bool,
) -> Result<Vec<services::payroll::Deduction>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["payroll.view", "system.manage"])?;
    services::payroll::deduction_list(&conn, pending_only)
}

#[tauri::command]
#[specta::specta]
fn payroll_deduction_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::payroll::DeductionInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["payroll.create", "payroll.update", "system.manage"],
    )?;
    services::payroll::deduction_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn payroll_deduction_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["payroll.delete", "system.manage"])?;
    services::payroll::deduction_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn vacancy_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::recruitment::Vacancy>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["recruitment.view", "system.manage"])?;
    services::recruitment::vacancy_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn vacancy_get(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::recruitment::Vacancy>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["recruitment.view", "system.manage"])?;
    services::recruitment::vacancy_get(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn vacancy_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::recruitment::VacancyInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["recruitment.create", "recruitment.update", "system.manage"],
    )?;
    services::recruitment::vacancy_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn vacancy_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.delete", "system.manage"])?;
    services::recruitment::vacancy_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn candidates_by_vacancy(
    state: tauri::State<AppState>,
    vacancy_id: i32,
) -> Result<Vec<services::recruitment::CandidateRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["recruitment.view", "system.manage"])?;
    services::recruitment::candidates_by_vacancy(&conn, vacancy_id as i64)
}

#[tauri::command]
#[specta::specta]
fn candidate_detail(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::recruitment::CandidateDetail>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["recruitment.view", "system.manage"])?;
    services::recruitment::candidate_detail(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn candidate_create(
    state: tauri::State<AppState>,
    vacancy_id: i32,
    input: services::recruitment::CandidateInput,
    cv: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.create", "system.manage"])?;
    let dir = files_dir(&state);
    services::recruitment::candidate_create(
        &conn,
        &dir,
        uid,
        vacancy_id as i64,
        &input,
        cv.as_ref(),
    )
}

#[tauri::command]
#[specta::specta]
fn candidate_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.delete", "system.manage"])?;
    services::recruitment::candidate_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn candidate_stage(
    state: tauri::State<AppState>,
    id: i32,
    stage: String,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.update", "system.manage"])?;
    services::recruitment::update_stage(&conn, uid, id as i64, &stage, notes.as_deref())
}

#[tauri::command]
#[specta::specta]
fn interview_add(
    state: tauri::State<AppState>,
    candidate_id: i32,
    input: services::recruitment::InterviewInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.update", "system.manage"])?;
    services::recruitment::add_interview(&conn, uid, candidate_id as i64, &input)
}

#[tauri::command]
#[specta::specta]
fn interview_decide(
    state: tauri::State<AppState>,
    id: i32,
    result: String,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.update", "system.manage"])?;
    services::recruitment::decide_interview(&conn, uid, id as i64, &result, notes.as_deref())
}

#[tauri::command]
#[specta::specta]
fn assessment_add(
    state: tauri::State<AppState>,
    candidate_id: i32,
    input: services::recruitment::AssessmentInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.update", "system.manage"])?;
    services::recruitment::add_assessment(&conn, uid, candidate_id as i64, &input)
}

#[tauri::command]
#[specta::specta]
fn candidate_hire(
    state: tauri::State<AppState>,
    id: i32,
    join_date: Option<String>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["recruitment.update", "system.manage"])?;
    let dir = files_dir(&state);
    services::recruitment::hire(&conn, &dir, uid, id as i64, join_date.as_deref())
}

#[tauri::command]
#[specta::specta]
fn candidate_cv(
    state: tauri::State<AppState>,
    candidate_id: i32,
    id: i32,
) -> Result<services::employees::DocumentBytes, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["recruitment.view", "system.manage"])?;
    let dir = files_dir(&state);
    services::recruitment::candidate_document_bytes(&conn, &dir, candidate_id as i64, id as i64)
}

#[tauri::command]
#[specta::specta]
fn onboarding_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::onboarding::Onboarding>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["onboarding.view", "system.manage"])?;
    services::onboarding::list(&conn)
}

#[tauri::command]
#[specta::specta]
fn onboarding_get(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::onboarding::Onboarding>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["onboarding.view", "system.manage"])?;
    services::onboarding::find(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn onboarding_mine(
    state: tauri::State<AppState>,
) -> Result<Option<services::onboarding::Onboarding>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::onboarding::for_employee(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn onboarding_toggle(
    state: tauri::State<AppState>,
    task_id: i32,
    completed: bool,
) -> Result<(i32, String), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["onboarding.create", "onboarding.update", "system.manage"],
    )?;
    services::onboarding::toggle_task(&conn, uid, task_id as i64, completed)
}

#[tauri::command]
#[specta::specta]
fn offboarding_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::offboarding::OffboardingRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["offboarding.view", "system.manage"])?;
    services::offboarding::list(&conn)
}

#[tauri::command]
#[specta::specta]
fn offboarding_my(
    state: tauri::State<AppState>,
) -> Result<Vec<services::offboarding::OffboardingRow>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::offboarding::my_requests(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn offboarding_get(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::offboarding::OffboardingDetail>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["offboarding.view", "system.manage"])?;
    services::offboarding::find(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn offboarding_create(
    state: tauri::State<AppState>,
    input: services::offboarding::OffboardingCreate,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::offboarding::create(&conn, uid, my_employee(&conn, uid)?, &input)
}

#[tauri::command]
#[specta::specta]
fn offboarding_decide(
    state: tauri::State<AppState>,
    id: i32,
    action: String,
) -> Result<String, String> {
    let conn = pooled(&state)?;
    let (uid, user) = current_actor(&state, &conn)?;
    let actor_emp = my_employee(&conn, uid).ok();
    let privileged = privileged(&user, "offboarding.approve");
    services::offboarding::decide(&conn, uid, actor_emp, privileged, id as i64, &action, None)
}

#[tauri::command]
#[specta::specta]
fn offboarding_exit_save(
    state: tauri::State<AppState>,
    id: i32,
    input: services::offboarding::ExitInterviewInput,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["offboarding.create", "offboarding.update", "system.manage"],
    )?;
    services::offboarding::save_exit_interview(&conn, uid, id as i64, &input)
}

#[tauri::command]
#[specta::specta]
fn offboarding_clearance(
    state: tauri::State<AppState>,
    item_id: i32,
    cleared: bool,
    notes: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["offboarding.create", "offboarding.update", "system.manage"],
    )?;
    services::offboarding::toggle_clearance(&conn, uid, item_id as i64, cleared, notes.as_deref())
}

#[tauri::command]
#[specta::specta]
fn performance_periods(
    state: tauri::State<AppState>,
) -> Result<Vec<services::performance::PerfPeriod>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["performance.view", "system.manage"])?;
    services::performance::period_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn performance_period_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::performance::PerfPeriodInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["performance.create", "performance.update", "system.manage"],
    )?;
    services::performance::period_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn performance_period_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["performance.delete", "system.manage"])?;
    services::performance::period_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn performance_kpis(
    state: tauri::State<AppState>,
) -> Result<Vec<services::performance::Kpi>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["performance.view", "system.manage"])?;
    services::performance::kpi_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn performance_kpi_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::performance::KpiInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["performance.create", "performance.update", "system.manage"],
    )?;
    services::performance::kpi_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn performance_kpi_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["performance.delete", "system.manage"])?;
    services::performance::kpi_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn performance_reviews(
    state: tauri::State<AppState>,
    period_id: i32,
) -> Result<Vec<services::performance::ReviewRow>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["performance.view", "system.manage"])?;
    services::performance::reviews_for_period(&conn, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn performance_my_reviews(
    state: tauri::State<AppState>,
) -> Result<Vec<services::performance::ReviewRow>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::performance::my_reviews(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn performance_review_detail(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::performance::ReviewDetail>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["performance.view", "system.manage"])?;
    services::performance::review_detail(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn performance_ensure_review(
    state: tauri::State<AppState>,
    period_id: i32,
    employee_id: i32,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    require(
        &state,
        &conn,
        &["performance.create", "performance.update", "system.manage"],
    )?;
    services::performance::ensure_review(&conn, period_id as i64, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn performance_assign_kpi(
    state: tauri::State<AppState>,
    period_id: i32,
    employee_id: i32,
    kpi_id: i32,
    target: f64,
    weight: f64,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["performance.create", "performance.update", "system.manage"],
    )?;
    services::performance::assign_kpi(
        &conn,
        uid,
        period_id as i64,
        employee_id as i64,
        kpi_id as i64,
        target,
        weight,
    )
}

#[tauri::command]
#[specta::specta]
fn performance_submit_actual(
    state: tauri::State<AppState>,
    employee_kpi_id: i32,
    actual: f64,
) -> Result<f64, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["performance.create", "performance.update", "system.manage"],
    )?;
    services::performance::submit_actual(&conn, uid, employee_kpi_id as i64, actual)
}

#[tauri::command]
#[specta::specta]
fn performance_submit_review(
    state: tauri::State<AppState>,
    review_id: i32,
    role: String,
    score: f64,
    comments: Option<String>,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["performance.create", "performance.review", "system.manage"],
    )?;
    services::performance::submit_review(
        &conn,
        uid,
        review_id as i64,
        &role,
        score,
        comments.as_deref(),
    )
}

#[tauri::command]
#[specta::specta]
fn training_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::training::Training>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["training.view", "system.manage"])?;
    services::training::list(&conn)
}

#[tauri::command]
#[specta::specta]
fn training_participants(
    state: tauri::State<AppState>,
    training_id: i32,
) -> Result<Vec<services::training::Participant>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["training.view", "system.manage"])?;
    services::training::detail_participants(&conn, training_id as i64)
}

#[tauri::command]
#[specta::specta]
fn training_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::training::TrainingInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["training.create", "training.update", "system.manage"],
    )?;
    services::training::save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn training_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["training.delete", "system.manage"])?;
    services::training::delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn training_add_participant(
    state: tauri::State<AppState>,
    training_id: i32,
    employee_id: i32,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["training.create", "training.update", "system.manage"],
    )?;
    services::training::add_participant(&conn, uid, training_id as i64, employee_id as i64)
}

#[tauri::command]
#[specta::specta]
fn training_participant_status(
    state: tauri::State<AppState>,
    participant_id: i32,
    status: String,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["training.create", "training.update", "system.manage"],
    )?;
    services::training::set_participant_status(&conn, uid, participant_id as i64, &status)
}

#[tauri::command]
#[specta::specta]
fn training_certifications(
    state: tauri::State<AppState>,
) -> Result<Vec<services::training::Certification>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["training.view", "system.manage"])?;
    services::training::certifications(&conn)
}

#[tauri::command]
#[specta::specta]
fn training_certification_add(
    state: tauri::State<AppState>,
    input: services::training::CertificationInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["training.create", "training.update", "system.manage"],
    )?;
    services::training::add_certification(&conn, uid, &input)
}

#[tauri::command]
#[specta::specta]
fn training_skill_matrix(
    state: tauri::State<AppState>,
) -> Result<Vec<services::training::SkillCell>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["training.view", "system.manage"])?;
    services::training::skill_matrix(&conn)
}

#[tauri::command]
#[specta::specta]
fn training_skill_set(
    state: tauri::State<AppState>,
    employee_id: i32,
    skill_name: String,
    level: i32,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["training.create", "training.update", "system.manage"],
    )?;
    services::training::set_skill(&conn, uid, employee_id as i64, &skill_name, level)
}

#[tauri::command]
#[specta::specta]
fn asset_categories(
    state: tauri::State<AppState>,
) -> Result<Vec<services::assets::Category>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["asset.view", "system.manage"])?;
    services::assets::category_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn asset_category_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    code: String,
    name: String,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["asset.create", "asset.update", "system.manage"],
    )?;
    services::assets::category_save(&conn, uid, id.map(|v| v as i64), &code, &name)
}

#[tauri::command]
#[specta::specta]
fn asset_category_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["asset.delete", "system.manage"])?;
    services::assets::category_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn assets_list(
    state: tauri::State<AppState>,
    search: String,
) -> Result<Vec<services::assets::Asset>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["asset.view", "system.manage"])?;
    services::assets::asset_list(&conn, &search)
}

#[tauri::command]
#[specta::specta]
fn asset_detail(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<Option<services::assets::AssetDetail>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["asset.view", "system.manage"])?;
    services::assets::asset_detail(&conn, id as i64)
}

#[tauri::command]
#[specta::specta]
fn asset_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    input: services::assets::AssetInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["asset.create", "asset.update", "system.manage"],
    )?;
    services::assets::asset_save(&conn, uid, id.map(|v| v as i64), &input)
}

#[tauri::command]
#[specta::specta]
fn asset_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["asset.delete", "system.manage"])?;
    services::assets::asset_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn asset_my(state: tauri::State<AppState>) -> Result<Vec<services::assets::MyAsset>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::assets::my_assets(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn asset_assign(
    state: tauri::State<AppState>,
    asset_id: i32,
    input: services::assets::AssignInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["asset.create", "asset.update", "system.manage"],
    )?;
    services::assets::assign(&conn, uid, asset_id as i64, &input)
}

#[tauri::command]
#[specta::specta]
fn asset_return(
    state: tauri::State<AppState>,
    assignment_id: i32,
    input: services::assets::ReturnInput,
) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["asset.create", "asset.update", "system.manage"],
    )?;
    services::assets::return_asset(&conn, uid, uid, assignment_id as i64, &input)
}

#[tauri::command]
#[specta::specta]
fn asset_maintenance_add(
    state: tauri::State<AppState>,
    asset_id: i32,
    input: services::assets::MaintenanceInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["asset.create", "asset.update", "system.manage"],
    )?;
    services::assets::add_maintenance(&conn, uid, asset_id as i64, &input)
}

#[tauri::command]
#[specta::specta]
fn asset_mark_available(state: tauri::State<AppState>, asset_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(
        &state,
        &conn,
        &["asset.create", "asset.update", "system.manage"],
    )?;
    services::assets::mark_available(&conn, uid, asset_id as i64)
}

#[tauri::command]
#[specta::specta]
fn trip_my(state: tauri::State<AppState>) -> Result<Vec<services::travel::Trip>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::travel::my_trips(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn trip_all(state: tauri::State<AppState>) -> Result<Vec<services::travel::Trip>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["business_trip.view", "system.manage"])?;
    services::travel::all_trips(&conn)
}

#[tauri::command]
#[specta::specta]
fn trip_pending(state: tauri::State<AppState>) -> Result<Vec<services::travel::Trip>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::travel::pending_for(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn trip_expenses(
    state: tauri::State<AppState>,
    trip_id: i32,
) -> Result<Vec<services::travel::TripExpense>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["business_trip.view", "system.manage"])?;
    services::travel::trip_expenses(&conn, trip_id as i64)
}

#[tauri::command]
#[specta::specta]
fn trip_create(
    state: tauri::State<AppState>,
    input: services::travel::TripInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::travel::trip_create(&conn, uid, my_employee(&conn, uid)?, &input)
}

#[tauri::command]
#[specta::specta]
fn trip_decide(state: tauri::State<AppState>, id: i32, decision: String) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::travel::trip_decide(&conn, uid, id as i64, &decision)
}

#[tauri::command]
#[specta::specta]
fn trip_expense_add(
    state: tauri::State<AppState>,
    trip_id: i32,
    input: services::travel::TripExpenseInput,
    receipt: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    let dir = files_dir(&state);
    services::travel::trip_add_expense(&conn, &dir, uid, trip_id as i64, &input, receipt.as_ref())
}

#[tauri::command]
#[specta::specta]
fn trip_settle(state: tauri::State<AppState>, trip_id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["business_trip.approve", "system.manage"])?;
    services::travel::trip_settle(&conn, uid, trip_id as i64)
}

#[tauri::command]
#[specta::specta]
fn reimburse_categories(
    state: tauri::State<AppState>,
) -> Result<Vec<services::travel::ReimburseCategory>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["reimbursement.view", "system.manage"])?;
    services::travel::category_list(&conn)
}

#[tauri::command]
#[specta::specta]
fn reimburse_category_save(
    state: tauri::State<AppState>,
    id: Option<i32>,
    code: String,
    name: String,
    max_amount: Option<f64>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::travel::category_save(&conn, uid, id.map(|v| v as i64), &code, &name, max_amount)
}

#[tauri::command]
#[specta::specta]
fn reimburse_category_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["system.manage"])?;
    services::travel::category_delete(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn reimburse_my(state: tauri::State<AppState>) -> Result<Vec<services::travel::Reimburse>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::travel::my_reimburse(&conn, my_employee(&conn, uid)?)
}

#[tauri::command]
#[specta::specta]
fn reimburse_all(
    state: tauri::State<AppState>,
) -> Result<Vec<services::travel::Reimburse>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["reimbursement.view", "system.manage"])?;
    services::travel::all_reimburse(&conn)
}

#[tauri::command]
#[specta::specta]
fn reimburse_create(
    state: tauri::State<AppState>,
    input: services::travel::ReimburseInput,
    receipt: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    let dir = files_dir(&state);
    services::travel::reimburse_create(
        &conn,
        &dir,
        uid,
        my_employee(&conn, uid)?,
        &input,
        receipt.as_ref(),
    )
}

#[tauri::command]
#[specta::specta]
fn reimburse_decide(
    state: tauri::State<AppState>,
    id: i32,
    action: String,
) -> Result<String, String> {
    let conn = pooled(&state)?;
    let (uid, user) = current_actor(&state, &conn)?;
    let actor_emp = my_employee(&conn, uid).ok();
    let privileged = privileged(&user, "reimbursement.approve");
    services::travel::reimburse_decide(&conn, uid, actor_emp, privileged, id as i64, &action)
}

// ---------------- Dasbor ----------------

#[tauri::command]
#[specta::specta]
fn dashboard_hr(state: tauri::State<AppState>) -> Result<services::dashboard::HrDashboard, String> {
    let conn = pooled(&state)?;
    require(
        &state,
        &conn,
        &["employee.view", "payroll.view", "system.manage"],
    )?;
    services::dashboard::hr(&conn)
}

#[tauri::command]
#[specta::specta]
fn dashboard_me(state: tauri::State<AppState>) -> Result<services::dashboard::MySummary, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::dashboard::mine(&conn, uid)
}

// ---------------- Notifikasi ----------------

#[tauri::command]
#[specta::specta]
fn notification_recent(
    state: tauri::State<AppState>,
) -> Result<Vec<services::notifications::Notification>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::notifications::recent(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn notification_all(
    state: tauri::State<AppState>,
) -> Result<Vec<services::notifications::Notification>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::notifications::all(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn notification_unread(state: tauri::State<AppState>) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::notifications::unread_count(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn notification_mark_read(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::notifications::mark_read(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn notification_mark_all(state: tauri::State<AppState>) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::notifications::mark_all(&conn, uid)
}

// ---------------- Pengumuman ----------------

#[tauri::command]
#[specta::specta]
fn announcement_visible(
    state: tauri::State<AppState>,
) -> Result<Vec<services::announcements::Announcement>, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::announcements::visible(&conn, uid)
}

#[tauri::command]
#[specta::specta]
fn announcement_list(
    state: tauri::State<AppState>,
) -> Result<Vec<services::announcements::Announcement>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["announcement.view", "system.manage"])?;
    services::announcements::all(&conn)
}

#[tauri::command]
#[specta::specta]
fn announcement_get(
    state: tauri::State<AppState>,
    id: i32,
) -> Result<services::announcements::Announcement, String> {
    let conn = pooled(&state)?;
    let (uid, _) = current_actor(&state, &conn)?;
    services::announcements::get(&conn, uid, id as i64)
}

#[tauri::command]
#[specta::specta]
fn announcement_create(
    state: tauri::State<AppState>,
    input: services::announcements::AnnouncementInput,
) -> Result<i32, String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["announcement.create", "system.manage"])?;
    services::announcements::create(&conn, uid, &input)
}

#[tauri::command]
#[specta::specta]
fn announcement_delete(state: tauri::State<AppState>, id: i32) -> Result<(), String> {
    let conn = pooled(&state)?;
    let (uid, _) = require(&state, &conn, &["announcement.delete", "system.manage"])?;
    services::announcements::delete(&conn, uid, id as i64)
}

// ---------------- Laporan ----------------

#[tauri::command]
#[specta::specta]
fn report_employees(
    state: tauri::State<AppState>,
    search: Option<String>,
    department_id: Option<i32>,
    status: Option<String>,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::employees(
        &conn,
        search.as_deref(),
        department_id.map(|v| v as i64),
        status.as_deref(),
    )
}

#[tauri::command]
#[specta::specta]
fn report_headcount(
    state: tauri::State<AppState>,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::headcount(&conn)
}

#[tauri::command]
#[specta::specta]
fn report_attendance(
    state: tauri::State<AppState>,
    month: String,
    department_id: Option<i32>,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::attendance(&conn, &month, department_id.map(|v| v as i64))
}

#[tauri::command]
#[specta::specta]
fn report_leave(
    state: tauri::State<AppState>,
    year: i32,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::leave(&conn, year)
}

#[tauri::command]
#[specta::specta]
fn report_payroll(
    state: tauri::State<AppState>,
    period_id: i32,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::payroll(&conn, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn report_pph21_annual(
    state: tauri::State<AppState>,
    year: i32,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::pph21_annual(&conn, year)
}

#[tauri::command]
#[specta::specta]
fn report_recruitment(
    state: tauri::State<AppState>,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::recruitment(&conn)
}

#[tauri::command]
#[specta::specta]
fn report_performance(
    state: tauri::State<AppState>,
    period_id: i32,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::performance(&conn, period_id as i64)
}

#[tauri::command]
#[specta::specta]
fn report_contracts(
    state: tauri::State<AppState>,
    before: String,
) -> Result<services::reports::ReportTable, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::contracts(&conn, &before)
}

#[tauri::command]
#[specta::specta]
fn report_analytics(
    state: tauri::State<AppState>,
) -> Result<Vec<services::reports::DeptStat>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.view", "system.manage"])?;
    services::reports::analytics(&conn)
}

#[tauri::command]
#[specta::specta]
fn report_export(
    state: tauri::State<AppState>,
    kind: String,
    format: String,
    arg1: Option<String>,
    arg2: Option<i32>,
) -> Result<services::reports::ExportFile, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["report.export", "system.manage"])?;
    services::reports::export(
        &conn,
        &kind,
        &format,
        arg1.as_deref(),
        arg2.map(|v| v as i64),
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
        services::backup::ensure_scheduled(&conn, &services::backup::backup_dir(&data_dir));
    }
    Ok(AppState {
        db: pool,
        data_dir,
        session: Mutex::new(None),
    })
}

#[tauri::command]
#[specta::specta]
fn backup_now(state: tauri::State<AppState>) -> Result<String, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::backup::backup_now(&conn, &services::backup::backup_dir(&state.data_dir))
}

#[tauri::command]
#[specta::specta]
fn backup_list(state: tauri::State<AppState>) -> Result<Vec<services::backup::BackupFile>, String> {
    let conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::backup::backup_list(&services::backup::backup_dir(&state.data_dir))
}

#[tauri::command]
#[specta::specta]
fn backup_restore(state: tauri::State<AppState>, name: String) -> Result<(), String> {
    let mut conn = pooled(&state)?;
    require(&state, &conn, &["system.manage"])?;
    services::backup::backup_restore(
        &mut conn,
        &services::backup::backup_dir(&state.data_dir),
        &name,
    )
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
        mfa_challenge,
        mfa_setup,
        mfa_enable,
        mfa_disable,
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
        backup_now,
        backup_list,
        backup_restore,
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
        contract_expiring,
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
        attendance_decide_correction,
        leave_balances,
        leave_types,
        leave_type_save,
        leave_type_delete,
        leave_my,
        leave_pending,
        leave_all,
        leave_create,
        leave_decide,
        leave_cancel,
        leave_calendar,
        overtime_my,
        overtime_pending,
        overtime_all,
        overtime_create,
        overtime_decide,
        permission_types,
        permission_my,
        permission_pending,
        permission_all,
        permission_create,
        permission_decide,
        payroll_periods,
        payroll_period_create,
        payroll_rows,
        payroll_detail,
        payroll_generate,
        payroll_approve,
        payroll_pay,
        payroll_lock,
        payroll_my_slips,
        payroll_payslip_render,
        payroll_payslip_file,
        payroll_components,
        payroll_component_save,
        payroll_component_delete,
        payroll_deductions,
        payroll_deduction_save,
        payroll_deduction_delete,
        vacancy_list,
        vacancy_get,
        vacancy_save,
        vacancy_delete,
        candidates_by_vacancy,
        candidate_detail,
        candidate_create,
        candidate_delete,
        candidate_stage,
        interview_add,
        interview_decide,
        assessment_add,
        candidate_hire,
        candidate_cv,
        onboarding_list,
        onboarding_get,
        onboarding_mine,
        onboarding_toggle,
        offboarding_list,
        offboarding_my,
        offboarding_get,
        offboarding_create,
        offboarding_decide,
        offboarding_exit_save,
        offboarding_clearance,
        performance_periods,
        performance_period_save,
        performance_period_delete,
        performance_kpis,
        performance_kpi_save,
        performance_kpi_delete,
        performance_reviews,
        performance_my_reviews,
        performance_review_detail,
        performance_ensure_review,
        performance_assign_kpi,
        performance_submit_actual,
        performance_submit_review,
        training_list,
        training_participants,
        training_save,
        training_delete,
        training_add_participant,
        training_participant_status,
        training_certifications,
        training_certification_add,
        training_skill_matrix,
        training_skill_set,
        asset_categories,
        asset_category_save,
        asset_category_delete,
        assets_list,
        asset_detail,
        asset_save,
        asset_delete,
        asset_my,
        asset_assign,
        asset_return,
        asset_maintenance_add,
        asset_mark_available,
        trip_my,
        trip_all,
        trip_pending,
        trip_expenses,
        trip_create,
        trip_decide,
        trip_expense_add,
        trip_settle,
        reimburse_categories,
        reimburse_category_save,
        reimburse_category_delete,
        reimburse_my,
        reimburse_all,
        reimburse_create,
        reimburse_decide,
        dashboard_hr,
        dashboard_me,
        notification_recent,
        notification_all,
        notification_unread,
        notification_mark_read,
        notification_mark_all,
        announcement_visible,
        announcement_list,
        announcement_get,
        announcement_create,
        announcement_delete,
        report_employees,
        report_headcount,
        report_attendance,
        report_leave,
        report_payroll,
        report_pph21_annual,
        report_recruitment,
        report_performance,
        report_contracts,
        report_analytics,
        report_export
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
