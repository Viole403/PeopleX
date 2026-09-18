//! Adapter Fingerspot/EasyLink: tarik scanlog dan dorong user via SDK HTTP lokal.
//!
//! EasyLink (Fingerspot) bukan protokol device langsung: device bicara ke
//! program SDK di PC/server lokal (contoh `http://localhost:7005`), dan
//! PeopleX bicara ke SDK itu lewat HTTP POST dengan parameter di query
//! string (`?sn=...`), header `application/x-www-form-urlencoded`, respons
//! JSON `{Result, ...}`.
//!
//! Konvensi kolom `fingerprint_devices` untuk protokol `easylink`:
//! - `serial` = SN mesin (parameter `sn` SDK)
//! - `endpoint` = base URL SDK (default `http://localhost:7005`)
//! - `protocol` wajib `easylink`
//!
//! Referensi: dewadg/easylink-js (`src/EasyLink.ts`),
//! ariefrahmansyah/fingerplus (`scanlog.go`, `device.go`).

use super::fingerprint::{cmd_ack_sea, cmd_pending_sea, ingest_sea};
use super::sea_raw::{q_one, value_i64, value_to_string, Value};
use crate::to_dto_int;

/// Base URL SDK EasyLink bila kolom endpoint device kosong.
pub const EASYLINK_DEFAULT_BASE: &str = "http://localhost:7005";

/// Satu baris scanlog EasyLink: `{SN, ScanDate, PIN, VerifyMode, IOMode, WorkCode}`.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EasyLinkScanRow {
    pub pin: String,
    pub scan_date: String,
    pub verify_mode: i32,
    pub io_mode: i32,
    pub work_code: i32,
}

/// Ringkasan DEVINFO EasyLink: `{Jam, Admin, User, FP, CARD, PWD,
/// 'All Operasional', 'All Presensi', 'New Operasional', 'New Presensi'}`.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct EasyLinkDeviceInfo {
    pub jam: String,
    pub admin: i32,
    pub user: i32,
    pub fp: i32,
    pub card: i32,
    pub pwd: i32,
    pub all_operasional: i32,
    pub all_presensi: i32,
    pub new_operasional: i32,
    pub new_presensi: i32,
}

fn el_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs.clamp(2, 60)))
        .build()
        .map_err(|e| format!("gagal membuat klien HTTP: {e}"))
}

/// Base URL SDK dari kolom endpoint device, atau default lokal.
pub fn el_base(endpoint: Option<&str>) -> String {
    match endpoint.map(str::trim).filter(|s| !s.is_empty()) {
        Some(b) => b.trim_end_matches('/').to_string(),
        None => EASYLINK_DEFAULT_BASE.to_string(),
    }
}

fn el_sn(serial: Option<&str>) -> Result<String, String> {
    serial
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| "Serial (SN) perangkat wajib diisi untuk EasyLink.".to_string())
}

/// POST ke SDK EasyLink: parameter di query string, respons JSON.
async fn el_post(
    client: &reqwest::Client,
    base: &str,
    path: &str,
    sn: &str,
    params: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let mut url = format!("{base}{path}?sn={sn}");
    for (k, v) in params {
        url.push_str(&format!("&{k}={v}"));
    }
    let text = client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(String::new())
        .send()
        .await
        .map_err(|e| format!("gagal menghubungi SDK EasyLink: {e}"))?
        .text()
        .await
        .map_err(|e| format!("gagal membaca respons SDK EasyLink: {e}"))?;
    serde_json::from_str(&text)
        .map_err(|_| format!("respons SDK bukan JSON: {}", text.chars().take(120).collect::<String>()))
}

/// Muat device easylink aktif: kembalikan (id, base, sn).
async fn el_device(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
) -> Result<(i64, String, String), String> {
    let row = q_one(
        db,
        "SELECT id, serial, endpoint, protocol FROM fingerprint_devices WHERE id = ?1 AND deleted_at IS NULL AND is_active = 1".to_string(),
        vec![Value::Int(device_id)],
        4,
        "fp.el.dev",
    )
    .await?
    .ok_or_else(|| "Perangkat tidak aktif.".to_string())?;
    if value_to_string(&row[3]) != "easylink" {
        return Err("Perangkat bukan protokol easylink.".to_string());
    }
    let sn = el_sn(match &row[1] {
        Value::Text(s) => Some(s.as_str()),
        _ => None,
    })?;
    let base = el_base(match &row[2] {
        Value::Text(s) => Some(s.as_str()),
        _ => None,
    });
    Ok((value_i64(&row[0]).unwrap_or(0), base, sn))
}

