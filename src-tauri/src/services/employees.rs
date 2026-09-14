//! Manajemen karyawan: profil, child generik, alamat, dokumen, kontrak, gaji.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::audit;
use super::sea_raw::{exec, q_all, q_one, value_i64, value_to_string, Value};
use crate::to_dto_int;

// ---------------- DTO ----------------

/// Baris daftar karyawan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EmployeeRow {
    pub id: i32,
    pub employee_number: String,
    pub first_name: String,
    pub last_name: Option<String>,
    pub photo: Option<String>,
    pub join_date: String,
    pub employment_status: String,
    pub employment_type: String,
    pub department_name: Option<String>,
    pub position_name: Option<String>,
}

/// Detail penuh + label relasi.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EmployeeDetail {
    pub id: i32,
    pub employee_number: String,
    pub nik: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub photo: Option<String>,
    pub birth_place: Option<String>,
    pub birth_date: Option<String>,
    pub gender: String,
    pub religion: Option<String>,
    pub marital_status: String,
    pub phone: Option<String>,
    pub personal_email: Option<String>,
    pub company_id: i32,
    pub branch_id: Option<i32>,
    pub department_id: Option<i32>,
    pub division_id: Option<i32>,
    pub section_id: Option<i32>,
    pub position_id: Option<i32>,
    pub job_level_id: Option<i32>,
    pub job_grade_id: Option<i32>,
    pub work_location_id: Option<i32>,
    pub cost_center_id: Option<i32>,
    pub supervisor_id: Option<i32>,
    pub manager_id: Option<i32>,
    pub join_date: String,
    pub appointment_date: Option<String>,
    pub resign_date: Option<String>,
    pub employment_status: String,
    pub employment_type: String,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_account_holder: Option<String>,
    pub npwp: Option<String>,
    pub ptkp_status: Option<String>,
    pub bpjs_health_number: Option<String>,
    pub bpjs_employment_number: Option<String>,
    pub company_name: Option<String>,
    pub branch_name: Option<String>,
    pub department_name: Option<String>,
    pub division_name: Option<String>,
    pub section_name: Option<String>,
    pub position_name: Option<String>,
    pub job_level_name: Option<String>,
    pub job_grade_name: Option<String>,
    pub work_location_name: Option<String>,
    pub cost_center_name: Option<String>,
    pub supervisor_name: Option<String>,
    pub manager_name: Option<String>,
}

/// Input form karyawan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct EmployeeInput {
    pub nik: Option<String>,
    pub first_name: String,
    pub last_name: Option<String>,
    pub gender: String,
    pub birth_place: Option<String>,
    pub birth_date: Option<String>,
    pub religion: Option<String>,
    pub marital_status: String,
    pub phone: Option<String>,
    pub personal_email: Option<String>,
    pub company_id: i32,
    pub branch_id: Option<i32>,
    pub department_id: Option<i32>,
    pub division_id: Option<i32>,
    pub section_id: Option<i32>,
    pub position_id: Option<i32>,
    pub job_level_id: Option<i32>,
    pub job_grade_id: Option<i32>,
    pub work_location_id: Option<i32>,
    pub cost_center_id: Option<i32>,
    pub supervisor_id: Option<i32>,
    pub manager_id: Option<i32>,
    pub join_date: String,
    pub appointment_date: Option<String>,
    pub employment_status: String,
    pub employment_type: String,
    pub bank_name: Option<String>,
    pub bank_account_number: Option<String>,
    pub bank_account_holder: Option<String>,
    pub npwp: Option<String>,
    pub ptkp_status: Option<String>,
    pub bpjs_health_number: Option<String>,
    pub bpjs_employment_number: Option<String>,
}

/// Filter daftar.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct EmployeeFilter {
    pub department_id: Option<i32>,
    pub position_id: Option<i32>,
    pub branch_id: Option<i32>,
    pub employment_status: Option<String>,
    pub gender: Option<String>,
    pub employment_type: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EmployeePage {
    pub rows: Vec<EmployeeRow>,
    pub total: i32,
}

/// Berkas dari frontend (dibaca via input file, ditulis Rust ke data dir).
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct FileUpload {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// Opsi dropdown form karyawan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct NamedOpt {
    pub id: i32,
    pub name: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Dropdowns {
    pub companies: Vec<NamedOpt>,
    pub branches: Vec<NamedOpt>,
    pub departments: Vec<NamedOpt>,
    pub divisions: Vec<NamedOpt>,
    pub sections: Vec<NamedOpt>,
    pub positions: Vec<NamedOpt>,
    pub job_levels: Vec<NamedOpt>,
    pub job_grades: Vec<NamedOpt>,
    pub work_locations: Vec<NamedOpt>,
    pub cost_centers: Vec<NamedOpt>,
    pub employees: Vec<NamedOpt>,
}

/// Opsi statis untuk form child.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct StaticOpt {
    pub value: String,
    pub label: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ChildField {
    pub name: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub options: Option<Vec<StaticOpt>>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ChildMeta {
    pub slug: String,
    pub title: String,
    pub fields: Vec<ChildField>,
}

/// Dokumen karyawan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Document {
    pub id: i32,
    pub category: String,
    pub name: String,
    pub file_path: String,
    pub file_size: Option<i32>,
    pub mime_type: Option<String>,
    pub expiry_date: Option<String>,
    pub created_at: String,
}

/// Isi berkas untuk unduh/pratinjau.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DocumentBytes {
    pub mime: String,
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Satu baris alamat.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct AddressRow {
    pub address: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
}

/// Pasangan alamat KTP dan domisili.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct Addresses {
    pub ktp: Option<AddressRow>,
    pub domicile: Option<AddressRow>,
}

/// Baris gaji pokok.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct SalaryRow {
    pub id: i32,
    pub basic_salary: f64,
    pub effective_date: String,
    pub is_active: bool,
}

/// Komponen gaji terpasang + katalog.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct SalaryComponentRow {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub component_type: String,
    pub amount: Option<f64>,
}

// ---------------- Konfigurasi child ----------------

struct ChildFieldDef {
    name: &'static str,
    label: &'static str,
    field_type: &'static str,
    required: bool,
    max_len: Option<usize>,
    in_values: Option<&'static [&'static str]>,
    unique: bool,
    is_date: bool,
    is_integer: bool,
    is_numeric: bool,
    static_options: Option<&'static [(&'static str, &'static str)]>,
}

struct ChildDef {
    slug: &'static str,
    title: &'static str,
    table: &'static str,
    soft_delete: bool,
    fields: &'static [ChildFieldDef],
}

