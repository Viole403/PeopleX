//! Autentikasi: login lockout, reset token, ganti password, sesi, MFA.

use chrono::Local;
use rand::RngCore;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set,
};
use sha2::{Digest, Sha256};

use crate::entities::{login_activity, permission, role, role_permission, user, user_role};
use crate::services::audit;
use crate::{to_dto_int, SessionUser};

const MAX_ATTEMPTS: i64 = 5;
const LOCKOUT_MINUTES: i64 = 15;
const RESET_TOKEN_BYTES: usize = 32;
const RESET_VALID_HOURS: i64 = 1;
const MIN_PASSWORD_LEN: usize = 8;

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

async fn find_by_login(
    db: &sea_orm::DatabaseConnection,
    login: &str,
) -> Result<Option<user::Model>, String> {
    let by_name = user::Entity::find()
        .filter(user::Column::Username.eq(login))
        .one(db)
        .await
        .map_err(|e| format!("gagal mencari pengguna: {e}"))?;
    if by_name.is_some() {
        return Ok(by_name);
    }
    user::Entity::find()
        .filter(user::Column::Email.eq(login))
        .one(db)
        .await
        .map_err(|e| format!("gagal mencari pengguna: {e}"))
}

async fn record_login_activity(
    db: &sea_orm::DatabaseConnection,
    user_id: Option<i64>,
    attempt: &str,
    status: &str,
) -> Result<(), String> {
    login_activity::ActiveModel {
        user_id: Set(user_id.map(|v| v as i32)),
        username_attempt: Set(if attempt.is_empty() {
            None
        } else {
            Some(attempt.to_string())
        }),
        ip_address: Set(Some("desktop".to_string())),
        user_agent: Set(Some("peoplex".to_string())),
        status: Set(status.to_string()),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("gagal mencatat aktivitas login: {e}"))?;
    Ok(())
}

/// Hasil login berhasil.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct LoginOk {
    pub user: SessionUser,
    pub must_change_password: bool,
    pub mfa_required: bool,
}

