//! Master organisasi: 10 entitas lewat satu engine konfigurasi.

use std::collections::BTreeMap;
use std::string::String;

use chrono::Local;

use super::audit;
use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};

use crate::to_dto_int;



/// Opsi dropdown untuk field select.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Opt {
    pub id: i32,
    pub name: String,
}

/// Metadata field untuk membangun form dinamis.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct FieldMeta {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub options: Option<Vec<Opt>>,
}

/// Metadata entitas untuk tab dan tabel.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EntityMeta {
    pub slug: String,
    pub title: String,
    pub columns: Vec<String>,
    pub fields: Vec<FieldMeta>,
}

/// Halaman daftar generik (nilai selalu string; frontend format ulang).
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct OrgPage {
    pub rows: Vec<BTreeMap<String, String>>,
    pub total: i32,
}

struct Rule {
    required: bool,
    max_len: Option<usize>,
    email: bool,
    date: bool,
    integer: bool,
    exists_table: Option<&'static str>,
    unique: Option<(&'static str, &'static str)>,
}

struct Field {
    name: &'static str,
    label: &'static str,
    field_type: &'static str,
    rule: Rule,
    options_sql: Option<&'static str>,
}

struct Join {
    fk: &'static str,
    table: &'static str,
    label_col: &'static str,
    alias: &'static str,
}

struct Entity {
    slug: &'static str,
    title: &'static str,
    table: &'static str,
    order_by: &'static str,
    columns: &'static [&'static str],
    fields: &'static [Field],
    joins: &'static [Join],
    guard: Option<(&'static str, &'static str)>,
}

const fn req() -> Rule {
    Rule {
        required: true,
        max_len: None,
        email: false,
        date: false,
        integer: false,
        exists_table: None,
        unique: None,
    }
}
const fn opt() -> Rule {
    Rule {
        required: false,
        max_len: None,
        email: false,
        date: false,
        integer: false,
        exists_table: None,
        unique: None,
    }
}

