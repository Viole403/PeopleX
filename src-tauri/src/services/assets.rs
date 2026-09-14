//! Aset: kategori, inventaris, penugasan, pengembalian, maintenance.

use chrono::NaiveDate;

use super::audit;
use crate::to_dto_int;

const CONDITIONS: &[&str] = &["new", "good", "fair", "damaged", "lost"];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Category {
    pub id: i32,
    pub code: String,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Asset {
    pub id: i32,
    pub asset_code: String,
    pub name: String,
    pub asset_category_id: i32,
    pub category_name: String,
    pub brand: Option<String>,
    pub serial_number: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub condition_status: String,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct AssetInput {
    pub asset_code: String,
    pub name: String,
    pub asset_category_id: i32,
    pub brand: Option<String>,
    pub serial_number: Option<String>,
    pub purchase_date: Option<String>,
    pub purchase_price: Option<f64>,
    pub condition_status: String,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct AssetAssignment {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub assigned_date: String,
    pub returned_date: Option<String>,
    pub condition_on_assign: Option<String>,
    pub condition_on_return: Option<String>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct AssignInput {
    pub employee_id: i32,
    pub assigned_date: String,
    pub condition_on_assign: Option<String>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct ReturnInput {
    pub returned_date: String,
    pub condition_on_return: Option<String>,
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Maintenance {
    pub id: i32,
    pub maintenance_date: String,
    pub description: String,
    pub cost: f64,
    pub performed_by: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct MaintenanceInput {
    pub maintenance_date: String,
    pub description: String,
    pub cost: f64,
    pub performed_by: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct AssetDetail {
    pub asset: Asset,
    pub assignments: Vec<AssetAssignment>,
    pub maintenance: Vec<Maintenance>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct MyAsset {
    pub assignment_id: i32,
    pub asset_code: String,
    pub name: String,
    pub brand: Option<String>,
    pub category_name: String,
    pub assigned_date: String,
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};

fn aopt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

fn aopt_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse().ok(),
    }
}

async fn arow_id(db: &sea_orm::DatabaseConnection, label: &str) -> Result<i64, String> {
    let row = q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        label,
    )
    .await
    .map_err(|e| format!("gagal membaca id baru: {e}"))?;
    Ok(row.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0))
}

pub async fn category_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<Category>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name FROM asset_categories WHERE deleted_at IS NULL ORDER BY name"
            .to_string(),
        vec![],
        3,
        "asset.cats",
    )
    .await
    .map_err(|e| format!("gagal membaca kategori: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(Category {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "category.id")?,
            code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
        });
    }
    Ok(out)
}

pub async fn category_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    code: &str,
    name: &str,
) -> Result<i32, String> {
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err("Kode dan nama kategori wajib diisi.".to_string());
    }
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE asset_categories SET code = ?1, name = ?2 WHERE id = ?3 AND deleted_at IS NULL"
                .to_string(),
            vec![
                Value::Text(code.trim().to_string()),
                Value::Text(name.trim().to_string()),
                Value::Int(rid),
            ],
            "asset.catupd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan kategori: {e}"))?;
        if n == 0 {
            return Err("Kategori tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "asset.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "category.id")
    } else {
        let res = exec(
            db,
            "INSERT INTO asset_categories (code, name) VALUES (?1, ?2)".to_string(),
            vec![
                Value::Text(code.trim().to_string()),
                Value::Text(name.trim().to_string()),
            ],
            "asset.catadd",
        )
        .await;
        let Err(e) = res else {
            let rid = arow_id(db, "asset.catadd").await?;
            audit::log_sea(
                db,
                Some(actor_id),
                "CREATE",
                "asset.category",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await?;
            return to_dto_int(rid, "category.id");
        };
        if e.contains("UNIQUE") {
            return Err("Kode kategori sudah dipakai.".to_string());
        }
        return Err(format!("gagal menambah kategori: {e}"));
    }
}

