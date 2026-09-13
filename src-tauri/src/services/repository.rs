//! Abstraksi repository: batas antara command dan backend penyimpanan.
//!
//! Seluruh baca tulis lewat SeaORM (`SeaOrmSettings` dkk) agar satu kode
//! mendukung SQLite, PostgreSQL, dan MySQL lewat config.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set,
};

use super::settings::{Company, CompanyInput, Setting};
use crate::entities::{company, system_setting};

/// Implementasi SeaORM: satu-satunya jalur baca tulis.
pub struct SeaOrmSettings<'a>(pub &'a sea_orm::DatabaseConnection);

impl SeaOrmSettings<'_> {
    pub async fn all_settings(&self) -> Result<Vec<Setting>, String> {
        let rows = system_setting::Entity::find()
            .all(self.0)
            .await
            .map_err(|e| format!("gagal memuat pengaturan: {e}"))?;
        Ok(rows
            .into_iter()
            .map(|m| Setting {
                key: m.setting_key,
                value: m.setting_value,
                group: m.setting_group,
            })
            .collect())
    }

    pub async fn save_settings(
        &self,
        items: &[(String, String)],
    ) -> Result<(), String> {
        for (key, value) in items {
            let key = key.trim();
            if key.is_empty() || key == "_csrf" {
                continue;
            }
            let existing = system_setting::Entity::find()
                .filter(system_setting::Column::SettingKey.eq(key))
                .one(self.0)
                .await
                .map_err(|e| format!("gagal mencari pengaturan: {e}"))?;
            match existing {
                Some(m) => {
                    let mut am = m.into_active_model();
                    am.setting_value = Set(Some(value.clone()));
                    am.update(self.0)
                        .await
                        .map_err(|e| format!("gagal memperbarui pengaturan: {e}"))?;
                }
                None => {
                    system_setting::ActiveModel {
                        setting_key: Set(key.to_string()),
                        setting_value: Set(Some(value.clone())),
                        setting_group: Set("general".to_string()),
                        ..Default::default()
                    }
                    .insert(self.0)
                    .await
                    .map_err(|e| format!("gagal menambah pengaturan: {e}"))?;
                }
            }
        }
        Ok(())
    }

    pub async fn get_company(&self) -> Result<Option<Company>, String> {        use sea_orm::QueryOrder;
        company::Entity::find()
            .order_by_asc(company::Column::Id)
            .one(self.0)
            .await
            .map_err(|e| format!("gagal memuat perusahaan: {e}"))?
            .map(|m| {
                crate::to_dto_int(m.id as i64, "company.id").map(|id| Company {
                    id,
                    code: m.code,
                    name: m.name,
                    legal_name: m.legal_name,
                    address: m.address,
                    city: m.city,
                    province: m.province,
                    postal_code: m.postal_code,
                    phone: m.phone,
                    email: m.email,
                    npwp: m.npwp,
                    established_date: m.established_date,
                })
            })
            .transpose()
    }

    pub async fn save_company(&self, input: &CompanyInput) -> Result<Company, String> {
        if let Some(name) = input.name.as_deref() {
            if !name.trim().is_empty() && name.len() > 150 {
                return Err("Nama perusahaan maksimal 150 karakter.".to_string());
            }
        }
        if let Some(email) = input.email.as_deref() {
            if !email.trim().is_empty() && !email.contains('@') {
                return Err("Email perusahaan tidak valid.".to_string());
            }
        }
        let cur = self
            .get_company()
            .await?
            .ok_or("Perusahaan belum ada.".to_string())?;
        let m = company::Entity::find_by_id(cur.id)
            .one(self.0)
            .await
            .map_err(|e| format!("gagal memuat perusahaan: {e}"))?
            .ok_or("Perusahaan belum ada.".to_string())?;
        let mut am = m.into_active_model();
        if let Some(v) = input.name.as_deref() {
            if !v.trim().is_empty() {
                am.name = Set(v.to_string());
            }
        }
        if let Some(v) = input.legal_name.as_deref() {
            am.legal_name = Set(Some(v.to_string()));
        }
        if let Some(v) = input.address.as_deref() {
            am.address = Set(Some(v.to_string()));
        }
        if let Some(v) = input.city.as_deref() {
            am.city = Set(Some(v.to_string()));
        }
        if let Some(v) = input.province.as_deref() {
            am.province = Set(Some(v.to_string()));
        }
        if let Some(v) = input.postal_code.as_deref() {
            am.postal_code = Set(Some(v.to_string()));
        }
        if let Some(v) = input.phone.as_deref() {
            am.phone = Set(Some(v.to_string()));
        }
        if let Some(v) = input.email.as_deref() {
            am.email = Set(Some(v.to_string()));
        }
        if let Some(v) = input.npwp.as_deref() {
            am.npwp = Set(Some(v.to_string()));
        }
        if let Some(v) = input.established_date.as_deref() {
            am.established_date = Set(Some(v.to_string()));
        }
        am.update(self.0)
            .await
            .map_err(|e| format!("gagal menyimpan perusahaan: {e}"))?;
        self.get_company()
            .await?
            .ok_or("Perusahaan belum ada.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_state;

    #[tokio::test]
    async fn repo_seaorm_baca_dan_tulis_pengaturan() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let repo = SeaOrmSettings(&state.sea);
        let before = repo.all_settings().await.expect("baca");
        assert!(before.iter().any(|s| s.key == "company_name"));
        let company = repo.get_company().await.expect("perusahaan");
        assert!(company.is_some());
        repo.save_settings(&[("company_phone".to_string(), "+62-99".to_string())])
            .await
            .expect("simpan");
        let after = repo.all_settings().await.expect("baca lagi");
        assert!(after
            .iter()
            .any(|s| s.key == "company_phone" && s.value.as_deref() == Some("+62-99")));
    }
}