const ENTITIES: &[Entity] = &[
    Entity {
        slug: "companies", title: "Perusahaan", table: "companies", order_by: "name ASC",
        columns: &["code", "name", "city", "phone"],
        fields: &[
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), unique: Some(("companies", "code")), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Perusahaan", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "legal_name", label: "Nama Legal", field_type: "text", rule: Rule { max_len: Some(150), ..opt() }, options_sql: None },
            Field { name: "address", label: "Alamat", field_type: "textarea", rule: opt(), options_sql: None },
            Field { name: "city", label: "Kota", field_type: "text", rule: Rule { max_len: Some(100), ..opt() }, options_sql: None },
            Field { name: "province", label: "Provinsi", field_type: "text", rule: Rule { max_len: Some(100), ..opt() }, options_sql: None },
            Field { name: "postal_code", label: "Kode Pos", field_type: "text", rule: Rule { max_len: Some(10), ..opt() }, options_sql: None },
            Field { name: "phone", label: "Telepon", field_type: "text", rule: Rule { max_len: Some(30), ..opt() }, options_sql: None },
            Field { name: "email", label: "Email", field_type: "email", rule: Rule { email: true, ..opt() }, options_sql: None },
            Field { name: "npwp", label: "NPWP", field_type: "text", rule: Rule { max_len: Some(30), ..opt() }, options_sql: None },
            Field { name: "established_date", label: "Tanggal Berdiri", field_type: "date", rule: Rule { date: true, ..opt() }, options_sql: None },
        ],
        joins: &[],
        guard: Some(("branches", "company_id")),
    },
    Entity {
        slug: "branches", title: "Cabang", table: "branches", order_by: "name ASC",
        columns: &["code", "name", "city"],
        fields: &[
            Field { name: "company_id", label: "Perusahaan", field_type: "select", rule: Rule { exists_table: Some("companies"), ..req() }, options_sql: Some("SELECT id, name FROM companies WHERE deleted_at IS NULL ORDER BY name") },
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Cabang", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "address", label: "Alamat", field_type: "textarea", rule: opt(), options_sql: None },
            Field { name: "city", label: "Kota", field_type: "text", rule: Rule { max_len: Some(100), ..opt() }, options_sql: None },
            Field { name: "phone", label: "Telepon", field_type: "text", rule: Rule { max_len: Some(30), ..opt() }, options_sql: None },
            Field { name: "is_head_office", label: "Kantor Pusat", field_type: "checkbox", rule: opt(), options_sql: None },
        ],
        joins: &[Join { fk: "company_id", table: "companies", label_col: "name", alias: "company_name" }],
        guard: Some(("departments", "branch_id")),
    },
    Entity {
        slug: "departments", title: "Departemen", table: "departments", order_by: "name ASC",
        columns: &["code", "name"],
        fields: &[
            Field { name: "company_id", label: "Perusahaan", field_type: "select", rule: Rule { exists_table: Some("companies"), ..req() }, options_sql: Some("SELECT id, name FROM companies WHERE deleted_at IS NULL ORDER BY name") },
            Field { name: "branch_id", label: "Cabang", field_type: "select", rule: opt(), options_sql: Some("SELECT id, name FROM branches WHERE deleted_at IS NULL ORDER BY name") },
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Departemen", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "head_employee_id", label: "Kepala Departemen", field_type: "select", rule: opt(), options_sql: Some("SELECT id, first_name || ' ' || COALESCE(last_name, '') as name FROM employees WHERE deleted_at IS NULL ORDER BY first_name") },
        ],
        joins: &[Join { fk: "company_id", table: "companies", label_col: "name", alias: "company_name" }],
        guard: Some(("divisions", "department_id")),
    },
    Entity {
        slug: "divisions", title: "Divisi", table: "divisions", order_by: "name ASC",
        columns: &["code", "name"],
        fields: &[
            Field { name: "department_id", label: "Departemen", field_type: "select", rule: Rule { exists_table: Some("departments"), ..req() }, options_sql: Some("SELECT id, name FROM departments WHERE deleted_at IS NULL ORDER BY name") },
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Divisi", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "head_employee_id", label: "Kepala Divisi", field_type: "select", rule: opt(), options_sql: Some("SELECT id, first_name || ' ' || COALESCE(last_name, '') as name FROM employees WHERE deleted_at IS NULL ORDER BY first_name") },
        ],
        joins: &[],
        guard: Some(("sections", "division_id")),
    },
    Entity {
        slug: "sections", title: "Seksi", table: "sections", order_by: "name ASC",
        columns: &["code", "name"],
        fields: &[
            Field { name: "division_id", label: "Divisi", field_type: "select", rule: Rule { exists_table: Some("divisions"), ..req() }, options_sql: Some("SELECT id, name FROM divisions WHERE deleted_at IS NULL ORDER BY name") },
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Seksi", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "head_employee_id", label: "Kepala Seksi", field_type: "select", rule: opt(), options_sql: Some("SELECT id, first_name || ' ' || COALESCE(last_name, '') as name FROM employees WHERE deleted_at IS NULL ORDER BY first_name") },
        ],
        joins: &[],
        guard: Some(("employees", "section_id")),
    },
    Entity {
        slug: "positions", title: "Jabatan", table: "positions", order_by: "name ASC",
        columns: &["code", "name"],
        fields: &[
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Jabatan", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "department_id", label: "Departemen", field_type: "select", rule: opt(), options_sql: Some("SELECT id, name FROM departments WHERE deleted_at IS NULL ORDER BY name") },
            Field { name: "job_level_id", label: "Level Jabatan", field_type: "select", rule: opt(), options_sql: Some("SELECT id, name FROM job_levels WHERE deleted_at IS NULL ORDER BY level_order") },
        ],
        joins: &[],
        guard: Some(("employees", "position_id")),
    },
    Entity {
        slug: "job-levels", title: "Job Level", table: "job_levels", order_by: "level_order ASC",
        columns: &["code", "name", "level_order"],
        fields: &[
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Level", field_type: "text", rule: Rule { max_len: Some(100), ..req() }, options_sql: None },
            Field { name: "level_order", label: "Urutan", field_type: "number", rule: Rule { integer: true, ..req() }, options_sql: None },
        ],
        joins: &[],
        guard: Some(("employees", "job_level_id")),
    },
    Entity {
        slug: "job-grades", title: "Job Grade", table: "job_grades", order_by: "grade_order ASC",
        columns: &["code", "name", "grade_order"],
        fields: &[
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Grade", field_type: "text", rule: Rule { max_len: Some(100), ..req() }, options_sql: None },
            Field { name: "grade_order", label: "Urutan", field_type: "number", rule: Rule { integer: true, ..req() }, options_sql: None },
        ],
        joins: &[],
        guard: Some(("employees", "job_grade_id")),
    },
    Entity {
        slug: "work-locations", title: "Lokasi Kerja", table: "work_locations", order_by: "name ASC",
        columns: &["name", "radius_meter"],
        fields: &[
            Field { name: "name", label: "Nama Lokasi", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "address", label: "Alamat", field_type: "textarea", rule: opt(), options_sql: None },
            Field { name: "latitude", label: "Latitude", field_type: "text", rule: opt(), options_sql: None },
            Field { name: "longitude", label: "Longitude", field_type: "text", rule: opt(), options_sql: None },
            Field { name: "radius_meter", label: "Radius (meter)", field_type: "number", rule: Rule { integer: true, ..req() }, options_sql: None },
        ],
        joins: &[],
        guard: Some(("employees", "work_location_id")),
    },
    Entity {
        slug: "cost-centers", title: "Cost Center", table: "cost_centers", order_by: "name ASC",
        columns: &["code", "name"],
        fields: &[
            Field { name: "code", label: "Kode", field_type: "text", rule: Rule { max_len: Some(20), ..req() }, options_sql: None },
            Field { name: "name", label: "Nama Cost Center", field_type: "text", rule: Rule { max_len: Some(150), ..req() }, options_sql: None },
            Field { name: "department_id", label: "Departemen", field_type: "select", rule: opt(), options_sql: Some("SELECT id, name FROM departments WHERE deleted_at IS NULL ORDER BY name") },
        ],
        joins: &[],
        guard: Some(("employees", "cost_center_id")),
    },
];

