//! Aset: kategori, inventaris, penugasan, pengembalian, maintenance.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

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

// ---------------- Kategori ----------------

pub fn category_list(conn: &Connection) -> Result<Vec<Category>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, code, name FROM asset_categories WHERE deleted_at IS NULL ORDER BY name",
        )
        .map_err(|e| format!("gagal menyiapkan kategori: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| format!("gagal membaca kategori: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Category {
            id: to_dto_int(id, "category.id")?,
            code,
            name,
        });
    }
    Ok(out)
}

pub fn category_save(
    conn: &Connection,
    actor_id: i64,
    id: Option<i64>,
    code: &str,
    name: &str,
) -> Result<i32, String> {
    if code.trim().is_empty() || name.trim().is_empty() {
        return Err("Kode dan nama kategori wajib diisi.".to_string());
    }
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE asset_categories SET code = ?1, name = ?2 WHERE id = ?3 AND deleted_at IS NULL",
            params![code.trim(), name.trim(), rid],
        ).map_err(|e| format!("gagal menyimpan kategori: {e}"))?;
        if n == 0 {
            return Err("Kategori tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "asset.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "category.id")
    } else {
        conn.execute(
            "INSERT INTO asset_categories (code, name) VALUES (?1, ?2)",
            params![code.trim(), name.trim()],
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Kode kategori sudah dipakai.".to_string()
            } else {
                format!("gagal menambah kategori: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "asset.category",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "category.id")
    }
}

pub fn category_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM assets WHERE asset_category_id = ?1 AND deleted_at IS NULL",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
    if n > 0 {
        return Err("Kategori masih dipakai aset.".to_string());
    }
    let d = conn.execute(
        "UPDATE asset_categories SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus kategori: {e}"))?;
    if d == 0 {
        return Err("Kategori tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "asset.category",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

// ---------------- Aset ----------------

pub fn asset_list(conn: &Connection, search: &str) -> Result<Vec<Asset>, String> {
    let search = search.trim();
    let has = !search.is_empty();
    let base = "SELECT a.id, a.asset_code, a.name, a.asset_category_id, c.name, a.brand, a.serial_number, a.purchase_date, a.purchase_price, a.condition_status, a.status FROM assets a INNER JOIN asset_categories c ON c.id = a.asset_category_id WHERE a.deleted_at IS NULL";
    let sql = if has {
        format!("{base} AND (a.asset_code LIKE ?1 OR a.name LIKE ?1 OR a.serial_number LIKE ?1) ORDER BY a.name")
    } else {
        format!("{base} ORDER BY a.name")
    };
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("gagal menyiapkan aset: {e}"))?;
    let like = format!("%{search}%");
    let map_row = |r: &rusqlite::Row<'_>| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
            r.get::<_, Option<f64>>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, String>(10)?,
        ))
    };
    let rows = if has {
        stmt.query_map(params![like], map_row)
            .map_err(|e| format!("gagal membaca aset: {e}"))?
    } else {
        stmt.query_map([], map_row)
            .map_err(|e| format!("gagal membaca aset: {e}"))?
    };
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name, cat, cat_name, brand, serial, pdate, price, cond, status) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(Asset {
            id: to_dto_int(id, "asset.id")?,
            asset_code: code,
            name,
            asset_category_id: to_dto_int(cat, "asset.cat")?,
            category_name: cat_name,
            brand,
            serial_number: serial,
            purchase_date: pdate,
            purchase_price: price,
            condition_status: cond,
            status,
        });
    }
    Ok(out)
}