const CHILD_TYPES: &[ChildDef] = &[
    ChildDef {
        slug: "families",
        title: "Keluarga",
        table: "employee_families",
        soft_delete: false,
        fields: &[
            ChildFieldDef {
                name: "name",
                label: "Nama",
                field_type: "text",
                required: true,
                max_len: Some(150),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "relationship",
                label: "Hubungan",
                field_type: "select",
                required: true,
                max_len: None,
                in_values: Some(&["spouse", "child", "other"]),
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: Some(&[
                    ("spouse", "Pasangan"),
                    ("child", "Anak"),
                    ("other", "Lainnya"),
                ]),
            },
            ChildFieldDef {
                name: "birth_date",
                label: "Tanggal Lahir",
                field_type: "date",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: true,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "occupation",
                label: "Pekerjaan",
                field_type: "text",
                required: false,
                max_len: Some(100),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "is_dependent",
                label: "Tanggungan",
                field_type: "checkbox",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
        ],
    },
    ChildDef {
        slug: "educations",
        title: "Pendidikan",
        table: "employee_educations",
        soft_delete: false,
        fields: &[
            ChildFieldDef {
                name: "level",
                label: "Jenjang",
                field_type: "select",
                required: true,
                max_len: None,
                in_values: Some(&[
                    "sd", "smp", "sma", "smk", "d1", "d2", "d3", "d4", "s1", "s2", "s3",
                ]),
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: Some(&[
                    ("sd", "SD"),
                    ("smp", "SMP"),
                    ("sma", "SMA"),
                    ("smk", "SMK"),
                    ("d1", "D1"),
                    ("d2", "D2"),
                    ("d3", "D3"),
                    ("d4", "D4"),
                    ("s1", "S1"),
                    ("s2", "S2"),
                    ("s3", "S3"),
                ]),
            },
            ChildFieldDef {
                name: "school_name",
                label: "Nama Sekolah/Universitas",
                field_type: "text",
                required: true,
                max_len: Some(150),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "major",
                label: "Jurusan",
                field_type: "text",
                required: false,
                max_len: Some(100),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "graduation_year",
                label: "Tahun Lulus",
                field_type: "number",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: true,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "gpa",
                label: "IPK",
                field_type: "number",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: true,
                static_options: None,
            },
        ],
    },
    ChildDef {
        slug: "experiences",
        title: "Pengalaman Kerja",
        table: "employee_experiences",
        soft_delete: false,
        fields: &[
            ChildFieldDef {
                name: "company_name",
                label: "Perusahaan",
                field_type: "text",
                required: true,
                max_len: Some(150),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "position",
                label: "Jabatan",
                field_type: "text",
                required: false,
                max_len: Some(150),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "start_date",
                label: "Tanggal Mulai",
                field_type: "date",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: true,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "end_date",
                label: "Tanggal Selesai",
                field_type: "date",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: true,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "description",
                label: "Deskripsi",
                field_type: "textarea",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
        ],
    },
    ChildDef {
        slug: "contacts",
        title: "Kontak Darurat",
        table: "employee_contacts",
        soft_delete: false,
        fields: &[
            ChildFieldDef {
                name: "name",
                label: "Nama",
                field_type: "text",
                required: true,
                max_len: Some(150),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "relationship",
                label: "Hubungan",
                field_type: "text",
                required: false,
                max_len: Some(50),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "phone",
                label: "Telepon",
                field_type: "text",
                required: false,
                max_len: Some(30),
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "address",
                label: "Alamat",
                field_type: "textarea",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
        ],
    },
    ChildDef {
        slug: "contracts",
        title: "Kontrak Kerja",
        table: "employee_contracts",
        soft_delete: true,
        fields: &[
            ChildFieldDef {
                name: "contract_number",
                label: "Nomor Kontrak",
                field_type: "text",
                required: true,
                max_len: Some(50),
                in_values: None,
                unique: true,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "type",
                label: "Jenis",
                field_type: "select",
                required: true,
                max_len: None,
                in_values: Some(&["probation", "pkwt", "pkwtt"]),
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: Some(&[
                    ("probation", "Probation"),
                    ("pkwt", "PKWT"),
                    ("pkwtt", "PKWTT"),
                ]),
            },
            ChildFieldDef {
                name: "start_date",
                label: "Tanggal Mulai",
                field_type: "date",
                required: true,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: true,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "end_date",
                label: "Tanggal Berakhir",
                field_type: "date",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: true,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "status",
                label: "Status",
                field_type: "select",
                required: true,
                max_len: None,
                in_values: Some(&["active", "expired", "terminated", "renewed"]),
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: Some(&[
                    ("active", "Aktif"),
                    ("expired", "Berakhir"),
                    ("terminated", "Diakhiri"),
                    ("renewed", "Diperpanjang"),
                ]),
            },
            ChildFieldDef {
                name: "notes",
                label: "Catatan",
                field_type: "textarea",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
        ],
    },
    ChildDef {
        slug: "career-histories",
        title: "Riwayat Karier",
        table: "employee_career_histories",
        soft_delete: false,
        fields: &[
            ChildFieldDef {
                name: "type",
                label: "Jenis Perubahan",
                field_type: "select",
                required: true,
                max_len: None,
                in_values: Some(&[
                    "promotion",
                    "mutation",
                    "transfer",
                    "demotion",
                    "position_change",
                    "department_change",
                    "salary_change",
                    "supervisor_change",
                ]),
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: Some(&[
                    ("promotion", "Promosi"),
                    ("mutation", "Mutasi"),
                    ("transfer", "Transfer"),
                    ("demotion", "Demosi"),
                    ("position_change", "Perubahan Jabatan"),
                    ("department_change", "Perubahan Departemen"),
                    ("salary_change", "Perubahan Gaji"),
                    ("supervisor_change", "Perubahan Supervisor"),
                ]),
            },
            ChildFieldDef {
                name: "effective_date",
                label: "Tanggal Efektif",
                field_type: "date",
                required: true,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: true,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
            ChildFieldDef {
                name: "notes",
                label: "Catatan",
                field_type: "textarea",
                required: false,
                max_len: None,
                in_values: None,
                unique: false,
                is_date: false,
                is_integer: false,
                is_numeric: false,
                static_options: None,
            },
        ],
    },
];

fn child_def(slug: &str) -> Result<&'static ChildDef, String> {
    CHILD_TYPES
        .iter()
        .find(|c| c.slug == slug)
        .ok_or_else(|| "Tipe data tidak dikenal.".to_string())
}

/// Metadata tipe child untuk form dinamis.
/// Kontrak aktif yang berakhir dalam N hari ke depan (untuk alert jatuh tempo).
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ContractAlert {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub employee_number: String,
    pub contract_number: String,
    pub contract_type: String,
    pub end_date: String,
    pub days_left: i32,
}

pub async fn expiring_contracts_sea(
    db: &sea_orm::DatabaseConnection,
    days: i64,
) -> Result<Vec<ContractAlert>, String> {
    if days < 1 || days > 365 {
        return Err("Rentang hari harus 1 sampai 365.".to_string());
    }
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let rows = q_all(
        db,
        "SELECT c.id, c.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), e.employee_number, c.contract_number, c.type, c.end_date FROM employee_contracts c INNER JOIN employees e ON e.id = c.employee_id WHERE c.status = 'active' AND c.deleted_at IS NULL AND c.end_date IS NOT NULL AND c.end_date >= ?1 ORDER BY c.end_date ASC".to_string(),
        vec![Value::Text(today.clone())],
        7,
        "employee.expiring",
    )
    .await
    .map_err(|e| format!("gagal membaca kontrak: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let (id, emp, end) = (
            value_i64(&r[0]).unwrap_or(0),
            value_i64(&r[1]).unwrap_or(0),
            value_to_string(&r[6]),
        );
        let left = (chrono::NaiveDate::parse_from_str(&end, "%Y-%m-%d")
            .map_err(|_| "Tanggal kontrak rusak.".to_string())?
            - chrono::NaiveDate::parse_from_str(&today, "%Y-%m-%d")
                .map_err(|_| "Tanggal hari ini rusak.".to_string())?)
        .num_days();
        if left <= days {
            out.push(ContractAlert {
                id: to_dto_int(id, "contract.id")?,
                employee_id: to_dto_int(emp, "contract.emp")?,
                employee_name: value_to_string(&r[2]),
                employee_number: value_to_string(&r[3]),
                contract_number: value_to_string(&r[4]),
                contract_type: value_to_string(&r[5]),
                end_date: end,
                days_left: left as i32,
            });
        }
    }
    Ok(out)
}

pub fn child_types() -> Vec<ChildMeta> {
    CHILD_TYPES
        .iter()
        .map(|c| ChildMeta {
            slug: c.slug.to_string(),
            title: c.title.to_string(),
            fields: c
                .fields
                .iter()
                .map(|f| ChildField {
                    name: f.name.to_string(),
                    label: f.label.to_string(),
                    field_type: f.field_type.to_string(),
                    required: f.required,
                    options: f.static_options.map(|opts| {
                        opts.iter()
                            .map(|(v, l)| StaticOpt {
                                value: v.to_string(),
                                label: l.to_string(),
                            })
                            .collect()
                    }),
                })
                .collect(),
        })
        .collect()
}

// ---------------- Validasi ----------------

fn req_str(v: Option<&String>, label: &str, max: usize) -> Result<Option<String>, String> {
    match v.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => {
            if s.len() > max {
                return Err(format!("{label} maksimal {max} karakter."));
            }
            Ok(Some(s.to_string()))
        }
    }
}

fn opt_date(v: Option<&String>, label: &str) -> Result<Option<String>, String> {
    match v.map(|s| s.trim()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => {
            chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|_| format!("{label} harus tanggal valid (YYYY-MM-DD)."))?;
            Ok(Some(s.to_string()))
        }
    }
}

fn req_date(v: Option<&String>, label: &str) -> Result<String, String> {
    opt_date(v, label)?.ok_or_else(|| format!("{label} wajib diisi."))
}

fn in_check(v: &str, allowed: &[&str], label: &str) -> Result<(), String> {
    if !allowed.contains(&v) {
        return Err(format!("{label} tidak valid."));
    }
    Ok(())
}

// ---------------- Berkas ----------------

const PHOTO_MIMES: &[&str] = &["image/jpeg", "image/png", "image/webp"];
const DOC_MIMES: &[&str] = &[
    "image/jpeg",
    "image/png",
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
];
const MAX_FILE_BYTES: usize = 5 * 1024 * 1024;

fn clean_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    base.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Tulis berkas ke data dir, kembalikan path relatif.
fn store_file(
    base: &Path,
    subdir: &str,
    file: &FileUpload,
    allowed: &[&str],
) -> Result<String, String> {
    if !allowed.contains(&file.mime.as_str()) {
        return Err("Tipe berkas tidak diizinkan.".to_string());
    }
    if file.bytes.is_empty() {
        return Err("Berkas kosong.".to_string());
    }
    if file.bytes.len() > MAX_FILE_BYTES {
        return Err("Ukuran berkas maksimal 5MB.".to_string());
    }
    let dir = base.join(subdir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("gagal membuat folder berkas: {e}"))?;
    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S%f").to_string();
    let rel = format!("{subdir}/{stamp}_{}", clean_name(&file.name));
    std::fs::write(base.join(&rel), &file.bytes)
        .map_err(|e| format!("gagal menyimpan berkas: {e}"))?;
    Ok(rel)
}

fn remove_file(base: &Path, rel: &str) {
    let p: PathBuf = base.join(rel);
    if p.starts_with(base) {
        let _ = std::fs::remove_file(p);
    }
}

// ---------------- Buat, ubah, hapus ----------------

/// Kolom profil yang dapat ditulis (tanpa employee_number/foto/timestamp).
const PROFILE_COLS: &[&str] = &[
    "nik",
    "first_name",
    "last_name",
    "gender",
    "birth_place",
    "birth_date",
    "religion",
    "marital_status",
    "phone",
    "personal_email",
    "company_id",
    "branch_id",
    "department_id",
    "division_id",
    "section_id",
    "position_id",
    "job_level_id",
    "job_grade_id",
    "work_location_id",
    "cost_center_id",
    "supervisor_id",
    "manager_id",
    "join_date",
    "appointment_date",
    "employment_status",
    "employment_type",
    "bank_name",
    "bank_account_number",
    "bank_account_holder",
    "npwp",
    "ptkp_status",
    "bpjs_health_number",
    "bpjs_employment_number",
];

fn input_value(input: &EmployeeInput, col: &str) -> Option<String> {
    let s: Option<&String> = match col {
        "nik" => input.nik.as_ref(),
        "first_name" => Some(&input.first_name),
        "last_name" => input.last_name.as_ref(),
        "gender" => Some(&input.gender),
        "birth_place" => input.birth_place.as_ref(),
        "birth_date" => input.birth_date.as_ref(),
        "religion" => input.religion.as_ref(),
        "marital_status" => Some(&input.marital_status),
        "phone" => input.phone.as_ref(),
        "personal_email" => input.personal_email.as_ref(),
        "join_date" => Some(&input.join_date),
        "appointment_date" => input.appointment_date.as_ref(),
        "employment_status" => Some(&input.employment_status),
        "employment_type" => Some(&input.employment_type),
        "bank_name" => input.bank_name.as_ref(),
        "bank_account_number" => input.bank_account_number.as_ref(),
        "bank_account_holder" => input.bank_account_holder.as_ref(),
        "npwp" => input.npwp.as_ref(),
        "ptkp_status" => input.ptkp_status.as_ref(),
        "bpjs_health_number" => input.bpjs_health_number.as_ref(),
        "bpjs_employment_number" => input.bpjs_employment_number.as_ref(),
        _ => None,
    };
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn input_id(input: &EmployeeInput, col: &str) -> Option<i64> {
    match col {
        "company_id" => Some(input.company_id as i64),
        "branch_id" => input.branch_id.map(|v| v as i64),
        "department_id" => input.department_id.map(|v| v as i64),
        "division_id" => input.division_id.map(|v| v as i64),
        "section_id" => input.section_id.map(|v| v as i64),
        "position_id" => input.position_id.map(|v| v as i64),
        "job_level_id" => input.job_level_id.map(|v| v as i64),
        "job_grade_id" => input.job_grade_id.map(|v| v as i64),
        "work_location_id" => input.work_location_id.map(|v| v as i64),
        "cost_center_id" => input.cost_center_id.map(|v| v as i64),
        "supervisor_id" => input.supervisor_id.map(|v| v as i64),
        "manager_id" => input.manager_id.map(|v| v as i64),
        _ => None,
    }
}

// ---------------- Varian SeaORM (profil) ----------------

fn opt_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        _ => Some(value_to_string(v)),
    }
}

fn opt_dto(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match v {
        Value::Null => Ok(None),
        _ => Ok(Some(to_dto_int(
            value_i64(v).ok_or_else(|| format!("{f} tidak valid."))?,
            f,
        )?)),
    }
}

async fn exists_active_sea(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    id: i64,
    field: &str,
) -> Result<(), String> {
    let row = q_one(
        db,
        format!("SELECT id FROM {table} WHERE id = ?1 AND deleted_at IS NULL"),
        vec![Value::from(id)],
        1,
        "memeriksa relasi",
    )
    .await?;
    if row.is_none() {
        return Err(format!("{field} tidak ditemukan."));
    }
    Ok(())
}

async fn next_number_sea(db: &sea_orm::DatabaseConnection) -> Result<String, String> {
    let row = q_one(
        db,
        "SELECT employee_number FROM employees ORDER BY id DESC LIMIT 1".to_string(),
        vec![],
        1,
        "membuat nomor karyawan",
    )
    .await?;
    let n: i64 = row
        .map(|v| value_to_string(&v[0]))
        .filter(|s| !s.is_empty())
        .as_deref()
        .and_then(|s| s.strip_prefix("EMP-"))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Ok(format!("EMP-{:04}", n + 1))
}

async fn validate_employee_sea(
    db: &sea_orm::DatabaseConnection,
    input: &EmployeeInput,
    exclude_id: Option<i64>,
) -> Result<(), String> {
    if input.first_name.trim().is_empty() {
        return Err("Nama depan wajib diisi.".to_string());
    }
    if input.first_name.len() > 100 {
        return Err("Nama depan maksimal 100 karakter.".to_string());
    }
    if let Some(nik) = input
        .nik
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if nik.len() > 20 {
            return Err("NIK maksimal 20 karakter.".to_string());
        }
        let mut sql =
            "SELECT id FROM employees WHERE nik = ?1 AND deleted_at IS NULL".to_string();
        let mut vals = vec![Value::from(nik.to_string())];
        if let Some(ex) = exclude_id {
            sql.push_str(" AND id != ?2");
            vals.push(Value::from(ex));
        }
        let found = q_one(db, sql, vals, 1, "memeriksa NIK").await?;
        if found.is_some() {
            return Err("NIK sudah ada.".to_string());
        }
    }
    in_check(&input.gender, &["male", "female"], "Jenis kelamin")?;
    in_check(
        &input.marital_status,
        &["single", "married", "divorced", "widowed"],
        "Status pernikahan",
    )?;
    in_check(
        &input.employment_status,
        &["active", "probation", "resigned", "terminated"],
        "Status kepegawaian",
    )?;
    in_check(
        &input.employment_type,
        &["permanent", "contract", "intern", "daily", "freelance"],
        "Jenis kepegawaian",
    )?;
    req_date(Some(&input.join_date), "Tanggal masuk")?;
    opt_date(input.birth_date.as_ref(), "Tanggal lahir")?;
    opt_date(input.appointment_date.as_ref(), "Tanggal pengangkatan")?;
    if let Some(email) = input
        .personal_email
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !email.contains('@') {
            return Err("Email pribadi tidak valid.".to_string());
        }
    }
    req_str(input.last_name.as_ref(), "Nama belakang", 100)?;
    req_str(input.birth_place.as_ref(), "Tempat lahir", 100)?;
    req_str(input.religion.as_ref(), "Agama", 30)?;
    req_str(input.phone.as_ref(), "Telepon", 30)?;
    req_str(input.bank_name.as_ref(), "Nama bank", 100)?;
    req_str(input.bank_account_number.as_ref(), "Nomor rekening", 50)?;
    req_str(
        input.bank_account_holder.as_ref(),
        "Nama pemilik rekening",
        150,
    )?;
    req_str(input.npwp.as_ref(), "NPWP", 30)?;
    req_str(input.ptkp_status.as_ref(), "Status PTKP", 10)?;
    req_str(input.bpjs_health_number.as_ref(), "No. BPJS Kesehatan", 30)?;
    req_str(
        input.bpjs_employment_number.as_ref(),
        "No. BPJS Ketenagakerjaan",
        30,
    )?;
    exists_active_sea(db, "companies", input.company_id as i64, "Perusahaan").await?;
    for (opt_id, table, label) in [
        (input.branch_id, "branches", "Cabang"),
        (input.department_id, "departments", "Departemen"),
        (input.division_id, "divisions", "Divisi"),
        (input.section_id, "sections", "Seksi"),
        (input.position_id, "positions", "Jabatan"),
        (input.job_level_id, "job_levels", "Job level"),
        (input.job_grade_id, "job_grades", "Job grade"),
        (input.work_location_id, "work_locations", "Lokasi kerja"),
        (input.cost_center_id, "cost_centers", "Cost center"),
    ] {
        if let Some(id) = opt_id {
            exists_active_sea(db, table, id as i64, label).await?;
        }
    }
    for (opt_id, label) in [
        (input.supervisor_id, "Supervisor"),
        (input.manager_id, "Manajer"),
    ] {
        if let Some(id) = opt_id {
            let check = format!("memeriksa {label}");
            let found = q_one(
                db,
                "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
                vec![Value::from(id as i64)],
                1,
                &check,
            )
            .await?;
            if found.is_none() {
                return Err(format!("{label} tidak ditemukan."));
            }
        }
    }
    Ok(())
}

