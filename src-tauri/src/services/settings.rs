//! Pengaturan sistem dan profil perusahaan.

use rusqlite::{params, Connection, OptionalExtension};

use super::audit;

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

pub fn all_settings(conn: &Connection) -> Result<Vec<Setting>, String> {
    let mut stmt = conn
        .prepare("SELECT setting_key, setting_value, setting_group FROM system_settings ORDER BY setting_group, setting_key")
        .map_err(|e| format!("gagal menyiapkan query pengaturan: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Setting {
                key: r.get(0)?,
                value: r.get(1)?,
                group: r.get(2)?,
            })
        })
        .map_err(|e| format!("gagal membaca pengaturan: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris pengaturan: {e}"))?);
    }
    Ok(out)
}

/// Simpan banyak pengaturan sekaligus (upsert; kunci baru masuk grup general).
pub fn save_settings(
    conn: &Connection,
    actor_id: i64,
    items: &[(String, String)],
) -> Result<(), String> {
    for (key, value) in items {
        let key = key.trim();
        if key.is_empty() || key == "_csrf" {
            continue;
        }
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM system_settings WHERE setting_key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| format!("gagal mencari pengaturan: {e}"))?;
        match existing {
            Some(id) => {
                conn.execute(
                    "UPDATE system_settings SET setting_value = ?1 WHERE id = ?2",
                    params![value, id],
                )
                .map_err(|e| format!("gagal memperbarui pengaturan: {e}"))?;
            }
            None => {
                conn.execute(
                    "INSERT INTO system_settings (setting_key, setting_value, setting_group) VALUES (?1, ?2, 'general')",
                    params![key, value],
                )
                .map_err(|e| format!("gagal menambah pengaturan: {e}"))?;
            }
        }
    }
    let after = serde_json::to_string(
        &items
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<std::collections::BTreeMap<_, _>>(),
    )
    .unwrap_or_default();
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "system_settings",
        None,
        None,
        Some(&after),
        Some("Memperbarui pengaturan sistem"),
    )?;
    Ok(())
}

pub fn get_company(conn: &Connection) -> Result<Option<Company>, String> {
    let row: Option<(
        i64,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = conn
        .query_row(
            "SELECT id, code, name, legal_name, address, city, province, postal_code, phone, email, npwp, established_date FROM companies ORDER BY id LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("gagal memuat perusahaan: {e}"))?;
    row.map(
        |(
            id,
            code,
            name,
            legal_name,
            address,
            city,
            province,
            postal_code,
            phone,
            email,
            npwp,
            established_date,
        )| {
            Ok(Company {
                id: crate::to_dto_int(id, "company.id")?,
                code,
                name,
                legal_name,
                address,
                city,
                province,
                postal_code,
                phone,
                email,
                npwp,
                established_date,
            })
        },
    )
    .transpose()
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

pub fn save_company(conn: &Connection, actor_id: i64, input: &CompanyInput) -> Result<(), String> {
    let company = get_company(conn)?.ok_or("Perusahaan belum ada.".to_string())?;
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
    let before = serde_json::to_string(&company).unwrap_or_default();
    conn.execute(
        "UPDATE companies SET name = COALESCE(NULLIF(?1, ''), name), legal_name = COALESCE(?2, legal_name), address = COALESCE(?3, address), city = COALESCE(?4, city), province = COALESCE(?5, province), postal_code = COALESCE(?6, postal_code), phone = COALESCE(?7, phone), email = COALESCE(?8, email), npwp = COALESCE(?9, npwp), established_date = COALESCE(?10, established_date) WHERE id = ?11",
        params![
            input.name.as_deref().unwrap_or(""),
            input.legal_name.as_deref(),
            input.address.as_deref(),
            input.city.as_deref(),
            input.province.as_deref(),
            input.postal_code.as_deref(),
            input.phone.as_deref(),
            input.email.as_deref(),
            input.npwp.as_deref(),
            input.established_date.as_deref(),
            company.id as i64,
        ],
    )
    .map_err(|e| format!("gagal menyimpan perusahaan: {e}"))?;
    let after =
        serde_json::to_string(&get_company(conn)?.expect("perusahaan ada")).unwrap_or_default();
    audit::log(
        conn,
        Some(actor_id),
        "UPDATE",
        "company",
        Some(&company.id.to_string()),
        Some(&before),
        Some(&after),
        None,
    )?;
    Ok(())
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

    #[test]
    fn pengaturan_roundtrip_dan_audit() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        save_settings(
            &conn,
            actor,
            &[
                ("work_start_time".to_string(), "07:30".to_string()),
                ("kunci_baru".to_string(), "nilai".to_string()),
            ],
        )
        .expect("save");
        let all = all_settings(&conn).expect("all");
        let map: std::collections::HashMap<_, _> = all
            .iter()
            .map(|s| (s.key.clone(), s.value.clone()))
            .collect();
        assert_eq!(map["work_start_time"].as_deref(), Some("07:30"));
        assert_eq!(map["kunci_baru"].as_deref(), Some("nilai"));
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_logs WHERE module = 'system_settings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(n >= 1);
    }

    #[test]
    fn perusahaan_parsial_mempertahankan_lama() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let actor: i64 = conn
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let before = get_company(&conn).expect("get").expect("ada");
        save_company(
            &conn,
            actor,
            &CompanyInput {
                city: Some("Bandung".to_string()),
                ..Default::default()
            },
        )
        .expect("save");
        let after = get_company(&conn).expect("get").expect("ada");
        assert_eq!(after.city.as_deref(), Some("Bandung"));
        assert_eq!(after.name, before.name);
        assert_eq!(after.npwp, before.npwp);
        let e = save_company(
            &conn,
            actor,
            &CompanyInput {
                email: Some("bukan-email".to_string()),
                ..Default::default()
            },
        )
        .expect_err("email invalid");
        assert!(e.contains("Email"));
    }
}
