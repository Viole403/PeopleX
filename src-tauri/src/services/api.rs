//! Token API ber-scopes dan webhook keluar.
//!
//! Token: format `px_<32hex>`, yang disimpan hanya hash SHA-256 + prefix
//! untuk identifikasi. Webhook: outbox `webhook_deliveries` + dispatcher
//! reqwest dengan signature HMAC bila secret diisi.

use rand::RngCore;
use sha2::{Digest, Sha256};

use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};
use crate::to_dto_int;

fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn random_hex(nbytes: usize) -> String {
    let mut b = vec![0u8; nbytes];
    rand::thread_rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())
}

fn hmac_hex(key: &str, msg: &str) -> String {
    let mut k = key.as_bytes().to_vec();
    if k.len() > 64 {
        k = Sha256::digest(&k).to_vec();
    }
    while k.len() < 64 {
        k.push(0);
    }
    let mut inner = Sha256::new();
    inner.update(k.iter().map(|b| b ^ 0x36).collect::<Vec<u8>>());
    inner.update(msg.as_bytes());
    let d = inner.finalize().to_vec();
    let mut outer = Sha256::new();
    outer.update(k.iter().map(|b| b ^ 0x5c).collect::<Vec<u8>>());
    outer.update(d);
    format!("{:x}", outer.finalize())
}

fn scopes_bersih(scopes: &str) -> String {
    scopes
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

// ---------------- Token ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ApiTokenRow {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub token_prefix: String,
    pub scopes: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

fn opt_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.clone()),
        _ => None,
    }
}

pub async fn api_token_issue_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    user_id: i64,
    name: &str,
    scopes: &str,
    ttl_days: Option<i64>,
) -> Result<(i32, String), String> {
    let nm = name.trim();
    if nm.is_empty() || nm.len() > 120 {
        return Err("Nama token wajib diisi sampai 120 karakter.".to_string());
    }
    let ada = q_one(
        db,
        "SELECT id FROM users WHERE id = ?1".to_string(),
        vec![Value::Int(user_id)],
        1,
        "api.user",
    )
    .await?;
    if ada.is_none() {
        return Err("Pengguna tidak ditemukan.".to_string());
    }
    let plain = format!("px_{}", random_hex(16));
    let prefix = plain.chars().take(8).collect::<String>();
    let exp = match ttl_days {
        Some(d) if d > 0 => Some(
            (chrono::Local::now() + chrono::Duration::days(d))
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        ),
        _ => None,
    };
    let now = now_str();
    let rid = exec_insert(
        db,
        "INSERT INTO api_tokens (user_id, name, token_hash, token_prefix, scopes, expires_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)".to_string(),
        vec![
            Value::Int(user_id),
            Value::Text(nm.to_string()),
            Value::Text(sha256_hex(&plain)),
            Value::Text(prefix),
            Value::Text(scopes_bersih(scopes)),
            exp.clone().map(Value::Text).unwrap_or(Value::Null),
            Value::Text(now),
        ],
        "api.issue",
    )
    .await?;
    super::audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "api.token",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok((to_dto_int(rid, "api.token.id")?, plain))
}

pub async fn api_token_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<ApiTokenRow>, String> {
    let rows = q_all(
        db,
        "SELECT id, user_id, name, token_prefix, scopes, expires_at, last_used_at, revoked_at FROM api_tokens ORDER BY id DESC".to_string(),
        vec![],
        8,
        "api.list",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(ApiTokenRow {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "api.token.id")?,
            user_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "api.token.user")?,
            name: value_to_string(&r[2]),
            token_prefix: value_to_string(&r[3]),
            scopes: value_to_string(&r[4]),
            expires_at: opt_text(&r[5]),
            last_used_at: opt_text(&r[6]),
            revoked_at: opt_text(&r[7]),
        });
    }
    Ok(out)
}