pub async fn list_sea(
    db: &sea_orm::DatabaseConnection,
    search: &str,
    filters: &EmployeeFilter,
    page: i32,
    per_page: i32,
) -> Result<EmployeePage, String> {
    let page = page.max(1);
    let per_page = per_page.clamp(1, 100) as i64;
    let offset = (page as i64 - 1) * per_page;
    let mut conds = vec!["e.deleted_at IS NULL".to_string()];
    let mut int_eqs: Vec<(&str, i64)> = Vec::new();
    if let Some(v) = filters.department_id {
        int_eqs.push(("e.department_id", v as i64));
    }
    if let Some(v) = filters.position_id {
        int_eqs.push(("e.position_id", v as i64));
    }
    if let Some(v) = filters.branch_id {
        int_eqs.push(("e.branch_id", v as i64));
    }
    for (col, v) in int_eqs {
        conds.push(format!("{col} = {v}"));
    }
    for (col, allowed, val) in [
        (
            "e.employment_status",
            &["active", "probation", "resigned", "terminated"] as &[&str],
            filters.employment_status.as_deref(),
        ),
        ("e.gender", &["male", "female"], filters.gender.as_deref()),
        (
            "e.employment_type",
            &["permanent", "contract", "intern", "daily", "freelance"],
            filters.employment_type.as_deref(),
        ),
    ] {
        if let Some(v) = val {
            if allowed.contains(&v) {
                conds.push(format!("{col} = '{v}'"));
            }
        }
    }
    let search = search.trim();
    let has_search = !search.is_empty();
    if has_search {
        conds.push("(e.first_name LIKE ?1 OR e.last_name LIKE ?1 OR e.employee_number LIKE ?1 OR e.nik LIKE ?1)".to_string());
    }
    let wh = conds.join(" AND ");
    let like = format!("%{search}%");
    let count_row = if has_search {
        q_one(
            db,
            format!("SELECT COUNT(*) FROM employees e WHERE {wh}"),
            vec![Value::from(like.clone())],
            1,
            "menghitung karyawan",
        )
        .await?
    } else {
        q_one(
            db,
            format!("SELECT COUNT(*) FROM employees e WHERE {wh}"),
            vec![],
            1,
            "menghitung karyawan",
        )
        .await?
    };
    let total: i64 = count_row
        .as_ref()
        .and_then(|v| value_i64(&v[0]))
        .unwrap_or(0);
    let tail = if has_search {
        "LIMIT ?2 OFFSET ?3"
    } else {
        "LIMIT ?1 OFFSET ?2"
    };
    let sql = format!(
        "SELECT e.id, e.employee_number, e.first_name, e.last_name, e.photo, e.join_date,
                e.employment_status, e.employment_type, d.name, p.name
         FROM employees e
         LEFT JOIN departments d ON d.id = e.department_id
         LEFT JOIN positions p ON p.id = e.position_id
         WHERE {wh} ORDER BY e.first_name ASC {tail}"
    );
    let mut vals = Vec::new();
    if has_search {
        vals.push(Value::from(like));
    }
    vals.push(Value::from(per_page));
    vals.push(Value::from(offset));
    let rows = q_all(db, sql, vals, 10, "membaca daftar").await?;
    let mut out = Vec::new();
    for v in rows {
        out.push(EmployeeRow {
            id: to_dto_int(value_i64(&v[0]).unwrap_or(0), "employee.id")?,
            employee_number: value_to_string(&v[1]),
            first_name: value_to_string(&v[2]),
            last_name: opt_text(&v[3]),
            photo: opt_text(&v[4]),
            join_date: value_to_string(&v[5]),
            employment_status: value_to_string(&v[6]),
            employment_type: value_to_string(&v[7]),
            department_name: opt_text(&v[8]),
            position_name: opt_text(&v[9]),
        });
    }
    Ok(EmployeePage {
        rows: out,
        total: to_dto_int(total, "employee.total")?,
    })
}

