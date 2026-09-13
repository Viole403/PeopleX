//! Autentikasi: login lockout, reset token, ganti password, sesi.

use chrono::Local;
use rand::RngCore;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

use super::audit;
use crate::{to_dto_int, SessionUser};

const MAX_ATTEMPTS: i64 = 5;
const LOCKOUT_MINUTES: i64 = 15;
const RESET_TOKEN_BYTES: usize = 32;
const RESET_VALID_HOURS: i64 = 1;
const MIN_PASSWORD_LEN: usize = 8;

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

struct UserRow {
    id: i64,
    username: String,
    password_hash: String,
    status: String,
    failed_attempts: i64,
    locked_until: Option<String>,
    must_change_password: bool,
    mfa_enabled: bool,
}

fn find_by_login(conn: &Connection, login: &str) -> Result<Option<UserRow>, String> {
    conn.query_row(
        "SELECT id, username, password, status, failed_login_attempts, locked_until, must_change_password, COALESCE(mfa_enabled, 0) FROM users WHERE username = ?1 OR email = ?1 LIMIT 1",
        params![login],
        |r| {
            Ok(UserRow {
                id: r.get(0)?,
                username: r.get(1)?,
                password_hash: r.get(2)?,
                status: r.get(3)?,
                failed_attempts: r.get(4)?,
                locked_until: r.get(5)?,
                must_change_password: r.get::<_, i64>(6)? != 0,
                mfa_enabled: r.get::<_, i64>(7)? != 0,
            })
        },
    )
    .optional()
    .map_err(|e| format!("gagal mencari pengguna: {e}"))
}