pub async fn api_token_revoke_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let n = exec(
        db,
        "UPDATE api_tokens SET revoked_at = ?1, updated_at = ?1 WHERE id = ?2 AND revoked_at IS NULL".to_string(),
        vec![Value::Text(now_str()), Value::Int(id)],
        "api.revoke",
    )
    .await?;
    if n == 0 {
        return Err("Token tidak ditemukan.".to_string());
    }
    super::audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "api.token",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ApiTokenIssued {
    pub id: i32,
    pub token: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ApiTokenAuthed {
    pub user_id: i32,
    pub scopes: String,
}

pub fn api_has_scope(scopes_csv: &str, need: &str) -> bool {
    scopes_csv.split(',').map(str::trim).any(|s| s == need || s == "*")
}

pub async fn api_auth_sea(
    db: &sea_orm::DatabaseConnection,
    plain: &str,
) -> Result<Option<(i64, String)>, String> {
    let row = q_one(
        db,
        "SELECT user_id, scopes, expires_at, revoked_at FROM api_tokens WHERE token_hash = ?1".to_string(),
        vec![Value::Text(sha256_hex(plain.trim()))],
        4,
        "api.auth",
    )
    .await?;
    let Some(r) = row else { return Ok(None) };
    if opt_text(&r[3]).is_some() {
        return Ok(None);
    }
    if let Some(exp) = opt_text(&r[2]) {
        if exp <= now_str() {
            return Ok(None);
        }
    }
    let uid = value_i64(&r[0]).unwrap_or(0);
    exec(
        db,
        "UPDATE api_tokens SET last_used_at = ?1 WHERE token_hash = ?2".to_string(),
        vec![Value::Text(now_str()), Value::Text(sha256_hex(plain.trim()))],
        "api.touch",
    )
    .await?;
    Ok(Some((uid, value_to_string(&r[1]))))
}

// ---------------- Webhook ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct WebhookRow {
    pub id: i32,
    pub name: String,
    pub url: String,
    pub has_secret: bool,
    pub events: String,
    pub is_active: bool,
    pub timeout_secs: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct WebhookInput {
    pub name: String,
    pub url: String,
    pub secret: Option<String>,
    pub events: String,
    pub is_active: bool,
    pub timeout_secs: i32,
}

pub async fn webhook_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &WebhookInput,
) -> Result<i32, String> {
    let nm = input.name.trim();
    if nm.is_empty() || nm.len() > 120 {
        return Err("Nama webhook wajib diisi sampai 120 karakter.".to_string());
    }
    let url = input.url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() > 500 {
        return Err("URL harus http(s) sampai 500 karakter.".to_string());
    }
    if input.timeout_secs < 1 || input.timeout_secs > 60 {
        return Err("Timeout 1 sampai 60 detik.".to_string());
    }
    let now = now_str();
    let ev = scopes_bersih(&input.events);
    let sec = input.secret.clone().filter(|s| !s.trim().is_empty());
    let new_id = match id {
        Some(x) => {
            let n = exec(
                db,
                "UPDATE webhooks SET name = ?1, url = ?2, secret = COALESCE(?3, secret), events = ?4, is_active = ?5, timeout_secs = ?6, updated_at = ?7 WHERE id = ?8".to_string(),
                vec![
                    Value::Text(nm.to_string()),
                    Value::Text(url.to_string()),
                    sec.map(Value::Text).unwrap_or(Value::Null),
                    Value::Text(ev),
                    Value::Int(if input.is_active { 1 } else { 0 }),
                    Value::Int(input.timeout_secs as i64),
                    Value::Text(now),
                    Value::Int(x),
                ],
                "wh.upd",
            )
            .await?;
            if n == 0 {
                return Err("Webhook tidak ditemukan.".to_string());
            }
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO webhooks (name, url, secret, events, is_active, timeout_secs, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)".to_string(),
                vec![
                    Value::Text(nm.to_string()),
                    Value::Text(url.to_string()),
                    sec.map(Value::Text).unwrap_or(Value::Null),
                    Value::Text(ev),
                    Value::Int(if input.is_active { 1 } else { 0 }),
                    Value::Int(input.timeout_secs as i64),
                    Value::Text(now),
                ],
                "wh.ins",
            )
            .await?
        }
    };
    super::audit::log_sea(
        db,
        Some(actor_id),
        if id.is_some() { "UPDATE" } else { "CREATE" },
        "api.webhook",
        Some(&new_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(new_id, "wh.id")
}

pub async fn webhook_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<WebhookRow>, String> {
    let rows = q_all(
        db,
        "SELECT id, name, url, secret, events, is_active, timeout_secs FROM webhooks ORDER BY id".to_string(),
        vec![],
        7,
        "wh.list",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(WebhookRow {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "wh.id")?,
            name: value_to_string(&r[1]),
            url: value_to_string(&r[2]),
            has_secret: opt_text(&r[3]).is_some(),
            events: value_to_string(&r[4]),
            is_active: value_i64(&r[5]).unwrap_or(0) != 0,
            timeout_secs: to_dto_int(value_i64(&r[6]).unwrap_or(10), "wh.timeout")?,
        });
    }
    Ok(out)
}