fn entity(slug: &str) -> Result<&'static Entity, String> {
    ENTITIES
        .iter()
        .find(|e| e.slug == slug)
        .ok_or_else(|| "Entitas tidak dikenal.".to_string())
}

pub fn entities() -> Vec<EntityMeta> {
    ENTITIES
        .iter()
        .map(|e| EntityMeta {
            slug: e.slug.to_string(),
            title: e.title.to_string(),
            columns: e.columns.iter().map(|c| c.to_string()).collect(),
            fields: e
                .fields
                .iter()
                .map(|f| FieldMeta {
                    name: f.name.to_string(),
                    label: f.label.to_string(),
                    field_type: f.field_type.to_string(),
                    required: f.rule.required,
                    options: None,
                })
                .collect(),
        })
        .collect()
}

/// Opsi dropdown untuk satu field select.
pub async fn options(
    db: &sea_orm::DatabaseConnection,
    slug: &str,
    field: &str,
) -> Result<Vec<Opt>, String> {
    let ent = entity(slug)?;
    let f = ent
        .fields
        .iter()
        .find(|f| f.name == field && f.field_type == "select")
        .ok_or("Field opsi tidak dikenal.".to_string())?;
    let sql = f
        .options_sql
        .ok_or("Field ini tidak punya opsi.".to_string())?;
    let rows = q_all(db, sql.to_string(), vec![], 2, "membaca opsi").await?;
    let mut out = Vec::new();
    for row in rows {
        let id = value_i64(&row[0]).ok_or("id opsi rusak.".to_string())?;
        out.push(Opt {
            id: to_dto_int(id, "option.id")?,
            name: value_to_string(&row[1]),
        });
    }
    Ok(out)
}

async fn read_row(
    db: &sea_orm::DatabaseConnection,
    ent: &Entity,
    id: i64,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let cols: Vec<String> = ent.fields.iter().map(|f| f.name.to_string()).collect();
    let select = format!("id, {}", cols.join(", "));
    let row = q_one(db, format!(
            "SELECT {select} FROM {} WHERE id = ?1 AND deleted_at IS NULL",
            ent.table
        ),
        vec![Value::from(id)],
        cols.len() + 1,
        "memuat data",
    )
    .await?;
    Ok(row.map(|vals| {
        let mut map = BTreeMap::new();
        map.insert("id".to_string(), value_to_string(&vals[0]));
        for (i, col) in cols.iter().enumerate() {
            map.insert(col.clone(), value_to_string(&vals[i + 1]));
        }
        map
    }))
}