pub async fn detail_sea(
    db: &sea_orm::DatabaseConnection,
    id: i64,
) -> Result<Option<EmployeeDetail>, String> {
    let row = q_one(
        db,
        "SELECT e.id, e.employee_number, e.nik, e.first_name, e.last_name, e.photo,
                e.birth_place, e.birth_date, e.gender, e.religion, e.marital_status, e.phone,
                e.personal_email, e.company_id, e.branch_id, e.department_id, e.division_id, e.section_id,
                e.position_id, e.job_level_id, e.job_grade_id, e.work_location_id, e.cost_center_id,
                e.supervisor_id, e.manager_id, e.join_date, e.appointment_date, e.resign_date, e.employment_status,
                e.employment_type, e.bank_name, e.bank_account_number, e.bank_account_holder, e.npwp,
                e.ptkp_status, e.bpjs_health_number, e.bpjs_employment_number,
                c.name, b.name, d.name, dv.name, s.name, p.name, jl.name, jg.name, wl.name, cc.name,
                sup.first_name || ' ' || COALESCE(sup.last_name, ''), mgr.first_name || ' ' || COALESCE(mgr.last_name, '')
         FROM employees e
         LEFT JOIN companies c ON c.id = e.company_id
         LEFT JOIN branches b ON b.id = e.branch_id
         LEFT JOIN departments d ON d.id = e.department_id
         LEFT JOIN divisions dv ON dv.id = e.division_id
         LEFT JOIN sections s ON s.id = e.section_id
         LEFT JOIN positions p ON p.id = e.position_id
         LEFT JOIN job_levels jl ON jl.id = e.job_level_id
         LEFT JOIN job_grades jg ON jg.id = e.job_grade_id
         LEFT JOIN work_locations wl ON wl.id = e.work_location_id
         LEFT JOIN cost_centers cc ON cc.id = e.cost_center_id
         LEFT JOIN employees sup ON sup.id = e.supervisor_id
         LEFT JOIN employees mgr ON mgr.id = e.manager_id
         WHERE e.id = ?1 AND e.deleted_at IS NULL"
            .to_string(),
        vec![Value::from(id)],
        49,
        "memuat karyawan",
    )
    .await?;
    row.map(|v| {
        Ok(EmployeeDetail {
            id: to_dto_int(value_i64(&v[0]).unwrap_or(0), "employee.id")?,
            employee_number: value_to_string(&v[1]),
            nik: opt_text(&v[2]),
            first_name: value_to_string(&v[3]),
            last_name: opt_text(&v[4]),
            photo: opt_text(&v[5]),
            birth_place: opt_text(&v[6]),
            birth_date: opt_text(&v[7]),
            gender: value_to_string(&v[8]),
            religion: opt_text(&v[9]),
            marital_status: value_to_string(&v[10]),
            phone: opt_text(&v[11]),
            personal_email: opt_text(&v[12]),
            company_id: to_dto_int(value_i64(&v[13]).unwrap_or(0), "employee.company")?,
            branch_id: opt_dto(&v[14], "employee.branch")?,
            department_id: opt_dto(&v[15], "employee.department")?,
            division_id: opt_dto(&v[16], "employee.division")?,
            section_id: opt_dto(&v[17], "employee.section")?,
            position_id: opt_dto(&v[18], "employee.position")?,
            job_level_id: opt_dto(&v[19], "employee.level")?,
            job_grade_id: opt_dto(&v[20], "employee.grade")?,
            work_location_id: opt_dto(&v[21], "employee.location")?,
            cost_center_id: opt_dto(&v[22], "employee.cost")?,
            supervisor_id: opt_dto(&v[23], "employee.supervisor")?,
            manager_id: opt_dto(&v[24], "employee.manager")?,
            join_date: value_to_string(&v[25]),
            appointment_date: opt_text(&v[26]),
            resign_date: opt_text(&v[27]),
            employment_status: value_to_string(&v[28]),
            employment_type: value_to_string(&v[29]),
            bank_name: opt_text(&v[30]),
            bank_account_number: opt_text(&v[31]),
            bank_account_holder: opt_text(&v[32]),
            npwp: opt_text(&v[33]),
            ptkp_status: opt_text(&v[34]),
            bpjs_health_number: opt_text(&v[35]),
            bpjs_employment_number: opt_text(&v[36]),
            company_name: opt_text(&v[37]),
            branch_name: opt_text(&v[38]),
            department_name: opt_text(&v[39]),
            division_name: opt_text(&v[40]),
            section_name: opt_text(&v[41]),
            position_name: opt_text(&v[42]),
            job_level_name: opt_text(&v[43]),
            job_grade_name: opt_text(&v[44]),
            work_location_name: opt_text(&v[45]),
            cost_center_name: opt_text(&v[46]),
            supervisor_name: opt_text(&v[47]),
            manager_name: opt_text(&v[48]),
        })
    })
    .transpose()
}

async fn named_opts_sea(
    db: &sea_orm::DatabaseConnection,
    sql: &str,
    label: &str,
) -> Result<Vec<NamedOpt>, String> {
    let check = format!("memuat {label}");
    let rows = q_all(db, sql.to_string(), vec![], 2, &check).await?;
    let mut out = Vec::new();
    for v in rows {
        out.push(NamedOpt {
            id: to_dto_int(
                value_i64(&v[0])
                    .ok_or_else(|| format!("gagal membaca {label}."))?,
                "dropdown.id",
            )?,
            name: value_to_string(&v[1]),
        });
    }
    Ok(out)
}

pub async fn dropdowns_sea(db: &sea_orm::DatabaseConnection) -> Result<Dropdowns, String> {
    Ok(Dropdowns {
        companies: named_opts_sea(db, "SELECT id, name FROM companies WHERE deleted_at IS NULL ORDER BY name", "perusahaan").await?,
        branches: named_opts_sea(db, "SELECT id, name FROM branches WHERE deleted_at IS NULL ORDER BY name", "cabang").await?,
        departments: named_opts_sea(db, "SELECT id, name FROM departments WHERE deleted_at IS NULL ORDER BY name", "departemen").await?,
        divisions: named_opts_sea(db, "SELECT id, name FROM divisions WHERE deleted_at IS NULL ORDER BY name", "divisi").await?,
        sections: named_opts_sea(db, "SELECT id, name FROM sections WHERE deleted_at IS NULL ORDER BY name", "seksi").await?,
        positions: named_opts_sea(db, "SELECT id, name FROM positions WHERE deleted_at IS NULL ORDER BY name", "jabatan").await?,
        job_levels: named_opts_sea(db, "SELECT id, name FROM job_levels WHERE deleted_at IS NULL ORDER BY level_order", "level").await?,
        job_grades: named_opts_sea(db, "SELECT id, name FROM job_grades WHERE deleted_at IS NULL ORDER BY grade_order", "grade").await?,
        work_locations: named_opts_sea(db, "SELECT id, name FROM work_locations WHERE deleted_at IS NULL ORDER BY name", "lokasi").await?,
        cost_centers: named_opts_sea(db, "SELECT id, name FROM cost_centers WHERE deleted_at IS NULL ORDER BY name", "cost center").await?,
        employees: named_opts_sea(db, "SELECT id, first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE deleted_at IS NULL ORDER BY first_name", "karyawan").await?,
    })
}

fn profile_vals_sea(input: &EmployeeInput) -> Vec<Value> {
    PROFILE_COLS
        .iter()
        .map(|c| match *c {
            "company_id" | "branch_id" | "department_id" | "division_id" | "section_id"
            | "position_id" | "job_level_id" | "job_grade_id" | "work_location_id"
            | "cost_center_id" | "supervisor_id" | "manager_id" => match input_id(input, c) {
                Some(v) => Value::from(v),
                None => Value::Null,
            },
            _ => match input_value(input, c) {
                Some(v) => Value::from(v),
                None => Value::Null,
            },
        })
        .collect()
}

pub async fn create_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    actor_id: i64,
    input: &EmployeeInput,
    photo: Option<&FileUpload>,
) -> Result<i32, String> {
    validate_employee_sea(db, input, None).await?;
    let number = next_number_sea(db).await?;
    let photo_rel = match photo {
        Some(f) => Some(store_file(files, "photos", f, PHOTO_MIMES)?),
        None => None,
    };
    let cols = format!("employee_number, photo, {}", PROFILE_COLS.join(", "));
    let holders: Vec<String> = (1..=PROFILE_COLS.len() + 2)
        .map(|i| format!("?{i}"))
        .collect();
    let mut vals = vec![
        Value::from(number.clone()),
        match &photo_rel {
            Some(p) => Value::from(p.clone()),
            None => Value::Null,
        },
    ];
    vals.extend(profile_vals_sea(input));
    exec(
        db,
        format!("INSERT INTO employees ({cols}) VALUES ({})", holders.join(", ")),
        vals,
        "menambah karyawan",
    )
    .await
    .map_err(|e| {
        if e.contains("UNIQUE") {
            "Data duplikat (NIK sudah ada).".to_string()
        } else {
            e
        }
    })?;
    let row = q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        "memuat id",
    )
    .await?;
    let id = row
        .as_ref()
        .and_then(|v| value_i64(&v[0]))
        .ok_or("gagal memuat id baru.".to_string())?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "employee",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Karyawan baru {number}")),
    )
    .await?;
    to_dto_int(id, "employee.id")
}

pub async fn update_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    actor_id: i64,
    id: i64,
    input: &EmployeeInput,
    resign_date: Option<&str>,
    photo: Option<&FileUpload>,
) -> Result<(), String> {
    let before = detail_sea(db, id)
        .await?
        .ok_or("Karyawan tidak ditemukan.".to_string())?;
    validate_employee_sea(db, input, Some(id)).await?;
    let photo_rel: Option<Option<String>> = match photo {
        Some(f) => Some(Some(store_file(files, "photos", f, PHOTO_MIMES)?)),
        None => None,
    };
    if let Some(Some(new_rel)) = photo_rel.as_ref() {
        if let Some(old) = before.photo.as_deref() {
            if old != new_rel {
                remove_file(files, old);
            }
        }
    }
    let mut sets: Vec<String> = PROFILE_COLS.iter().map(|c| format!("{c} = ?")).collect();
    if resign_date.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        sets.push("resign_date = ?".to_string());
    }
    if photo_rel.is_some() {
        sets.push("photo = ?".to_string());
    }
    let mut vals = profile_vals_sea(input);
    if let Some(rd) = resign_date {
        if !rd.trim().is_empty() {
            vals.push(Value::from(rd.trim().to_string()));
        }
    }
    if let Some(opt) = photo_rel {
        vals.push(match opt {
            Some(p) => Value::from(p),
            None => Value::Null,
        });
    }
    vals.push(Value::from(id));
    exec(
        db,
        format!("UPDATE employees SET {} WHERE id = ?", sets.join(", ")),
        vals,
        "memperbarui karyawan",
    )
    .await
    .map_err(|e| {
        if e.contains("UNIQUE") {
            "Data duplikat (NIK sudah ada).".to_string()
        } else {
            e
        }
    })?;
    let after = serde_json::to_string(&detail_sea(db, id).await?.expect("ada")).unwrap_or_default();
    let before_json = serde_json::to_string(&before).unwrap_or_default();
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "employee",
        Some(&id.to_string()),
        Some(&before_json),
        Some(&after),
        Some(&format!("Karyawan {} diperbarui", before.employee_number)),
    )
    .await?;
    Ok(())
}

pub async fn delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let before = detail_sea(db, id)
        .await?
        .ok_or("Karyawan tidak ditemukan.".to_string())?;
    exec(
        db,
        "UPDATE employees SET deleted_at = datetime('now','localtime') WHERE id = ?1".to_string(),
        vec![Value::from(id)],
        "menghapus karyawan",
    )
    .await?;
    let before_json = serde_json::to_string(&before).unwrap_or_default();
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "employee",
        Some(&id.to_string()),
        Some(&before_json),
        None,
        Some(&format!("Karyawan {} dihapus", before.employee_number)),
    )
    .await?;
    Ok(())
}

// ---------------- Child generik (SeaORM) ----------------

async fn employee_exists_sea(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(), String> {
    let found = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::from(id)],
        1,
        "memuat karyawan",
    )
    .await?;
    if found.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    Ok(())
}

pub async fn child_list_sea(
    db: &sea_orm::DatabaseConnection,
    slug: &str,
    employee_id: i64,
) -> Result<Vec<BTreeMap<String, String>>, String> {
    let def = child_def(slug)?;
    employee_exists_sea(db, employee_id).await?;
    let cols: Vec<&str> = def.fields.iter().map(|f| f.name).collect();
    let select = format!("id, employee_id, {}", cols.join(", "));
    let rows = q_all(
        db,
        format!(
            "SELECT {select} FROM {} WHERE {} ORDER BY id DESC",
            def.table,
            child_where(def)
        ),
        vec![Value::from(employee_id)],
        cols.len() + 2,
        "membaca daftar",
    )
    .await?;
    let mut out = Vec::new();
    for row in rows {
        let mut map = BTreeMap::new();
        map.insert("id".to_string(), value_to_string(&row[0]));
        map.insert("employee_id".to_string(), value_to_string(&row[1]));
        for (i, col) in cols.iter().enumerate() {
            map.insert((*col).to_string(), value_to_string(&row[i + 2]));
        }
        out.push(map);
    }
    Ok(out)
}