fn el_i64(v: &serde_json::Value) -> i64 {
    match v {
        serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
        serde_json::Value::String(s) => s.trim().parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn el_str(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

/// Parse array `Data` respons scanlog menjadi baris terstruktur.
pub fn parse_scanlog_rows(body: &serde_json::Value) -> Vec<EasyLinkScanRow> {
    let mut out = Vec::new();
    let arr = match body.get("Data").and_then(|d| d.as_array()) {
        Some(a) => a,
        None => return out,
    };
    for r in arr {
        let pin = r.get("PIN").map(el_str).unwrap_or_default();
        if pin.trim().is_empty() {
            continue;
        }
        out.push(EasyLinkScanRow {
            pin,
            scan_date: r.get("ScanDate").map(el_str).unwrap_or_default(),
            verify_mode: r.get("VerifyMode").map(el_i64).unwrap_or(0) as i32,
            io_mode: r.get("IOMode").map(el_i64).unwrap_or(0) as i32,
            work_code: r.get("WorkCode").map(el_i64).unwrap_or(0) as i32,
        });
    }
    out
}

/// Samakan format waktu scanlog ke `YYYY-MM-DD HH:MM:SS` untuk antrean.
pub fn normalize_scan_time(raw: &str) -> Result<String, String> {
    let s = raw.trim();
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y/%m/%d %H:%M:%S", "%d-%m-%Y %H:%M:%S"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(dt.format("%Y-%m-%d %H:%M:%S").to_string());
        }
    }
    Err("Waktu scan tidak valid (diharapkan YYYY-MM-DD HH:MM:SS).".to_string())
}

/// Label mode verifikasi EasyLink (angka mentah SDK, tanpa klaim berlebih).
pub fn easylink_verify_label(n: i32) -> String {
    format!("easylink:{n}")
}

/// Baca info device dari SDK (`POST /dev/info`).
pub async fn easylink_info_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
) -> Result<EasyLinkDeviceInfo, String> {
    let (_, base, sn) = el_device(db, device_id).await?;
    let client = el_client(10)?;
    let body = el_post(&client, &base, "/dev/info", &sn, &[]).await?;
    let g = |k: &str| el_i64(body.get("DEVINFO").and_then(|d| d.get(k)).unwrap_or(&serde_json::Value::Null)) as i32;
    Ok(EasyLinkDeviceInfo {
        jam: body.get("DEVINFO").and_then(|d| d.get("Jam")).map(el_str).unwrap_or_default(),
        admin: g("Admin"),
        user: g("User"),
        fp: g("FP"),
        card: g("CARD"),
        pwd: g("PWD"),
        all_operasional: g("All Operasional"),
        all_presensi: g("All Presensi"),
        new_operasional: g("New Operasional"),
        new_presensi: g("New Presensi"),
    })
}

/// Tarik scanlog dari SDK lalu masukkan ke antrean (`ingest_sea`).
/// `only_new` = pakai `/scanlog/new`, bila salah pakai `/scanlog/all`.
/// Mengembalikan jumlah baris yang masuk antrean.
pub async fn easylink_pull_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    only_new: bool,
) -> Result<i32, String> {
    let (id, base, sn) = el_device(db, device_id).await?;
    let client = el_client(30)?;
    let path = if only_new { "/scanlog/new" } else { "/scanlog/all" };
    let body = el_post(&client, &base, path, &sn, &[]).await?;
    let rows = parse_scanlog_rows(&body);
    let mut masuk = 0i64;
    for r in &rows {
        let waktu = match normalize_scan_time(&r.scan_date) {
            Ok(w) => w,
            Err(_) => continue,
        };
        if ingest_sea(db, id, &r.pin, &waktu, Some(&easylink_verify_label(r.verify_mode))).await.is_ok() {
            masuk += 1;
        }
    }
    super::audit::log_sea(db, Some(actor_id), "PULL", "attendance.fingerprint.easylink", Some(&id.to_string()), None, None, Some(&format!("{masuk} scanlog masuk antrean."))).await?;
    to_dto_int(masuk, "fp.el.masuk")
}

