//! Master organisasi: 10 entitas lewat satu engine konfigurasi.

use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

use super::audit;
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
pub fn options(conn: &Connection, slug: &str, field: &str) -> Result<Vec<Opt>, String> {
    let ent = entity(slug)?;
    let f = ent
        .fields
        .iter()
        .find(|f| f.name == field && f.field_type == "select")
        .ok_or("Field opsi tidak dikenal.".to_string())?;
    let sql = f
        .options_sql
        .ok_or("Field ini tidak punya opsi.".to_string())?;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("gagal menyiapkan opsi: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            Ok((id, r.get::<_, String>(1)?))
        })
        .map_err(|e| format!("gagal membaca opsi: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        let (id, name) = row.map_err(|e| format!("gagal membaca baris opsi: {e}"))?;
        out.push(Opt {
            id: to_dto_int(id, "option.id")?,
            name,
        });
    }
    Ok(out)
}

fn cell_to_string(v: rusqlite::types::Value) -> String {
    use rusqlite::types::Value::*;
    match v {
        Null => String::new(),
        Integer(i) => i.to_string(),
        Real(f) => {
            if f.fract() == 0.0 {
                format!("{}", f as i64)
            } else {
                f.to_string()
            }
        }
        Text(t) => t,
        Blob(_) => String::new(),
    }
}