pub async fn category_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM assets WHERE asset_category_id = ?1 AND deleted_at IS NULL"
            .to_string(),
        vec![Value::Int(id)],
        1,
        "asset.catrel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Kategori masih dipakai aset.".to_string());
    }
    let d = exec(
        db,
        "UPDATE asset_categories SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "asset.catdel",
    )
    .await
    .map_err(|e| format!("gagal menghapus kategori: {e}"))?;
    if d == 0 {
        return Err("Kategori tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "asset.category",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

fn map_asset_row(r: &[Value]) -> Result<Asset, String> {
    Ok(Asset {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "asset.id")?,
        asset_code: value_to_string(&r[1]),
        name: value_to_string(&r[2]),
        asset_category_id: to_dto_int(value_i64(&r[3]).unwrap_or(0), "asset.cat")?,
        category_name: value_to_string(&r[4]),
        brand: aopt_text(&r[5]),
        serial_number: aopt_text(&r[6]),
        purchase_date: aopt_text(&r[7]),
        purchase_price: aopt_f64(&r[8]),
        condition_status: value_to_string(&r[9]),
        status: value_to_string(&r[10]),
    })
}

pub async fn asset_list_sea(
    db: &sea_orm::DatabaseConnection,
    search: &str,
) -> Result<Vec<Asset>, String> {
    let search = search.trim();
    let base = "SELECT a.id, a.asset_code, a.name, a.asset_category_id, c.name, a.brand, a.serial_number, a.purchase_date, a.purchase_price, a.condition_status, a.status FROM assets a INNER JOIN asset_categories c ON c.id = a.asset_category_id WHERE a.deleted_at IS NULL";
    let (sql, vals) = if search.is_empty() {
        (format!("{base} ORDER BY a.name"), vec![])
    } else {
        (
            format!("{base} AND (a.asset_code LIKE ?1 OR a.name LIKE ?1 OR a.serial_number LIKE ?1) ORDER BY a.name"),
            vec![Value::Text(format!("%{search}%"))],
        )
    };
    let rows = q_all(db, sql, vals, 11, "asset.list")
        .await
        .map_err(|e| format!("gagal membaca aset: {e}"))?;
    rows.iter().map(|r| map_asset_row(r)).collect()
}

pub async fn asset_detail_sea(
    db: &sea_orm::DatabaseConnection,
    id: i64,
) -> Result<Option<AssetDetail>, String> {
    let asset = asset_list_sea(db, "")
        .await?
        .into_iter()
        .find(|a| a.id as i64 == id);
    let Some(asset) = asset else {
        let exists = q_one(
            db,
            "SELECT id FROM assets WHERE id = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::Int(id)],
            1,
            "asset.excheck",
        )
        .await
        .map_err(|e| format!("gagal memeriksa aset: {e}"))?;
        if exists.is_none() {
            return Ok(None);
        }
        return Err("Gagal memuat aset.".to_string());
    };
    let mut assignments = Vec::new();
    let arows = q_all(
        db,
        "SELECT aa.id, aa.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, aa.assigned_date, aa.returned_date, aa.condition_on_assign, aa.condition_on_return, aa.notes FROM asset_assignments aa INNER JOIN employees e ON e.id = aa.employee_id WHERE aa.asset_id = ?1 ORDER BY aa.assigned_date DESC".to_string(),
        vec![Value::Int(id)],
        9,
        "asset.assigns",
    )
    .await
    .map_err(|e| format!("gagal membaca riwayat: {e}"))?;
    for a in &arows {
        assignments.push(AssetAssignment {
            id: to_dto_int(value_i64(&a[0]).unwrap_or(0), "assign.id")?,
            employee_id: to_dto_int(value_i64(&a[1]).unwrap_or(0), "assign.emp")?,
            employee_name: value_to_string(&a[2]),
            employee_number: value_to_string(&a[3]),
            assigned_date: value_to_string(&a[4]),
            returned_date: aopt_text(&a[5]),
            condition_on_assign: aopt_text(&a[6]),
            condition_on_return: aopt_text(&a[7]),
            notes: aopt_text(&a[8]),
        });
    }
    let mut maintenance = Vec::new();
    let mrows = q_all(
        db,
        "SELECT id, maintenance_date, description, cost, performed_by FROM asset_maintenance WHERE asset_id = ?1 ORDER BY maintenance_date DESC".to_string(),
        vec![Value::Int(id)],
        5,
        "asset.maints",
    )
    .await
    .map_err(|e| format!("gagal membaca maintenance: {e}"))?;
    for m in &mrows {
        maintenance.push(Maintenance {
            id: to_dto_int(value_i64(&m[0]).unwrap_or(0), "maint.id")?,
            maintenance_date: value_to_string(&m[1]),
            description: value_to_string(&m[2]),
            cost: aopt_f64(&m[3]).unwrap_or(0.0),
            performed_by: aopt_text(&m[4]),
        });
    }
    Ok(Some(AssetDetail {
        asset,
        assignments,
        maintenance,
    }))
}

pub async fn asset_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &AssetInput,
) -> Result<i32, String> {
    if input.asset_code.trim().is_empty() || input.name.trim().is_empty() {
        return Err("Kode dan nama aset wajib diisi.".to_string());
    }
    if !CONDITIONS.contains(&input.condition_status.as_str()) {
        return Err("Kondisi tidak valid.".to_string());
    }
    if !["available", "assigned", "maintenance", "disposed"].contains(&input.status.as_str()) {
        return Err("Status tidak valid.".to_string());
    }
    let cat = q_one(
        db,
        "SELECT id FROM asset_categories WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(input.asset_category_id as i64)],
        1,
        "asset.catchk",
    )
    .await
    .map_err(|e| format!("gagal memeriksa kategori: {e}"))?;
    if cat.is_none() {
        return Err("Kategori tidak ditemukan.".to_string());
    }
    if let Some(pd) = input
        .purchase_date
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        NaiveDate::parse_from_str(pd, "%Y-%m-%d")
            .map_err(|_| "Tanggal beli harus valid (YYYY-MM-DD).".to_string())?;
    }
    if let Some(price) = input.purchase_price {
        if !(price >= 0.0) {
            return Err("Harga minimal 0.".to_string());
        }
    }
    let opt_t = |v: Option<&str>| match v {
        Some(s) => Value::Text(s.to_string()),
        None => Value::Null,
    };
    if let Some(rid) = id {
        let n = exec(
            db,
            "UPDATE assets SET asset_code = ?1, name = ?2, asset_category_id = ?3, brand = ?4, serial_number = ?5, purchase_date = ?6, purchase_price = ?7, condition_status = ?8, status = ?9 WHERE id = ?10 AND deleted_at IS NULL".to_string(),
            vec![
                Value::Text(input.asset_code.trim().to_string()),
                Value::Text(input.name.trim().to_string()),
                Value::Int(input.asset_category_id as i64),
                opt_t(input.brand.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.serial_number.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.purchase_date.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                match input.purchase_price {
                    Some(v) => Value::Float(v),
                    None => Value::Null,
                },
                Value::Text(input.condition_status.clone()),
                Value::Text(input.status.clone()),
                Value::Int(rid),
            ],
            "asset.upd",
        )
        .await
        .map_err(|e| format!("gagal menyimpan aset: {e}"))?;
        if n == 0 {
            return Err("Aset tidak ditemukan.".to_string());
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "asset",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        to_dto_int(rid, "asset.id")
    } else {
        let res = exec(
            db,
            "INSERT INTO assets (asset_code, name, asset_category_id, brand, serial_number, purchase_date, purchase_price, condition_status, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available')".to_string(),
            vec![
                Value::Text(input.asset_code.trim().to_string()),
                Value::Text(input.name.trim().to_string()),
                Value::Int(input.asset_category_id as i64),
                opt_t(input.brand.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.serial_number.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                opt_t(input.purchase_date.as_deref().map(str::trim).filter(|s| !s.is_empty())),
                match input.purchase_price {
                    Some(v) => Value::Float(v),
                    None => Value::Null,
                },
                Value::Text(input.condition_status.clone()),
            ],
            "asset.add",
        )
        .await;
        let Err(e) = res else {
            let rid = arow_id(db, "asset.add").await?;
            audit::log_sea(
                db,
                Some(actor_id),
                "CREATE",
                "asset",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await?;
            return to_dto_int(rid, "asset.id");
        };
        if e.contains("UNIQUE") {
            return Err("Kode aset sudah dipakai.".to_string());
        }
        return Err(format!("gagal menambah aset: {e}"));
    }
}

pub async fn asset_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let rel = q_one(
        db,
        "SELECT COUNT(*) FROM asset_assignments WHERE asset_id = ?1 AND returned_date IS NULL"
            .to_string(),
        vec![Value::Int(id)],
        1,
        "asset.assignrel",
    )
    .await
    .map_err(|e| format!("gagal memeriksa penugasan: {e}"))?;
    if rel.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0) > 0 {
        return Err("Aset sedang ditugaskan.".to_string());
    }
    let d = exec(
        db,
        "UPDATE assets SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        "asset.del",
    )
    .await
    .map_err(|e| format!("gagal menghapus aset: {e}"))?;
    if d == 0 {
        return Err("Aset tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "asset",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn my_assets_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<MyAsset>, String> {
    let rows = q_all(
        db,
        "SELECT aa.id, a.asset_code, a.name, a.brand, c.name, aa.assigned_date FROM asset_assignments aa INNER JOIN assets a ON a.id = aa.asset_id INNER JOIN asset_categories c ON c.id = a.asset_category_id WHERE aa.employee_id = ?1 AND aa.returned_date IS NULL ORDER BY aa.assigned_date DESC".to_string(),
        vec![Value::Int(employee_id)],
        6,
        "asset.mine",
    )
    .await
    .map_err(|e| format!("gagal membaca aset: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(MyAsset {
            assignment_id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "myasset.id")?,
            asset_code: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
            brand: aopt_text(&r[3]),
            category_name: value_to_string(&r[4]),
            assigned_date: value_to_string(&r[5]),
        });
    }
    Ok(out)
}

/// Tugaskan aset available; kondisi awal default kondisi aset saat ini.
pub async fn assign_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    asset_id: i64,
    input: &AssignInput,
) -> Result<i32, String> {
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(input.employee_id as i64)],
        1,
        "asset.empcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    NaiveDate::parse_from_str(input.assigned_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal tugaskan tidak valid.".to_string())?;
    let asset = q_one(
        db,
        "SELECT status, condition_status FROM assets WHERE id = ?1 AND deleted_at IS NULL"
            .to_string(),
        vec![Value::Int(asset_id)],
        2,
        "asset.check",
    )
    .await
    .map_err(|e| format!("gagal memuat aset: {e}"))?;
    let Some(asset) = asset else {
        return Err("Aset tidak ditemukan.".to_string());
    };
    let (status, condition) = (value_to_string(&asset[0]), value_to_string(&asset[1]));
    if status != "available" {
        return Err("Aset tidak tersedia untuk ditugaskan.".to_string());
    }
    let cond = input
        .condition_on_assign
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&condition);
    exec(
        db,
        "INSERT INTO asset_assignments (asset_id, employee_id, assigned_date, condition_on_assign, notes) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(asset_id),
            Value::Int(input.employee_id as i64),
            Value::Text(input.assigned_date.trim().to_string()),
            Value::Text(cond.to_string()),
            match input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
        ],
        "asset.assign",
    )
    .await
    .map_err(|e| format!("gagal menugaskan aset: {e}"))?;
    let rid = arow_id(db, "asset.assign").await?;
    exec(
        db,
        "UPDATE assets SET status = 'assigned' WHERE id = ?1".to_string(),
        vec![Value::Int(asset_id)],
        "asset.markassigned",
    )
    .await
    .map_err(|e| format!("gagal mengubah status aset: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "asset.assignment",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "assign.id")
}

/// Kembalikan: rusak jadi maintenance, selain itu available.
pub async fn return_asset_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    verified_by: i64,
    assignment_id: i64,
    input: &ReturnInput,
) -> Result<(), String> {
    let row = q_one(
        db,
        "SELECT asset_id, returned_date FROM asset_assignments WHERE id = ?1".to_string(),
        vec![Value::Int(assignment_id)],
        2,
        "asset.retcheck",
    )
    .await
    .map_err(|e| format!("gagal memuat penugasan: {e}"))?;
    let Some(row) = row else {
        return Err("Penugasan tidak ditemukan.".to_string());
    };
    let asset_id = value_i64(&row[0]).unwrap_or(0);
    if aopt_text(&row[1]).is_some() {
        return Err("Sudah dikembalikan.".to_string());
    }
    NaiveDate::parse_from_str(input.returned_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal kembali tidak valid.".to_string())?;
    let cond = input
        .condition_on_return
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("good");
    if !CONDITIONS.contains(&cond) {
        return Err("Kondisi tidak valid.".to_string());
    }
    exec(
        db,
        "UPDATE asset_assignments SET returned_date = ?1, condition_on_return = ?2, notes = COALESCE(?3, notes), verified_by = ?4 WHERE id = ?5".to_string(),
        vec![
            Value::Text(input.returned_date.trim().to_string()),
            Value::Text(cond.to_string()),
            match input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
            Value::Int(verified_by),
            Value::Int(assignment_id),
        ],
        "asset.return",
    )
    .await
    .map_err(|e| format!("gagal mengembalikan: {e}"))?;
    let status = if cond == "damaged" {
        "maintenance"
    } else {
        "available"
    };
    exec(
        db,
        "UPDATE assets SET status = ?1, condition_status = ?2 WHERE id = ?3".to_string(),
        vec![
            Value::Text(status.to_string()),
            Value::Text(cond.to_string()),
            Value::Int(asset_id),
        ],
        "asset.retstatus",
    )
    .await
    .map_err(|e| format!("gagal mengubah status aset: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "asset.return",
        Some(&assignment_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn add_maintenance_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    asset_id: i64,
    input: &MaintenanceInput,
) -> Result<i32, String> {
    let exists = q_one(
        db,
        "SELECT id FROM assets WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(asset_id)],
        1,
        "asset.maintcheck",
    )
    .await
    .map_err(|e| format!("gagal memeriksa aset: {e}"))?;
    if exists.is_none() {
        return Err("Aset tidak ditemukan.".to_string());
    }
    NaiveDate::parse_from_str(input.maintenance_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal maintenance tidak valid.".to_string())?;
    if input.description.trim().is_empty() {
        return Err("Deskripsi wajib diisi.".to_string());
    }
    if input.description.len() > 255 {
        return Err("Deskripsi maksimal 255 karakter.".to_string());
    }
    if !(input.cost >= 0.0) {
        return Err("Biaya minimal 0.".to_string());
    }
    exec(
        db,
        "INSERT INTO asset_maintenance (asset_id, maintenance_date, description, cost, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(asset_id),
            Value::Text(input.maintenance_date.trim().to_string()),
            Value::Text(input.description.trim().to_string()),
            Value::Float(input.cost),
            match input.performed_by.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => Value::Text(s.to_string()),
                None => Value::Null,
            },
        ],
        "asset.maintadd",
    )
    .await
    .map_err(|e| format!("gagal mencatat maintenance: {e}"))?;
    let rid = arow_id(db, "asset.maintadd").await?;
    exec(
        db,
        "UPDATE assets SET status = 'maintenance' WHERE id = ?1".to_string(),
        vec![Value::Int(asset_id)],
        "asset.markmaint",
    )
    .await
    .map_err(|e| format!("gagal mengubah status: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "asset.maintenance",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "maint.id")
}

pub async fn mark_available_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    asset_id: i64,
) -> Result<(), String> {
    let n = exec(
        db,
        "UPDATE assets SET status = 'available' WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(asset_id)],
        "asset.available",
    )
    .await
    .map_err(|e| format!("gagal mengubah status: {e}"))?;
    if n == 0 {
        return Err("Aset tidak ditemukan.".to_string());
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "asset",
        Some(&asset_id.to_string()),
        None,
        None,
        Some("Status menjadi available"),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn one(db: &sea_orm::DatabaseConnection, sql: &str, label: &str) -> i64 {
        q_one(db, sql.to_string(), vec![], 1, label)
            .await
            .expect("one")
            .and_then(|r| value_i64(&r[0]))
            .expect("id")
    }

    async fn state() -> (tempfile::TempDir, crate::AppState) {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        (dir, state)
    }

    fn aset_input(cat: i32) -> AssetInput {
        AssetInput {
            asset_code: "LT-001".to_string(),
            name: "ThinkPad".to_string(),
            asset_category_id: cat,
            brand: Some("Lenovo".to_string()),
            serial_number: None,
            purchase_date: Some("2026-01-10".to_string()),
            purchase_price: Some(15000000.0),
            condition_status: "good".to_string(),
            status: "available".to_string(),
        }
    }

    fn assign_input(emp: i32, date: &str) -> AssignInput {
        AssignInput {
            employee_id: emp,
            assigned_date: date.to_string(),
            condition_on_assign: None,
            notes: None,
        }
    }

    fn return_input(date: &str, cond: Option<&str>) -> ReturnInput {
        ReturnInput {
            returned_date: date.to_string(),
            condition_on_return: cond.map(str::to_string),
            notes: None,
        }
    }

    #[tokio::test]
    async fn aset_tugas_kembali_rusak_masuk_maintenance_sea() {
        let (_d, state) = state().await;
        let db = &state.sea;
        let actor = one(
            db,
            "SELECT id FROM users WHERE username = 'admin'",
            "t.admin",
        )
        .await;
        let eid = one(
            db,
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            "t.emp",
        )
        .await;

        let cat = category_save_sea(db, actor, None, "LTP", "Laptop")
            .await
            .expect("kategori");
        assert_eq!(
            category_save_sea(db, actor, None, "LTP", "Dobel")
                .await
                .expect_err("ganda"),
            "Kode kategori sudah dipakai."
        );

        let aid = asset_save_sea(db, actor, None, &aset_input(cat))
            .await
            .expect("aset");
        assert_eq!(asset_list_sea(db, "think").await.expect("cari").len(), 1);

        let eid32 = to_dto_int(eid, "e").unwrap();
        let asg = assign_sea(db, actor, aid as i64, &assign_input(eid32, "2026-09-01"))
            .await
            .expect("assign");
        assert_eq!(
            assign_sea(db, actor, aid as i64, &assign_input(eid32, "2026-09-02"))
                .await
                .expect_err("assign ganda"),
            "Aset tidak tersedia untuk ditugaskan."
        );
        assert_eq!(my_assets_sea(db, eid).await.expect("mine").len(), 1);

        return_asset_sea(
            db,
            actor,
            actor,
            asg as i64,
            &return_input("2026-09-10", Some("damaged")),
        )
        .await
        .expect("kembali");
        let det = asset_detail_sea(db, aid as i64)
            .await
            .expect("det")
            .expect("ada");
        assert_eq!(det.asset.status, "maintenance");
        assert_eq!(det.asset.condition_status, "damaged");
        assert_eq!(det.assignments.len(), 1);
        assert_eq!(
            return_asset_sea(
                db,
                actor,
                actor,
                asg as i64,
                &return_input("2026-09-11", None)
            )
            .await
            .expect_err("kembali ganda"),
            "Sudah dikembalikan."
        );

        mark_available_sea(db, actor, aid as i64)
            .await
            .expect("available");
        add_maintenance_sea(
            db,
            actor,
            aid as i64,
            &MaintenanceInput {
                maintenance_date: "2026-09-12".to_string(),
                description: "Ganti SSD".to_string(),
                cost: 800000.0,
                performed_by: Some("Teknisi".to_string()),
            },
        )
        .await
        .expect("maintenance");
        let det = asset_detail_sea(db, aid as i64)
            .await
            .expect("det2")
            .expect("ada2");
        assert_eq!(det.maintenance.len(), 1);
        assert_eq!(det.maintenance[0].cost, 800000.0);
        mark_available_sea(db, actor, aid as i64)
            .await
            .expect("available2");

        asset_delete_sea(db, actor, aid as i64)
            .await
            .expect("hapus");
        assert!(asset_detail_sea(db, aid as i64)
            .await
            .expect("det3")
            .is_none());
        category_delete_sea(db, actor, cat as i64)
            .await
            .expect("hapus kategori");
        let gone = q_one(
            db,
            "SELECT id FROM asset_categories WHERE code = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::Text("LTP".to_string())],
            1,
            "t.catgone",
        )
        .await
        .expect("cek");
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn validasi_batas_perilaku_aset_sea() {
        let (_d, state) = state().await;
        let db = &state.sea;
        let actor = one(
            db,
            "SELECT id FROM users WHERE username = 'admin'",
            "t.admin",
        )
        .await;
        let eid = one(
            db,
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            "t.emp",
        )
        .await;
        let cat32 = to_dto_int(
            one(
                db,
                "SELECT id FROM asset_categories WHERE code = 'LAPTOP'",
                "t.laptop",
            )
            .await,
            "cat",
        )
        .unwrap();

        assert_eq!(
            category_save_sea(db, actor, None, "  ", "Kosong")
                .await
                .expect_err("kode kosong"),
            "Kode dan nama kategori wajib diisi."
        );
        assert_eq!(
            category_save_sea(db, actor, Some(999_999), "X", "X")
                .await
                .expect_err("tak ada"),
            "Kategori tidak ditemukan."
        );

        let mut bad = aset_input(cat32);
        bad.asset_category_id = 999_999;
        assert_eq!(
            asset_save_sea(db, actor, None, &bad)
                .await
                .expect_err("kategori"),
            "Kategori tidak ditemukan."
        );
        let mut bad = aset_input(cat32);
        bad.condition_status = "berkarat".to_string();
        assert_eq!(
            asset_save_sea(db, actor, None, &bad)
                .await
                .expect_err("kondisi"),
            "Kondisi tidak valid."
        );
        let mut bad = aset_input(cat32);
        bad.status = "dibuang".to_string();
        assert_eq!(
            asset_save_sea(db, actor, None, &bad)
                .await
                .expect_err("status"),
            "Status tidak valid."
        );
        let mut bad = aset_input(cat32);
        bad.purchase_date = Some("10-01-2026".to_string());
        assert_eq!(
            asset_save_sea(db, actor, None, &bad)
                .await
                .expect_err("tanggal"),
            "Tanggal beli harus valid (YYYY-MM-DD)."
        );
        let mut bad = aset_input(cat32);
        bad.purchase_price = Some(-1.0);
        assert_eq!(
            asset_save_sea(db, actor, None, &bad)
                .await
                .expect_err("harga"),
            "Harga minimal 0."
        );

        let aid = asset_save_sea(db, actor, None, &aset_input(cat32))
            .await
            .expect("aset");
        assert_eq!(
            asset_save_sea(db, actor, None, &aset_input(cat32))
                .await
                .expect_err("kode ganda"),
            "Kode aset sudah dipakai."
        );

        assert_eq!(
            assign_sea(db, actor, aid as i64, &assign_input(999_999, "2026-09-01"))
                .await
                .expect_err("karyawan"),
            "Karyawan tidak ditemukan."
        );
        assert_eq!(
            assign_sea(
                db,
                actor,
                aid as i64,
                &assign_input(to_dto_int(eid, "e").unwrap(), "2026-13-01")
            )
            .await
            .expect_err("tanggal assign"),
            "Tanggal tugaskan tidak valid."
        );
        assert_eq!(
            assign_sea(
                db,
                actor,
                999_999,
                &assign_input(to_dto_int(eid, "e").unwrap(), "2026-09-01")
            )
            .await
            .expect_err("aset assign"),
            "Aset tidak ditemukan."
        );

        let asg = assign_sea(
            db,
            actor,
            aid as i64,
            &assign_input(to_dto_int(eid, "e").unwrap(), "2026-09-01"),
        )
        .await
        .expect("assign");
        assert_eq!(
            return_asset_sea(
                db,
                actor,
                actor,
                asg as i64,
                &return_input("2026-02-30", None)
            )
            .await
            .expect_err("tanggal return"),
            "Tanggal kembali tidak valid."
        );
        assert_eq!(
            return_asset_sea(
                db,
                actor,
                actor,
                asg as i64,
                &return_input("2026-09-05", Some("patah"))
            )
            .await
            .expect_err("kondisi return"),
            "Kondisi tidak valid."
        );
        return_asset_sea(
            db,
            actor,
            actor,
            asg as i64,
            &return_input("2026-09-05", None),
        )
        .await
        .expect("kembali good");
        let det = asset_detail_sea(db, aid as i64)
            .await
            .expect("det")
            .expect("ada");
        assert_eq!(det.asset.status, "available");
        assert_eq!(det.asset.condition_status, "good");

        assert_eq!(
            add_maintenance_sea(
                db,
                actor,
                999_999,
                &MaintenanceInput {
                    maintenance_date: "2026-09-12".to_string(),
                    description: "X".to_string(),
                    cost: 0.0,
                    performed_by: None,
                },
            )
            .await
            .expect_err("aset maint"),
            "Aset tidak ditemukan."
        );
        assert_eq!(
            add_maintenance_sea(
                db,
                actor,
                aid as i64,
                &MaintenanceInput {
                    maintenance_date: "2026-09-12".to_string(),
                    description: "x".repeat(256),
                    cost: 0.0,
                    performed_by: None,
                },
            )
            .await
            .expect_err("deskripsi panjang"),
            "Deskripsi maksimal 255 karakter."
        );
        assert_eq!(
            add_maintenance_sea(
                db,
                actor,
                aid as i64,
                &MaintenanceInput {
                    maintenance_date: "2026-09-12".to_string(),
                    description: "  ".to_string(),
                    cost: 0.0,
                    performed_by: None,
                },
            )
            .await
            .expect_err("deskripsi kosong"),
            "Deskripsi wajib diisi."
        );
        assert_eq!(
            add_maintenance_sea(
                db,
                actor,
                aid as i64,
                &MaintenanceInput {
                    maintenance_date: "2026-09-12".to_string(),
                    description: "Ganti HDD".to_string(),
                    cost: -5.0,
                    performed_by: None,
                },
            )
            .await
            .expect_err("biaya negatif"),
            "Biaya minimal 0."
        );

        assert_eq!(
            mark_available_sea(db, actor, 999_999)
                .await
                .expect_err("mark tak ada"),
            "Aset tidak ditemukan."
        );

        let asg2 = assign_sea(
            db,
            actor,
            aid as i64,
            &assign_input(to_dto_int(eid, "e").unwrap(), "2026-09-06"),
        )
        .await
        .expect("assign2");
        assert_eq!(
            asset_delete_sea(db, actor, aid as i64)
                .await
                .expect_err("hapus saat assigned"),
            "Aset sedang ditugaskan."
        );
        return_asset_sea(
            db,
            actor,
            actor,
            asg2 as i64,
            &return_input("2026-09-08", Some("fair")),
        )
        .await
        .expect("kembali2");
        asset_delete_sea(db, actor, aid as i64)
            .await
            .expect("hapus");
        let mobile = one(
            db,
            "SELECT id FROM asset_categories WHERE code = 'MOBILE'",
            "t.mobile",
        )
        .await;
        category_delete_sea(db, actor, mobile)
            .await
            .expect("kategori tanpa aset harus terhapus");
    }
}