/// Dorong satu user ke mesin via SDK (`POST /user/set`).
#[allow(clippy::too_many_arguments)]
pub async fn easylink_push_user_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    pin: &str,
    name: &str,
    card: Option<&str>,
) -> Result<(), String> {
    let (id, base, sn) = el_device(db, device_id).await?;
    let p = pin.trim();
    if p.is_empty() || p.len() > 32 {
        return Err("PIN wajib diisi.".to_string());
    }
    let nama = name.trim();
    if nama.is_empty() {
        return Err("Nama wajib diisi.".to_string());
    }
    let client = el_client(30)?;
    let kartu = card.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    el_post(&client, &base, "/user/set", &sn, &[("pin", p), ("nama", nama), ("pwd", ""), ("rfid", kartu), ("priv", "0"), ("tmp", "[]")]).await?;
    super::audit::log_sea(db, Some(actor_id), "PUSH", "attendance.fingerprint.easylink", Some(&id.to_string()), None, None, Some(&format!("user {p} didorong ke mesin."))).await?;
    Ok(())
}

/// Hapus satu user di mesin via SDK (`POST /user/del`).
pub async fn easylink_delete_user_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    pin: &str,
) -> Result<(), String> {
    let (id, base, sn) = el_device(db, device_id).await?;
    let p = pin.trim();
    if p.is_empty() {
        return Err("PIN wajib diisi.".to_string());
    }
    let client = el_client(30)?;
    el_post(&client, &base, "/user/del", &sn, &[("pin", p)]).await?;
    super::audit::log_sea(db, Some(actor_id), "DELETE", "attendance.fingerprint.easylink", Some(&id.to_string()), None, None, Some(&format!("user {p} dihapus dari mesin."))).await?;
    Ok(())
}

/// Nama karyawan untuk PIN enroll dari payload outbox.
async fn nama_karyawan(db: &sea_orm::DatabaseConnection, employee_id: i64) -> String {
    q_one(
        db,
        "SELECT first_name || ' ' || COALESCE(last_name, '') FROM employees WHERE id = ?1".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "fp.el.nama",
    )
    .await
    .ok()
    .flatten()
    .map(|r| value_to_string(&r[0]).trim().to_string())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| format!("Karyawan {employee_id}"))
}

/// Kartu RFID karyawan dari kredensial sentral, bila ada.
async fn kartu_karyawan(db: &sea_orm::DatabaseConnection, employee_id: i64) -> Option<String> {
    q_one(
        db,
        "SELECT value FROM fp_credentials WHERE employee_id = ?1 AND cred_type = 'card'".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "fp.el.kartu",
    )
    .await
    .ok()
    .flatten()
    .map(|r| value_to_string(&r[0]))
    .filter(|s| !s.trim().is_empty())
}

/// Konsumsi outbox `enroll_pin`/`delete_pin` untuk device easylink:
/// jalankan ke SDK lalu `cmd_ack_sea`. Perintah op lain dilewati (tetap pending).
/// Mengembalikan jumlah perintah yang dieksekusi.
pub async fn easylink_sync_outbox_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    limit: i64,
) -> Result<i32, String> {
    let (id, _, _) = el_device(db, device_id).await?;
    let cmds = cmd_pending_sea(db, id, limit).await?;
    let mut jalan = 0i64;
    for c in &cmds {
        let hasil: Result<(), String> = match c.op.as_str() {
            "enroll_pin" => {
                let body: serde_json::Value = serde_json::from_str(&c.payload).unwrap_or(serde_json::Value::Null);
                let eid = body.get("employee_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let pin = body.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if eid <= 0 || pin.trim().is_empty() {
                    Err("Payload enroll tidak valid.".to_string())
                } else {
                    let nama = nama_karyawan(db, eid).await;
                    let kartu = kartu_karyawan(db, eid).await;
                    easylink_push_user_inner(db, actor_id, id, &pin, &nama, kartu.as_deref()).await
                }
            }
            "delete_pin" => {
                let body: serde_json::Value = serde_json::from_str(&c.payload).unwrap_or(serde_json::Value::Null);
                let pin = body.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if pin.trim().is_empty() {
                    Err("Payload hapus tidak valid.".to_string())
                } else {
                    easylink_delete_user_inner(db, actor_id, id, &pin).await
                }
            }
            _ => continue,
        };
        match hasil {
            Ok(()) => {
                cmd_ack_sea(db, id, c.id as i64, true, None).await?;
                jalan += 1;
            }
            Err(e) => {
                cmd_ack_sea(db, id, c.id as i64, false, Some(&e)).await?;
            }
        }
    }
    to_dto_int(jalan, "fp.el.sync")
}

async fn easylink_push_user_inner(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    pin: &str,
    name: &str,
    card: Option<&str>,
) -> Result<(), String> {
    let (id, base, sn) = el_device(db, device_id).await?;
    let client = el_client(30)?;
    let kartu = card.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("");
    el_post(&client, &base, "/user/set", &sn, &[("pin", pin.trim()), ("nama", name.trim()), ("pwd", ""), ("rfid", kartu), ("priv", "0"), ("tmp", "[]")]).await?;
    super::audit::log_sea(db, Some(actor_id), "PUSH", "attendance.fingerprint.easylink", Some(&id.to_string()), None, None, Some(&format!("user {} didorong ke mesin.", pin.trim()))).await?;
    Ok(())
}