/// Daftar + cari (hanya kolom code/name) + paginasi + label join.
pub async fn list(
    db: &sea_orm::DatabaseConnection,
    slug: &str,
    search: &str,
    page: i32,
    per_page: i32,
) -> Result<OrgPage, String> {
    let ent = entity(slug)?;
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100);
    let offset = (page - 1) as i64 * per_page as i64;

    let searchable: Vec<&str> = ent
        .fields
        .iter()
        .map(|f| f.name)
        .filter(|c| *c == "code" || *c == "name")
        .collect();
    let search = search.trim();
    let has_search = !search.is_empty() && !searchable.is_empty();
    let where_sql = if has_search {
        let conds: Vec<String> = searchable.iter().map(|c| format!("{c} LIKE ?1")).collect();
        format!("deleted_at IS NULL AND ({})", conds.join(" OR "))
    } else {
        "deleted_at IS NULL".to_string()
    };
    let like = format!("%{search}%");

    let total_rows = if has_search {
        q_all(db, format!("SELECT COUNT(*) FROM {} WHERE {where_sql}", ent.table),
            vec![Value::from(like.clone())],
            1,
            "menghitung",
        )
        .await?
    } else {
        q_all(db, format!("SELECT COUNT(*) FROM {} WHERE {where_sql}", ent.table),
            vec![],
            1,
            "menghitung",
        )
        .await?
    };
    let total = value_i64(&total_rows[0][0]).unwrap_or(0);

    let cols: Vec<&str> = ent.fields.iter().map(|f| f.name).collect();
    let select = format!("id, {}", cols.join(", "));
    let tail = if has_search {
        "LIMIT ?2 OFFSET ?3"
    } else {
        "LIMIT ?1 OFFSET ?2"
    };
    let sql = format!(
        "SELECT {select} FROM {} WHERE {where_sql} ORDER BY {} {tail}",
        ent.table, ent.order_by
    );
    let raw = if has_search {
        q_all(db, sql.clone(),
            vec![
                Value::from(like),
                Value::from(per_page as i64),
                Value::from(offset),
            ],
            cols.len() + 1,
            "membaca daftar",
        )
        .await?
    } else {
        q_all(db, sql.clone(),
            vec![Value::from(per_page as i64), Value::from(offset)],
            cols.len() + 1,
            "membaca daftar",
        )
        .await?
    };
    let mut rows: Vec<BTreeMap<String, String>> = Vec::new();
    for vals in raw {
        let mut map = BTreeMap::new();
        map.insert("id".to_string(), value_to_string(&vals[0]));
        for (i, col) in cols.iter().enumerate() {
            map.insert(col.to_string(), value_to_string(&vals[i + 1]));
        }
        rows.push(map);
    }

    // label join
    for row in rows.iter_mut() {
        for j in ent.joins {
            let label = match row.get(j.fk).map(|s| s.as_str()) {
                Some("") | None => "-".to_string(),
                Some(raw) => match raw.parse::<i64>() {
                    Ok(rid) => {
                        let found = q_one(db, format!("SELECT {} FROM {} WHERE id = ?1", j.label_col, j.table),
                            vec![Value::from(rid)],
                            1,
                            "memuat label",
                        )
                        .await
                        .unwrap_or(None);
                        found
                            .map(|v| value_to_string(&v[0]))
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "-".to_string())
                    }
                    Err(_) => "-".to_string(),
                },
            };
            row.insert(j.alias.to_string(), label);
        }
    }

    Ok(OrgPage {
        rows,
        total: to_dto_int(total, "org.total")?,
    })
}

async fn validate(
    db: &sea_orm::DatabaseConnection,
    ent: &Entity,
    values: &BTreeMap<String, String>,
    exclude_id: Option<i64>,
) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut cleaned = BTreeMap::new();
    for f in ent.fields {
        let raw = values.get(f.name).map(|s| s.trim()).unwrap_or("");
        if f.field_type == "checkbox" {
            cleaned.insert(
                f.name.to_string(),
                Some(
                    if raw == "1" || raw.eq_ignore_ascii_case("true") {
                        "1"
                    } else {
                        "0"
                    }
                    .to_string(),
                ),
            );
            continue;
        }
        if raw.is_empty() {
            if f.rule.required {
                return Err(format!("{} wajib diisi.", f.label));
            }
            cleaned.insert(f.name.to_string(), None);
            continue;
        }
        if let Some(max) = f.rule.max_len {
            if raw.len() > max {
                return Err(format!("{} maksimal {max} karakter.", f.label));
            }
        }
        if f.rule.email && (!raw.contains('@') || !raw.contains('.')) {
            return Err(format!("{} tidak valid.", f.label));
        }
        if f.rule.date && chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_err() {
            return Err(format!("{} harus tanggal valid (YYYY-MM-DD).", f.label));
        }
        if f.rule.integer && raw.parse::<i64>().is_err() {
            return Err(format!("{} harus bilangan bulat.", f.label));
        }
        if let Some(table) = f.rule.exists_table {
            let id: i64 = raw
                .parse()
                .map_err(|_| format!("{} tidak valid.", f.label))?;
            let found = q_one(db, format!("SELECT id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
                vec![Value::from(id)],
                1,
                "memeriksa relasi",
            )
            .await?;
            if found.is_none() {
                return Err(format!("{} tidak ditemukan.", f.label));
            }
        }
        if let Some((table, col)) = f.rule.unique {
            let mut sql = format!("SELECT id FROM {table} WHERE {col} = ?1");
            if exclude_id.is_some() {
                sql.push_str(" AND id != ?2");
            }
            let mut vals = vec![Value::from(raw.to_string())];
            if let Some(ex) = exclude_id {
                vals.push(Value::from(ex));
            }
            let found = q_one(db, sql.clone(), vals, 1, "memeriksa keunikan").await?;
            if found.is_some() {
                return Err(format!("{} sudah dipakai.", f.label));
            }
        }
        cleaned.insert(f.name.to_string(), Some(raw.to_string()));
    }
    Ok(cleaned)
}

