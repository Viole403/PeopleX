use super::audit;
use super::sea_raw::{exec, q_all, q_one, value_to_string, Value};

const PII_PREFIX: &str = "px1$";
pub const SESSION_TTL_SECS: i64 = 28_800;

fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16)?;
        let lo = (bytes[i + 1] as char).to_digit(16)?;
        out.push(((hi * 16) + lo) as u8);
        i += 2;
    }
    Some(out)
}

fn keystream(base: &[u8; 32], len: usize) -> Vec<u8> {
    let mut stream = Vec::with_capacity(len + 32);
    let mut block: u64 = 0;
    while stream.len() < len {
        let mut input = base.to_vec();
        input.extend_from_slice(&block.to_le_bytes());
        stream.extend_from_slice(&sha256_bytes(&input));
        block += 1;
    }
    stream.truncate(len);
    stream
}

async fn pii_key_of(db: &sea_orm::DatabaseConnection) -> Result<Option<String>, String> {
    let row = q_one(
        db,
        "SELECT setting_value FROM system_settings WHERE setting_key = 'pii_key'".to_string(),
        vec![],
        1,
        "security.pii_key",
    )
    .await?;
    Ok(match row {
        Some(r) => match &r[0] {
            Value::Text(s) if !s.trim().is_empty() => Some(s.clone()),
            _ => None,
        },
        None => None,
    })
}

pub async fn pii_encrypt(db: &sea_orm::DatabaseConnection, plain: &str) -> Result<String, String> {
    if plain.trim().is_empty() || plain.starts_with(PII_PREFIX) {
        return Ok(plain.to_string());
    }
    let key = match pii_key_of(db).await? {
        Some(k) => k,
        None => return Ok(plain.to_string()),
    };
    let base = sha256_bytes(key.as_bytes());
    let data = plain.as_bytes();
    let stream = keystream(&base, data.len());
    let cipher: Vec<u8> = data
        .iter()
        .zip(stream.iter())
        .map(|(a, b)| a ^ b)
        .collect();
    let mut mac_input = base.to_vec();
    mac_input.extend_from_slice(&cipher);
    let mac = hex_encode(&sha256_bytes(&mac_input)[..8]);
    Ok(format!("{}${}${}", PII_PREFIX.trim_end_matches('$'), hex_encode(&cipher), mac))
}

pub async fn pii_decrypt(db: &sea_orm::DatabaseConnection, stored: &str) -> Result<String, String> {
    if !stored.starts_with(PII_PREFIX) {
        return Ok(stored.to_string());
    }
    let key = match pii_key_of(db).await? {
        Some(k) => k,
        None => return Err("Kunci PII belum diatur.".to_string()),
    };
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 3 || parts[0] != "px1" {
        return Err("Data PII rusak.".to_string());
    }
    let cipher = match hex_decode(parts[1]) {
        Some(c) => c,
        None => return Err("Data PII rusak.".to_string()),
    };
    let base = sha256_bytes(key.as_bytes());
    let mut mac_input = base.to_vec();
    mac_input.extend_from_slice(&cipher);
    let expect = hex_encode(&sha256_bytes(&mac_input)[..8]);
    if expect != parts[2] {
        return Err("Data PII rusak.".to_string());
    }
    let stream = keystream(&base, cipher.len());
    let plain: Vec<u8> = cipher
        .iter()
        .zip(stream.iter())
        .map(|(a, b)| a ^ b)
        .collect();
    String::from_utf8(plain).map_err(|_| "Data PII rusak.".to_string())
}

pub fn session_remaining_expired(expiry: Option<i64>, now: i64) -> i64 {
    match expiry {
        None => -1,
        Some(e) => {
            if e <= now {
                0
            } else {
                e - now
            }
        }
    }
}

async fn setting_text(db: &sea_orm::DatabaseConnection, key: &str) -> Result<String, String> {
    let sql = "SELECT setting_value FROM system_settings WHERE setting_key = ?1".to_string();
    let row = q_one(db, sql, vec![key.to_string().into()], 1, "security.setting").await?;
    Ok(match row {
        Some(r) => match &r[0] {
            Value::Text(s) => s.clone(),
            _ => String::new(),
        },
        None => String::new(),
    })
}

pub async fn ip_allowed(db: &sea_orm::DatabaseConnection, ip: &str) -> Result<bool, String> {
    let list = setting_text(db, "ip_whitelist").await?;
    if list.trim().is_empty() {
        return Ok(true);
    }
    let target = ip.trim();
    Ok(list.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).any(|s| s == target))
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct SecurityStatus {
    pub pii_enabled: bool,
    pub ip_whitelist: String,
    pub session_ttl_secs: i64,
}