async fn easylink_delete_user_inner(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    pin: &str,
) -> Result<(), String> {
    let (id, base, sn) = el_device(db, device_id).await?;
    let client = el_client(30)?;
    el_post(&client, &base, "/user/del", &sn, &[("pin", pin.trim())]).await?;
    super::audit::log_sea(db, Some(actor_id), "DELETE", "attendance.fingerprint.easylink", Some(&id.to_string()), None, None, Some(&format!("user {} dihapus dari mesin.", pin.trim()))).await?;
    Ok(())
}

/// Samakan jam mesin dengan server (`POST /dev/settime`).
pub async fn easylink_settime_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
) -> Result<(), String> {
    let (id, base, sn) = el_device(db, device_id).await?;
    let client = el_client(15)?;
    el_post(&client, &base, "/dev/settime", &sn, &[]).await?;
    super::audit::log_sea(db, Some(actor_id), "SYNC", "attendance.fingerprint.easylink", Some(&id.to_string()), None, None, Some("Jam mesin disamakan.")).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_base_default_dan_kustom() {
        assert_eq!(el_base(None), "http://localhost:7005");
        assert_eq!(el_base(Some("  ")), "http://localhost:7005");
        assert_eq!(el_base(Some("http://192.168.1.50:7005/")), "http://192.168.1.50:7005");
    }

    #[test]
    fn el_sn_wajib_ada() {
        assert!(el_sn(None).is_err());
        assert!(el_sn(Some(" ")).is_err());
        assert_eq!(el_sn(Some("  SN123 ")).expect("sn"), "SN123");
    }

    #[test]
    fn parse_scanlog_data_array() {
        let body: serde_json::Value = serde_json::from_str(
            r#"{"Result":true,"Data":[
                {"SN":"SN1","ScanDate":"2026-09-20 08:01:02","PIN":"1001","VerifyMode":1,"IOMode":0,"WorkCode":0},
                {"SN":"SN1","ScanDate":"2026-09-20 17:02:03","PIN":"1002","VerifyMode":2,"IOMode":1,"WorkCode":0},
                {"SN":"SN1","ScanDate":"2026-09-20 17:02:03","PIN":"   ","VerifyMode":1,"IOMode":1,"WorkCode":0}
            ]}"#,
        )
        .expect("json");
        let rows = parse_scanlog_rows(&body);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pin, "1001");
        assert_eq!(rows[0].verify_mode, 1);
        assert_eq!(rows[1].pin, "1002");
        assert_eq!(rows[1].work_code, 0);
    }

    #[test]
    fn parse_scanlog_tanpa_data_kosong() {
        let body: serde_json::Value = serde_json::from_str(r#"{"Result":false}"#).expect("json");
        assert!(parse_scanlog_rows(&body).is_empty());
    }

    #[test]
    fn normalize_waktu_tiga_format() {
        assert_eq!(normalize_scan_time("2026-09-20 08:01:02").expect("w"), "2026-09-20 08:01:02");
        assert_eq!(normalize_scan_time("2026/09/20 08:01:02").expect("w"), "2026-09-20 08:01:02");
        assert_eq!(normalize_scan_time("20-09-2026 08:01:02").expect("w"), "2026-09-20 08:01:02");
        assert!(normalize_scan_time("kemarin sore").is_err());
    }

    #[test]
    fn label_verify_mentah() {
        assert_eq!(easylink_verify_label(1), "easylink:1");
        assert_eq!(easylink_verify_label(15), "easylink:15");
    }

    #[tokio::test]
    async fn host_mati_mengembalikan_galat() {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let n = super::super::fingerprint::device_save_sea(
            db,
            1,
            None,
            &super::super::fingerprint::FingerDeviceInput {
                name: "EL Tes".to_string(),
                brand: "Fingerspot".to_string(),
                model: None,
                serial: Some("SN-TES".to_string()),
                protocol: "easylink".to_string(),
                driver: None,
                endpoint: Some("http://127.0.0.1:9".to_string()),
                location_id: None,
                active: true,
            },
        )
        .await
        .expect("device");
        let e = easylink_info_sea(db, n as i64).await.expect_err("mati");
        assert!(e.contains("gagal menghubungi SDK EasyLink"));
    }
}