fn read_row(
    conn: &Connection,
    ent: &Entity,
    id: i64,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let cols: Vec<String> = ent.fields.iter().map(|f| f.name.to_string()).collect();
    let select = format!("id, {}", cols.join(", "));
    let row: Option<Vec<rusqlite::types::Value>> = conn
        .query_row(
            &format!(
                "SELECT {select} FROM {} WHERE id = ?1 AND deleted_at IS NULL",
                ent.table
            ),
            params![id],
            |r| {
                let mut v = Vec::with_capacity(cols.len() + 1);
                for i in 0..=cols.len() {
                    v.push(r.get(i)?);
                }
                Ok(v)
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat data: {e}"))?;
    Ok(row.map(|vals| {
        let mut map = BTreeMap::new();
        map.insert("id".to_string(), cell_to_string(vals[0].clone()));
        for (i, col) in cols.iter().enumerate() {
            map.insert(col.clone(), cell_to_string(vals[i + 1].clone()));
        }
        map
    }))
}

/// Daftar + cari (hanya kolom code/name) + paginasi + label join.
pub fn list(
    conn: &Connection,
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
    let where_sql = if search.is_empty() || searchable.is_empty() {
        "deleted_at IS NULL".to_string()
    } else {
        let conds: Vec<String> = searchable.iter().map(|c| format!("{c} LIKE ?1")).collect();
        format!("deleted_at IS NULL AND ({})", conds.join(" OR "))
    };
    let like = format!("%{search}%");

    let total: i64 = if search.is_empty() || searchable.is_empty() {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {} WHERE {where_sql}", ent.table),
            [],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung: {e}"))?
    } else {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM {} WHERE {where_sql}", ent.table),
            params![like],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal menghitung: {e}"))?
    };

    let cols: Vec<&str> = ent.fields.iter().map(|f| f.name).collect();
    let select = format!("id, {}", cols.join(", "));
    let has_search = !search.is_empty() && !searchable.is_empty();
    let tail = if has_search {
        "LIMIT ?2 OFFSET ?3"
    } else {
        "LIMIT ?1 OFFSET ?2"
    };
    let sql = format!(
        "SELECT {select} FROM {} WHERE {where_sql} ORDER BY {} {tail}",
        ent.table, ent.order_by
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("gagal menyiapkan daftar: {e}"))?;
    let mut rows: Vec<BTreeMap<String, String>> = Vec::new();
    if search.is_empty() || searchable.is_empty() {
        let mapped = stmt
            .query_map(params![per_page as i64, offset], |r| {
                let mut v: Vec<rusqlite::types::Value> = Vec::with_capacity(cols.len() + 1);
                for i in 0..=cols.len() {
                    v.push(r.get(i)?);
                }
                Ok(v)
            })
            .map_err(|e| format!("gagal membaca daftar: {e}"))?;
        for row in mapped {
            let vals = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
            let mut map = BTreeMap::new();
            map.insert("id".to_string(), cell_to_string(vals[0].clone()));
            for (i, col) in cols.iter().enumerate() {
                map.insert(col.to_string(), cell_to_string(vals[i + 1].clone()));
            }
            rows.push(map);
        }
    } else {
        let mapped = stmt
            .query_map(params![like, per_page as i64, offset], |r| {
                let mut v: Vec<rusqlite::types::Value> = Vec::with_capacity(cols.len() + 1);
                for i in 0..=cols.len() {
                    v.push(r.get(i)?);
                }
                Ok(v)
            })
            .map_err(|e| format!("gagal membaca daftar: {e}"))?;
        for row in mapped {
            let vals = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
            let mut map = BTreeMap::new();
            map.insert("id".to_string(), cell_to_string(vals[0].clone()));
            for (i, col) in cols.iter().enumerate() {
                map.insert(col.to_string(), cell_to_string(vals[i + 1].clone()));
            }
            rows.push(map);
        }
    }

    // label join
    for row in rows.iter_mut() {
        for j in ent.joins {
            let label = match row.get(j.fk).map(|s| s.as_str()) {
                Some("") | None => "-".to_string(),
                Some(raw) => raw
                    .parse::<i64>()
                    .ok()
                    .and_then(|id| {
                        conn.query_row(
                            &format!("SELECT {} FROM {} WHERE id = ?1", j.label_col, j.table),
                            params![id],
                            |r| r.get::<_, String>(0),
                        )
                        .optional()
                        .unwrap_or(None)
                    })
                    .unwrap_or_else(|| "-".to_string()),
            };
            row.insert(j.alias.to_string(), label);
        }
    }

    Ok(OrgPage {
        rows,
        total: to_dto_int(total, "org.total")?,
    })
}

fn validate(
    conn: &Connection,
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
            let found: Option<i64> = conn
                .query_row(
                    &format!("SELECT id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
                    params![id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| format!("gagal memeriksa {}: {e}", f.label))?;
            if found.is_none() {
                return Err(format!("{} tidak ditemukan.", f.label));
            }
        }
        if let Some((table, col)) = f.rule.unique {
            let mut sql = format!("SELECT id FROM {table} WHERE {col} = ?1");
            if exclude_id.is_some() {
                sql.push_str(" AND id != ?2");
            }
            let found: Option<i64> = if let Some(ex) = exclude_id {
                conn.query_row(&sql, params![raw, ex], |r| r.get(0))
                    .optional()
            } else {
                conn.query_row(&sql, params![raw], |r| r.get(0)).optional()
            }
            .map_err(|e| format!("gagal memeriksa keunikan: {e}"))?;
            if found.is_some() {
                return Err(format!("{} sudah dipakai.", f.label));
            }
        }
        cleaned.insert(f.name.to_string(), Some(raw.to_string()));
    }
    Ok(cleaned)
}

/// Simpan (buat/ubah). Kembalikan id.
pub fn save(
    conn: &Connection,
    actor_id: i64,
    slug: &str,
    id: Option<i64>,
    values: &BTreeMap<String, String>,
) -> Result<i32, String> {
    let ent = entity(slug)?;
    let cleaned = validate(conn, ent, values, id)?;
    let module = format!("organization.{slug}");
    if let Some(rid) = id {
        let before = read_row(conn, ent, rid)?.unwrap_or_default();
        let sets: Vec<String> = cleaned.keys().map(|c| format!("{c} = ?")).collect();
        // rusqlite params dinamis: rakit Vec<Value>
        let mut vals: Vec<rusqlite::types::Value> = cleaned
            .values()
            .map(|v| match v {
                Some(s) => rusqlite::types::Value::Text(s.clone()),
                None => rusqlite::types::Value::Null,
            })
            .collect();
        vals.push(rusqlite::types::Value::Integer(rid));
        let refs: Vec<&dyn rusqlite::ToSql> =
            vals.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        let n = conn
            .execute(
                &format!("UPDATE {} SET {} WHERE id = ?", ent.table, sets.join(", ")),
                refs.as_slice(),
            )
            .map_err(|e| format!("gagal menyimpan: {e}"))?;
        if n == 0 {
            return Err("Data tidak ditemukan.".to_string());
        }
        let after = serde_json::to_string(&cleaned).unwrap_or_default();
        let before_json = serde_json::to_string(&before).unwrap_or_default();
        audit::log(
            conn,
            Some(actor_id),
            "UPDATE",
            &module,
            Some(&rid.to_string()),
            Some(&before_json),
            Some(&after),
            None,
        )?;
        to_dto_int(rid, "org.id")
    } else {
        let cols: Vec<String> = cleaned.keys().cloned().collect();
        let holders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let vals: Vec<rusqlite::types::Value> = cleaned
            .values()
            .map(|v| match v {
                Some(s) => rusqlite::types::Value::Text(s.clone()),
                None => rusqlite::types::Value::Null,
            })
            .collect();
        let refs: Vec<&dyn rusqlite::ToSql> =
            vals.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        conn.execute(
            &format!(
                "INSERT INTO {} ({}) VALUES ({})",
                ent.table,
                cols.join(", "),
                holders.join(", ")
            ),
            refs.as_slice(),
        )
        .map_err(|e| {
            if e.to_string().contains("UNIQUE") {
                "Data duplikat (batas unik dilanggar).".to_string()
            } else {
                format!("gagal menyimpan: {e}")
            }
        })?;
        let rid = conn.last_insert_rowid();
        let after = serde_json::to_string(&cleaned).unwrap_or_default();
        audit::log(
            conn,
            Some(actor_id),
            "CREATE",
            &module,
            Some(&rid.to_string()),
            None,
            Some(&after),
            None,
        )?;
        to_dto_int(rid, "org.id")
    }
}

/// Hapus lunak dengan guard relasi.
pub fn delete(conn: &Connection, actor_id: i64, slug: &str, id: i64) -> Result<(), String> {
    let ent = entity(slug)?;
    if let Some((table, column)) = ent.guard {
        let n: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?1 AND deleted_at IS NULL"),
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("gagal memeriksa relasi: {e}"))?;
        if n > 0 {
            return Err(
                "Data tidak dapat dihapus karena masih digunakan oleh data lain.".to_string(),
            );
        }
    }
    let before = read_row(conn, ent, id)?.unwrap_or_default();
    let n = conn
        .execute(
            &format!("UPDATE {} SET deleted_at = datetime('now','localtime') WHERE id = ?1 AND deleted_at IS NULL", ent.table),
            params![id],
        )
        .map_err(|e| format!("gagal menghapus: {e}"))?;
    if n == 0 {
        return Err("Data tidak ditemukan.".to_string());
    }
    let before_json = serde_json::to_string(&before).unwrap_or_default();
    audit::log(
        conn,
        Some(actor_id),
        "DELETE",
        &format!("organization.{slug}"),
        Some(&id.to_string()),
        Some(&before_json),
        None,
        None,
    )?;
    Ok(())
}

/// Ambil satu baris (termasuk label join) untuk form ubah.
pub fn get(
    conn: &Connection,
    slug: &str,
    id: i64,
) -> Result<Option<BTreeMap<String, String>>, String> {
    let ent = entity(slug)?;
    let mut row = match read_row(conn, ent, id)? {
        Some(r) => r,
        None => return Ok(None),
    };
    for j in ent.joins {
        let label = match row.get(j.fk).map(|s| s.as_str()) {
            Some("") | None => "-".to_string(),
            Some(raw) => raw
                .parse::<i64>()
                .ok()
                .and_then(|rid| {
                    conn.query_row(
                        &format!("SELECT {} FROM {} WHERE id = ?1", j.label_col, j.table),
                        params![rid],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()
                    .unwrap_or(None)
                })
                .unwrap_or_else(|| "-".to_string()),
        };
        row.insert(j.alias.to_string(), label);
    }
    Ok(Some(row))
}

// ---------------- Struktur organisasi ----------------

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

pub fn org_chart(conn: &Connection) -> Result<Vec<CompanyNode>, String> {
    let mut companies: Vec<(i64, String)> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id, name FROM companies WHERE deleted_at IS NULL ORDER BY name")
        .map_err(|e| format!("gagal memuat perusahaan: {e}"))?;
    for row in stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| format!("gagal memuat perusahaan: {e}"))?
    {
        companies.push(row.map_err(|e| format!("gagal membaca perusahaan: {e}"))?);
    }
    let mut out = Vec::new();
    for (cid, cname) in companies {
        let emp: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM employees WHERE company_id = ?1 AND deleted_at IS NULL",
                params![cid],
                |r| r.get(0),
            )
            .map_err(|e| format!("gagal menghitung karyawan: {e}"))?;
        let mut branches = Vec::new();
        let mut bstmt = conn
            .prepare("SELECT id, name FROM branches WHERE company_id = ?1 AND deleted_at IS NULL ORDER BY name")
            .map_err(|e| format!("gagal memuat cabang: {e}"))?;
        let brows = bstmt
            .query_map(params![cid], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| format!("gagal memuat cabang: {e}"))?;
        for brow in brows {
            let (bid, bname) = brow.map_err(|e| format!("gagal membaca cabang: {e}"))?;
            let mut departments = Vec::new();
            let mut dstmt = conn
                .prepare("SELECT id, name, head_employee_id FROM departments WHERE branch_id = ?1 AND deleted_at IS NULL ORDER BY name")
                .map_err(|e| format!("gagal memuat departemen: {e}"))?;
            let drows = dstmt
                .query_map(params![bid], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<i64>>(2)?,
                    ))
                })
                .map_err(|e| format!("gagal memuat departemen: {e}"))?;
            for drow in drows {
                let (did, dname, head) =
                    drow.map_err(|e| format!("gagal membaca departemen: {e}"))?;
                let head_name: Option<String> = match head {
                    Some(hid) => conn
                        .query_row(
                            "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1",
                            params![hid],
                            |r| r.get(0),
                        )
                        .optional()
                        .map_err(|e| format!("gagal memuat kepala: {e}"))?,
                    None => None,
                };
                let demp: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM employees WHERE department_id = ?1 AND deleted_at IS NULL",
                        params![did],
                        |r| r.get(0),
                    )
                    .map_err(|e| format!("gagal menghitung karyawan: {e}"))?;
                let mut divisions = Vec::new();
                let mut vstmt = conn
                    .prepare("SELECT id, name FROM divisions WHERE department_id = ?1 AND deleted_at IS NULL ORDER BY name")
                    .map_err(|e| format!("gagal memuat divisi: {e}"))?;
                for vrow in vstmt
                    .query_map(params![did], |r| {
                        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                    })
                    .map_err(|e| format!("gagal memuat divisi: {e}"))?
                {
                    let (vid, vname) = vrow.map_err(|e| format!("gagal membaca divisi: {e}"))?;
                    divisions.push(DivisionNode {
                        id: to_dto_int(vid, "division.id")?,
                        name: vname,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, seed};

    fn live() -> (tempfile::TempDir, crate::db::DbPool) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        (dir, pool)
    }

    fn actor(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .expect("admin")
    }

    fn vals(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn entitas_lengkap_dan_daftar_terisi() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        assert_eq!(entities().len(), 10);
        let page = list(&conn, "departments", "", 1, 20).expect("list");
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 3);
        let search = list(&conn, "departments", "tech", 1, 20).expect("cari");
        assert_eq!(search.total, 1);
        assert_eq!(search.rows[0]["company_name"], "PT Contoh Sukses Indonesia");
    }

    #[test]
    fn validasi_menolak_dan_guard_melindungi() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor = actor(&conn);
        // wajib diisi
        let e = save(&conn, actor, "branches", None, &vals(&[("code", "X")]))
            .expect_err("company wajib");
        assert!(e.contains("wajib diisi"));
        // exists
        let e = save(
            &conn,
            actor,
            "branches",
            None,
            &vals(&[("company_id", "9999"), ("code", "X"), ("name", "X")]),
        )
        .expect_err("company tak ada");
        assert!(e.contains("tidak ditemukan"));
        // duplikat unik
        let e = save(
            &conn,
            actor,
            "branches",
            None,
            &vals(&[("company_id", "1"), ("code", "HO"), ("name", "Duplikat")]),
        )
        .expect_err("duplikat");
        assert!(e.contains("duplikat") || e.contains("dipakai"));
        // guard: HQ dipakai branch
        let e = delete(&conn, actor, "companies", 1).expect_err("guard");
        assert!(e.contains("digunakan"));
        // buat lalu hapus cost center tak terpakai
        let id = save(
            &conn,
            actor,
            "cost-centers",
            None,
            &vals(&[("code", "CC-TMP"), ("name", "Sementara")]),
        )
        .expect("buat");
        let row = get(&conn, "cost-centers", id as i64)
            .expect("get")
            .expect("ada");
        assert_eq!(row["code"], "CC-TMP");
        delete(&conn, actor, "cost-centers", id as i64).expect("hapus");
        assert!(get(&conn, "cost-centers", id as i64)
            .expect("get")
            .is_none());
    }

    #[test]
    fn opsi_dan_bagan_terbentuk() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let opts = options(&conn, "branches", "company_id").expect("opsi");
        assert!(!opts.is_empty());
        assert!(options(&conn, "branches", "code").is_err());
        let chart = org_chart(&conn).expect("bagan");
        assert_eq!(chart.len(), 1);
        assert_eq!(chart[0].branches.len(), 1);
        assert_eq!(chart[0].branches[0].departments.len(), 3);
        assert!(chart[0].employee_count >= 1);
    }
}