pub async fn webhook_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    exec(
        db,
        "DELETE FROM webhook_deliveries WHERE webhook_id = ?1".to_string(),
        vec![Value::Int(id)],
        "wh.del.hist",
    )
    .await?;
    let n = exec(
        db,
        "DELETE FROM webhooks WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        "wh.del",
    )
    .await?;
    if n == 0 {
        return Err("Webhook tidak ditemukan.".to_string());
    }
    super::audit::log_sea(
        db,
        Some(actor_id),
        "DELETE",
        "api.webhook",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

pub async fn webhook_emit_sea(
    db: &sea_orm::DatabaseConnection,
    event: &str,
    payload_json: &str,
) -> Result<i32, String> {
    let ev = event.trim();
    if ev.is_empty() || ev.len() > 80 {
        return Err("Nama event wajib diisi sampai 80 karakter.".to_string());
    }
    let hooks = webhook_list_sea(db).await?;
    let mut n = 0i64;
    for h in hooks {
        if !h.is_active {
            continue;
        }
        if !h.events.is_empty() && !h.events.split(',').map(str::trim).any(|e| e == ev) {
            continue;
        }
        exec_insert(
            db,
            "INSERT INTO webhook_deliveries (webhook_id, event, payload, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)".to_string(),
            vec![
                Value::Int(h.id as i64),
                Value::Text(ev.to_string()),
                Value::Text(payload_json.to_string()),
                Value::Text(now_str()),
            ],
            "wh.emit",
        )
        .await?;
        n += 1;
    }
    to_dto_int(n, "wh.emit.n")
}

pub async fn webhook_dispatch_sea(
    db: &sea_orm::DatabaseConnection,
    max: i64,
) -> Result<i64, String> {
    let rows = q_all(
        db,
        "SELECT d.id, w.url, w.secret, w.timeout_secs, d.event, d.payload FROM webhook_deliveries d INNER JOIN webhooks w ON w.id = d.webhook_id WHERE d.status = 'pending' ORDER BY d.id LIMIT ?1".to_string(),
        vec![Value::Int(max.max(1).min(50))],
        6,
        "wh.due",
    )
    .await?;
    let mut done = 0i64;
    for r in rows {
        let id = value_i64(&r[0]).unwrap_or(0);
        let url = value_to_string(&r[1]);
        let secret = opt_text(&r[2]);
        let timeout = value_i64(&r[3]).unwrap_or(10).clamp(1, 60) as u64;
        let event = value_to_string(&r[4]);
        let payload = value_to_string(&r[5]);
        let body = serde_json::json!({
            "event": event,
            "payload": serde_json::from_str::<serde_json::Value>(&payload).unwrap_or(serde_json::Value::Null),
            "delivered_at": now_str(),
        })
        .to_string();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .map_err(|e| format!("gagal membuat klien http: {e}"))?;
        let mut req = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-PeopleX-Event", event.clone());
        if let Some(sec) = secret.filter(|s| !s.is_empty()) {
            req = req.header("X-PeopleX-Signature", hmac_hex(&sec, &body));
        }
        let res = req.body(body).send().await;
        match res {
            Ok(resp) if resp.status().is_success() => {
                exec(
                    db,
                    "UPDATE webhook_deliveries SET status = 'ok', attempts = attempts + 1, updated_at = ?1 WHERE id = ?2".to_string(),
                    vec![Value::Text(now_str()), Value::Int(id)],
                    "wh.ok",
                )
                .await?;
                done += 1;
            }
            Ok(resp) => {
                let e = format!("http {}", resp.status());
                exec(
                    db,
                    "UPDATE webhook_deliveries SET status = 'failed', attempts = attempts + 1, last_error = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
                    vec![Value::Text(e), Value::Text(now_str()), Value::Int(id)],
                    "wh.fail",
                )
                .await?;
            }
            Err(e) => {
                exec(
                    db,
                    "UPDATE webhook_deliveries SET status = 'failed', attempts = attempts + 1, last_error = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
                    vec![Value::Text(e.to_string()), Value::Text(now_str()), Value::Int(id)],
                    "wh.fail",
                )
                .await?;
            }
        }
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn state() -> (tempfile::TempDir, sea_orm::DatabaseConnection) {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        (dir, app.sea)
    }

    async fn admin(db: &sea_orm::DatabaseConnection) -> i64 {
        q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "t.admin",
        )
        .await
        .expect("admin")
        .expect("ada")
        .first()
        .and_then(value_i64)
        .expect("id")
    }

    #[tokio::test]
    async fn token_terbit_dan_diautentikasi() {
        let (_d, db) = state().await;
        let a = admin(&db).await;
        let (id, plain) = api_token_issue_sea(&db, a, a, "integrasi", "employee.read,payroll.read", Some(30))
            .await
            .expect("terbit");
        assert!(id > 0);
        assert!(plain.starts_with("px_"));
        assert!(api_has_scope("employee.read,payroll.read", "employee.read"));
        assert!(!api_has_scope("employee.read", "payroll.read"));
        let authed = api_auth_sea(&db, &plain).await.expect("auth").expect("ada");
        assert_eq!(authed.0, a);
        assert!(api_auth_sea(&db, "px_salah").await.expect("salah").is_none());
        api_token_revoke_sea(&db, a, id as i64).await.expect("cabut");
        assert!(api_auth_sea(&db, &plain).await.expect("cabut2").is_none());
    }

    #[tokio::test]
    async fn webhook_emit_masuk_outbox() {
        let (_d, db) = state().await;
        let a = admin(&db).await;
        let id = webhook_save_sea(
            &db,
            a,
            None,
            &WebhookInput {
                name: "uji".to_string(),
                url: "https://contoh.test/hook".to_string(),
                secret: Some("rahasia".to_string()),
                events: "leave.decided".to_string(),
                is_active: true,
                timeout_secs: 5,
            },
        )
        .await
        .expect("simpan");
        assert!(id > 0);
        assert!(webhook_save_sea(
            &db,
            a,
            None,
            &WebhookInput {
                name: "x".to_string(),
                url: "ftp://salah".to_string(),
                secret: None,
                events: String::new(),
                is_active: true,
                timeout_secs: 5,
            },
        )
        .await
        .is_err());
        let n = webhook_emit_sea(&db, "leave.decided", "{\"id\":1}").await.expect("emit");
        assert_eq!(n, 1);
        let n2 = webhook_emit_sea(&db, "lain", "{}").await.expect("emit2");
        assert_eq!(n2, 0);
        webhook_delete_sea(&db, a, id as i64).await.expect("hapus");
        assert!(webhook_list_sea(&db).await.expect("list").is_empty());
    }
}