async fn validate_child_sea(
    db: &sea_orm::DatabaseConnection,
    def: &ChildDef,
    values: &BTreeMap<String, String>,
    exclude_id: Option<i64>,
) -> Result<BTreeMap<String, Option<String>>, String> {
    let mut cleaned = BTreeMap::new();
    for f in def.fields {
        let raw = values.get(f.name).map(|s| s.trim()).unwrap_or("");
        if f.field_type == "checkbox" {
            let v = raw == "1" || raw.eq_ignore_ascii_case("true");
            cleaned.insert(
                f.name.to_string(),
                Some(if v { "1".to_string() } else { "0".to_string() }),
            );
            continue;
        }
        if raw.is_empty() {
            if f.required {
                return Err(format!("{} wajib diisi.", f.label));
            }
            cleaned.insert(f.name.to_string(), None);
            continue;
        }
        if let Some(max) = f.max_len {
            if raw.len() > max {
                return Err(format!("{} maksimal {max} karakter.", f.label));
            }
        }
        if let Some(allowed) = f.in_values {
            if !allowed.contains(&raw) {
                return Err(format!("{} tidak valid.", f.label));
            }
        }
        if f.is_date && chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").is_err() {
            return Err(format!("{} harus tanggal valid (YYYY-MM-DD).", f.label));
        }
        if f.is_integer && raw.parse::<i64>().is_err() {
            return Err(format!("{} harus bilangan bulat.", f.label));
        }
        if f.is_numeric && raw.parse::<f64>().is_err() {
            return Err(format!("{} harus angka.", f.label));
        }
        if f.unique {
            let mut sql = format!("SELECT id FROM {} WHERE {} = ?1", def.table, f.name);
            let mut uvals = vec![Value::from(raw.to_string())];
            if let Some(ex) = exclude_id {
                sql.push_str(" AND id != ?2");
                uvals.push(Value::from(ex));
            }
            let found = q_one(db, sql, uvals, 1, "memeriksa keunikan").await?;
            if found.is_some() {
                return Err(format!("{} sudah dipakai.", f.label));
            }
        }
        cleaned.insert(f.name.to_string(), Some(raw.to_string()));
    }
    Ok(cleaned)
}

pub async fn child_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    slug: &str,
    employee_id: i64,
    id: Option<i64>,
    values: &BTreeMap<String, String>,
) -> Result<i32, String> {
    let def = child_def(slug)?;
    employee_exists_sea(db, employee_id).await?;
    let cleaned = validate_child_sea(db, def, values, id).await?;
    let module = format!("employee.{slug}");
    if let Some(rid) = id {
        let owned = q_one(
            db,
            format!(
                "SELECT id FROM {} WHERE id = ?1 AND employee_id = ?2",
                def.table
            ),
            vec![Value::from(rid), Value::from(employee_id)],
            1,
            "memuat data",
        )
        .await?;
        if owned.is_none() {
            return Err("Data tidak ditemukan.".to_string());
        }
        let sets: Vec<String> = cleaned
            .keys()
            .enumerate()
            .map(|(i, c)| format!("{c} = ?{}", i + 1))
            .collect();
        let mut vals: Vec<Value> = cleaned
            .values()
            .map(|v| match v {
                Some(s) => Value::from(s.clone()),
                None => Value::Null,
            })
            .collect();
        vals.push(Value::from(rid));
        vals.push(Value::from(employee_id));
        let n = vals.len();
        exec(
            db,
            format!(
                "UPDATE {} SET {}, updated_at = datetime('now','localtime') WHERE id = ?{} AND employee_id = ?{}",
                def.table,
                sets.join(", "),
                n - 1,
                n
            ),
            vals,
            "menyimpan",
        )
        .await?;
        let after = serde_json::to_string(&cleaned).unwrap_or_default();
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            &module,
            Some(&rid.to_string()),
            None,
            Some(&after),
            None,
        )
        .await?;
        to_dto_int(rid, "child.id")
    } else {
        let mut cols = vec!["employee_id".to_string()];
        cols.extend(cleaned.keys().cloned());
        let holders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let mut vals = vec![Value::from(employee_id)];
        vals.extend(cleaned.values().map(|v| match v {
            Some(s) => Value::from(s.clone()),
            None => Value::Null,
        }));
        exec(
            db,
            format!(
                "INSERT INTO {} ({}) VALUES ({})",
                def.table,
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
        let row = q_one(
            db,
            "SELECT last_insert_rowid()".to_string(),
            vec![],
            1,
            "memuat id",
        )
        .await?;
        let rid = row
            .as_ref()
            .and_then(|v| value_i64(&v[0]))
            .ok_or("gagal memuat id baru.".to_string())?;
        let after = serde_json::to_string(&cleaned).unwrap_or_default();
        audit::log_sea(
            db,
            Some(actor_id),
            "CREATE",
            &module,
            Some(&rid.to_string()),
            None,
            Some(&after),
            None,
        )
        .await?;
        to_dto_int(rid, "child.id")
    }
}

pub async fn child_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    slug: &str,
    employee_id: i64,
    id: i64,
) -> Result<(), String> {
    if slug == "documents" {
        return delete_document_sea(db, actor_id, employee_id, id).await;
    }
    let def = child_def(slug)?;
    let owned = q_one(
        db,
        format!(
            "SELECT id FROM {} WHERE id = ?1 AND employee_id = ?2",
            def.table
        ),
        vec![Value::from(id), Value::from(employee_id)],
        1,
        "memuat data",
    )
    .await?;
    if owned.is_none() {
        return Err("Data tidak ditemukan.".to_string());
    }
    if def.soft_delete {
        exec(
            db,
            format!(
                "UPDATE {} SET deleted_at = datetime('now','localtime') WHERE id = ?1",
                def.table
            ),
            vec![Value::from(id)],
            "menghapus",
        )
        .await?;
    } else {
        exec(
            db,
            format!("DELETE FROM {} WHERE id = ?1", def.table),
            vec![Value::from(id)],
            "menghapus",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        &format!("employee.{slug}"),
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

// ---------------- Alamat (SeaORM) ----------------

async fn read_address_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    kind: &str,
) -> Result<Option<AddressRow>, String> {
    let row = q_one(
        db,
        "SELECT address, city, province, postal_code FROM employee_addresses WHERE employee_id = ?1 AND type = ?2".to_string(),
        vec![Value::from(employee_id), Value::from(kind)],
        4,
        "memuat alamat",
    )
    .await?;
    Ok(row.map(|r| AddressRow {
        address: opt_text(&r[0]),
        city: opt_text(&r[1]),
        province: opt_text(&r[2]),
        postal_code: opt_text(&r[3]),
    }))
}

pub async fn addresses_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Addresses, String> {
    employee_exists_sea(db, employee_id).await?;
    Ok(Addresses {
        ktp: read_address_sea(db, employee_id, "ktp").await?,
        domicile: read_address_sea(db, employee_id, "domicile").await?,
    })
}

pub async fn save_address_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    kind: &str,
    values: &BTreeMap<String, String>,
) -> Result<(), String> {
    if kind != "ktp" && kind != "domicile" {
        return Err("Tipe alamat tidak valid.".to_string());
    }
    employee_exists_sea(db, employee_id).await?;
    let get = |k: &str, max: usize, label: &str| -> Result<Option<String>, String> {
        match values.get(k).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => Ok(None),
            Some(s) => {
                if s.len() > max {
                    return Err(format!("{label} maksimal {max} karakter."));
                }
                Ok(Some(s.to_string()))
            }
        }
    };
    let address = get("address", 1000, "Alamat")?;
    let city = get("city", 100, "Kota")?;
    let province = get("province", 100, "Provinsi")?;
    let postal = get("postal_code", 10, "Kode pos")?;
    let opt_val = |v: &Option<String>| match v {
        Some(s) => Value::from(s.clone()),
        None => Value::Null,
    };
    let existing = q_one(
        db,
        "SELECT id FROM employee_addresses WHERE employee_id = ?1 AND type = ?2".to_string(),
        vec![Value::from(employee_id), Value::from(kind)],
        1,
        "memeriksa alamat",
    )
    .await?;
    match existing.and_then(|v| value_i64(&v[0])) {
        Some(id) => {
            exec(
                db,
                "UPDATE employee_addresses SET address = ?1, city = ?2, province = ?3, postal_code = ?4 WHERE id = ?5".to_string(),
                vec![
                    opt_val(&address),
                    opt_val(&city),
                    opt_val(&province),
                    opt_val(&postal),
                    Value::from(id),
                ],
                "menyimpan alamat",
            )
            .await?;
        }
        None => {
            exec(
                db,
                "INSERT INTO employee_addresses (employee_id, type, address, city, province, postal_code) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
                vec![
                    Value::from(employee_id),
                    Value::from(kind),
                    opt_val(&address),
                    opt_val(&city),
                    opt_val(&province),
                    opt_val(&postal),
                ],
                "menyimpan alamat",
            )
            .await?;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "employee.address",
        Some(&employee_id.to_string()),
        None,
        None,
        Some(&format!("Alamat {kind} karyawan {employee_id}")),
    )
    .await?;
    Ok(())
}

// ---------------- Dokumen (SeaORM) ----------------

pub async fn documents_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<Document>, String> {
    employee_exists_sea(db, employee_id).await?;
    let rows = q_all(
        db,
        "SELECT id, category, name, file_path, file_size, mime_type, expiry_date, created_at FROM employee_documents WHERE employee_id = ?1 AND deleted_at IS NULL ORDER BY created_at DESC".to_string(),
        vec![Value::from(employee_id)],
        8,
        "membaca dokumen",
    )
    .await?;
    let mut out = Vec::new();
    for r in rows {
        out.push(Document {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "document.id")?,
            category: value_to_string(&r[1]),
            name: value_to_string(&r[2]),
            file_path: value_to_string(&r[3]),
            file_size: opt_dto(&r[4], "document.size")?,
            mime_type: opt_text(&r[5]),
            expiry_date: opt_text(&r[6]),
            created_at: value_to_string(&r[7]),
        });
    }
    Ok(out)
}