fn record_login_activity(
    conn: &Connection,
    user_id: Option<i64>,
    attempt: &str,
    status: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO login_activities (user_id, username_attempt, ip_address, user_agent, status) VALUES (?1, ?2, 'desktop', 'peoplex', ?3)",
        params![user_id, attempt, status],
    )
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
pub fn load_session_user(conn: &Connection, user_id: i64) -> Result<Option<SessionUser>, String> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT username, status FROM users WHERE id = ?1",
            params![user_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat sesi: {e}"))?;
    let Some((username, status)) = row else {
        return Ok(None);
    };
    if status != "active" {
        return Ok(None);
    }
    let mut stmt = conn
        .prepare("SELECT slug FROM roles r JOIN user_roles ur ON ur.role_id = r.id WHERE ur.user_id = ?1")
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let roles: Vec<String> = stmt
        .query_map(params![user_id], |r| r.get(0))
        .map_err(|e| format!("gagal memuat peran: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("gagal memuat peran: {e}"))?;
    let mut stmt = conn
        .prepare("SELECT DISTINCT p.slug FROM permissions p JOIN role_permissions rp ON rp.permission_id = p.id JOIN user_roles ur ON ur.role_id = rp.role_id WHERE ur.user_id = ?1")
        .map_err(|e| format!("gagal memuat izin: {e}"))?;
    let permissions: Vec<String> = stmt
        .query_map(params![user_id], |r| r.get(0))
        .map_err(|e| format!("gagal memuat izin: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("gagal memuat izin: {e}"))?;
    Ok(Some(SessionUser {
        id: to_dto_int(user_id, "user.id")?,
        username,
        must_change_password: must_change_flag(conn, user_id)?,
        is_super_admin: roles.iter().any(|r| r == "super-administrator"),
        roles,
        permissions,
    }))
}

fn must_change_flag(conn: &Connection, user_id: i64) -> Result<bool, String> {
    let v: i64 = conn
        .query_row(
            "SELECT must_change_password FROM users WHERE id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .map_err(|e| format!("gagal memuat flag password: {e}"))?;
    Ok(v != 0)
}

fn locked_minutes_remaining(locked_until: &str, now: &str) -> Option<i64> {
    if locked_until > now {
        // format ISO terurut leksikal; hitung selisih via chrono
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
pub fn attempt_login(conn: &Connection, login: &str, password: &str) -> Result<LoginOk, String> {
    let login = login.trim();
    if login.is_empty() || password.is_empty() {
        return Err("Username/email dan password wajib diisi.".to_string());
    }
    let user = find_by_login(conn, login)?;
    let now = now_str();

    if let Some(u) = &user {
        if let Some(locked) = u.locked_until.as_deref() {
            if let Some(mins) = locked_minutes_remaining(locked, &now) {
                record_login_activity(conn, None, login, "failed")?;
                return Err(format!(
                    "Akun terkunci sementara. Coba lagi dalam {mins} menit."
                ));
            }
        }
    }

    let valid = match &user {
        Some(u) if u.status == "active" => bcrypt::verify(password, &u.password_hash)
            .map_err(|e| format!("gagal verifikasi password: {e}"))?,
        _ => false,
    };
    if !valid {
        if let Some(u) = &user {
            let attempts = u.failed_attempts + 1;
            if attempts >= MAX_ATTEMPTS {
                let locked = (Local::now() + chrono::Duration::minutes(LOCKOUT_MINUTES))
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string();
                conn.execute(
                    "UPDATE users SET failed_login_attempts = ?1, locked_until = ?2 WHERE id = ?3",
                    params![attempts, locked, u.id],
                )
                .map_err(|e| format!("gagal mencatat percobaan: {e}"))?;
            } else {
                conn.execute(
                    "UPDATE users SET failed_login_attempts = ?1 WHERE id = ?2",
                    params![attempts, u.id],
                )
                .map_err(|e| format!("gagal mencatat percobaan: {e}"))?;
            }
        }
        record_login_activity(conn, user.as_ref().map(|u| u.id), login, "failed")?;
        return Err("Username/email atau password salah.".to_string());
    }

    let u = user.expect("user valid");
    conn.execute(
        "UPDATE users SET failed_login_attempts = 0, locked_until = NULL, last_login_at = ?1, last_login_ip = 'desktop' WHERE id = ?2",
        params![now, u.id],
    )
    .map_err(|e| format!("gagal memperbarui login: {e}"))?;
    record_login_activity(conn, Some(u.id), login, "success")?;
    audit::log(
        conn,
        Some(u.id),
        "LOGIN",
        "auth",
        Some(&u.id.to_string()),
        None,
        None,
        Some(&format!("User {} logged in", u.username)),
    )?;
    let session = load_session_user(conn, u.id)?.expect("sesi user aktif");
    Ok(LoginOk {
        must_change_password: u.must_change_password,
        mfa_required: u.mfa_enabled,
        user: session,
    })
}

/// Catat logout + audit.
pub fn logout(conn: &Connection, user_id: i64) -> Result<(), String> {
    audit::log(
        conn,
        Some(user_id),
        "LOGOUT",
        "auth",
        Some(&user_id.to_string()),
        None,
        None,
        None,
    )?;
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
pub fn mfa_setup(conn: &Connection, user_id: i64) -> Result<MfaSetup, String> {
    let username: Option<String> = conn
        .query_row(
            "SELECT username FROM users WHERE id = ?1 AND status = 'active'",
            params![user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat akun: {e}"))?
        .flatten();
    let Some(name) = username else {
        return Err("Akun tidak ditemukan.".to_string());
    };
    let mut bytes = [0u8; MFA_SECRET_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    let secret = base32_encode(&bytes);
    conn.execute(
        "UPDATE users SET mfa_secret = ?1, mfa_enabled = 0 WHERE id = ?2",
        params![secret, user_id],
    )
    .map_err(|e| format!("gagal menyimpan secret MFA: {e}"))?;
    Ok(MfaSetup {
        otpauth_url: format!(
            "otpauth://totp/PeopleX:{name}?secret={secret}&issuer=PeopleX&algorithm=SHA256&digits=6&period=30"
        ),
        secret,
    })
}

/// Aktifkan MFA setelah kode pertama terbukti benar.
pub fn mfa_enable(conn: &Connection, user_id: i64, code: &str) -> Result<(), String> {
    let secret: Option<String> = conn
        .query_row(
            "SELECT mfa_secret FROM users WHERE id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat MFA: {e}"))?
        .flatten();
    let Some(sec) = secret else {
        return Err("MFA belum disiapkan.".to_string());
    };
    let now = Local::now().timestamp();
    if !totp_valid(&sec, code, now)? {
        return Err("Kode MFA salah.".to_string());
    }
    conn.execute(
        "UPDATE users SET mfa_enabled = 1 WHERE id = ?1",
        params![user_id],
    )
    .map_err(|e| format!("gagal mengaktifkan MFA: {e}"))?;
    audit::log(
        conn,
        Some(user_id),
        "UPDATE",
        "mfa",
        Some(&user_id.to_string()),
        None,
        None,
        Some("MFA diaktifkan"),
    )?;
    Ok(())
}

/// Matikan MFA dengan verifikasi password.
pub fn mfa_disable(conn: &Connection, user_id: i64, password: &str) -> Result<(), String> {
    let hash: Option<String> = conn
        .query_row(
            "SELECT password FROM users WHERE id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat akun: {e}"))?
        .flatten();
    let Some(h) = hash else {
        return Err("Akun tidak ditemukan.".to_string());
    };
    if !bcrypt::verify(password, &h).map_err(|e| format!("gagal verifikasi password: {e}"))? {
        return Err("Password salah.".to_string());
    }
    conn.execute(
        "UPDATE users SET mfa_secret = NULL, mfa_enabled = 0 WHERE id = ?1",
        params![user_id],
    )
    .map_err(|e| format!("gagal mematikan MFA: {e}"))?;
    audit::log(
        conn,
        Some(user_id),
        "UPDATE",
        "mfa",
        Some(&user_id.to_string()),
        None,
        None,
        Some("MFA dimatikan"),
    )?;
    Ok(())
}

/// Verifikasi kode tahap kedua dan kembalikan profil sesi.
pub fn verify_mfa(conn: &Connection, user_id: i64, code: &str) -> Result<SessionUser, String> {
    let row: Option<(Option<String>, i64)> = conn
        .query_row(
            "SELECT mfa_secret, mfa_enabled FROM users WHERE id = ?1 AND status = 'active'",
            params![user_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat MFA: {e}"))?;
    let Some((Some(sec), enabled)) = row else {
        return Err("MFA tidak aktif untuk akun ini.".to_string());
    };
    if enabled == 0 {
        return Err("MFA tidak aktif untuk akun ini.".to_string());
    }
    let now = Local::now().timestamp();
    if !totp_valid(&sec, code, now)? {
        record_login_activity(conn, Some(user_id), "", "failed")?;
        return Err("Kode MFA salah.".to_string());
    }
    record_login_activity(conn, Some(user_id), "", "success")?;
    load_session_user(conn, user_id)?.ok_or("Sesi berakhir. Masuk kembali.".to_string())
}

/// Minta reset: simpan hash token + kedaluwarsa 1 jam.
/// Kembalikan token plaintext bila email aktif dikenal, None bila tidak
/// (pemanggil menampilkan pesan generik yang sama agar anti-enumerasi).
/// Tanpa SMTP: token diserahkan ke admin/HR untuk diteruskan ke user.
pub fn request_password_reset(conn: &Connection, email: &str) -> Result<Option<String>, String> {
    let email = email.trim();
    let user_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE email = ?1 AND status = 'active'",
            params![email],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal mencari email: {e}"))?;
    let Some(uid) = user_id else {
        return Ok(None);
    };
    let token = random_token();
    let expires = (Local::now() + chrono::Duration::hours(RESET_VALID_HOURS))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string();
    conn.execute(
        "UPDATE users SET password_reset_token = ?1, password_reset_expires_at = ?2 WHERE id = ?3",
        params![sha256_hex(&token), expires, uid],
    )
    .map_err(|e| format!("gagal menyimpan token reset: {e}"))?;
    audit::log(
        conn,
        Some(uid),
        "PASSWORD_RESET_REQUEST",
        "auth",
        Some(&uid.to_string()),
        None,
        None,
        None,
    )?;
    Ok(Some(token))
}

/// Terapkan password baru via token (sekali pakai, 1 jam).
pub fn reset_password(conn: &Connection, token: &str, new_password: &str) -> Result<(), String> {
    check_password_policy(new_password)?;
    let now = now_str();
    let user_id: Option<i64> = conn
        .query_row(
            "SELECT id FROM users WHERE password_reset_token = ?1 AND password_reset_expires_at > ?2",
            params![sha256_hex(token.trim()), now],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memvalidasi token: {e}"))?;
    let Some(uid) = user_id else {
        return Err("Tautan reset tidak valid atau sudah kedaluwarsa.".to_string());
    };
    let hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password: {e}"))?;
    conn.execute(
        "UPDATE users SET password = ?1, password_reset_token = NULL, password_reset_expires_at = NULL, must_change_password = 0, failed_login_attempts = 0, locked_until = NULL WHERE id = ?2",
        params![hash, uid],
    )
    .map_err(|e| format!("gagal menyimpan password baru: {e}"))?;
    audit::log(
        conn,
        Some(uid),
        "PASSWORD_RESET",
        "auth",
        Some(&uid.to_string()),
        None,
        None,
        None,
    )?;
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
pub fn change_password(
    conn: &Connection,
    user_id: i64,
    current_password: &str,
    new_password: &str,
) -> Result<(), String> {
    let hash: Option<String> = conn
        .query_row(
            "SELECT password FROM users WHERE id = ?1",
            params![user_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| format!("gagal memuat pengguna: {e}"))?;
    let Some(hash) = hash else {
        return Err("Pengguna tidak ditemukan.".to_string());
    };
    if !bcrypt::verify(current_password, &hash)
        .map_err(|e| format!("gagal verifikasi password: {e}"))?
    {
        return Err("Password saat ini tidak sesuai.".to_string());
    }
    check_password_policy(new_password)?;
    let new_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("gagal hash password: {e}"))?;
    conn.execute(
        "UPDATE users SET password = ?1, must_change_password = 0 WHERE id = ?2",
        params![new_hash, user_id],
    )
    .map_err(|e| format!("gagal menyimpan password: {e}"))?;
    audit::log(
        conn,
        Some(user_id),
        "PASSWORD_CHANGE",
        "auth",
        Some(&user_id.to_string()),
        None,
        None,
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

    fn admin_id(conn: &Connection) -> i64 {
        conn.query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
            r.get(0)
        })
        .expect("admin")
    }

    #[test]
    fn login_admin_berhasil_dan_membawa_izin() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let ok = attempt_login(&conn, "admin", "Admin@123").expect("login");
        assert!(ok.must_change_password);
        assert!(ok.user.is_super_admin);
        assert!(!ok.user.permissions.is_empty());
    }

    #[test]
    fn mfa_enroll_aktif_lalu_challenge() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let uid = admin_id(&conn);
        let setup = mfa_setup(&conn, uid).expect("setup");
        assert_eq!(setup.secret.len(), 32);
        assert!(setup.otpauth_url.starts_with("otpauth://totp/"));
        assert!(mfa_enable(&conn, uid, "000000").is_err());
        let now = Local::now().timestamp();
        let secret = base32_decode(&setup.secret).expect("decode");
        let code = totp_code(&secret, now);
        mfa_enable(&conn, uid, &code).expect("enable");
        let ok = attempt_login(&conn, "admin", "Admin@123").expect("login");
        assert!(ok.mfa_required);
        assert!(verify_mfa(&conn, uid, "000000").is_err());
        let user = verify_mfa(&conn, uid, &code).expect("challenge");
        assert_eq!(user.id as i64, uid);
        mfa_disable(&conn, uid, "salah").expect_err("password salah");
        mfa_disable(&conn, uid, "Admin@123").expect("disable");
        let ok = attempt_login(&conn, "admin", "Admin@123").expect("login lagi");
        assert!(!ok.mfa_required);
    }

    #[test]
    fn login_gagal_lima_kali_mengunci() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        for _ in 0..5 {
            let e = attempt_login(&conn, "admin", "salah").expect_err("harus gagal");
            assert_eq!(e, "Username/email atau password salah.");
        }
        let e = attempt_login(&conn, "admin", "Admin@123").expect_err("harus terkunci");
        assert!(e.contains("terkunci"), "pesan: {e}");
        let attempts: i64 = conn
            .query_row(
                "SELECT failed_login_attempts FROM users WHERE username = 'admin'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(attempts, 5);
    }

    #[test]
    fn login_user_nonaktif_dan_tak_dikenal_generik() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        conn.execute(
            "UPDATE users SET status = 'inactive' WHERE username = 'admin'",
            [],
        )
        .unwrap();
        let e = attempt_login(&conn, "admin", "Admin@123").expect_err("nonaktif ditolak");
        assert_eq!(e, "Username/email atau password salah.");
        conn.execute(
            "UPDATE users SET status = 'active' WHERE username = 'admin'",
            [],
        )
        .unwrap();
        let e = attempt_login(&conn, "hantu", "apapun").expect_err("unknown ditolak");
        assert_eq!(e, "Username/email atau password salah.");
    }

    #[test]
    fn ganti_password_butuh_password_lama() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let id = admin_id(&conn);
        let e = change_password(&conn, id, "keliru", "PasswordBaru1").expect_err("harus gagal");
        assert_eq!(e, "Password saat ini tidak sesuai.");
        let e = change_password(&conn, id, "Admin@123", "pendek").expect_err("kebijakan");
        assert!(e.contains("minimal 8"), "pesan: {e}");
        change_password(&conn, id, "Admin@123", "PasswordBaru1").expect("ganti ok");
        let ok = attempt_login(&conn, "admin", "PasswordBaru1").expect("login baru");
        assert!(!ok.must_change_password);
    }

    #[test]
    fn reset_token_sekali_pakai_lalu_hangus() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let token = request_password_reset(&conn, "admin@hris.local")
            .expect("request")
            .expect("token ada");
        reset_password(&conn, &token, "ResetBaru12").expect("reset ok");
        let e = reset_password(&conn, &token, "LainLagi12").expect_err("reuse ditolak");
        assert!(e.contains("tidak valid"), "pesan: {e}");
        attempt_login(&conn, "admin", "ResetBaru12").expect("login password reset");
    }

    #[test]
    fn reset_token_kedaluwarsa_ditolak_dan_email_asing_senyap() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        assert!(request_password_reset(&conn, "tidak@ada.local")
            .expect("request")
            .is_none());
        let token = request_password_reset(&conn, "admin@hris.local")
            .expect("request")
            .expect("token");
        conn.execute(
            "UPDATE users SET password_reset_expires_at = '2000-01-01 00:00:00' WHERE username = 'admin'",
            [],
        )
        .unwrap();
        let e = reset_password(&conn, &token, "ResetBaru12").expect_err("expired ditolak");
        assert!(e.contains("kedaluwarsa"), "pesan: {e}");
    }

    #[test]
    fn logout_menulis_audit() {
        let (_dir, pool) = live();
        let conn = pool.get().expect("get");
        let id = admin_id(&conn);
        logout(&conn, id).expect("logout");
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM audit_logs WHERE action = 'LOGOUT' AND user_id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