/// Simpan (buat/ubah). Kembalikan id.
pub async fn save(
    db: &sea_orm::DatabaseConnection,
    slug: &str,
    id: Option<i64>,
    values: &BTreeMap<String, String>,
) -> Result<i32, String> {
    let ent = entity(slug)?;
    let cleaned = validate(db, ent, values, id).await?;
    if let Some(rid) = id {
        let sets: Vec<String> = cleaned.keys().map(|c| format!("{c} = ?")).collect();
        let mut vals: Vec<Value> = cleaned
            .values()
            .map(|v| match v {
                Some(s) => Value::from(s.clone()),
                None => Value::Null,
            })
            .collect();
        vals.push(Value::from(rid));
        let n = exec(
            db, format!("UPDATE {} SET {} WHERE id = ?", ent.table, sets.join(", ")),
            vals,
            "menyimpan",
        )
        .await?;
        if n == 0 {
            return Err("Data tidak ditemukan.".to_string());
        }
        to_dto_int(rid, "org.id")
    } else {
        let cols: Vec<String> = cleaned.keys().cloned().collect();
        let holders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let vals: Vec<Value> = cleaned
            .values()
            .map(|v| match v {
                Some(s) => Value::from(s.clone()),
                None => Value::Null,
            })
            .collect();
        let rid = exec_insert(
            db, format!(
                "INSERT INTO {} ({}) VALUES ({})",
                ent.table,
                cols.join(", "),
                holders.join(", ")
            ),
            vals,
            "menyimpan",
        )
        .await
        .map_err(|e| {
            if e.contains("UNIQUE") {
                "Data duplikat (batas unik dilanggar).".to_string()
            } else {
                e
            }
        })?;
        to_dto_int(rid, "org.id")
    }
}

/// Hapus lunak dengan guard relasi.
pub async fn delete(db: &sea_orm::DatabaseConnection, slug: &str, id: i64) -> Result<(), String> {
    let ent = entity(slug)?;
    if let Some((table, column)) = ent.guard {
        let rows = q_all(db, format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1 AND deleted_at IS NULL"),
            vec![Value::from(id)],
            1,
            "memeriksa relasi",
        )
        .await?;
        if value_i64(&rows[0][0]).unwrap_or(0) > 0 {
            return Err(
                "Data tidak dapat dihapus karena masih digunakan oleh data lain.".to_string(),
            );
        }
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = exec(
        db, format!(
            "UPDATE {} SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            ent.table
        ),
        vec![Value::from(now), Value::from(id)],
        "menghapus",
    )
    .await?;
    if n == 0 {
        return Err("Data tidak ditemukan.".to_string());
    }
    Ok(())
}

/// Ambil satu baris (termasuk label join) untuk form ubah.
pub async fn get(
    db: &sea_orm::DatabaseConnection,
    slug: &str,
    id: i64,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let ent = entity(slug)?;
    let mut row = match read_row(db, ent, id).await? {
        Some(r) => r,
        None => return Ok(None),
    };
    for j in ent.joins {
        let label = match row.get(j.fk).map(|s| s.as_str()) {
            Some("") | None => "-".to_string(),
            Some(raw) => match raw.parse::<i64>() {
                Ok(rid) => {
                    let found = q_one(db, format!("SELECT {} FROM {} WHERE id = ?1", j.label_col, j.table),
                        vec![Value::from(rid)],
                        1,
                        "memuat label",
                    )
                    .await
                    .unwrap_or(None);
                    found
                        .map(|v| value_to_string(&v[0]))
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "-".to_string())
                }
                Err(_) => "-".to_string(),
            },
        };
        row.insert(j.alias.to_string(), label);
    }
    Ok(Some(row))
}