pub async fn upload_document_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    actor_id: i64,
    employee_id: i64,
    category: &str,
    name: &str,
    expiry_date: Option<&str>,
    file: &FileUpload,
) -> Result<i32, String> {
    employee_exists_sea(db, employee_id).await?;
    if !DOC_CATEGORIES.contains(&category) {
        return Err("Kategori dokumen tidak valid.".to_string());
    }
    if name.trim().is_empty() {
        return Err("Nama dokumen wajib diisi.".to_string());
    }
    if name.len() > 150 {
        return Err("Nama dokumen maksimal 150 karakter.".to_string());
    }
    if let Some(exp) = expiry_date.map(str::trim).filter(|s| !s.is_empty()) {
        chrono::NaiveDate::parse_from_str(exp, "%Y-%m-%d")
            .map_err(|_| "Tanggal kedaluwarsa harus valid (YYYY-MM-DD).".to_string())?;
    }
    let subdir = format!("documents/{employee_id}");
    let rel = store_file(files, &subdir, file, DOC_MIMES)?;
    let expiry = expiry_date.map(str::trim).filter(|s| !s.is_empty());
    exec(
        db,
        "INSERT INTO employee_documents (employee_id, category, name, file_path, file_size, mime_type, expiry_date, uploaded_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)".to_string(),
        vec![
            Value::from(employee_id),
            Value::from(category),
            Value::from(name.trim()),
            Value::from(rel),
            Value::from(file.bytes.len() as i64),
            Value::from(file.mime.clone()),
            match expiry {
                Some(s) => Value::from(s),
                None => Value::Null,
            },
            Value::from(actor_id),
        ],
        "mencatat dokumen",
    )
    .await?;
    let row = q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        "memuat id",
    )
    .await?;
    let id = row
        .as_ref()
        .and_then(|v| value_i64(&v[0]))
        .ok_or("gagal memuat id baru.".to_string())?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "employee.document",
        Some(&id.to_string()),
        None,
        None,
        Some(&format!("Dokumen {category} {name}")),
    )
    .await?;
    to_dto_int(id, "document.id")
}

pub async fn document_bytes_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    employee_id: i64,
    id: i64,
) -> Result<DocumentBytes, String> {
    let row = q_one(
        db,
        "SELECT file_path, mime_type, name FROM employee_documents WHERE id = ?1 AND employee_id = ?2 AND deleted_at IS NULL".to_string(),
        vec![Value::from(id), Value::from(employee_id)],
        3,
        "memuat dokumen",
    )
    .await?;
    let Some(r) = row else {
        return Err("Dokumen tidak ditemukan.".to_string());
    };
    let rel = value_to_string(&r[0]);
    let mime = opt_text(&r[1]).unwrap_or_else(|| "application/octet-stream".to_string());
    let name = value_to_string(&r[2]);
    let path = files.join(&rel);
    if !path.starts_with(files) {
        return Err("Path dokumen tidak valid.".to_string());
    }
    let bytes =
        std::fs::read(&path).map_err(|_| "Berkas dokumen hilang dari penyimpanan.".to_string())?;
    Ok(DocumentBytes { mime, name, bytes })
}

pub async fn delete_document_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    id: i64,
) -> Result<(), String> {
    let row = q_one(
        db,
        "SELECT file_path, name FROM employee_documents WHERE id = ?1 AND employee_id = ?2 AND deleted_at IS NULL".to_string(),
        vec![Value::from(id), Value::from(employee_id)],
        2,
        "memuat dokumen",
    )
    .await?;
    if row.is_none() {
        return Err("Dokumen tidak ditemukan.".to_string());
    }
    exec(
        db,
        "UPDATE employee_documents SET deleted_at = datetime('now','localtime') WHERE id = ?1".to_string(),
        vec![Value::from(id)],
        "menghapus dokumen",
    )
    .await?;
    // berkas fisik dipertahankan sebagai arsip; hanya baris yang dihapus lunak
    audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "employee.document",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

// ---------------- Child generik ----------------

fn child_where(def: &ChildDef) -> &'static str {
    if def.soft_delete {
        "employee_id = ?1 AND deleted_at IS NULL"
    } else {
        "employee_id = ?1"
    }
}

// ---------------- Dokumen ----------------

const DOC_CATEGORIES: &[&str] = &[
    "ktp",
    "kk",
    "npwp",
    "bpjs",
    "ijazah",
    "certificate",
    "cv",
    "contract",
    "appointment_letter",
    "promotion_letter",
    "mutation_letter",
    "other",
];

// ====== Varian SeaORM (gaji) ======
fn salary_f64(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        _ => value_to_string(v).parse::<f64>().unwrap_or(0.0),
    }
}

fn salary_opt_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Null => None,
        Value::Float(f) => Some(*f),
        Value::Int(i) => Some(*i as f64),
        Value::Text(s) => s.parse().ok(),
    }
}

fn salary_row_sea(cols: &[Value]) -> Result<SalaryRow, String> {
    Ok(SalaryRow {
        id: to_dto_int(value_i64(&cols[0]).unwrap_or(0), "salary.id")?,
        basic_salary: salary_f64(&cols[1]),
        effective_date: value_to_string(&cols[2]),
        is_active: value_i64(&cols[3]).unwrap_or(0) != 0,
    })
}

fn salary_component_row_sea(
    cols: &[Value],
    with_amount: bool,
) -> Result<SalaryComponentRow, String> {
    Ok(SalaryComponentRow {
        id: to_dto_int(value_i64(&cols[0]).unwrap_or(0), "component.id")?,
        code: value_to_string(&cols[1]),
        name: value_to_string(&cols[2]),
        component_type: value_to_string(&cols[3]),
        amount: if with_amount {
            Some(salary_f64(&cols[4]))
        } else {
            None
        },
    })
}

pub async fn salary_current_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Option<SalaryRow>, String> {
    employee_exists_sea(db, employee_id).await?;
    let cols = q_one(
        db,
        "SELECT id, basic_salary, effective_date, is_active FROM employee_salaries WHERE employee_id = ?1 AND is_active = 1 ORDER BY effective_date DESC LIMIT 1".to_string(),
        vec![Value::from(employee_id)],
        4,
        "memuat gaji",
    )
    .await?;
    cols.map(|c| salary_row_sea(&c)).transpose()
}

pub async fn salary_history_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<SalaryRow>, String> {
    employee_exists_sea(db, employee_id).await?;
    let rows = q_all(
        db,
        "SELECT id, basic_salary, effective_date, is_active FROM employee_salaries WHERE employee_id = ?1 ORDER BY effective_date DESC".to_string(),
        vec![Value::from(employee_id)],
        4,
        "memuat riwayat gaji",
    )
    .await?;
    rows.iter().map(salary_row_sea_ref).collect()
}

fn salary_row_sea_ref(c: &Vec<Value>) -> Result<SalaryRow, String> {
    salary_row_sea(c)
}

pub async fn salary_components_sea(
    db: &sea_orm::DatabaseConnection,
    salary_id: i64,
) -> Result<Vec<SalaryComponentRow>, String> {
    let rows = q_all(
        db,
        "SELECT esc.salary_component_id, sc.code, sc.name, sc.type, esc.amount FROM employee_salary_components esc INNER JOIN salary_components sc ON sc.id = esc.salary_component_id WHERE esc.employee_salary_id = ?1 ORDER BY sc.name".to_string(),
        vec![Value::from(salary_id)],
        5,
        "memuat komponen",
    )
    .await?;
    rows.iter().map(|c| salary_component_row_sea(c, true)).collect()
}