pub async fn security_status_sea(db: &sea_orm::DatabaseConnection) -> Result<SecurityStatus, String> {
    let pii_enabled = pii_key_of(db).await?.is_some();
    let ip_whitelist = setting_text(db, "ip_whitelist").await?;
    Ok(SecurityStatus {
        pii_enabled,
        ip_whitelist,
        session_ttl_secs: SESSION_TTL_SECS,
    })
}

async fn upsert_text(db: &sea_orm::DatabaseConnection, key: &str, val: &str) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let sql = "INSERT INTO system_settings (setting_key, setting_value, updated_at) VALUES ?1, ?2, ?3 \
               ON CONFLICT(setting_key) DO UPDATE SET setting_value = ?4, updated_at = ?5"
        .to_string();
    exec(
        db,
        sql,
        vec![
            key.to_string().into(),
            val.to_string().into(),
            now.clone().into(),
            val.to_string().into(),
            now.into(),
        ],
        "security.upsert",
    )
    .await?;
    Ok(())
}

pub async fn set_security_settings(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    ip_whitelist: Option<&str>,
    pii_key: Option<&str>,
) -> Result<(), String> {
    if let Some(v) = ip_whitelist {
        upsert_text(db, "ip_whitelist", v).await?;
    }
    if let Some(v) = pii_key {
        upsert_text(db, "pii_key", v).await?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "security.settings",
        None,
        None,
        None,
        Some("Pengaturan keamanan diperbarui."),
    )
    .await?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct LoginActivityRow {
    pub username_attempt: String,
    pub ip_address: String,
    pub user_agent: String,
    pub status: String,
    pub created_at: String,
}

fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => "-".to_string(),
        other => value_to_string(other),
    }
}

pub async fn login_activity_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<LoginActivityRow>, String> {
    let rows = q_all(
        db,
        "SELECT username_attempt, ip_address, user_agent, status, created_at FROM login_activity ORDER BY id DESC LIMIT 100".to_string(),
        vec![],
        5,
        "security.activity",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(LoginActivityRow {
            username_attempt: cell_text(&r[0]),
            ip_address: cell_text(&r[1]),
            user_agent: cell_text(&r[2]),
            status: cell_text(&r[3]),
            created_at: cell_text(&r[4]),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db_seed() -> (tempfile::TempDir, sea_orm::DatabaseConnection) {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        (dir, state.sea)
    }

    async fn put(db: &sea_orm::DatabaseConnection, key: &str, val: &str) {
        upsert_text(db, key, val).await.expect("upsert");
    }

    #[tokio::test]
    async fn pii_bulat_balik_dan_rusak_terdeteksi() {
        let (_d, db) = db_seed().await;
        let asli = "3201011203900001";
        let tanpa = pii_encrypt(&db, asli).await.expect("enkripsi");
        assert_eq!(tanpa, asli);
        put(&db, "pii_key", "kunci-tes-1").await;
        let enc = pii_encrypt(&db, asli).await.expect("enkripsi 2");
        assert!(enc.starts_with(PII_PREFIX));
        assert_ne!(enc, asli);
        let dec = pii_decrypt(&db, &enc).await.expect("dekripsi");
        assert_eq!(dec, asli);
        let enc2 = pii_encrypt(&db, &enc).await.expect("enkripsi ulang");
        assert_eq!(enc2, enc);
        let mut rusak = enc.clone();
        rusak.pop();
        rusak.push('0');
        let e = pii_decrypt(&db, &rusak).await.unwrap_err();
        assert!(e.contains("Data PII rusak."));
    }

    #[tokio::test]
    async fn whitelist_ips_dan_sesi_murni() {
        let (_d, db) = db_seed().await;
        put(&db, "ip_whitelist", "10.0.0.5, 10.0.0.9").await;
        assert!(ip_allowed(&db, "10.0.0.9").await.expect("boleh"));
        assert!(!ip_allowed(&db, "203.0.113.9").await.expect("tolak"));
        put(&db, "ip_whitelist", "").await;
        assert!(ip_allowed(&db, "1.2.3.4").await.expect("semua"));
        assert_eq!(session_remaining_expired(None, 100), -1);
        assert_eq!(session_remaining_expired(Some(50), 100), 0);
        assert_eq!(session_remaining_expired(Some(160), 100), 60);
    }

    #[tokio::test]
    async fn status_dan_aktivitas_aman() {
        let (_d, db) = db_seed().await;
        put(&db, "pii_key", "kunci-tes-2").await;
        let st = security_status_sea(&db).await.expect("status");
        assert!(st.pii_enabled);
        assert_eq!(st.session_ttl_secs, SESSION_TTL_SECS);
        let aktivitas = login_activity_list_sea(&db).await.expect("aktivitas");
        let _ = aktivitas.len();
    }
}