/// Node struktur untuk tampilan bagan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DivisionNode {
    pub id: i32,
    pub name: String,
}
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DepartmentNode {
    pub id: i32,
    pub name: String,
    pub head_name: Option<String>,
    pub employee_count: i32,
    pub divisions: Vec<DivisionNode>,
}
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct BranchNode {
    pub id: i32,
    pub name: String,
    pub departments: Vec<DepartmentNode>,
}
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CompanyNode {
    pub id: i32,
    pub name: String,
    pub employee_count: i32,
    pub branches: Vec<BranchNode>,
}

pub async fn org_chart(db: &sea_orm::DatabaseConnection) -> Result<Vec<CompanyNode>, String> {
    let companies = q_all(
        db,
        "SELECT id, name FROM companies WHERE deleted_at IS NULL ORDER BY name".to_string(),
        vec![],
        2,
        "memuat perusahaan",
    )
    .await?;
    let mut out = Vec::new();
    for row in companies {
        let cid = value_i64(&row[0]).unwrap_or(0);
        let cname = value_to_string(&row[1]);
        let emp = q_all(
            db,
            "SELECT COUNT(*) FROM employees WHERE company_id = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::from(cid)],
            1,
            "menghitung karyawan",
        )
        .await?;
        let emp = value_i64(&emp[0][0]).unwrap_or(0);
        let mut branches = Vec::new();
        let brows = q_all(
            db,
            "SELECT id, name FROM branches WHERE company_id = ?1 AND deleted_at IS NULL ORDER BY name".to_string(),
            vec![Value::from(cid)],
            2,
            "memuat cabang",
        )
        .await?;
        for brow in brows {
            let bid = value_i64(&brow[0]).unwrap_or(0);
            let bname = value_to_string(&brow[1]);
            let mut departments = Vec::new();
            let drows = q_all(
                db,
                "SELECT id, name, head_employee_id FROM departments WHERE branch_id = ?1 AND deleted_at IS NULL ORDER BY name".to_string(),
                vec![Value::from(bid)],
                3,
                "memuat departemen",
            )
            .await?;
            for drow in drows {
                let did = value_i64(&drow[0]).unwrap_or(0);
                let dname = value_to_string(&drow[1]);
                let head = value_i64(&drow[2]);
                let head_name: Option<String> = match head {
                    Some(hid) => {
                        q_one(
                            db,
                            "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
                            vec![Value::from(hid)],
                            1,
                            "memuat kepala",
                        )
                        .await?
                        .map(|v| value_to_string(&v[0]))
                    }
                    None => None,
                };
                let demp = q_all(
                    db,
                    "SELECT COUNT(*) FROM employees WHERE department_id = ?1 AND deleted_at IS NULL".to_string(),
                    vec![Value::from(did)],
                    1,
                    "menghitung karyawan",
                )
                .await?;
                let demp = value_i64(&demp[0][0]).unwrap_or(0);
                let mut divisions = Vec::new();
                let vrows = q_all(
                    db,
                    "SELECT id, name FROM divisions WHERE department_id = ?1 AND deleted_at IS NULL ORDER BY name".to_string(),
                    vec![Value::from(did)],
                    2,
                    "memuat divisi",
                )
                .await?;
                for vrow in vrows {
                    divisions.push(DivisionNode {
                        id: to_dto_int(value_i64(&vrow[0]).unwrap_or(0), "division.id")?,
                        name: value_to_string(&vrow[1]),
                    });
                }
                departments.push(DepartmentNode {
                    id: to_dto_int(did, "department.id")?,
                    name: dname,
                    head_name,
                    employee_count: to_dto_int(demp, "department.employees")?,
                    divisions,
                });
            }
            branches.push(BranchNode {
                id: to_dto_int(bid, "branch.id")?,
                name: bname,
                departments,
            });
        }
        out.push(CompanyNode {
            id: to_dto_int(cid, "company.id")?,
            name: cname,
            employee_count: to_dto_int(emp, "company.employees")?,
            branches,
        });
    }
    Ok(out)
}


