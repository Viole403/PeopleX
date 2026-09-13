//! Abstraksi repository: batas antara command dan backend penyimpanan.
//!
//! Implementasi saat ini memakai SQLite (`rusqlite`) di balik trait ini.
//! Backend lain di masa depan cukup mengimplementasikan trait yang sama
//! tanpa mengubah tanda tangan command.

use rusqlite::Connection;

use super::settings::{Company, CompanyInput, Setting};

/// Penyimpanan pengaturan dan profil perusahaan.
pub trait SettingsRepo {
    fn all_settings(&self) -> Result<Vec<Setting>, String>;
    fn save_settings(&self, actor_id: i64, items: &[(String, String)]) -> Result<(), String>;
    fn get_company(&self) -> Result<Option<Company>, String>;
    fn save_company(&self, actor_id: i64, input: &CompanyInput) -> Result<(), String>;
}

/// Implementasi SQLite untuk trait di atas.
pub struct SqliteSettings<'a>(pub &'a Connection);

impl SettingsRepo for SqliteSettings<'_> {
    fn all_settings(&self) -> Result<Vec<Setting>, String> {
        super::settings::all_settings(self.0)
    }

    fn save_settings(&self, actor_id: i64, items: &[(String, String)]) -> Result<(), String> {
        super::settings::save_settings(self.0, actor_id, items)
    }

    fn get_company(&self) -> Result<Option<Company>, String> {
        super::settings::get_company(self.0)
    }

    fn save_company(&self, actor_id: i64, input: &CompanyInput) -> Result<(), String> {
        super::settings::save_company(self.0, actor_id, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, seed};

    #[test]
    fn repo_settings_sejalan_dengan_fungsi_langsung() {
        let dir = tempfile::tempdir().expect("dir");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        let repo = SqliteSettings(&c);
        let via_trait = repo.all_settings().expect("trait");
        let direct = crate::services::settings::all_settings(&c).expect("langsung");
        assert_eq!(via_trait.len(), direct.len());
        assert!(via_trait.iter().any(|s| s.key == "company_name"));
        let company = repo.get_company().expect("perusahaan");
        assert!(company.is_some());
    }
}
