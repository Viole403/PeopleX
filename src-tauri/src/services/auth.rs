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
}

fn find_by_login(conn: &Connection, login: &str) -> Result<Option<UserRow>, String> {
    conn.query_row(
        "SELECT id, username, password, status, failed_login_attempts, locked_until, must_change_password FROM users WHERE username = ?1 OR email = ?1 LIMIT 1",
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
