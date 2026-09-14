//! Pengaturan sistem dan profil perusahaan.
//!
//! Baca tulis lewat `repository::SeaOrmSettings` (SeaORM murni).

/// Satu baris pengaturan.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Setting {
    pub key: String,
    pub value: Option<String>,
    pub group: String,
}

/// Profil perusahaan (baris pertama).
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Company {
    pub id: i32,
    pub code: String,
    pub name: String,
    pub legal_name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub npwp: Option<String>,
    pub established_date: Option<String>,
}

/// Pembaruan parsial profil perusahaan (field kosong = pertahankan nilai lama).
#[derive(serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct CompanyInput {
    pub name: Option<String>,
    pub legal_name: Option<String>,
    pub address: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub postal_code: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub npwp: Option<String>,
    pub established_date: Option<String>,
}