#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct RegionComplianceRow {
    pub work_location_id: Option<i32>,
    pub location_name: String,
    pub total: i32,
    pub ok_count: i32,
    pub violation_count: i32,
    pub verdict: String,
}

pub async fn compliance_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    work_location_id: Option<i64>,
    item: &str,
    status: &str,
    notes: Option<&str>,
) -> Result<i32, String> {
    if item.trim().is_empty() {
        return Err("Item wajib diisi.".to_string());
    }
    if !["pending", "ok", "violation"].contains(&status) {
        return Err("Status kepatuhan tidak valid.".to_string());
    }
    if let Some(w) = work_location_id {
        let ada = q_one(
            db,
            "SELECT id FROM work_locations WHERE id = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::Int(w)],
            1,
            "cmp.loc",
        )
        .await?;
        if ada.is_none() {
            return Err("Wilayah kerja tidak ditemukan.".to_string());
        }
    }
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let loc = match work_location_id {
        Some(v) => Value::Int(v),
        None => Value::Null,
    };
    let cat = match notes {
        Some(v) => Value::Text(v.to_string()),
        None => Value::Null,
    };
    let new_id = match id {
        Some(x) => {
            let n = exec(
                db,
                "UPDATE compliance_checks SET work_location_id = ?1, item = ?2, status = ?3, notes = ?4, checked_at = ?5, updated_at = ?6 WHERE id = ?7".to_string(),
                vec![
                    loc,
                    Value::Text(item.trim().to_string()),
                    Value::Text(status.to_string()),
                    cat,
                    Value::Text(now),
                    Value::Int(x),
                ],
                "cmp.upd",
            )
            .await?;
            if n == 0 {
                return Err("Pemeriksaan tidak ditemukan.".to_string());
            }
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO compliance_checks (work_location_id, item, status, notes, checked_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)".to_string(),
                vec![
                    loc,
                    Value::Text(item.trim().to_string()),
                    Value::Text(status.to_string()),
                    cat,
                    Value::Text(now.clone()),
                    Value::Text(now.clone()),
                    Value::Text(now),
                ],
                "cmp.ins",
            )
            .await?
        }
    };
    audit::log_sea(
        db,
        Some(actor_id),
        if id.is_some() { "UPDATE" } else { "CREATE" },
        "organization.compliance",
        Some(&new_id.to_string()),
        None,
        None,
        Some("Pemeriksaan kepatuhan disimpan."),
    )
    .await?;
    to_dto_int(new_id, "cmp.id")
}