/// Muat profil sesi (peran + izin). None bila user tak aktif/ tak ada.
pub async fn load_session_user(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Option<SessionUser>, String> {
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat sesi: {e}"))?;
    let Some(u) = u else {
        return Ok(None);
    };
    if u.status != "active" {
        return Ok(None);
    }
    let urs = user_role::Entity::find()
        .filter(user_role::Column::UserId.eq(user_id as i32))
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let mut roles = Vec::new();
    let mut perm_ids = Vec::new();
    for ur in &urs {
        if let Some(r) = role::Entity::find_by_id(ur.role_id)
            .one(db)
            .await
            .map_err(|e| format!("gagal memuat peran: {e}"))?
        {
            roles.push(r.slug);
        }
        let rps = role_permission::Entity::find()
            .filter(role_permission::Column::RoleId.eq(ur.role_id))
            .all(db)
            .await
            .map_err(|e| format!("gagal memuat izin: {e}"))?;
        perm_ids.extend(rps.into_iter().map(|r| r.permission_id));
    }
    let mut permissions = Vec::new();
    for pid in perm_ids {
        if let Some(p) = permission::Entity::find_by_id(pid)
            .one(db)
            .await
            .map_err(|e| format!("gagal memuat izin: {e}"))?
        {
            if !permissions.contains(&p.slug) {
                permissions.push(p.slug);
            }
        }
    }
    Ok(Some(SessionUser {
        id: to_dto_int(user_id, "user.id")?,
        username: u.username,
        must_change_password: u.must_change_password != 0,
        is_super_admin: roles.iter().any(|r| r == "super-administrator"),
        roles,
        permissions,
    }))
}

fn locked_minutes_remaining(locked_until: &str, now: &str) -> Option<i64> {
    if locked_until > now {
        let fmt = "%Y-%m-%d %H:%M:%S";
        let end = chrono::NaiveDateTime::parse_from_str(locked_until, fmt).ok()?;
        let cur = chrono::NaiveDateTime::parse_from_str(now, fmt).ok()?;
        let secs = (end - cur).num_seconds().max(0);
        Some((secs + 59) / 60)
    } else {
        None
    }
}

/// Coba login. Pesan gagal generik agar tak membocorkan akun mana yang ada.
pub async fn attempt_login(
    db: &sea_orm::DatabaseConnection,
    login: &str,
    password: &str,
) -> Result<LoginOk, String> {
    let login = login.trim();
    if login.is_empty() || password.is_empty() {
        return Err("Username/email dan password wajib diisi.".to_string());
    }
    let user = find_by_login(db, login).await?;
    let now = now_str();

    if let Some(u) = &user {
        if let Some(locked) = u.locked_until.as_deref() {
            if let Some(mins) = locked_minutes_remaining(locked, &now) {
                record_login_activity(db, None, login, "failed").await?;
                return Err(format!(
                    "Akun terkunci sementara. Coba lagi dalam {mins} menit."
                ));
            }
        }
    }

    let valid = match &user {
        Some(u) if u.status == "active" => bcrypt::verify(password, &u.password)
            .map_err(|e| format!("gagal verifikasi password: {e}"))?,
        _ => false,
    };
    if !valid {
        if let Some(u) = &user {
            let attempts = u.failed_login_attempts as i64 + 1;
            let mut am = u.clone().into_active_model();
            am.failed_login_attempts = Set(attempts as i32);
            if attempts >= MAX_ATTEMPTS {
                let locked = (Local::now() + chrono::Duration::minutes(LOCKOUT_MINUTES))
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                am.locked_until = Set(Some(locked));
            }
            am.update(db)
                .await
                .map_err(|e| format!("gagal mencatat percobaan: {e}"))?;
        }
        record_login_activity(db, user.as_ref().map(|u| u.id as i64), login, "failed").await?;
        return Err("Username/email atau password salah.".to_string());
    }

    let u = user.expect("user valid");
    let mut am = u.clone().into_active_model();
    am.failed_login_attempts = Set(0);
    am.locked_until = Set(None);
    am.last_login_at = Set(Some(now.clone()));
    am.last_login_ip = Set(Some("desktop".to_string()));
    am.update(db)
        .await
        .map_err(|e| format!("gagal memperbarui login: {e}"))?;
    record_login_activity(db, Some(u.id as i64), login, "success").await?;
    audit::log_sea(
        db,
        Some(u.id as i64),
        "LOGIN",
        "auth",
        Some(&u.id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    let session = load_session_user(db, u.id as i64)
        .await?
        .ok_or("Sesi berakhir. Masuk kembali.".to_string())?;
    Ok(LoginOk {
        user: session,
        must_change_password: u.must_change_password != 0,
        mfa_required: u.mfa_enabled != 0,
    })
}

/// Catat logout + audit.
pub async fn logout(db: &sea_orm::DatabaseConnection, user_id: i64) -> Result<(), String> {
    audit::log_sea(
        db,
        Some(user_id),
        "LOGOUT",
        "auth",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

fn random_token() -> String {
    let mut bytes = [0u8; RESET_TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

const MFA_SECRET_BYTES: usize = 20;
const MFA_STEP_SECS: i64 = 30;

/// base32 RFC 4648 tanpa padding.
fn base32_encode(raw: &[u8]) -> String {
    const ALPH: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buf: u32 = 0;
    let mut bits = 0;
    for b in raw {
        buf = (buf << 8) | *b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPH[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPH[((buf << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn base32_decode(s: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf: u32 = 0;
    let mut bits = 0;
    for c in s.trim().chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            '2'..='7' => c as u32 - '2' as u32 + 26,
            '=' => continue,
            _ => return Err("Secret MFA tidak valid.".to_string()),
        };
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let hashed;
    let k: &[u8] = if key.len() > 64 {
        hashed = Sha256::digest(key).to_vec();
        &hashed
    } else {
        key
    };
    let mut block = [0u8; 64];
    block[..k.len()].copy_from_slice(k);
    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(msg);
    let mid = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(&mid);
    let done = outer.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&done);
    out
}

/// Kode TOTP 6 digit (langkah 30 detik, SHA256).
fn totp_code(secret: &[u8], unix_secs: i64) -> String {
    let counter = (unix_secs / MFA_STEP_SECS) as u64;
    let mac = hmac_sha256(secret, &counter.to_be_bytes());
    let off = (mac[31] & 0x0f) as usize;
    let n = ((mac[off] as u32 & 0x7f) << 24)
        | ((mac[off + 1] as u32) << 16)
        | ((mac[off + 2] as u32) << 8)
        | (mac[off + 3] as u32);
    format!("{:06}", n % 1_000_000)
}

fn totp_valid(secret_b32: &str, code: &str, unix_secs: i64) -> Result<bool, String> {
    let secret = base32_decode(secret_b32)?;
    let code = code.trim();
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(false);
    }
    for step in [-1, 0, 1] {
        if totp_code(&secret, unix_secs + step * MFA_STEP_SECS) == code {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Data enroll MFA untuk dipindai/diketik ke aplikasi authenticator.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct MfaSetup {
    pub secret: String,
    pub otpauth_url: String,
}

/// Mulai enroll: simpan secret baru (belum aktif) dan kembalikan untuk dipindai.
pub async fn mfa_setup(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<MfaSetup, String> {
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat akun: {e}"))?;
    let Some(u) = u else {
        return Err("Akun tidak ditemukan.".to_string());
    };
    if u.status != "active" {
        return Err("Akun tidak ditemukan.".to_string());
    }
    let mut bytes = [0u8; MFA_SECRET_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret = base32_encode(&bytes);
    let username = u.username.clone();
    let mut am = u.into_active_model();
    am.mfa_secret = Set(Some(secret.clone()));
    am.mfa_enabled = Set(0);
    am.update(db)
        .await
        .map_err(|e| format!("gagal menyimpan secret MFA: {e}"))?;
    Ok(MfaSetup {
        otpauth_url: format!(
            "otpauth://totp/PeopleX:{}?secret={secret}&issuer=PeopleX&algorithm=SHA256&digits=6&period=30",
            username
        ),
        secret,
    })
}

/// Aktifkan MFA setelah kode pertama terbukti benar.
pub async fn mfa_enable(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    code: &str,
) -> Result<(), String> {
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat MFA: {e}"))?;
    let Some(u) = u else {
        return Err("MFA belum disiapkan.".to_string());
    };
    let Some(sec) = u.mfa_secret.clone() else {
        return Err("MFA belum disiapkan.".to_string());
    };
    let now = Local::now().timestamp();
    if !totp_valid(&sec, code, now)? {
        return Err("Kode MFA salah.".to_string());
    }
    let mut am = u.into_active_model();
    am.mfa_enabled = Set(1);
    am.update(db)
        .await
        .map_err(|e| format!("gagal mengaktifkan MFA: {e}"))?;
    audit::log_sea(
        db,
        Some(user_id),
        "UPDATE",
        "mfa",
        Some(&user_id.to_string()),
        None,
        None,
        Some("MFA diaktifkan"),
    )
    .await?;
    Ok(())
}

/// Matikan MFA dengan verifikasi password.
pub async fn mfa_disable(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    password: &str,
) -> Result<(), String> {
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat akun: {e}"))?;
    let Some(u) = u else {
        return Err("Akun tidak ditemukan.".to_string());
    };
    if !bcrypt::verify(password, &u.password)
        .map_err(|e| format!("gagal verifikasi password: {e}"))?
    {
        return Err("Password salah.".to_string());
    }
    let mut am = u.into_active_model();
    am.mfa_secret = Set(None);
    am.mfa_enabled = Set(0);
    am.update(db)
        .await
        .map_err(|e| format!("gagal mematikan MFA: {e}"))?;
    audit::log_sea(
        db,
        Some(user_id),
        "UPDATE",
        "mfa",
        Some(&user_id.to_string()),
        None,
        None,
        Some("MFA dimatikan"),
    )
    .await?;
    Ok(())
}

/// Verifikasi kode tahap kedua dan kembalikan profil sesi.
pub async fn verify_mfa(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    code: &str,
) -> Result<SessionUser, String> {
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat MFA: {e}"))?;
    let Some(u) = u else {
        return Err("MFA tidak aktif untuk akun ini.".to_string());
    };
    if u.status != "active" {
        return Err("MFA tidak aktif untuk akun ini.".to_string());
    }
    let Some(sec) = u.mfa_secret.clone() else {
        return Err("MFA tidak aktif untuk akun ini.".to_string());
    };
    if u.mfa_enabled == 0 {
        return Err("MFA tidak aktif untuk akun ini.".to_string());
    }
    let now = Local::now().timestamp();
    if !totp_valid(&sec, code, now)? {
        record_login_activity(db, Some(user_id), "", "failed").await?;
        return Err("Kode MFA salah.".to_string());
    }
    record_login_activity(db, Some(user_id), "", "success").await?;
    load_session_user(db, user_id)
        .await?
        .ok_or("Sesi berakhir. Masuk kembali.".to_string())
}

/// Minta reset: simpan hash token + kedaluwarsa 1 jam.
/// Kembalikan token plaintext bila email aktif dikenal, None bila tidak
/// (pemanggil menampilkan pesan generik yang sama agar anti-enumerasi).
/// Tanpa SMTP: token diserahkan ke admin/HR untuk diteruskan ke user.
pub async fn request_password_reset(
    db: &sea_orm::DatabaseConnection,
    email: &str,
) -> Result<Option<String>, String> {
    let email = email.trim();
    let u = user::Entity::find()
        .filter(user::Column::Email.eq(email))
        .filter(user::Column::Status.eq("active"))
        .one(db)
        .await
        .map_err(|e| format!("gagal mencari email: {e}"))?;
    let Some(u) = u else {
        return Ok(None);
    };
    let token = random_token();
    let expires = (Local::now() + chrono::Duration::hours(RESET_VALID_HOURS))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    let mut am = u.clone().into_active_model();
    am.password_reset_token = Set(Some(sha256_hex(&token)));
    am.password_reset_expires_at = Set(Some(expires));
    am.update(db)
        .await
        .map_err(|e| format!("gagal menyimpan token reset: {e}"))?;
    audit::log_sea(
        db,
        Some(u.id as i64),
        "PASSWORD_RESET_REQUEST",
        "auth",
        Some(&u.id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(Some(token))
}

/// Terapkan password baru via token (sekali pakai, 1 jam).
pub async fn reset_password(
    db: &sea_orm::DatabaseConnection,
    token: &str,
    new_password: &str,
) -> Result<(), String> {
    check_password_policy(new_password)?;
    let now = now_str();
    let users = user::Entity::find()
        .filter(user::Column::PasswordResetToken.eq(sha256_hex(token.trim())))
        .all(db)
        .await
        .map_err(|e| format!("gagal memvalidasi token: {e}"))?;
    let u = users
        .into_iter()
        .find(|u| u.password_reset_expires_at.as_deref().unwrap_or("") > now.as_str())
        .ok_or("Tautan reset tidak valid atau sudah kedaluwarsa.".to_string())?;
    let hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password: {e}"))?;
    let uid = u.id;
    let mut am = u.into_active_model();
    am.password = Set(hash);
    am.password_reset_token = Set(None);
    am.password_reset_expires_at = Set(None);
    am.must_change_password = Set(0);
    am.failed_login_attempts = Set(0);
    am.locked_until = Set(None);
    am.update(db)
        .await
        .map_err(|e| format!("gagal menyimpan password baru: {e}"))?;
    audit::log_sea(
        db,
        Some(uid as i64),
        "PASSWORD_RESET",
        "auth",
        Some(&uid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

fn check_password_policy(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "Password baru minimal {MIN_PASSWORD_LEN} karakter."
        ));
    }
    Ok(())
}

/// Ganti password sendiri (wajib tahu password lama).
pub async fn change_password(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    current_password: &str,
    new_password: &str,
) -> Result<(), String> {
    let u = user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat pengguna: {e}"))?;
    let Some(u) = u else {
        return Err("Pengguna tidak ditemukan.".to_string());
    };
    if !bcrypt::verify(current_password, &u.password)
        .map_err(|e| format!("gagal verifikasi password: {e}"))?
    {
        return Err("Password saat ini tidak sesuai.".to_string());
    }
    check_password_policy(new_password)?;
    let new_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password: {e}"))?;
    let mut am = u.into_active_model();
    am.password = Set(new_hash);
    am.must_change_password = Set(0);
    am.update(db)
        .await
        .map_err(|e| format!("gagal menyimpan password: {e}"))?;
    audit::log_sea(
        db,
        Some(user_id),
        "PASSWORD_CHANGE",
        "auth",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_state;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        user::Entity::find()
            .filter(user::Column::Username.eq("admin"))
            .one(db)
            .await
            .expect("admin")
            .expect("ada")
            .id as i64
    }

    #[tokio::test]
    async fn login_admin_berhasil_dan_membawa_izin() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let ok = attempt_login(db, "admin", "Admin@123").await.expect("login");
        assert!(ok.must_change_password);
        assert!(ok.user.is_super_admin);
        assert!(!ok.user.permissions.is_empty());
    }

    #[tokio::test]
    async fn mfa_enroll_aktif_lalu_challenge() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let uid = admin_id(db).await;
        let setup = mfa_setup(db, uid).await.expect("setup");
        assert_eq!(setup.secret.len(), 32);
        assert!(setup.otpauth_url.starts_with("otpauth://totp/"));
        assert!(mfa_enable(db, uid, "000000").await.is_err());
        let now = Local::now().timestamp();
        let secret = base32_decode(&setup.secret).expect("decode");
        let code = totp_code(&secret, now);
        mfa_enable(db, uid, &code).await.expect("enable");
        let ok = attempt_login(db, "admin", "Admin@123").await.expect("login");
        assert!(ok.mfa_required);
        assert!(verify_mfa(db, uid, "000000").await.is_err());
        let user = verify_mfa(db, uid, &code).await.expect("challenge");
        assert_eq!(user.id as i64, uid);
        mfa_disable(db, uid, "salah").await.expect_err("password salah");
        mfa_disable(db, uid, "Admin@123").await.expect("disable");
        let ok = attempt_login(db, "admin", "Admin@123").await.expect("login lagi");
        assert!(!ok.mfa_required);
    }

    #[tokio::test]
    async fn login_gagal_lima_kali_mengunci() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        for _ in 0..5 {
            let e = attempt_login(db, "admin", "salah").await.expect_err("harus gagal");
            assert_eq!(e, "Username/email atau password salah.");
        }
        let e = attempt_login(db, "admin", "Admin@123")
            .await
            .expect_err("harus terkunci");
        assert!(e.contains("terkunci"), "pesan: {e}");
        let u = user::Entity::find()
            .filter(user::Column::Username.eq("admin"))
            .one(db)
            .await
            .expect("baca")
            .expect("ada");
        assert_eq!(u.failed_login_attempts, 5);
    }

    #[tokio::test]
    async fn login_user_nonaktif_dan_tak_dikenal_generik() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin = user::Entity::find()
            .filter(user::Column::Username.eq("admin"))
            .one(db)
            .await
            .expect("baca")
            .expect("ada");
        let mut am = admin.into_active_model();
        am.status = Set("inactive".to_string());
        am.update(db).await.expect("nonaktif");
        let e = attempt_login(db, "admin", "Admin@123")
            .await
            .expect_err("nonaktif ditolak");
        assert_eq!(e, "Username/email atau password salah.");
        let admin = user::Entity::find()
            .filter(user::Column::Username.eq("admin"))
            .one(db)
            .await
            .expect("baca")
            .expect("ada");
        let mut am = admin.into_active_model();
        am.status = Set("active".to_string());
        am.update(db).await.expect("aktif");
        let e = attempt_login(db, "hantu", "apapun")
            .await
            .expect_err("unknown ditolak");
        assert_eq!(e, "Username/email atau password salah.");
    }

    #[tokio::test]
    async fn ganti_password_butuh_password_lama() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let id = admin_id(db).await;
        let e = change_password(db, id, "keliru", "PasswordBaru1")
            .await
            .expect_err("harus gagal");
        assert_eq!(e, "Password saat ini tidak sesuai.");
        let e = change_password(db, id, "Admin@123", "pendek")
            .await
            .expect_err("kebijakan");
        assert!(e.contains("minimal 8"), "pesan: {e}");
        change_password(db, id, "Admin@123", "PasswordBaru1")
            .await
            .expect("ganti ok");
        let ok = attempt_login(db, "admin", "PasswordBaru1")
            .await
            .expect("login baru");
        assert!(!ok.must_change_password);
    }

    #[tokio::test]
    async fn reset_token_sekali_pakai_lalu_hangus() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let token = request_password_reset(db, "admin@hris.local")
            .await
            .expect("request")
            .expect("token ada");
        reset_password(db, &token, "ResetBaru12")
            .await
            .expect("reset ok");
        let e = reset_password(db, &token, "LainLagi12")
            .await
            .expect_err("reuse ditolak");
        assert!(e.contains("tidak valid"), "pesan: {e}");
        attempt_login(db, "admin", "ResetBaru12")
            .await
            .expect("login password reset");
    }

    #[tokio::test]
    async fn reset_token_kedaluwarsa_ditolak_dan_email_asing_senyap() {
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        assert!(request_password_reset(db, "tidak@ada.local")
            .await
            .expect("request")
            .is_none());
        let token = request_password_reset(db, "admin@hris.local")
            .await
            .expect("request")
            .expect("token");
        let admin = user::Entity::find()
            .filter(user::Column::Username.eq("admin"))
            .one(db)
            .await
            .expect("baca")
            .expect("ada");
        let mut am = admin.into_active_model();
        am.password_reset_expires_at = Set(Some("2000-01-01 00:00:00".to_string()));
        am.update(db).await.expect("kedaluwarsa");
        let e = reset_password(db, &token, "ResetBaru12")
            .await
            .expect_err("expired ditolak");
        assert!(e.contains("kedaluwarsa"), "pesan: {e}");
    }

    #[tokio::test]
    async fn logout_menulis_audit() {
        use crate::entities::audit_log;
        let dir = tempfile::tempdir().expect("dir");
        let state = init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let id = admin_id(db).await;
        logout(db, id).await.expect("logout");
        let n = audit_log::Entity::find()
            .filter(audit_log::Column::Action.eq("LOGOUT"))
            .filter(audit_log::Column::UserId.eq(id as i32))
            .all(db)
            .await
            .expect("baca")
            .len();
        assert_eq!(n, 1);
    }
}