pub async fn available_components_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<SalaryComponentRow>, String> {
    let rows = q_all(
        db,
        "SELECT id, code, name, type FROM salary_components WHERE is_active = 1 AND type = 'income' ORDER BY name".to_string(),
        vec![],
        4,
        "memuat katalog",
    )
    .await?;
    rows.iter().map(|c| salary_component_row_sea(c, false)).collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn set_salary_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    basic_salary: f64,
    effective_date: &str,
    components: &[(i64, f64)],
) -> Result<i32, String> {
    employee_exists_sea(db, employee_id).await?;
    if !(basic_salary >= 0.0) {
        return Err("Gaji pokok minimal 0.".to_string());
    }
    let band = q_one(
        db,
        "SELECT g.min_salary, g.max_salary, g.name FROM job_grades g INNER JOIN employees e ON e.job_grade_id = g.id WHERE e.id = ?1".to_string(),
        vec![Value::from(employee_id)],
        3,
        "memuat band gaji",
    )
    .await?;
    if let Some(b) = band {
        let min = salary_opt_f64(&b[0]);
        let max = salary_opt_f64(&b[1]);
        let name = value_to_string(&b[2]);
        if min.is_some_and(|m| basic_salary < m) || max.is_some_and(|m| basic_salary > m) {
            return Err(format!("Gaji pokok di luar band {name}."));
        }
    }
    chrono::NaiveDate::parse_from_str(effective_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal efektif harus valid (YYYY-MM-DD).".to_string())?;
    exec(
        db,
        "UPDATE employee_salaries SET is_active = 0 WHERE employee_id = ?1".to_string(),
        vec![Value::from(employee_id)],
        "menonaktifkan gaji lama",
    )
    .await?;
    exec(
        db,
        "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active, created_by) VALUES (?1, ?2, ?3, 1, ?4)".to_string(),
        vec![
            Value::from(employee_id),
            Value::Float(basic_salary),
            Value::from(effective_date.trim()),
            Value::from(actor_id),
        ],
        "menetapkan gaji",
    )
    .await?;
    let salary_id = q_one(
        db,
        "SELECT last_insert_rowid()".to_string(),
        vec![],
        1,
        "memuat id gaji",
    )
    .await?
    .map(|c| value_i64(&c[0]).unwrap_or(0))
    .unwrap_or(0);
    for (comp_id, amount) in components {
        exec(
            db,
            "INSERT INTO employee_salary_components (employee_salary_id, salary_component_id, amount) VALUES (?1, ?2, ?3)".to_string(),
            vec![
                Value::from(salary_id),
                Value::from(*comp_id),
                Value::Float(*amount),
            ],
            "menyimpan komponen",
        )
        .await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "employee.salary",
        Some(&salary_id.to_string()),
        None,
        None,
        Some(&format!("Gaji baru karyawan {employee_id}")),
    )
    .await?;
    to_dto_int(salary_id, "salary.id")
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

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        one(db, "SELECT id FROM users WHERE username = 'admin'", "test.admin").await
    }

    async fn hq(db: &sea_orm::DatabaseConnection) -> i64 {
        one(db, "SELECT id FROM companies WHERE code = 'HQ'", "test.hq").await
    }

    fn base_input(company: i64) -> EmployeeInput {
        EmployeeInput {
            first_name: "Budi".to_string(),
            gender: "male".to_string(),
            marital_status: "single".to_string(),
            company_id: to_dto_int(company, "c").unwrap(),
            join_date: "2026-01-05".to_string(),
            employment_status: "active".to_string(),
            employment_type: "permanent".to_string(),
            ..Default::default()
        }
    }

    async fn mkemp(db: &sea_orm::DatabaseConnection, files: &Path) -> (i64, i64) {
        let actor = admin_id(db).await;
        let input = base_input(hq(db).await);
        let emp = create_sea(db, files, actor, &input, None)
            .await
            .expect("buat") as i64;
        (actor, emp)
    }

    #[tokio::test]
    async fn kontrak_pkwt_pkwtt_valid_tipe_lain_ditolak() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (actor, emp) = mkemp(db, files.path()).await;
        let vals = |number: &str, kind: &str| {
            BTreeMap::from([
                ("contract_number".to_string(), number.to_string()),
                ("type".to_string(), kind.to_string()),
                ("start_date".to_string(), "2026-01-05".to_string()),
                ("status".to_string(), "active".to_string()),
            ])
        };
        assert!(child_save_sea(db, actor, "contracts", emp, None, &vals("K-001", "pkwt"))
            .await
            .is_ok());
        assert!(child_save_sea(db, actor, "contracts", emp, None, &vals("K-002", "pkwtt"))
            .await
            .is_ok());
        let e = child_save_sea(db, actor, "contracts", emp, None, &vals("K-003", "tetap"))
            .await
            .expect_err("tipe tetap ditolak");
        assert_eq!(e, "Jenis tidak valid.");
    }

    #[tokio::test]
    async fn kontrak_jatuh_tempo_terbatas_hari() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (actor, emp) = mkemp(db, files.path()).await;
        let today = chrono::Local::now().date_naive();
        let soon = (today + chrono::Duration::days(10))
            .format("%Y-%m-%d")
            .to_string();
        let far = (today + chrono::Duration::days(100))
            .format("%Y-%m-%d")
            .to_string();
        let vals = |number: &str, end: &str| {
            BTreeMap::from([
                ("contract_number".to_string(), number.to_string()),
                ("type".to_string(), "pkwt".to_string()),
                ("start_date".to_string(), "2026-01-05".to_string()),
                ("end_date".to_string(), end.to_string()),
                ("status".to_string(), "active".to_string()),
            ])
        };
        child_save_sea(db, actor, "contracts", emp, None, &vals("K-101", &soon))
            .await
            .expect("dekat");
        child_save_sea(db, actor, "contracts", emp, None, &vals("K-102", &far))
            .await
            .expect("jauh");
        let e = expiring_contracts_sea(db, 0).await.expect_err("nol hari");
        assert_eq!(e, "Rentang hari harus 1 sampai 365.");
        let near = expiring_contracts_sea(db, 30).await.expect("30 hari");
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].contract_number, "K-101");
        assert_eq!(near[0].days_left, 10);
        assert_eq!(near[0].employee_id as i64, emp);
        let wide = expiring_contracts_sea(db, 120).await.expect("120 hari");
        assert_eq!(wide.len(), 2);
    }

    #[tokio::test]
    async fn riwayat_karier_mutasi_promosi_demosi() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (actor, emp) = mkemp(db, files.path()).await;
        let vals = |kind: &str| {
            BTreeMap::from([
                ("type".to_string(), kind.to_string()),
                ("effective_date".to_string(), "2026-02-01".to_string()),
            ])
        };
        for kind in ["promotion", "mutation", "transfer", "demotion"] {
            assert!(
                child_save_sea(db, actor, "career-histories", emp, None, &vals(kind))
                    .await
                    .is_ok(),
                "jenis {kind} harus diterima"
            );
        }
        let e = child_save_sea(db, actor, "career-histories", emp, None, &vals("phk"))
            .await
            .expect_err("phk ditolak");
        assert_eq!(e, "Jenis Perubahan tidak valid.");
        let rows = child_list_sea(db, "career-histories", emp)
            .await
            .expect("daftar");
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].get("type").map(|s| s.as_str()), Some("demotion"));
    }

    #[tokio::test]
    async fn grade_level_tersimpan_di_profil() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        exec(
            db,
            "INSERT INTO job_levels (code, name, level_order) VALUES (?1, ?2, ?3)".to_string(),
            vec![
                Value::Text("L9".to_string()),
                Value::Text("Level 9".to_string()),
                Value::Int(9),
            ],
            "test.level",
        )
        .await
        .expect("level");
        exec(
            db,
            "INSERT INTO job_grades (code, name, grade_order) VALUES (?1, ?2, ?3)".to_string(),
            vec![
                Value::Text("G9".to_string()),
                Value::Text("Grade 9".to_string()),
                Value::Int(9),
            ],
            "test.grade",
        )
        .await
        .expect("grade");
        let level_id = one(db, "SELECT id FROM job_levels WHERE code = 'L9'", "test.lid").await;
        let grade_id = one(db, "SELECT id FROM job_grades WHERE code = 'G9'", "test.gid").await;
        let actor = admin_id(db).await;
        let mut input = base_input(hq(db).await);
        input.job_level_id = Some(level_id as i32);
        input.job_grade_id = Some(grade_id as i32);
        let emp = create_sea(db, files.path(), actor, &input, None)
            .await
            .expect("buat") as i64;
        let det = detail_sea(db, emp).await.expect("detail").expect("ada");
        assert_eq!(det.job_level_name.as_deref(), Some("Level 9"));
        assert_eq!(det.job_grade_name.as_deref(), Some("Grade 9"));
        let dd = dropdowns_sea(db).await.expect("dropdown");
        assert!(dd.job_levels.iter().any(|o| o.name == "Level 9"));
        assert!(dd.job_grades.iter().any(|o| o.name == "Grade 9"));
    }

    #[tokio::test]
    async fn gaji_divalidasi_masuk_band_grade() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        exec(
            db,
            "INSERT INTO job_grades (code, name, grade_order, min_salary, max_salary) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
            vec![
                Value::Text("GB".to_string()),
                Value::Text("Grade Band".to_string()),
                Value::Int(1),
                Value::Float(5_000_000.0),
                Value::Float(10_000_000.0),
            ],
            "test.band",
        )
        .await
        .expect("grade");
        let grade_id = one(db, "SELECT id FROM job_grades WHERE code = 'GB'", "test.gid").await;
        let actor = admin_id(db).await;
        let mut input = base_input(hq(db).await);
        input.job_grade_id = Some(grade_id as i32);
        let emp = create_sea(db, files.path(), actor, &input, None)
            .await
            .expect("buat") as i64;
        assert!(set_salary_sea(db, actor, emp, 4_000_000.0, "2026-01-01", &[])
            .await
            .is_err());
        assert!(set_salary_sea(db, actor, emp, 12_000_000.0, "2026-01-01", &[])
            .await
            .is_err());
        assert!(set_salary_sea(db, actor, emp, 7_000_000.0, "2026-01-01", &[])
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn buat_cari_ubah_hapus_berurutan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (actor, emp) = mkemp(db, files.path()).await;
        let det = detail_sea(db, emp).await.expect("detail").expect("ada");
        assert_eq!(det.employee_number, "EMP-0002");
        let page = list_sea(db, "bud", &EmployeeFilter::default(), 1, 20)
            .await
            .expect("cari");
        assert_eq!(page.total, 1);
        assert_eq!(page.rows[0].id as i64, emp);
        let mut bad = base_input(hq(db).await);
        bad.first_name = "  ".to_string();
        let e = create_sea(db, files.path(), actor, &bad, None)
            .await
            .expect_err("nama kosong");
        assert_eq!(e, "Nama depan wajib diisi.");
        let mut upd = base_input(hq(db).await);
        upd.last_name = Some("Santoso".to_string());
        update_sea(db, files.path(), actor, emp, &upd, Some("2026-12-31"), None)
            .await
            .expect("ubah");
        let det = detail_sea(db, emp).await.expect("detail").expect("ada");
        assert_eq!(det.last_name.as_deref(), Some("Santoso"));
        assert_eq!(det.resign_date.as_deref(), Some("2026-12-31"));
        assert_eq!(
            det.company_name.as_deref(),
            Some("PT Contoh Sukses Indonesia")
        );
        delete_sea(db, actor, emp).await.expect("hapus");
        let page = list_sea(db, "bud", &EmployeeFilter::default(), 1, 20)
            .await
            .expect("cari");
        assert_eq!(page.total, 0);
        assert!(detail_sea(db, emp).await.expect("detail").is_none());
    }

    #[tokio::test]
    async fn child_alamat_dokumen_gaji_mengalir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (actor, emp) = mkemp(db, files.path()).await;
        let mut fam = BTreeMap::new();
        fam.insert("name".to_string(), "Ani".to_string());
        fam.insert("relationship".to_string(), "spouse".to_string());
        fam.insert("is_dependent".to_string(), "1".to_string());
        let fid = child_save_sea(db, actor, "families", emp, None, &fam)
            .await
            .expect("fam");
        let list1 = child_list_sea(db, "families", emp).await.expect("list");
        assert_eq!(list1.len(), 1);
        assert_eq!(list1[0].get("name").map(|s| s.as_str()), Some("Ani"));
        assert_eq!(list1[0].get("is_dependent").map(|s| s.as_str()), Some("1"));
        fam.insert("name".to_string(), "Ani Wijaya".to_string());
        child_save_sea(db, actor, "families", emp, Some(fid as i64), &fam)
            .await
            .expect("ubah");
        child_delete_sea(db, actor, "families", emp, fid as i64)
            .await
            .expect("hapus");
        assert!(child_list_sea(db, "families", emp)
            .await
            .expect("list")
            .is_empty());
        let e = child_list_sea(db, "alien", emp).await.expect_err("tipe asing");
        assert_eq!(e, "Tipe data tidak dikenal.");
        let mut addr = BTreeMap::new();
        addr.insert("city".to_string(), "Jakarta".to_string());
        save_address_sea(db, actor, emp, "ktp", &addr)
            .await
            .expect("alamat");
        let addrs = addresses_sea(db, emp).await.expect("get");
        assert_eq!(addrs.ktp.expect("ktp").city.as_deref(), Some("Jakarta"));
        let e = save_address_sea(db, actor, emp, "rumah", &addr)
            .await
            .expect_err("tipe rumah");
        assert_eq!(e, "Tipe alamat tidak valid.");
        let file = FileUpload {
            name: "ktp.pdf".to_string(),
            mime: "application/pdf".to_string(),
            bytes: vec![0x25, 0x50, 0x44, 0x46],
        };
        let did = upload_document_sea(
            db,
            files.path(),
            actor,
            emp,
            "ktp",
            "KTP Budi",
            None,
            &file,
        )
        .await
        .expect("upload");
        assert_eq!(documents_sea(db, emp).await.expect("docs").len(), 1);
        let got = document_bytes_sea(db, files.path(), emp, did as i64)
            .await
            .expect("bytes");
        assert_eq!(got.mime, "application/pdf");
        assert_eq!(got.name, "KTP Budi");
        assert_eq!(got.bytes, vec![0x25, 0x50, 0x44, 0x46]);
        let bad = FileUpload {
            name: "x.exe".to_string(),
            mime: "application/x-ms".to_string(),
            bytes: vec![1],
        };
        let e = upload_document_sea(db, files.path(), actor, emp, "ktp", "X", None, &bad)
            .await
            .expect_err("mime salah");
        assert_eq!(e, "Tipe berkas tidak diizinkan.");
        let e = upload_document_sea(db, files.path(), actor, emp, "salah", "X", None, &file)
            .await
            .expect_err("kategori salah");
        assert_eq!(e, "Kategori dokumen tidak valid.");
        set_salary_sea(db, actor, emp, 5_000_000.0, "2026-01-01", &[])
            .await
            .expect("gaji1");
        assert!(salary_current_sea(db, emp).await.expect("cur").is_some());
        let comp_id = one(
            db,
            "SELECT id FROM salary_components WHERE code = 'TRANSPORT'",
            "test.comp",
        )
        .await;
        set_salary_sea(
            db,
            actor,
            emp,
            6_000_000.0,
            "2026-06-01",
            &[(comp_id, 500_000.0)],
        )
        .await
        .expect("gaji2");
        let cur = salary_current_sea(db, emp)
            .await
            .expect("cur")
            .expect("ada");
        assert_eq!(cur.basic_salary, 6_000_000.0);
        assert_eq!(salary_history_sea(db, emp).await.expect("hist").len(), 2);
        let comps = salary_components_sea(db, cur.id as i64)
            .await
            .expect("comps");
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].code, "TRANSPORT");
        assert_eq!(comps[0].amount, Some(500_000.0));
        let avail = available_components_sea(db).await.expect("katalog");
        assert!(!avail.is_empty());
        let e = set_salary_sea(db, actor, emp, -1.0, "2026-06-01", &[])
            .await
            .expect_err("gaji negatif");
        assert_eq!(e, "Gaji pokok minimal 0.");
    }

    #[tokio::test]
    async fn child_types_memuat_enam_tipe() {
        assert_eq!(child_types().len(), 6);
        let slugs: Vec<String> = child_types().iter().map(|c| c.slug.clone()).collect();
        for s in [
            "families",
            "educations",
            "experiences",
            "contacts",
            "contracts",
            "career-histories",
        ] {
            assert!(slugs.contains(&s.to_string()), "kurang {s}");
        }
    }

    #[tokio::test]
    async fn profil_sea_gema_bidang_dan_dropdown() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin_id(db).await;
        let mut input = base_input(hq(db).await);
        input.nik = Some("  3201010101  ".to_string());
        input.personal_email = Some("budi@x.id".to_string());
        input.phone = Some("0811".to_string());
        input.bank_name = Some("BCA".to_string());
        input.bank_account_number = Some("123456".to_string());
        let photo = FileUpload {
            name: "wajah.png".to_string(),
            mime: "image/png".to_string(),
            bytes: vec![1, 2, 3],
        };
        let emp = create_sea(db, files.path(), actor, &input, Some(&photo))
            .await
            .expect("buat") as i64;
        let det = detail_sea(db, emp).await.expect("detail").expect("ada");
        assert_eq!(det.employee_number, "EMP-0002");
        assert_eq!(det.nik.as_deref(), Some("3201010101"));
        assert_eq!(det.personal_email.as_deref(), Some("budi@x.id"));
        assert_eq!(det.phone.as_deref(), Some("0811"));
        assert_eq!(det.bank_name.as_deref(), Some("BCA"));
        assert_eq!(det.bank_account_number.as_deref(), Some("123456"));
        assert!(det.photo.as_deref().expect("foto").starts_with("photos/"));
        let bynik = list_sea(db, "3201010101", &EmployeeFilter::default(), 1, 20)
            .await
            .expect("cari nik");
        assert_eq!(bynik.total, 1);
        let page2 = list_sea(db, "bud", &EmployeeFilter::default(), 2, 1)
            .await
            .expect("halaman dua");
        assert_eq!(page2.total, 1);
        assert!(page2.rows.is_empty());
        let mut dup = base_input(hq(db).await);
        dup.nik = Some("3201010101".to_string());
        let e = create_sea(db, files.path(), actor, &dup, None)
            .await
            .expect_err("nik duplikat");
        assert_eq!(e, "NIK sudah ada.");
        let mut noat = base_input(hq(db).await);
        noat.personal_email = Some("budi.x.id".to_string());
        let e = create_sea(db, files.path(), actor, &noat, None)
            .await
            .expect_err("email tanpa @");
        assert_eq!(e, "Email pribadi tidak valid.");
        let dd = dropdowns_sea(db).await.expect("dropdowns");
        assert!(!dd.companies.is_empty());
        assert!(dd.employees.iter().any(|o| o.name.trim() == "Budi"));
    }

    #[tokio::test]
    async fn child_alamat_dokumen_sea_alur_lengkap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let (actor, emp) = mkemp(db, files.path()).await;
        let mut vals = BTreeMap::new();
        vals.insert("name".to_string(), "Ibu Ani".to_string());
        vals.insert("phone".to_string(), "081234".to_string());
        let cid = child_save_sea(db, actor, "contacts", emp, None, &vals)
            .await
            .expect("contact baru");
        let rows = child_list_sea(db, "contacts", emp).await.expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("name").map(|s| s.as_str()), Some("Ibu Ani"));
        assert_eq!(rows[0].get("phone").map(|s| s.as_str()), Some("081234"));
        assert_eq!(rows[0].get("id").map(|s| s.as_str()), Some(cid.to_string().as_str()));
        vals.insert("phone".to_string(), "08999".to_string());
        child_save_sea(db, actor, "contacts", emp, Some(cid as i64), &vals)
            .await
            .expect("contact ubah");
        let rows = child_list_sea(db, "contacts", emp).await.expect("list2");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].get("phone").map(|s| s.as_str()), Some("08999"));
        let mut addr = BTreeMap::new();
        addr.insert("address".to_string(), "Jl. Mawar 1".to_string());
        addr.insert("city".to_string(), "Bandung".to_string());
        save_address_sea(db, actor, emp, "ktp", &addr)
            .await
            .expect("alamat baru");
        addr.insert("city".to_string(), "Jakarta".to_string());
        save_address_sea(db, actor, emp, "ktp", &addr)
            .await
            .expect("alamat ubah");
        let n = one(
            db,
            &format!("SELECT COUNT(*) FROM employee_addresses WHERE employee_id = {emp}"),
            "test.addrcount",
        )
        .await;
        assert_eq!(n, 1);
        let addrs = addresses_sea(db, emp).await.expect("addresses");
        let ktp = addrs.ktp.expect("ktp");
        assert_eq!(ktp.city.as_deref(), Some("Jakarta"));
        assert_eq!(ktp.address.as_deref(), Some("Jl. Mawar 1"));
        assert!(addrs.domicile.is_none());
        let up = FileUpload {
            name: "ktp.png".to_string(),
            mime: "image/png".to_string(),
            bytes: vec![1, 2, 3],
        };
        let did1 = upload_document_sea(db, files.path(), actor, emp, "ktp", "KTP A", None, &up)
            .await
            .expect("dok1");
        let _did2 = upload_document_sea(db, files.path(), actor, emp, "ijazah", "KTP B", None, &up)
            .await
            .expect("dok2");
        assert_eq!(documents_sea(db, emp).await.expect("docs").len(), 2);
        child_delete_sea(db, actor, "documents", emp, did1 as i64)
            .await
            .expect("hapus dok");
        let sisa = documents_sea(db, emp).await.expect("docs2");
        assert_eq!(sisa.len(), 1);
        assert_eq!(sisa[0].category, "ijazah");
        let e = document_bytes_sea(db, files.path(), emp, did1 as i64)
            .await
            .expect_err("dok terhapus");
        assert_eq!(e, "Dokumen tidak ditemukan.");
    }

    #[tokio::test]
    async fn gaji_sea_band_dan_komponen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        exec(
            db,
            "INSERT INTO job_grades (code, name, grade_order, min_salary, max_salary) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
            vec![
                Value::Text("JHT01".to_string()),
                Value::Text("Junior".to_string()),
                Value::Int(1),
                Value::Float(5_000_000.0),
                Value::Float(7_000_000.0),
            ],
            "test.grade",
        )
        .await
        .expect("grade");
        let grade_id = one(db, "SELECT id FROM job_grades WHERE code = 'JHT01'", "test.gid").await;
        let actor = admin_id(db).await;
        let mut input = base_input(hq(db).await);
        input.job_grade_id = Some(grade_id as i32);
        let emp = create_sea(db, files.path(), actor, &input, None)
            .await
            .expect("buat") as i64;
        let comp_id = one(
            db,
            "SELECT id FROM salary_components WHERE code = 'TRANSPORT'",
            "test.comp",
        )
        .await;
        let sid = set_salary_sea(
            db,
            actor,
            emp,
            5_000_000.0,
            "2026-09-01",
            &[(comp_id, 1_500_000.0)],
        )
        .await
        .expect("set_sea");
        assert!(sid > 0);
        let cur = salary_current_sea(db, emp)
            .await
            .expect("cur")
            .expect("gaji ada");
        assert_eq!(cur.id, sid);
        assert_eq!(cur.basic_salary, 5_000_000.0);
        assert!(cur.is_active);
        let comps = salary_components_sea(db, sid as i64).await.expect("comps");
        assert_eq!(comps.len(), 1);
        assert_eq!(comps[0].code, "TRANSPORT");
        assert_eq!(comps[0].amount, Some(1_500_000.0));
        let avail = available_components_sea(db).await.expect("katalog");
        assert!(!avail.is_empty());
        assert!(avail.iter().all(|c| c.amount.is_none()));
        let e = set_salary_sea(db, actor, emp, 4_999_999.0, "2026-10-01", &[])
            .await
            .expect_err("di luar band");
        assert_eq!(e, "Gaji pokok di luar band Junior.");
        exec(
            db,
            format!("UPDATE employees SET job_grade_id = NULL WHERE id = {emp}"),
            vec![],
            "test.detach",
        )
        .await
        .expect("lepas grade");
        let e = set_salary_sea(db, actor, emp, 1_000_000.0, "31-12-2026", &[])
            .await
            .expect_err("tanggal salah format");
        assert_eq!(e, "Tanggal efektif harus valid (YYYY-MM-DD).");
        let sid0 = set_salary_sea(db, actor, emp, 0.0, "2026-11-01", &[])
            .await
            .expect("gaji nol");
        let comps0 = salary_components_sea(db, sid0 as i64)
            .await
            .expect("comps0");
        assert!(comps0.is_empty());
        let cur0 = salary_current_sea(db, emp)
            .await
            .expect("cur0")
            .expect("gaji nol ada");
        assert_eq!(cur0.id, sid0);
        assert_eq!(cur0.basic_salary, 0.0);
        assert_eq!(salary_history_sea(db, emp).await.expect("hist").len(), 2);
    }
}
