pub mod config;
pub mod db;
pub mod entities;
pub mod schema_sql;
pub mod seed;
pub mod services;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

/// State global: koneksi database, direktori data, dan sesi login (user id).
pub struct AppState {
    pub sea: sea_orm::DatabaseConnection,
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
async fn db_status(state: tauri::State<'_, AppState>) -> Result<DbStatus, String> {
    let tables_sql = schema_sql::count_tables_sql(state.sea.get_database_backend());
    let tables_row = services::sea_raw::q_one(
        &state.sea,
        tables_sql,
        vec![],
        1,
        "menghitung tabel",
    )
    .await?
    .ok_or("hasil hitung tabel kosong.".to_string())?;
    let tables = services::sea_raw::value_i64(&tables_row[0])
        .ok_or("hasil hitung tabel kosong.".to_string())?;
    let users_row = services::sea_raw::q_one(
        &state.sea,
        "SELECT COUNT(*) AS n FROM users".to_string(),
        vec![],
        1,
        "menghitung users",
    )
    .await?
    .ok_or("hasil hitung users kosong.".to_string())?;
    let users = services::sea_raw::value_i64(&users_row[0])
        .ok_or("hasil hitung users kosong.".to_string())?;
    Ok(DbStatus {
        ok: true,
        tables: to_dto_int(tables, "tables")?,
        users: to_dto_int(users, "users")?,
        data_dir: state.data_dir.display().to_string(),
    })
}

/// Pengguna aktif dari sesi (izin dimuat ulang tiap panggilan).
async fn current_actor(
    state: &tauri::State<'_, AppState>,
) -> Result<(i64, SessionUser), String> {
    let uid = state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())?
        .ok_or("Belum login.".to_string())?;
    let user = services::auth::load_session_user(&state.sea, uid)
        .await?
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
async fn require(
    state: &tauri::State<'_, AppState>,
    any_of: &[&str],
) -> Result<(i64, SessionUser), String> {
    let (uid, user) = current_actor(state).await?;
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
async fn login(
    state: tauri::State<'_, AppState>,
    username: String,
    password: String,
) -> Result<services::auth::LoginOk, String> {
    let ok = services::auth::attempt_login(&state.sea, &username, &password).await?;
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
async fn mfa_challenge(
    state: tauri::State<'_, AppState>,
    user_id: i32,
    code: String,
) -> Result<services::auth::LoginOk, String> {
    let user = services::auth::verify_mfa(&state.sea, user_id as i64, &code).await?;
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
async fn mfa_setup(state: tauri::State<'_, AppState>) -> Result<services::auth::MfaSetup, String> {
    let (uid, _) = current_actor(&state).await?;
    services::auth::mfa_setup(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn mfa_enable(state: tauri::State<'_, AppState>, code: String) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::auth::mfa_enable(&state.sea, uid, &code).await
}

#[tauri::command]
#[specta::specta]
async fn mfa_disable(state: tauri::State<'_, AppState>, password: String) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::auth::mfa_disable(&state.sea, uid, &password).await
}

#[tauri::command]
#[specta::specta]
async fn logout(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let uid = state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())?
        .take();
    if let Some(id) = uid {
        services::auth::logout(&state.sea, id).await?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn session_state(state: tauri::State<'_, AppState>) -> Result<Option<SessionUser>, String> {
    let uid = state
        .session
        .lock()
        .map_err(|_| "Sesi terkunci.".to_string())?
        .to_owned();
    match uid {
        None => Ok(None),
        Some(id) => match services::auth::load_session_user(&state.sea, id).await? {
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
async fn change_password(
    state: tauri::State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::auth::change_password(&state.sea, uid, &current_password, &new_password).await
}

#[tauri::command]
#[specta::specta]
async fn request_password_reset(
    state: tauri::State<'_, AppState>,
    email: String,
) -> Result<Option<String>, String> {
    services::auth::request_password_reset(&state.sea, &email).await
}

#[tauri::command]
#[specta::specta]
async fn reset_password(
    state: tauri::State<'_, AppState>,
    token: String,
    new_password: String,
) -> Result<(), String> {
    services::auth::reset_password(&state.sea, &token, &new_password).await
}

#[tauri::command]
#[specta::specta]
async fn list_roles(state: tauri::State<'_, AppState>) -> Result<Vec<services::rbac::Role>, String> {
    require(&state, &["rbac.manage"]).await?;
    services::rbac::list_roles(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn create_role(
    state: tauri::State<'_, AppState>,
    slug: String,
    name: String,
    description: Option<String>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    let id = services::rbac::create_role(&state.sea, &slug, &name, description.as_deref()).await?;
    services::audit::log_sea(&state.sea, Some(uid), "CREATE", "roles", Some(&id.to_string()), None, None, Some(&format!("Peran {name} dibuat"))).await?;
    Ok(id)
}

#[tauri::command]
#[specta::specta]
async fn update_role(
    state: tauri::State<'_, AppState>,
    role_id: i32,
    name: String,
    description: Option<String>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    services::rbac::update_role(&state.sea, role_id as i64, &name, description.as_deref()).await?;
    services::audit::log_sea(&state.sea, Some(uid), "UPDATE", "roles", Some(&role_id.to_string()), None, None, Some(&format!("Peran {name} diperbarui"))).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn delete_role(state: tauri::State<'_, AppState>, role_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    services::rbac::delete_role(&state.sea, role_id as i64).await?;
    services::audit::log_sea(&state.sea, Some(uid), "DELETE", "roles", Some(&role_id.to_string()), None, None, None).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn list_permissions(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::rbac::Permission>, String> {
    require(&state, &["rbac.manage"]).await?;
    services::rbac::list_permissions(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn role_permission_ids(state: tauri::State<'_, AppState>, role_id: i32) -> Result<Vec<i32>, String> {
    require(&state, &["rbac.manage"]).await?;
    services::rbac::role_permission_ids(&state.sea, role_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn sync_role_permissions(
    state: tauri::State<'_, AppState>,
    role_id: i32,
    permission_ids: Vec<i32>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    let ids: Vec<i64> = permission_ids.iter().map(|v| *v as i64).collect();
    services::rbac::sync_role_permissions(&state.sea, role_id as i64, &ids).await?;
    services::audit::log_sea(&state.sea, Some(uid), "UPDATE", "role_permissions", Some(&role_id.to_string()), None, None, None).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn list_users(state: tauri::State<'_, AppState>) -> Result<Vec<services::rbac::UserRow>, String> {
    require(&state, &["rbac.manage"]).await?;
    services::rbac::list_users(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn user_role_ids(state: tauri::State<'_, AppState>, user_id: i32) -> Result<Vec<i32>, String> {
    require(&state, &["rbac.manage"]).await?;
    services::rbac::user_role_ids(&state.sea, user_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn sync_user_roles(
    state: tauri::State<'_, AppState>,
    user_id: i32,
    role_ids: Vec<i32>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    let ids: Vec<i64> = role_ids.iter().map(|v| *v as i64).collect();
    services::rbac::sync_user_roles(&state.sea, uid, user_id as i64, &ids).await?;
    services::audit::log_sea(&state.sea, Some(uid), "UPDATE", "user_roles", Some(&user_id.to_string()), None, None, None).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn toggle_user_status(
    state: tauri::State<'_, AppState>,
    user_id: i32,
    status: String,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    services::rbac::toggle_user_status(&state.sea, uid, user_id as i64, &status).await?;
    services::audit::log_sea(&state.sea, Some(uid), "UPDATE", "users", Some(&user_id.to_string()), None, None, None).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn admin_reset_password(
    state: tauri::State<'_, AppState>,
    user_id: i32,
    new_password: String,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["rbac.manage"]).await?;
    services::rbac::admin_reset_password(&state.sea, user_id as i64, &new_password).await?;
    services::audit::log_sea(&state.sea, Some(uid), "PASSWORD_RESET", "users", Some(&user_id.to_string()), None, None, None).await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn audit_list(
    state: tauri::State<'_, AppState>,
    module: Option<String>,
    limit: Option<i32>,
) -> Result<Vec<services::audit::AuditEntry>, String> {
    require(&state, &["audit.view", "system.manage"]).await?;
    services::audit::list_sea(&state.sea, module.as_deref(), limit.unwrap_or(100) as i64).await
}

#[tauri::command]
#[specta::specta]
async fn get_settings(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::settings::Setting>, String> {
    require(&state, &["settings.manage"]).await?;
    services::repository::SeaOrmSettings(&state.sea)
        .all_settings()
        .await
}

#[tauri::command]
#[specta::specta]
async fn save_settings(
    state: tauri::State<'_, AppState>,
    items: Vec<(String, String)>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["settings.manage"]).await?;
    services::repository::SeaOrmSettings(&state.sea)
        .save_settings(&items)
        .await?;
    let after = serde_json::to_string(
        &items
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<std::collections::BTreeMap<_, _>>(),
    )
    .unwrap_or_default();
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "UPDATE",
        "system_settings",
        None,
        None,
        Some(&after),
        Some("Memperbarui pengaturan sistem"),
    )
    .await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn get_company(
    state: tauri::State<'_, AppState>,
) -> Result<Option<services::settings::Company>, String> {
    require(&state, &["settings.manage"]).await?;
    services::repository::SeaOrmSettings(&state.sea)
        .get_company()
        .await
}

#[tauri::command]
#[specta::specta]
async fn save_company(
    state: tauri::State<'_, AppState>,
    input: services::settings::CompanyInput,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["settings.manage"]).await?;
    let repo = services::repository::SeaOrmSettings(&state.sea);
    let before = repo.get_company().await?;
    let after = repo.save_company(&input).await?;
    let before_json = serde_json::to_string(&before).unwrap_or_default();
    let after_json = serde_json::to_string(&after).unwrap_or_default();
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "UPDATE",
        "company",
        before.as_ref().map(|c| c.id.to_string()).as_deref(),
        Some(&before_json),
        Some(&after_json),
        None,
    )
    .await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn org_entities(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::organization::EntityMeta>, String> {
    require(&state, &["organization.view", "system.manage"]).await?;
    Ok(services::organization::entities())
}

#[tauri::command]
#[specta::specta]
async fn org_list(
    state: tauri::State<'_, AppState>,
    slug: String,
    search: String,
    page: i32,
    per_page: i32,
) -> Result<services::organization::OrgPage, String> {
    require(&state, &["organization.view", "system.manage"]).await?;
    services::organization::list(&state.sea, &slug, &search, page, per_page).await
}

#[tauri::command]
#[specta::specta]
async fn org_get(
    state: tauri::State<'_, AppState>,
    slug: String,
    id: i32,
) -> Result<Option<std::collections::BTreeMap<String, String>>, String> {
    require(&state, &["organization.view", "system.manage"]).await?;
    services::organization::get(&state.sea, &slug, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn org_options(
    state: tauri::State<'_, AppState>,
    slug: String,
    field: String,
) -> Result<Vec<services::organization::Opt>, String> {
    require(&state, &["organization.view", "system.manage"]).await?;
    services::organization::options(&state.sea, &slug, &field).await
}

#[tauri::command]
#[specta::specta]
async fn org_save(
    state: tauri::State<'_, AppState>,
    slug: String,
    id: Option<i32>,
    values: std::collections::BTreeMap<String, String>,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &[
            "organization.create",
            "organization.update",
            "system.manage",
        ],
    ).await?;
    let rid = services::organization::save(&state.sea, &slug, id.map(|v| v as i64), &values).await?;
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        if id.is_some() { "UPDATE" } else { "CREATE" },
        &format!("organization.{slug}"),
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(rid)
}

#[tauri::command]
#[specta::specta]
async fn org_delete(state: tauri::State<'_, AppState>, slug: String, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["organization.delete", "system.manage"]).await?;
    services::organization::delete(&state.sea, &slug, id as i64).await?;
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "DELETE",
        &format!("organization.{slug}"),
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
async fn org_chart(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::organization::CompanyNode>, String> {
    require(&state, &["organization.view", "system.manage"]).await?;
    services::organization::org_chart(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn list_workflows(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::workflows::Workflow>, String> {
    require(&state, &["workflow.manage"]).await?;
    services::workflows::list(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn add_workflow_step(
    state: tauri::State<'_, AppState>,
    workflow_id: i32,
    approver_type: String,
    role_id: Option<i32>,
    user_id: Option<i32>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["workflow.manage"]).await?;
    let id = services::workflows::add_step(
        &state.sea,
        workflow_id as i64,
        &approver_type,
        role_id.map(|v| v as i64),
        user_id.map(|v| v as i64),
    )
    .await?;
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "CREATE",
        "approval_step",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Tahap ditambah ({approver_type})")),
    )
    .await?;
    Ok(id)
}

#[tauri::command]
#[specta::specta]
async fn remove_workflow_step(state: tauri::State<'_, AppState>, step_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["workflow.manage"]).await?;
    services::workflows::remove_step(&state.sea, step_id as i64).await?;
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "DELETE",
        "approval_step",
        Some(&step_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

fn files_dir(state: &tauri::State<AppState>) -> std::path::PathBuf {
    state.data_dir.join("uploads")
}

#[tauri::command]
#[specta::specta]
async fn employee_list(
    state: tauri::State<'_, AppState>,
    search: String,
    filters: services::employees::EmployeeFilter,
    page: i32,
    per_page: i32,
) -> Result<services::employees::EmployeePage, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::list_sea(&state.sea, &search, &filters, page, per_page).await
}

#[tauri::command]
#[specta::specta]
async fn employee_detail(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::employees::EmployeeDetail>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::detail_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_dropdowns(
    state: tauri::State<'_, AppState>,
) -> Result<services::employees::Dropdowns, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::dropdowns_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn employee_create(
    state: tauri::State<'_, AppState>,
    input: services::employees::EmployeeInput,
    photo: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["employee.create", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::employees::create_sea(&state.sea, &dir, uid, &input, photo.as_ref()).await
}

#[tauri::command]
#[specta::specta]
async fn employee_update(
    state: tauri::State<'_, AppState>,
    id: i32,
    input: services::employees::EmployeeInput,
    resign_date: Option<String>,
    photo: Option<services::employees::FileUpload>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["employee.update", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::employees::update_sea(
        &state.sea,
        &dir,
        uid,
        id as i64,
        &input,
        resign_date.as_deref(),
        photo.as_ref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn employee_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["employee.delete", "system.manage"]).await?;
    services::employees::delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_child_types(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::employees::ChildMeta>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    Ok(services::employees::child_types())
}

#[tauri::command]
#[specta::specta]
async fn employee_child_list(
    state: tauri::State<'_, AppState>,
    child: String,
    employee_id: i32,
) -> Result<Vec<std::collections::BTreeMap<String, String>>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::child_list_sea(&state.sea, &child, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_child_save(
    state: tauri::State<'_, AppState>,
    child: String,
    employee_id: i32,
    id: Option<i32>,
    values: std::collections::BTreeMap<String, String>,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["employee.create", "employee.update", "system.manage"],
    ).await?;
    services::employees::child_save_sea(
        &state.sea,
        uid,
        &child,
        employee_id as i64,
        id.map(|v| v as i64),
        &values,
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn employee_child_delete(
    state: tauri::State<'_, AppState>,
    child: String,
    employee_id: i32,
    id: i32,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["employee.delete", "system.manage"]).await?;
    services::employees::child_delete_sea(&state.sea, uid, &child, employee_id as i64, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn contract_expiring(
    state: tauri::State<'_, AppState>,
    days: i32,
) -> Result<Vec<services::employees::ContractAlert>, String> {
    require(&state, &["contract.view", "system.manage"]).await?;
    services::employees::expiring_contracts_sea(&state.sea, days as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_addresses(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
) -> Result<services::employees::Addresses, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::addresses_sea(&state.sea, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_address_save(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
    address_type: String,
    values: std::collections::BTreeMap<String, String>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["employee.update", "system.manage"]).await?;
    services::employees::save_address_sea(&state.sea, uid, employee_id as i64, &address_type, &values).await
}

#[tauri::command]
#[specta::specta]
async fn employee_documents(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
) -> Result<Vec<services::employees::Document>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::documents_sea(&state.sea, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_document_upload(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
    category: String,
    name: String,
    expiry_date: Option<String>,
    file: services::employees::FileUpload,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["employee.create", "employee.update", "system.manage"],
    ).await?;
    let dir = files_dir(&state);
    services::employees::upload_document_sea(
        &state.sea,
        &dir,
        uid,
        employee_id as i64,
        &category,
        &name,
        expiry_date.as_deref(),
        &file,
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn employee_document_bytes(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
    id: i32,
) -> Result<services::employees::DocumentBytes, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::employees::document_bytes_sea(&state.sea, &dir, employee_id as i64, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_document_delete(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
    id: i32,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["employee.delete", "system.manage"]).await?;
    services::employees::delete_document_sea(&state.sea, uid, employee_id as i64, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_salary_current(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
) -> Result<Option<services::employees::SalaryRow>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::salary_current_sea(&state.sea, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_salary_history(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
) -> Result<Vec<services::employees::SalaryRow>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::salary_history_sea(&state.sea, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_salary_components(
    state: tauri::State<'_, AppState>,
    salary_id: i32,
) -> Result<Vec<services::employees::SalaryComponentRow>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::salary_components_sea(&state.sea, salary_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn employee_available_components(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::employees::SalaryComponentRow>, String> {
    require(&state, &["employee.view", "system.manage"]).await?;
    services::employees::available_components_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn employee_set_salary(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
    basic_salary: f64,
    effective_date: String,
    components: Vec<(i32, f64)>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["employee.update", "system.manage"]).await?;
    let comps: Vec<(i64, f64)> = components.iter().map(|(c, a)| (*c as i64, *a)).collect();
    services::employees::set_salary_sea(
        &state.sea,
        uid,
        employee_id as i64,
        basic_salary,
        &effective_date,
        &comps,
    )
    .await
}


async fn my_employee_sea(sea: &sea_orm::DatabaseConnection, user_id: i64) -> Result<i64, String> {
    let row = services::sea_raw::q_one(
        sea,
        "SELECT employee_id FROM users WHERE id = ?1".to_string(),
        vec![services::sea_raw::Value::Int(user_id)],
        1,
        "lib.my_employee",
    )
    .await
    .map_err(|e| format!("gagal memuat akun: {e}"))?;
    let id = row
        .as_ref()
        .and_then(|r| services::sea_raw::value_i64(&r[0]))
        .unwrap_or(0);
    if id == 0 {
        return Err("Akun belum tertaut karyawan.".to_string());
    }
    Ok(id)
}

#[tauri::command]
#[specta::specta]
async fn shift_list(state: tauri::State<'_, AppState>) -> Result<Vec<services::attendance::Shift>, String> {
    require(&state, &["attendance.view", "system.manage"]).await?;
    services::attendance::shift_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn shift_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::attendance::ShiftInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::shift_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn shift_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::shift_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn schedule_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::attendance::Schedule>, String> {
    require(&state, &["attendance.view", "system.manage"]).await?;
    services::attendance::schedule_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn schedule_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::attendance::ScheduleInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::schedule_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn schedule_save_days(
    state: tauri::State<'_, AppState>,
    schedule_id: i32,
    days: Vec<(i32, Option<i32>, bool)>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    let mapped: Vec<(i64, Option<i64>, bool)> = days
        .iter()
        .map(|(d, s, w)| (*d as i64, s.map(|v| v as i64), *w))
        .collect();
    services::attendance::schedule_save_days_sea(&state.sea, uid, schedule_id as i64, &mapped).await
}

#[tauri::command]
#[specta::specta]
async fn schedule_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::schedule_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn assignment_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::attendance::Assignment>, String> {
    require(&state, &["attendance.view", "system.manage"]).await?;
    services::attendance::assignment_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn assignment_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::attendance::AssignmentInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::assignment_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn assignment_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::assignment_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn holiday_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::attendance::Holiday>, String> {
    require(&state, &["attendance.view", "system.manage"]).await?;
    services::attendance::holiday_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn holiday_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::attendance::HolidayInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::holiday_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn holiday_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["attendance.update", "system.manage"]).await?;
    services::attendance::holiday_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_today(
    state: tauri::State<'_, AppState>,
) -> Result<Option<services::attendance::Attendance>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::attendance::today_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_clock_in(
    state: tauri::State<'_, AppState>,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<services::attendance::ClockResult, String> {
    let (uid, _) = current_actor(&state).await?;
    services::attendance::clock_in_sea(
        &state.sea,
        uid,
        my_employee_sea(&state.sea, uid).await?,
        lat,
        lng,
        Some("desktop"),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn attendance_clock_out(
    state: tauri::State<'_, AppState>,
    lat: Option<f64>,
    lng: Option<f64>,
) -> Result<services::attendance::ClockResult, String> {
    let (uid, _) = current_actor(&state).await?;
    services::attendance::clock_out_sea(
        &state.sea,
        uid,
        my_employee_sea(&state.sea, uid).await?,
        lat,
        lng,
        Some("desktop"),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn attendance_history(
    state: tauri::State<'_, AppState>,
    month: String,
) -> Result<Vec<services::attendance::Attendance>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::attendance::history_sea(&state.sea, my_employee_sea(&state.sea, uid).await?, &month).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_recap(
    state: tauri::State<'_, AppState>,
    date: String,
    search: String,
) -> Result<Vec<services::attendance::RecapRow>, String> {
    require(&state, &["attendance.view", "system.manage"]).await?;
    services::attendance::recap_sea(&state.sea, &date, &search).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_manual(
    state: tauri::State<'_, AppState>,
    input: services::attendance::ManualInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["attendance.create", "attendance.correct", "system.manage"],
    ).await?;
    services::attendance::manual_entry_sea(&state.sea, uid, &input).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_my_corrections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::attendance::Correction>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::attendance::my_corrections_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_pending_corrections(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::attendance::Correction>, String> {
    require(&state, &["attendance.approve", "system.manage"]).await?;
    services::attendance::pending_corrections_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_request_correction(
    state: tauri::State<'_, AppState>,
    input: services::attendance::CorrectionInput,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::attendance::request_correction_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, &input).await
}

#[tauri::command]
#[specta::specta]
async fn attendance_decide_correction(
    state: tauri::State<'_, AppState>,
    id: i32,
    decision: String,
    notes: Option<String>,
) -> Result<(), String> {
    let (uid, user) = require(&state, &["attendance.approve", "system.manage"]).await?;
    let actor_emp = my_employee_sea(&state.sea, uid).await.ok();
    let privileged = privileged(&user, "attendance.approve");
    services::attendance::decide_correction_sea(
        &state.sea,
        uid,
        actor_emp,
        privileged,
        id as i64,
        &decision,
        notes.as_deref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn leave_balances(
    state: tauri::State<'_, AppState>,
    year: i32,
) -> Result<Vec<services::leave::Balance>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::leave::balances_sea(&state.sea, my_employee_sea(&state.sea, uid).await?, year).await
}

#[tauri::command]
#[specta::specta]
async fn leave_types(state: tauri::State<'_, AppState>) -> Result<Vec<services::leave::LeaveType>, String> {
    require(&state, &["leave.view", "system.manage"]).await?;
    services::leave::type_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn leave_type_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::leave::LeaveTypeInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["leave.update", "system.manage"]).await?;
    services::leave::type_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn leave_type_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["leave.update", "system.manage"]).await?;
    services::leave::type_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn leave_my(state: tauri::State<'_, AppState>) -> Result<Vec<services::leave::LeaveRequest>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::leave::my_requests_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn leave_pending(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::leave::LeaveRequest>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::leave::pending_for_sea(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn leave_all(state: tauri::State<'_, AppState>) -> Result<Vec<services::leave::LeaveRequest>, String> {
    require(&state, &["leave.view", "system.manage"]).await?;
    services::leave::all_requests_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn leave_create(
    state: tauri::State<'_, AppState>,
    input: services::leave::LeaveCreate,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::leave::create_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, &input).await
}

#[tauri::command]
#[specta::specta]
async fn leave_decide(
    state: tauri::State<'_, AppState>,
    id: i32,
    decision: String,
    notes: Option<String>,
) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::leave::decide_sea(&state.sea, uid, id as i64, &decision, notes.as_deref()).await
}

#[tauri::command]
#[specta::specta]
async fn leave_cancel(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::leave::cancel_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn leave_calendar(
    state: tauri::State<'_, AppState>,
    month: String,
) -> Result<Vec<services::leave::CalendarDay>, String> {
    require(&state, &["leave.view", "system.manage"]).await?;
    services::leave::calendar_sea(&state.sea, &month).await
}

#[tauri::command]
#[specta::specta]
async fn overtime_my(state: tauri::State<'_, AppState>) -> Result<Vec<services::overtime::Overtime>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::overtime::my_requests_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn overtime_pending(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::overtime::Overtime>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::overtime::pending_for_sea(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn overtime_all(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::overtime::Overtime>, String> {
    require(&state, &["overtime.view", "system.manage"]).await?;
    services::overtime::all_requests_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn overtime_create(
    state: tauri::State<'_, AppState>,
    input: services::overtime::OvertimeCreate,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::overtime::create_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, &input).await
}

#[tauri::command]
#[specta::specta]
async fn overtime_decide(
    state: tauri::State<'_, AppState>,
    id: i32,
    decision: String,
    notes: Option<String>,
) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::overtime::decide_sea(&state.sea, uid, id as i64, &decision, notes.as_deref()).await
}

#[tauri::command]
#[specta::specta]
async fn permission_types(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::permission::PermissionType>, String> {
    require(&state, &["permission.view", "system.manage"]).await?;
    services::permission::types_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn permission_my(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::permission::PermissionRequest>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::permission::my_requests_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn permission_pending(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::permission::PermissionRequest>, String> {
    let (uid, user) = current_actor(&state).await?;
    let privileged = privileged(&user, "permission.approve");
    services::permission::pending_for_sea(&state.sea, uid, privileged).await
}

#[tauri::command]
#[specta::specta]
async fn permission_all(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::permission::PermissionRequest>, String> {
    require(&state, &["permission.view", "system.manage"]).await?;
    services::permission::all_requests_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn permission_create(
    state: tauri::State<'_, AppState>,
    input: services::permission::PermissionCreate,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::permission::create_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, &input).await
}

#[tauri::command]
#[specta::specta]
async fn permission_decide(
    state: tauri::State<'_, AppState>,
    id: i32,
    decision: String,
) -> Result<(), String> {
    let (uid, user) = current_actor(&state).await?;
    let actor_emp = my_employee_sea(&state.sea, uid).await.ok();
    let privileged = privileged(&user, "permission.approve");
    services::permission::decide_sea(&state.sea, uid, actor_emp, privileged, id as i64, &decision).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_periods(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::payroll::Period>, String> {
    require(&state, &["payroll.view", "system.manage"]).await?;
    services::payroll::period_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_period_create(
    state: tauri::State<'_, AppState>,
    input: services::payroll::PeriodInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["payroll.create", "system.manage"]).await?;
    services::payroll::period_create_sea(&state.sea, uid, uid, &input).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_rows(
    state: tauri::State<'_, AppState>,
    period_id: i32,
) -> Result<Vec<services::payroll::PayrollRow>, String> {
    require(&state, &["payroll.view", "system.manage"]).await?;
    services::payroll::payrolls_for_period_sea(&state.sea, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_detail(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::payroll::PayrollDetail>, String> {
    require(&state, &["payroll.view", "system.manage"]).await?;
    services::payroll::payroll_detail_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_generate(state: tauri::State<'_, AppState>, period_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["payroll.generate", "system.manage"]).await?;
    services::payroll::generate_sea(&state.sea, uid, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_approve(state: tauri::State<'_, AppState>, period_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["payroll.approve", "system.manage"]).await?;
    services::payroll::approve_period_sea(&state.sea, uid, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_pay(state: tauri::State<'_, AppState>, period_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["payroll.approve", "system.manage"]).await?;
    services::payroll::mark_paid_sea(&state.sea, uid, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_lock(state: tauri::State<'_, AppState>, period_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["payroll.approve", "system.manage"]).await?;
    services::payroll::lock_period_sea(&state.sea, uid, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_my_slips(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::payroll::PayslipInfo>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::payroll::my_payslips_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_payslip_render(
    state: tauri::State<'_, AppState>,
    payroll_id: i32,
) -> Result<String, String> {
    require(&state, &["payroll.view", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::payslip::render_sea(&state.sea, &dir, payroll_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_payslip_file(
    state: tauri::State<'_, AppState>,
    payroll_id: i32,
) -> Result<services::payslip::PayslipFile, String> {
    let (uid, user) = current_actor(&state).await?;
    let owner_row = services::sea_raw::q_one(
        &state.sea,
        "SELECT p.employee_id FROM payrolls p WHERE p.id = ?1".to_string(),
        vec![services::sea_raw::Value::Int(payroll_id as i64)],
        1,
        "lib.payslipowner",
    )
    .await
    .map_err(|e| format!("gagal memuat payroll: {e}"))?;
    let owner = owner_row
        .as_ref()
        .and_then(|r| services::sea_raw::value_i64(&r[0]));
    let mine = my_employee_sea(&state.sea, uid).await.ok();
    let allowed = privileged(&user, "payroll.view") || (mine.is_some() && owner == mine);
    if !allowed {
        return Err("Akses ditolak.".to_string());
    }
    let dir = files_dir(&state);
    services::payslip::read_file_sea(&state.sea, &dir, payroll_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_components(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::payroll::Component>, String> {
    require(&state, &["payroll.view", "system.manage"]).await?;
    services::payroll::component_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_component_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::payroll::ComponentInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["payroll.create", "payroll.update", "system.manage"],
    ).await?;
    services::payroll::component_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_component_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["payroll.delete", "system.manage"]).await?;
    services::payroll::component_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_deductions(
    state: tauri::State<'_, AppState>,
    pending_only: bool,
) -> Result<Vec<services::payroll::Deduction>, String> {
    require(&state, &["payroll.view", "system.manage"]).await?;
    services::payroll::deduction_list_sea(&state.sea, pending_only).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_deduction_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::payroll::DeductionInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["payroll.create", "payroll.update", "system.manage"],
    ).await?;
    services::payroll::deduction_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn payroll_deduction_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["payroll.delete", "system.manage"]).await?;
    services::payroll::deduction_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn vacancy_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::recruitment::Vacancy>, String> {
    require(&state, &["recruitment.view", "system.manage"]).await?;
    services::recruitment::vacancy_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn vacancy_get(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::recruitment::Vacancy>, String> {
    require(&state, &["recruitment.view", "system.manage"]).await?;
    services::recruitment::vacancy_get_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn vacancy_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::recruitment::VacancyInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["recruitment.create", "recruitment.update", "system.manage"],
    ).await?;
    services::recruitment::vacancy_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn vacancy_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["recruitment.delete", "system.manage"]).await?;
    services::recruitment::vacancy_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn candidates_by_vacancy(
    state: tauri::State<'_, AppState>,
    vacancy_id: i32,
) -> Result<Vec<services::recruitment::CandidateRow>, String> {
    require(&state, &["recruitment.view", "system.manage"]).await?;
    services::recruitment::candidates_by_vacancy_sea(&state.sea, vacancy_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn candidate_detail(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::recruitment::CandidateDetail>, String> {
    require(&state, &["recruitment.view", "system.manage"]).await?;
    services::recruitment::candidate_detail_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn candidate_create(
    state: tauri::State<'_, AppState>,
    vacancy_id: i32,
    input: services::recruitment::CandidateInput,
    cv: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["recruitment.create", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::recruitment::candidate_create_sea(
        &state.sea,
        &dir,
        uid,
        vacancy_id as i64,
        &input,
        cv.as_ref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn candidate_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["recruitment.delete", "system.manage"]).await?;
    services::recruitment::candidate_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn candidate_stage(
    state: tauri::State<'_, AppState>,
    id: i32,
    stage: String,
    notes: Option<String>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["recruitment.update", "system.manage"]).await?;
    services::recruitment::update_stage_sea(&state.sea, uid, id as i64, &stage, notes.as_deref()).await
}

#[tauri::command]
#[specta::specta]
async fn interview_add(
    state: tauri::State<'_, AppState>,
    candidate_id: i32,
    input: services::recruitment::InterviewInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["recruitment.update", "system.manage"]).await?;
    services::recruitment::add_interview_sea(&state.sea, uid, candidate_id as i64, &input).await
}

#[tauri::command]
#[specta::specta]
async fn interview_decide(
    state: tauri::State<'_, AppState>,
    id: i32,
    result: String,
    notes: Option<String>,
) -> Result<(), String> {
    let (uid, _) = require(&state, &["recruitment.update", "system.manage"]).await?;
    services::recruitment::decide_interview_sea(&state.sea, uid, id as i64, &result, notes.as_deref()).await
}

#[tauri::command]
#[specta::specta]
async fn assessment_add(
    state: tauri::State<'_, AppState>,
    candidate_id: i32,
    input: services::recruitment::AssessmentInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["recruitment.update", "system.manage"]).await?;
    services::recruitment::add_assessment_sea(&state.sea, uid, candidate_id as i64, &input).await
}

#[tauri::command]
#[specta::specta]
async fn candidate_hire(
    state: tauri::State<'_, AppState>,
    id: i32,
    join_date: Option<String>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["recruitment.update", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::recruitment::hire_sea(&state.sea, &dir, uid, id as i64, join_date.as_deref()).await
}

#[tauri::command]
#[specta::specta]
async fn candidate_cv(
    state: tauri::State<'_, AppState>,
    candidate_id: i32,
    id: i32,
) -> Result<services::employees::DocumentBytes, String> {
    require(&state, &["recruitment.view", "system.manage"]).await?;
    let dir = files_dir(&state);
    services::recruitment::candidate_document_bytes_sea(&state.sea, &dir, candidate_id as i64, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn onboarding_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::onboarding::Onboarding>, String> {
    require(&state, &["onboarding.view", "system.manage"]).await?;
    services::onboarding::list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn onboarding_get(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::onboarding::Onboarding>, String> {
    require(&state, &["onboarding.view", "system.manage"]).await?;
    services::onboarding::find_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn onboarding_mine(
    state: tauri::State<'_, AppState>,
) -> Result<Option<services::onboarding::Onboarding>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::onboarding::for_employee_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn onboarding_toggle(
    state: tauri::State<'_, AppState>,
    task_id: i32,
    completed: bool,
) -> Result<(i32, String), String> {
    let (uid, _) = require(&state,
        &["onboarding.create", "onboarding.update", "system.manage"],
    ).await?;
    services::onboarding::toggle_task_sea(&state.sea, uid, task_id as i64, completed).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::offboarding::OffboardingRow>, String> {
    require(&state, &["offboarding.view", "system.manage"]).await?;
    services::offboarding::list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_my(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::offboarding::OffboardingRow>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::offboarding::my_requests_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_get(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::offboarding::OffboardingDetail>, String> {
    require(&state, &["offboarding.view", "system.manage"]).await?;
    services::offboarding::find_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_create(
    state: tauri::State<'_, AppState>,
    input: services::offboarding::OffboardingCreate,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::offboarding::create_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, &input).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_decide(
    state: tauri::State<'_, AppState>,
    id: i32,
    action: String,
) -> Result<String, String> {
    let (uid, user) = current_actor(&state).await?;
    let actor_emp = my_employee_sea(&state.sea, uid).await.ok();
    let privileged = privileged(&user, "offboarding.approve");
    services::offboarding::decide_sea(&state.sea, uid, actor_emp, privileged, id as i64, &action, None).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_exit_save(
    state: tauri::State<'_, AppState>,
    id: i32,
    input: services::offboarding::ExitInterviewInput,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["offboarding.create", "offboarding.update", "system.manage"],
    ).await?;
    services::offboarding::save_exit_interview_sea(&state.sea, uid, id as i64, &input).await
}

#[tauri::command]
#[specta::specta]
async fn offboarding_clearance(
    state: tauri::State<'_, AppState>,
    item_id: i32,
    cleared: bool,
    notes: Option<String>,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["offboarding.create", "offboarding.update", "system.manage"],
    ).await?;
    services::offboarding::toggle_clearance_sea(&state.sea, uid, item_id as i64, cleared, notes.as_deref()).await
}

#[tauri::command]
#[specta::specta]
async fn performance_periods(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::performance::PerfPeriod>, String> {
    require(&state, &["performance.view", "system.manage"]).await?;
    services::performance::period_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn performance_period_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::performance::PerfPeriodInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["performance.create", "performance.update", "system.manage"],
    ).await?;
    services::performance::period_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn performance_period_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["performance.delete", "system.manage"]).await?;
    services::performance::period_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn performance_kpis(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::performance::Kpi>, String> {
    require(&state, &["performance.view", "system.manage"]).await?;
    services::performance::kpi_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn performance_kpi_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::performance::KpiInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["performance.create", "performance.update", "system.manage"],
    ).await?;
    services::performance::kpi_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn performance_kpi_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["performance.delete", "system.manage"]).await?;
    services::performance::kpi_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn performance_reviews(
    state: tauri::State<'_, AppState>,
    period_id: i32,
) -> Result<Vec<services::performance::ReviewRow>, String> {
    require(&state, &["performance.view", "system.manage"]).await?;
    services::performance::reviews_for_period_sea(&state.sea, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn performance_my_reviews(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::performance::ReviewRow>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::performance::my_reviews_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn performance_review_detail(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::performance::ReviewDetail>, String> {
    require(&state, &["performance.view", "system.manage"]).await?;
    services::performance::review_detail_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn performance_ensure_review(
    state: tauri::State<'_, AppState>,
    period_id: i32,
    employee_id: i32,
) -> Result<i32, String> {
    require(&state,
        &["performance.create", "performance.update", "system.manage"],
    ).await?;
    services::performance::ensure_review_sea(&state.sea, period_id as i64, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn performance_assign_kpi(
    state: tauri::State<'_, AppState>,
    period_id: i32,
    employee_id: i32,
    kpi_id: i32,
    target: f64,
    weight: f64,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["performance.create", "performance.update", "system.manage"],
    ).await?;
    services::performance::assign_kpi_sea(
        &state.sea,
        uid,
        period_id as i64,
        employee_id as i64,
        kpi_id as i64,
        target,
        weight,
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn performance_submit_actual(
    state: tauri::State<'_, AppState>,
    employee_kpi_id: i32,
    actual: f64,
) -> Result<f64, String> {
    let (uid, _) = require(&state,
        &["performance.create", "performance.update", "system.manage"],
    ).await?;
    services::performance::submit_actual_sea(&state.sea, uid, employee_kpi_id as i64, actual).await
}

#[tauri::command]
#[specta::specta]
async fn performance_submit_review(
    state: tauri::State<'_, AppState>,
    review_id: i32,
    role: String,
    score: f64,
    comments: Option<String>,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["performance.create", "performance.review", "system.manage"],
    ).await?;
    services::performance::submit_review_sea(
        &state.sea,
        uid,
        review_id as i64,
        &role,
        score,
        comments.as_deref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn training_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::training::Training>, String> {
    require(&state, &["training.view", "system.manage"]).await?;
    services::training::list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn training_participants(
    state: tauri::State<'_, AppState>,
    training_id: i32,
) -> Result<Vec<services::training::Participant>, String> {
    require(&state, &["training.view", "system.manage"]).await?;
    services::training::detail_participants_sea(&state.sea, training_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn training_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::training::TrainingInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn training_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["training.delete", "system.manage"]).await?;
    services::training::delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn training_add_participant(
    state: tauri::State<'_, AppState>,
    training_id: i32,
    employee_id: i32,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::add_participant_sea(&state.sea, uid, training_id as i64, employee_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn training_participant_status(
    state: tauri::State<'_, AppState>,
    participant_id: i32,
    status: String,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::set_participant_status_sea(&state.sea, uid, participant_id as i64, &status).await
}

#[tauri::command]
#[specta::specta]
async fn training_certifications(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::training::Certification>, String> {
    require(&state, &["training.view", "system.manage"]).await?;
    services::training::certifications_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn training_certification_add(
    state: tauri::State<'_, AppState>,
    input: services::training::CertificationInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::add_certification_sea(&state.sea, uid, &input).await
}

#[tauri::command]
#[specta::specta]
async fn training_materials(
    state: tauri::State<'_, AppState>,
    training_id: i32,
) -> Result<Vec<services::training::Material>, String> {
    require(&state, &["training.view", "system.manage"]).await?;
    services::training::material_list_sea(&state.sea, training_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn training_material_add(
    state: tauri::State<'_, AppState>,
    training_id: i32,
    title: String,
    kind: String,
    url: Option<String>,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::material_add_sea(
        &state.sea,
        uid,
        training_id as i64,
        &title,
        &kind,
        url.as_deref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn training_material_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["training.delete", "system.manage"]).await?;
    services::training::material_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn training_quiz_score(
    state: tauri::State<'_, AppState>,
    participant_id: i32,
    score: f64,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::set_quiz_score_sea(&state.sea, uid, participant_id as i64, score).await
}

#[tauri::command]
#[specta::specta]
async fn training_skill_matrix(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::training::SkillCell>, String> {
    require(&state, &["training.view", "system.manage"]).await?;
    services::training::skill_matrix_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn training_skill_set(
    state: tauri::State<'_, AppState>,
    employee_id: i32,
    skill_name: String,
    level: i32,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["training.create", "training.update", "system.manage"],
    ).await?;
    services::training::set_skill_sea(&state.sea, uid, employee_id as i64, &skill_name, level).await
}

#[tauri::command]
#[specta::specta]
async fn asset_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::assets::Category>, String> {
    require(&state, &["asset.view", "system.manage"]).await?;
    services::assets::category_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn asset_category_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    code: String,
    name: String,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["asset.create", "asset.update", "system.manage"],
    ).await?;
    services::assets::category_save_sea(&state.sea, uid, id.map(|v| v as i64), &code, &name).await
}

#[tauri::command]
#[specta::specta]
async fn asset_category_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["asset.delete", "system.manage"]).await?;
    services::assets::category_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn assets_list(
    state: tauri::State<'_, AppState>,
    search: String,
) -> Result<Vec<services::assets::Asset>, String> {
    require(&state, &["asset.view", "system.manage"]).await?;
    services::assets::asset_list_sea(&state.sea, &search).await
}

#[tauri::command]
#[specta::specta]
async fn asset_detail(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<Option<services::assets::AssetDetail>, String> {
    require(&state, &["asset.view", "system.manage"]).await?;
    services::assets::asset_detail_sea(&state.sea, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn asset_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    input: services::assets::AssetInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["asset.create", "asset.update", "system.manage"],
    ).await?;
    services::assets::asset_save_sea(&state.sea, uid, id.map(|v| v as i64), &input).await
}

#[tauri::command]
#[specta::specta]
async fn asset_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["asset.delete", "system.manage"]).await?;
    services::assets::asset_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn asset_my(state: tauri::State<'_, AppState>) -> Result<Vec<services::assets::MyAsset>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::assets::my_assets_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn asset_assign(
    state: tauri::State<'_, AppState>,
    asset_id: i32,
    input: services::assets::AssignInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["asset.create", "asset.update", "system.manage"],
    ).await?;
    services::assets::assign_sea(&state.sea, uid, asset_id as i64, &input).await
}

#[tauri::command]
#[specta::specta]
async fn asset_return(
    state: tauri::State<'_, AppState>,
    assignment_id: i32,
    input: services::assets::ReturnInput,
) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["asset.create", "asset.update", "system.manage"],
    ).await?;
    services::assets::return_asset_sea(&state.sea, uid, uid, assignment_id as i64, &input).await
}

#[tauri::command]
#[specta::specta]
async fn asset_maintenance_add(
    state: tauri::State<'_, AppState>,
    asset_id: i32,
    input: services::assets::MaintenanceInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state,
        &["asset.create", "asset.update", "system.manage"],
    ).await?;
    services::assets::add_maintenance_sea(&state.sea, uid, asset_id as i64, &input).await
}

#[tauri::command]
#[specta::specta]
async fn asset_mark_available(state: tauri::State<'_, AppState>, asset_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state,
        &["asset.create", "asset.update", "system.manage"],
    ).await?;
    services::assets::mark_available_sea(&state.sea, uid, asset_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn trip_my(state: tauri::State<'_, AppState>) -> Result<Vec<services::travel::Trip>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::travel::my_trips_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn trip_all(state: tauri::State<'_, AppState>) -> Result<Vec<services::travel::Trip>, String> {
    require(&state, &["business_trip.view", "system.manage"]).await?;
    services::travel::all_trips_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn trip_pending(state: tauri::State<'_, AppState>) -> Result<Vec<services::travel::Trip>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::travel::pending_for_sea(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn trip_expenses(
    state: tauri::State<'_, AppState>,
    trip_id: i32,
) -> Result<Vec<services::travel::TripExpense>, String> {
    require(&state, &["business_trip.view", "system.manage"]).await?;
    services::travel::trip_expenses_sea(&state.sea, trip_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn trip_create(
    state: tauri::State<'_, AppState>,
    input: services::travel::TripInput,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::travel::trip_create_sea(&state.sea, uid, my_employee_sea(&state.sea, uid).await?, &input).await
}

#[tauri::command]
#[specta::specta]
async fn trip_decide(state: tauri::State<'_, AppState>, id: i32, decision: String) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::travel::trip_decide_sea(&state.sea, uid, id as i64, &decision).await
}

#[tauri::command]
#[specta::specta]
async fn trip_expense_add(
    state: tauri::State<'_, AppState>,
    trip_id: i32,
    input: services::travel::TripExpenseInput,
    receipt: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    let dir = files_dir(&state);
    services::travel::trip_add_expense_sea(&state.sea, &dir, uid, trip_id as i64, &input, receipt.as_ref()).await
}

#[tauri::command]
#[specta::specta]
async fn trip_settle(state: tauri::State<'_, AppState>, trip_id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["business_trip.approve", "system.manage"]).await?;
    services::travel::trip_settle_sea(&state.sea, uid, trip_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::travel::ReimburseCategory>, String> {
    require(&state, &["reimbursement.view", "system.manage"]).await?;
    services::travel::category_list_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_category_save(
    state: tauri::State<'_, AppState>,
    id: Option<i32>,
    code: String,
    name: String,
    max_amount: Option<f64>,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["system.manage"]).await?;
    services::travel::category_save_sea(&state.sea, uid, id.map(|v| v as i64), &code, &name, max_amount).await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_category_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["system.manage"]).await?;
    services::travel::category_delete_sea(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_my(state: tauri::State<'_, AppState>) -> Result<Vec<services::travel::Reimburse>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::travel::my_reimburse_sea(&state.sea, my_employee_sea(&state.sea, uid).await?).await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_all(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::travel::Reimburse>, String> {
    require(&state, &["reimbursement.view", "system.manage"]).await?;
    services::travel::all_reimburse_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_create(
    state: tauri::State<'_, AppState>,
    input: services::travel::ReimburseInput,
    receipt: Option<services::employees::FileUpload>,
) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    let dir = files_dir(&state);
    services::travel::reimburse_create_sea(
        &state.sea,
        &dir,
        uid,
        my_employee_sea(&state.sea, uid).await?,
        &input,
        receipt.as_ref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn reimburse_decide(
    state: tauri::State<'_, AppState>,
    id: i32,
    action: String,
) -> Result<String, String> {
    let (uid, user) = current_actor(&state).await?;
    let actor_emp = my_employee_sea(&state.sea, uid).await.ok();
    let privileged = privileged(&user, "reimbursement.approve");
    services::travel::reimburse_decide_sea(&state.sea, uid, actor_emp, privileged, id as i64, &action).await
}

// ---------------- Dasbor ----------------

#[tauri::command]
#[specta::specta]
async fn dashboard_hr(state: tauri::State<'_, AppState>) -> Result<services::dashboard::HrDashboard, String> {
    require(&state,
        &["employee.view", "payroll.view", "system.manage"],
    ).await?;
    services::dashboard::hr_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn dashboard_me(state: tauri::State<'_, AppState>) -> Result<services::dashboard::MySummary, String> {
    let (uid, _) = current_actor(&state).await?;
    services::dashboard::mine_sea(&state.sea, uid).await
}

// ---------------- Notifikasi ----------------

#[tauri::command]
#[specta::specta]
async fn notification_recent(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::notifications::Notification>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::notifications::recent(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn notification_all(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::notifications::Notification>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::notifications::all(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn notification_unread(state: tauri::State<'_, AppState>) -> Result<i32, String> {
    let (uid, _) = current_actor(&state).await?;
    services::notifications::unread_count(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn notification_mark_read(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::notifications::mark_read(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn notification_mark_all(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let (uid, _) = current_actor(&state).await?;
    services::notifications::mark_all(&state.sea, uid).await
}

// ---------------- Pengumuman ----------------

#[tauri::command]
#[specta::specta]
async fn announcement_visible(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::announcements::Announcement>, String> {
    let (uid, _) = current_actor(&state).await?;
    services::announcements::visible(&state.sea, uid).await
}

#[tauri::command]
#[specta::specta]
async fn announcement_list(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::announcements::Announcement>, String> {
    require(&state, &["announcement.view", "system.manage"]).await?;
    services::announcements::all(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn announcement_get(
    state: tauri::State<'_, AppState>,
    id: i32,
) -> Result<services::announcements::Announcement, String> {
    let (uid, _) = current_actor(&state).await?;
    services::announcements::get(&state.sea, uid, id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn announcement_create(
    state: tauri::State<'_, AppState>,
    input: services::announcements::AnnouncementInput,
) -> Result<i32, String> {
    let (uid, _) = require(&state, &["announcement.create", "system.manage"]).await?;
    let id = services::announcements::create(&state.sea, uid, &input).await?;
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "CREATE",
        "announcement",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(id)
}

#[tauri::command]
#[specta::specta]
async fn announcement_delete(state: tauri::State<'_, AppState>, id: i32) -> Result<(), String> {
    let (uid, _) = require(&state, &["announcement.delete", "system.manage"]).await?;
    services::announcements::delete(&state.sea, id as i64).await?;
    services::audit::log_sea(
        &state.sea,
        Some(uid),
        "DELETE",
        "announcement",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

// ---------------- Laporan ----------------

#[tauri::command]
#[specta::specta]
async fn report_employees(
    state: tauri::State<'_, AppState>,
    search: Option<String>,
    department_id: Option<i32>,
    status: Option<String>,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::employees_sea(
        &state.sea,
        search.as_deref(),
        department_id.map(|v| v as i64),
        status.as_deref(),
    )
    .await
}

#[tauri::command]
#[specta::specta]
async fn report_headcount(
    state: tauri::State<'_, AppState>,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::headcount_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn report_attendance(
    state: tauri::State<'_, AppState>,
    month: String,
    department_id: Option<i32>,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::attendance_sea(&state.sea, &month, department_id.map(|v| v as i64)).await
}

#[tauri::command]
#[specta::specta]
async fn report_leave(
    state: tauri::State<'_, AppState>,
    year: i32,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::leave_sea(&state.sea, year).await
}

#[tauri::command]
#[specta::specta]
async fn report_payroll(
    state: tauri::State<'_, AppState>,
    period_id: i32,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::payroll_sea(&state.sea, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn report_pph21_annual(
    state: tauri::State<'_, AppState>,
    year: i32,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::pph21_annual_sea(&state.sea, year).await
}

#[tauri::command]
#[specta::specta]
async fn report_recruitment(
    state: tauri::State<'_, AppState>,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::recruitment_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn report_performance(
    state: tauri::State<'_, AppState>,
    period_id: i32,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::performance_sea(&state.sea, period_id as i64).await
}

#[tauri::command]
#[specta::specta]
async fn report_contracts(
    state: tauri::State<'_, AppState>,
    before: String,
) -> Result<services::reports::ReportTable, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::contracts_sea(&state.sea, &before).await
}

#[tauri::command]
#[specta::specta]
async fn report_analytics(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<services::reports::DeptStat>, String> {
    require(&state, &["report.view", "system.manage"]).await?;
    services::reports::analytics_sea(&state.sea).await
}

#[tauri::command]
#[specta::specta]
async fn report_export(
    state: tauri::State<'_, AppState>,
    kind: String,
    format: String,
    arg1: Option<String>,
    arg2: Option<i32>,
) -> Result<services::reports::ExportFile, String> {
    require(&state, &["report.export", "system.manage"]).await?;
    services::reports::export_sea(
        &state.sea,
        &kind,
        &format,
        arg1.as_deref(),
        arg2.map(|v| v as i64),
    )
    .await
}

fn init_state(data_dir: PathBuf) -> Result<AppState, String> {
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("gagal membuat direktori data: {e}"))?;
    let cfg = config::load(&data_dir)?;
    // Konek + migrasi + seed di thread terpisah agar aman dipanggil dari dalam runtime async (mis. test).
    let sea = std::thread::scope(|s| {
        s.spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("gagal membuat runtime async: {e}"))?;
            rt.block_on(async {
                let sea = db::connect_sea(&cfg, &data_dir).await?;
                db::migrate_sea(&sea).await?;
                seed::seed(&sea).await?;
                services::backup::ensure_scheduled_sea(
                    &sea,
                    &services::backup::backup_dir(&data_dir),
                )
                .await;
                Ok::<_, String>(sea)
            })
        })
        .join()
        .map_err(|_| "thread koneksi SeaORM panik.".to_string())?
    })?;
    Ok(AppState {
        sea,
        data_dir,
        session: Mutex::new(None),
    })
}

#[tauri::command]
#[specta::specta]
async fn backup_now(state: tauri::State<'_, AppState>) -> Result<String, String> {
    require(&state, &["system.manage"]).await?;
    services::backup::backup_now_sea(&state.sea, &services::backup::backup_dir(&state.data_dir)).await
}

#[tauri::command]
#[specta::specta]
async fn backup_list(state: tauri::State<'_, AppState>) -> Result<Vec<services::backup::BackupFile>, String> {
    require(&state, &["system.manage"]).await?;
    services::backup::backup_list(&services::backup::backup_dir(&state.data_dir))
}

#[tauri::command]
#[specta::specta]
async fn backup_restore(state: tauri::State<'_, AppState>, name: String) -> Result<String, String> {
    require(&state, &["system.manage"]).await?;
    services::backup::backup_restore_sea(
        &state.data_dir.join("peoplex.db"),
        &services::backup::backup_dir(&state.data_dir),
        &name,
    )?;
    Ok("Cadangan dipulihkan. Mulai ulang aplikasi untuk memakai data pulihan.".to_string())
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
        training_materials,
        training_material_add,
        training_material_delete,
        training_quiz_score,
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

    #[tokio::test]
    async fn init_state_menyiapkan_db_lengkap() {
        use services::sea_raw::{q_one, Value};
        let dir = tempfile::tempdir().expect("tempdir");
        let state = init_state(dir.path().to_path_buf()).expect("init_state");
        assert!(dir.path().join("peoplex.db").exists());
        assert_eq!(state.data_dir, dir.path());
        assert!(state.session.lock().unwrap().is_none());
        use sea_orm::ConnectionTrait as _;
        let tables = q_one(
            &state.sea,
            schema_sql::count_tables_sql(state.sea.get_database_backend()),
            vec![],
            1,
            "hitung tabel",
        )
        .await
        .expect("tabel")
        .expect("baris");
        let admin = q_one(
            &state.sea,
            "SELECT COUNT(*) FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "hitung admin",
        )
        .await
        .expect("admin")
        .expect("baris");
        match (&tables[0], &admin[0]) {
            (Value::Int(t), Value::Int(a)) => {
                assert_eq!(t, &89);
                assert_eq!(a, &1);
            }
            other => panic!("tipe tak terduga: {other:?}"),
        }
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
