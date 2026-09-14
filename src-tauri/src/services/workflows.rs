//! Konfigurasi alur persetujuan per modul.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};

use crate::entities::{approval_step, approval_workflow, role, user};
use crate::to_dto_int;

/// Satu tahap persetujuan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct WorkflowStep {
    pub id: i32,
    pub step_order: i32,
    pub approver_type: String,
    pub role_id: Option<i32>,
    pub user_id: Option<i32>,
    pub role_name: Option<String>,
    pub username: Option<String>,
}

/// Alur beserta tahap-tahapnya.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Workflow {
    pub id: i32,
    pub module: String,
    pub name: String,
    pub is_active: bool,
    pub steps: Vec<WorkflowStep>,
}

const APPROVER_TYPES: &[&str] = &[
    "supervisor",
    "manager",
    "role",
    "specific_user",
    "department_head",
];

pub async fn list(db: &sea_orm::DatabaseConnection) -> Result<Vec<Workflow>, String> {
    let wfs = approval_workflow::Entity::find()
        .order_by_asc(approval_workflow::Column::Module)
        .all(db)
        .await
        .map_err(|e| format!("gagal membaca alur: {e}"))?;
    let mut out = Vec::new();
    for w in wfs {
        let steps = approval_step::Entity::find()
            .filter(approval_step::Column::ApprovalWorkflowId.eq(w.id))
            .order_by_asc(approval_step::Column::StepOrder)
            .all(db)
            .await
            .map_err(|e| format!("gagal membaca tahap: {e}"))?;
        let mut dto_steps = Vec::new();
        for s in steps {
            let role_name = match s.role_id {
                Some(rid) => role::Entity::find_by_id(rid)
                    .one(db)
                    .await
                    .map_err(|e| format!("gagal memuat peran: {e}"))?
                    .map(|r| r.name),
                None => None,
            };
            let username = match s.user_id {
                Some(uid) => user::Entity::find_by_id(uid)
                    .one(db)
                    .await
                    .map_err(|e| format!("gagal memuat pengguna: {e}"))?
                    .map(|u| u.username),
                None => None,
            };
            dto_steps.push(WorkflowStep {
                id: to_dto_int(s.id as i64, "step.id")?,
                step_order: to_dto_int(s.step_order as i64, "step.order")?,
                approver_type: s.approver_type,
                role_id: s.role_id,
                user_id: s.user_id,
                role_name,
                username,
            });
        }
        out.push(Workflow {
            id: to_dto_int(w.id as i64, "workflow.id")?,
            module: w.module,
            name: w.name,
            is_active: w.is_active != 0,
            steps: dto_steps,
        });
    }
    Ok(out)
}

/// Tambah tahap di urutan akhir.
pub async fn add_step(
    db: &sea_orm::DatabaseConnection,
    workflow_id: i64,
    approver_type: &str,
    role_id: Option<i64>,
    user_id: Option<i64>,
) -> Result<i32, String> {
    if !APPROVER_TYPES.contains(&approver_type) {
        return Err("Tipe approver tidak valid.".to_string());
    }
    let exists = approval_workflow::Entity::find_by_id(workflow_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat alur: {e}"))?;
    if exists.is_none() {
        return Err("Alur tidak ditemukan.".to_string());
    }
    if approver_type == "role" {
        match role_id {
            Some(rid) => {
                let found = role::Entity::find_by_id(rid as i32)
                    .one(db)
                    .await
                    .map_err(|e| format!("gagal memeriksa peran: {e}"))?;
                if found.is_none() {
                    return Err("Peran tidak ditemukan.".to_string());
                }
            }
            None => return Err("Tahap peran wajib memilih peran.".to_string()),
        }
    }
    if approver_type == "specific_user" {
        match user_id {
            Some(uid) => {
                let found = user::Entity::find_by_id(uid as i32)
                    .one(db)
                    .await
                    .map_err(|e| format!("gagal memeriksa pengguna: {e}"))?;
                if found.is_none() {
                    return Err("Pengguna tidak ditemukan.".to_string());
                }
            }
            None => return Err("Tahap pengguna wajib memilih pengguna.".to_string()),
        }
    }
    let max_order = approval_step::Entity::find()
        .filter(approval_step::Column::ApprovalWorkflowId.eq(workflow_id as i32))
        .order_by_desc(approval_step::Column::StepOrder)
        .one(db)
        .await
        .map_err(|e| format!("gagal menghitung urutan: {e}"))?
        .map(|s| s.step_order)
        .unwrap_or(0);
    let order = max_order + 1;
    let m = approval_step::ActiveModel {
        approval_workflow_id: Set(workflow_id as i32),
        step_order: Set(order),
        approver_type: Set(approver_type.to_string()),
        role_id: Set(role_id.map(|v| v as i32)),
        user_id: Set(user_id.map(|v| v as i32)),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("gagal menambah tahap: {e}"))?;
    to_dto_int(m.id as i64, "step.id")
}

pub async fn remove_step(db: &sea_orm::DatabaseConnection, step_id: i64) -> Result<(), String> {
    let r = approval_step::Entity::delete_by_id(step_id as i32)
        .exec(db)
        .await
        .map_err(|e| format!("gagal menghapus tahap: {e}"))?;
    if r.rows_affected == 0 {
        return Err("Tahap tidak ditemukan.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_state;

    #[tokio::test]
    async fn alur_lengkap_dan_tambah_hapus_tahap() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let all = list(db).await.expect("list");
        assert_eq!(all.len(), 5);
        let leave = all.iter().find(|w| w.module == "leave").expect("cuti");
        assert_eq!(leave.steps.len(), 3);

        let e = add_step(db, leave.id as i64, "jelangkung", None, None)
            .await
            .expect_err("tipe invalid");
        assert!(e.contains("Tipe approver"));
        let e = add_step(db, leave.id as i64, "role", None, None)
            .await
            .expect_err("peran wajib");
        assert!(e.contains("memilih peran"));

        let role_id = role::Entity::find()
            .filter(role::Column::Name.eq("Finance"))
            .one(db)
            .await
            .expect("peran")
            .expect("ada")
            .id;
        let sid = add_step(db, leave.id as i64, "role", Some(role_id as i64), None)
            .await
            .expect("tambah");
        let again = list(db).await.expect("list");
        let leave2 = again.iter().find(|w| w.module == "leave").expect("cuti");
        assert_eq!(leave2.steps.len(), 4);
        assert_eq!(leave2.steps.last().expect("akhir").step_order, 4);
        remove_step(db, sid as i64).await.expect("hapus");
        assert!(remove_step(db, 999999).await.is_err());
    }
}