pub fn asset_detail(conn: &Connection, id: i64) -> Result<Option<AssetDetail>, String> {
    let asset = asset_list(conn, "")?
        .into_iter()
        .find(|a| a.id as i64 == id);
    let Some(asset) = asset else {
        // pastikan benar-benar tidak ada (bukan sekadar tak cocok cari kosong)
        let exists: Option<i64> = conn
            .query_row(
                "SELECT id FROM assets WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal memeriksa aset: {e}"))?;
        if exists.is_none() {
            return Ok(None);
        }
        return Err("Gagal memuat aset.".to_string());
    };
    let mut assignments = Vec::new();
    let mut astmt = conn
        .prepare("SELECT aa.id, aa.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, aa.assigned_date, aa.returned_date, aa.condition_on_assign, aa.condition_on_return, aa.notes FROM asset_assignments aa INNER JOIN employees e ON e.id = aa.employee_id WHERE aa.asset_id = ?1 ORDER BY aa.assigned_date DESC")
        .map_err(|e| format!("gagal menyiapkan riwayat: {e}"))?;
    for arow in astmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|e| format!("gagal membaca riwayat: {e}"))?
    {
        let (aid, emp, name, number, assigned, returned, cond_a, cond_r, notes) =
            arow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        assignments.push(AssetAssignment {
            id: to_dto_int(aid, "assign.id")?,
            employee_id: to_dto_int(emp, "assign.emp")?,
            employee_name: name,
            employee_number: number,
            assigned_date: assigned,
            returned_date: returned,
            condition_on_assign: cond_a,
            condition_on_return: cond_r,
            notes,
        });
    }
    let mut maintenance = Vec::new();
    let mut mstmt = conn
        .prepare("SELECT id, maintenance_date, description, cost, performed_by FROM asset_maintenance WHERE asset_id = ?1 ORDER BY maintenance_date DESC")
        .map_err(|e| format!("gagal menyiapkan maintenance: {e}"))?;
    for mrow in mstmt
        .query_map(params![id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|e| format!("gagal membaca maintenance: {e}"))?
    {
        let (mid, date, desc, cost, by) = mrow.map_err(|e| format!("gagal membaca baris: {e}"))?;
        maintenance.push(Maintenance {
            id: to_dto_int(mid, "maint.id")?,
            maintenance_date: date,
            description: desc,
            cost,
            performed_by: by,
        });
    }
    Ok(Some(AssetDetail {
        asset,
        assignments,
        maintenance,
    }))
}

pub fn asset_save(
    conn: &Connection,
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
    let cat: Option<i64> = conn
        .query_row(
            "SELECT id FROM asset_categories WHERE id = ?1 AND deleted_at IS NULL",
            params![input.asset_category_id as i64],
            |r| r.get(0),
        )
        .optional()
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
    if let Some(rid) = id {
        let n = conn.execute(
            "UPDATE assets SET asset_code = ?1, name = ?2, asset_category_id = ?3, brand = ?4, serial_number = ?5, purchase_date = ?6, purchase_price = ?7, condition_status = ?8, status = ?9 WHERE id = ?10 AND deleted_at IS NULL",
            params![
                input.asset_code.trim(), input.name.trim(), input.asset_category_id as i64,
                input.brand.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.serial_number.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.purchase_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.purchase_price,
                input.condition_status, input.status, rid,
            ],
        ).map_err(|e| format!("gagal menyimpan aset: {e}"))?;
        if n == 0 {
            return Err("Aset tidak ditemukan.".to_string());
        }
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            "asset",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "asset.id")
    } else {
        conn.execute(
            "INSERT INTO assets (asset_code, name, asset_category_id, brand, serial_number, purchase_date, purchase_price, condition_status, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'available')",
            params![
                input.asset_code.trim(), input.name.trim(), input.asset_category_id as i64,
                input.brand.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.serial_number.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.purchase_date.as_deref().map(str::trim).filter(|s| !s.is_empty()),
                input.purchase_price,
                input.condition_status,
            ],
        ).map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Kode aset sudah dipakai.".to_string()
            } else {
                format!("gagal menambah aset: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            "asset",
            Some(&rid.to_string()),
            None,
            None,
            None,
        )?;
        to_dto_int(rid, "asset.id")
    }
}