pub async fn compliance_status_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<RegionComplianceRow>, String> {
    let rows = q_all(
        db,
        "SELECT w.id, w.name, COUNT(c.id), SUM(CASE WHEN c.status = 'ok' THEN 1 ELSE 0 END), SUM(CASE WHEN c.status = 'violation' THEN 1 ELSE 0 END), SUM(CASE WHEN c.status = 'pending' THEN 1 ELSE 0 END) FROM work_locations w LEFT JOIN compliance_checks c ON c.work_location_id = w.id WHERE w.deleted_at IS NULL GROUP BY w.id ORDER BY w.id".to_string(),
        vec![],
        6,
        "cmp.status",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let tot = value_i64(&r[2]).unwrap_or(0);
        let vok = value_i64(&r[3]).unwrap_or(0);
        let vio = value_i64(&r[4]).unwrap_or(0);
        let pen = value_i64(&r[5]).unwrap_or(0);
        out.push(RegionComplianceRow {
            work_location_id: match &r[0] {
                Value::Null => None,
                v => Some(to_dto_int(value_i64(v).unwrap_or(0), "cmp.loc")?),
            },
            location_name: value_to_string(&r[1]),
            total: to_dto_int(tot, "cmp.total")?,
            ok_count: to_dto_int(vok, "cmp.ok")?,
            violation_count: to_dto_int(vio, "cmp.vio")?,
            verdict: if vio > 0 {
                "bertindak".to_string()
            } else if pen > 0 {
                "berjalan".to_string()
            } else {
                "patuh".to_string()
            },
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_state;

    fn vals(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[tokio::test]
    async fn entitas_lengkap_dan_daftar_terisi() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        assert_eq!(entities().len(), 10);
        let page = list(db, "departments", "", 1, 20).await.expect("list");
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 3);
        let search = list(db, "departments", "tech", 1, 20).await.expect("cari");
        assert_eq!(search.total, 1);
        assert_eq!(search.rows[0]["company_name"], "PT Contoh Sukses Indonesia");
    }

    #[tokio::test]
    async fn validasi_menolak_dan_guard_melindungi() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        // wajib diisi
        let e = save(db, "branches", None, &vals(&[("code", "X")]))
            .await
            .expect_err("company wajib");
        assert!(e.contains("wajib diisi"));
        // exists
        let e = save(
            db,
            "branches",
            None,
            &vals(&[("company_id", "9999"), ("code", "X"), ("name", "X")]),
        )
        .await
        .expect_err("company tak ada");
        assert!(e.contains("tidak ditemukan"));
        // duplikat unik
        let e = save(
            db,
            "branches",
            None,
            &vals(&[("company_id", "1"), ("code", "HO"), ("name", "Duplikat")]),
        )
        .await
        .expect_err("duplikat");
        assert!(e.contains("duplikat") || e.contains("dipakai"));
        // guard: HQ dipakai branch
        let e = delete(db, "companies", 1).await.expect_err("guard");
        assert!(e.contains("digunakan"));
        // buat lalu hapus cost center tak terpakai
        let id = save(
            db,
            "cost-centers",
            None,
            &vals(&[("code", "CC-TMP"), ("name", "Sementara")]),
        )
        .await
        .expect("buat");
        let row = get(db, "cost-centers", id as i64)
            .await
            .expect("get")
            .expect("ada");
        assert_eq!(row["code"], "CC-TMP");
        delete(db, "cost-centers", id as i64).await.expect("hapus");
        assert!(get(db, "cost-centers", id as i64)
            .await
            .expect("get")
            .is_none());
    }

    #[tokio::test]
    async fn opsi_dan_bagan_terbentuk() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let opts = options(db, "branches", "company_id").await.expect("opsi");
        assert!(!opts.is_empty());
        assert!(options(db, "branches", "code").await.is_err());
        let chart = org_chart(db).await.expect("bagan");
        assert_eq!(chart.len(), 1);
        assert_eq!(chart[0].branches.len(), 1);
        assert_eq!(chart[0].branches[0].departments.len(), 3);
        assert!(chart[0].employee_count >= 1);
    }

    #[tokio::test]
    async fn kepatuhan_teragregasi_per_wilayah() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let adm = q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "t.admin",
        )
        .await
        .expect("admin")
        .expect("baris");
        let admin = value_i64(&adm[0]).expect("uid");
        let wjak = exec_insert(
            db,
            "INSERT INTO work_locations (name, address) VALUES ('Jakarta', 'Jalan Satu')".to_string(),
            vec![],
            "t.wj",
        )
        .await
        .expect("jakarta");
        let wbdg = exec_insert(
            db,
            "INSERT INTO work_locations (name, address) VALUES ('Bandung', 'Jalan Dua')".to_string(),
            vec![],
            "t.wb",
        )
        .await
        .expect("bandung");
        compliance_save_sea(db, admin, None, Some(wjak), "Upah minimum", "ok", None)
            .await
            .expect("ok");
        compliance_save_sea(db, admin, None, Some(wjak), "BPJS", "violation", Some("tunggak"))
            .await
            .expect("vio");
        compliance_save_sea(db, admin, None, Some(wbdg), "Upah minimum", "pending", None)
            .await
            .expect("pen");
        let salah = compliance_save_sea(db, admin, None, Some(wjak), "X", "aneh", None)
            .await
            .expect_err("status");
        assert!(salah.contains("Status kepatuhan tidak valid."));
        let ghost = compliance_save_sea(db, admin, None, Some(999999), "X", "ok", None)
            .await
            .expect_err("lokasi");
        assert!(ghost.contains("Wilayah kerja tidak ditemukan."));
        let rows = compliance_status_sea(db).await.expect("status");
        let j = rows
            .iter()
            .find(|r| r.work_location_id == Some(wjak as i32))
            .expect("jkt");
        assert_eq!(j.total, 2);
        assert_eq!(j.ok_count, 1);
        assert_eq!(j.violation_count, 1);
        assert_eq!(j.verdict, "bertindak");
        let b = rows
            .iter()
            .find(|r| r.work_location_id == Some(wbdg as i32))
            .expect("bdg");
        assert_eq!(b.verdict, "berjalan");
    }
}