pub fn asset_delete(conn: &Connection, actor_id: i64, id: i64) -> Result<(), String> {
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM asset_assignments WHERE asset_id = ?1 AND returned_date IS NULL",
            params![id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memeriksa penugasan: {e}"))?;
    if n > 0 {
        return Err("Aset sedang ditugaskan.".to_string());
    }
    let d = conn.execute(
        "UPDATE assets SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
    ).map_err(|e| format!("gagal menghapus aset: {e}"))?;
    if d == 0 {
        return Err("Aset tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        "asset",
        Some(&id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn my_assets(conn: &Connection, employee_id: i64) -> Result<Vec<MyAsset>, String> {
    let mut stmt = conn
        .prepare("SELECT aa.id, a.asset_code, a.name, a.brand, c.name, aa.assigned_date FROM asset_assignments aa INNER JOIN assets a ON a.id = aa.asset_id INNER JOIN asset_categories c ON c.id = a.asset_category_id WHERE aa.employee_id = ?1 AND aa.returned_date IS NULL ORDER BY aa.assigned_date DESC")
        .map_err(|e| format!("gagal menyiapkan aset saya: {e}"))?;
    let rows = stmt
        .query_map(params![employee_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| format!("gagal membaca aset: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, code, name, brand, cat, assigned) =
            row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        out.push(MyAsset {
            assignment_id: to_dto_int(id, "myasset.id")?,
            asset_code: code,
            name,
            brand,
            category_name: cat,
            assigned_date: assigned,
        });
    }
    Ok(out)
}

/// Tugaskan aset available; kondisi awal default kondisi aset saat ini.
pub fn assign(
    conn: &Connection,
    actor_id: i64,
    asset_id: i64,
    input: &AssignInput,
) -> Result<i32, String> {
    let emp: Option<i64> = conn
        .query_row(
            "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL",
            params![input.employee_id as i64],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    NaiveDate::parse_from_str(input.assigned_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal tugaskan tidak valid.".to_string())?;
    let asset: Option<(String, String)> = conn
        .query_row(
            "SELECT status, condition_status FROM assets WHERE id = ?1 AND deleted_at IS NULL",
            params![asset_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat aset: {e}"))?;
    let Some((status, condition)) = asset else {
        return Err("Aset tidak ditemukan.".to_string());
    };
    if status != "available" {
        return Err("Aset tidak tersedia untuk ditugaskan.".to_string());
    }
    let cond = input
        .condition_on_assign
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&condition);
    conn.execute(
        "INSERT INTO asset_assignments (asset_id, employee_id, assigned_date, condition_on_assign, notes) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![asset_id, input.employee_id as i64, input.assigned_date.trim(), cond, input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty())],
    )
    .map_err(|e| format!("gagal menugaskan aset: {e}"))?;
    let rid = conn.last_insert_rowid();
    conn.execute(
        "UPDATE assets SET status = 'assigned' WHERE id = ?1",
        params![asset_id],
    )
    .map_err(|e| format!("gagal mengubah status aset: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "asset.assignment",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "assign.id")
}

/// Kembalikan: rusak jadi maintenance, selain itu available.
pub fn return_asset(
    conn: &Connection,
    actor_id: i64,
    verified_by: i64,
    assignment_id: i64,
    input: &ReturnInput,
) -> Result<(), String> {
    let row: Option<(i64, Option<String>)> = conn
        .query_row(
            "SELECT asset_id, returned_date FROM asset_assignments WHERE id = ?1",
            params![assignment_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat penugasan: {e}"))?;
    let Some((asset_id, returned)) = row else {
        return Err("Penugasan tidak ditemukan.".to_string());
    };
    if returned.is_some() {
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
    conn.execute(
        "UPDATE asset_assignments SET returned_date = ?1, condition_on_return = ?2, notes = COALESCE(?3, notes), verified_by = ?4 WHERE id = ?5",
        params![input.returned_date.trim(), cond, input.notes.as_deref().map(str::trim).filter(|s| !s.is_empty()), verified_by, assignment_id],
    )
    .map_err(|e| format!("gagal mengembalikan: {e}"))?;
    let status = if cond == "damaged" {
        "maintenance"
    } else {
        "available"
    };
    conn.execute(
        "UPDATE assets SET status = ?1, condition_status = ?2 WHERE id = ?3",
        params![status, cond, asset_id],
    )
    .map_err(|e| format!("gagal mengubah status aset: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "asset.return",
        Some(&assignment_id.to_string()),
        None,
        None,
        None,
    )?;
    Ok(())
}

pub fn add_maintenance(
    conn: &Connection,
    actor_id: i64,
    asset_id: i64,
    input: &MaintenanceInput,
) -> Result<i32, String> {
    let exists: Option<i64> = conn
        .query_row(
            "SELECT id FROM assets WHERE id = ?1 AND deleted_at IS NULL",
            params![asset_id],
            |r| r.get(0),
        )
        .optional()
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
    conn.execute(
        "INSERT INTO asset_maintenance (asset_id, maintenance_date, description, cost, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            asset_id, input.maintenance_date.trim(), input.description.trim(), input.cost,
            input.performed_by.as_deref().map(str::trim).filter(|s| !s.is_empty()),
        ],
    )
    .map_err(|e| format!("gagal mencatat maintenance: {e}"))?;
    let rid = conn.last_insert_rowid();
    conn.execute(
        "UPDATE assets SET status = 'maintenance' WHERE id = ?1",
        params![asset_id],
    )
    .map_err(|e| format!("gagal mengubah status: {e}"))?;
    audit::log(
        conn,
        Some(actor_id),
        "CREATE",
        "asset.maintenance",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )?;
    to_dto_int(rid, "maint.id")
}

pub fn mark_available(conn: &Connection, actor_id: i64, asset_id: i64) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE assets SET status = 'available' WHERE id = ?1 AND deleted_at IS NULL",
            params![asset_id],
        )
        .map_err(|e| format!("gagal mengubah status: {e}"))?;
    if n == 0 {
        return Err("Aset tidak ditemukan.".to_string());
    }
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "asset",
        Some(&asset_id.to_string()),
        None,
        None,
        Some("Status menjadi available"),
    )?;
    Ok(())
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

pub async fn category_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Category>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name FROM asset_categories WHERE deleted_at IS NULL ORDER BY name".to_string(),
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
            "UPDATE asset_categories SET code = ?1, name = ?2 WHERE id = ?3 AND deleted_at IS NULL".to_string(),
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
        "SELECT COUNT(*) FROM assets WHERE asset_category_id = ?1 AND deleted_at IS NULL".to_string(),
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
        "SELECT COUNT(*) FROM asset_assignments WHERE asset_id = ?1 AND returned_date IS NULL".to_string(),
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
        "SELECT status, condition_status FROM assets WHERE id = ?1 AND deleted_at IS NULL".to_string(),
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
    use crate::{db, seed};

    fn live() -> (tempfile::TempDir, crate::db::DbPool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool, files)
    }

    fn admin(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    fn emp(conn: &Connection) -> i64 {
        conn.query_row(
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn aset_tugas_kembali_rusak_masuk_maintenance() {
        let (_d, pool, _f) = live();
        let conn = pool.get().expect("get");
        let actor = admin(&conn);
        let eid = emp(&conn);
        let cat = category_save(&conn, actor, None, "LTP", "Laptop").expect("kategori");
        assert!(category_save(&conn, actor, None, "LTP", "Dobel").is_err());
        let aid = asset_save(
            &conn,
            actor,
            None,
            &AssetInput {
                asset_code: "LT-001".to_string(),
                name: "ThinkPad".to_string(),
                asset_category_id: cat,
                brand: Some("Lenovo".to_string()),
                serial_number: None,
                purchase_date: Some("2026-01-10".to_string()),
                purchase_price: Some(15000000.0),
                condition_status: "good".to_string(),
                status: "available".to_string(),
            },
        )
        .expect("aset");
        assert_eq!(asset_list(&conn, "think").expect("cari").len(), 1);
        let eid32 = to_dto_int(eid, "e").unwrap();
        let asg = assign(
            &conn,
            actor,
            aid as i64,
            &AssignInput {
                employee_id: eid32,
                assigned_date: "2026-09-01".to_string(),
                condition_on_assign: None,
                notes: None,
            },
        )
        .expect("assign");
        assert!(assign(
            &conn,
            actor,
            aid as i64,
            &AssignInput {
                employee_id: eid32,
                assigned_date: "2026-09-02".to_string(),
                condition_on_assign: None,
                notes: None,
            },
        )
        .is_err());
        assert_eq!(my_assets(&conn, eid).expect("mine").len(), 1);
        return_asset(
            &conn,
            actor,
            actor,
            asg as i64,
            &ReturnInput {
                returned_date: "2026-09-10".to_string(),
                condition_on_return: Some("damaged".to_string()),
                notes: None,
            },
        )
        .expect("kembali");
        let det = asset_detail(&conn, aid as i64).expect("det").expect("ada");
        assert_eq!(det.asset.status, "maintenance");
        assert_eq!(det.assignments.len(), 1);
        assert!(return_asset(
            &conn,
            actor,
            actor,
            asg as i64,
            &ReturnInput {
                returned_date: "2026-09-11".to_string(),
                condition_on_return: None,
                notes: None,
            },
        )
        .is_err());
        mark_available(&conn, actor, aid as i64).expect("available");
        add_maintenance(
            &conn,
            actor,
            aid as i64,
            &MaintenanceInput {
                maintenance_date: "2026-09-12".to_string(),
                description: "Ganti SSD".to_string(),
                cost: 800000.0,
                performed_by: Some("Teknisi".to_string()),
            },
        )
        .expect("maintenance");
        let det = asset_detail(&conn, aid as i64).expect("det").expect("ada");
        assert_eq!(det.maintenance.len(), 1);
        mark_available(&conn, actor, aid as i64).expect("available2");
        asset_delete(&conn, actor, aid as i64).expect("hapus");
        assert!(asset_detail(&conn, aid as i64).expect("det").is_none());
        category_delete(&conn, actor, cat as i64).expect("hapus kategori");
    }

    #[tokio::test]
    async fn aset_sea_paritas_dengan_sync() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let conn = state.db.get().expect("get");
        let db = &state.sea;
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let eid: i64 = conn
            .query_row(
                "SELECT id FROM employees WHERE employee_number = 'EMP-0001'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let cat = category_save_sea(db, actor, None, "LTP", "Laptop")
            .await
            .expect("kategori");
        let e_sea = category_save_sea(db, actor, None, "LTP", "Dobel")
            .await
            .expect_err("ganda");
        let e_sync = category_save(&conn, actor, None, "LTP", "Dobel")
            .expect_err("ganda sync");
        assert_eq!(e_sea, e_sync);
        let aid = asset_save_sea(
            db,
            actor,
            None,
            &AssetInput {
                asset_code: "LT-001".to_string(),
                name: "ThinkPad".to_string(),
                asset_category_id: cat,
                brand: Some("Lenovo".to_string()),
                serial_number: None,
                purchase_date: Some("2026-01-10".to_string()),
                purchase_price: Some(15000000.0),
                condition_status: "good".to_string(),
                status: "available".to_string(),
            },
        )
        .await
        .expect("aset");
        let l_sync = serde_json::to_string(&asset_list(&conn, "think").expect("ls")).unwrap();
        let l_sea = serde_json::to_string(&asset_list_sea(db, "think").await.expect("lse")).unwrap();
        assert_eq!(l_sync, l_sea);
        assert_eq!(l_sea.matches("LT-001").count(), 1);
        let eid32 = to_dto_int(eid, "e").unwrap();
        let asg = assign_sea(
            db,
            actor,
            aid as i64,
            &AssignInput {
                employee_id: eid32,
                assigned_date: "2026-09-01".to_string(),
                condition_on_assign: None,
                notes: None,
            },
        )
        .await
        .expect("assign");
        assert!(assign_sea(
            db,
            actor,
            aid as i64,
            &AssignInput {
                employee_id: eid32,
                assigned_date: "2026-09-02".to_string(),
                condition_on_assign: None,
                notes: None,
            },
        )
        .await
        .is_err());
        let m_sync = serde_json::to_string(&my_assets(&conn, eid).expect("ms")).unwrap();
        let m_sea = serde_json::to_string(&my_assets_sea(db, eid).await.expect("mse")).unwrap();
        assert_eq!(m_sync, m_sea);
        return_asset_sea(
            db,
            actor,
            actor,
            asg as i64,
            &ReturnInput {
                returned_date: "2026-09-10".to_string(),
                condition_on_return: Some("damaged".to_string()),
                notes: None,
            },
        )
        .await
        .expect("kembali");
        let d_sync =
            serde_json::to_string(&asset_detail(&conn, aid as i64).expect("ds")).unwrap();
        let d_sea =
            serde_json::to_string(&asset_detail_sea(db, aid as i64).await.expect("dse")).unwrap();
        assert_eq!(d_sync, d_sea);
        assert!(d_sea.contains("\"status\":\"maintenance\""));
        mark_available_sea(db, actor, aid as i64).await.expect("available");
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
        mark_available_sea(db, actor, aid as i64).await.expect("available2");
        asset_delete_sea(db, actor, aid as i64).await.expect("hapus");
        category_delete_sea(db, actor, cat as i64).await.expect("hapus kategori");
    }
}
