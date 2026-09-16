//! Mesin fingerprint: registrasi perangkat, antrean log kehadiran, pemetaan PIN.
//!
//! Alur: mesin (ADMS/ZK-pull/USB/agent) kirim event mentah ke antrean
//! `fingerprint_logs`. `process_queue_sea` memetakan PIN ke karyawan lalu
//! mencatat clock-in/out ke `attendances`, menandai baris antrean diproses.
//! Perintah baca pakai izin `attendance.view`, aksi tulis pakai
//! `attendance.approve`.

use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};
use crate::to_dto_int;

const PROTOCOLS: &[&str] = &["adms", "zk_pull", "usb", "cloud", "agent"];

/// Merek umum untuk pemilih di FE; protokol bebas tetap diizinkan.
pub const BRANDS: &[&str] = &[
    "ZKTeco", "Solution", "Fingerspot", "Revo", "Deli", "Krisbow", "eSSL",
    "Hikvision", "Dahua", "Anviz", "Suprema", "Matrix", "Nitgen", "Lainnya",
];

/// Driver default per protokol bila kolom driver kosong.
pub fn default_driver(protocol: &str) -> &'static str {
    match protocol {
        "adms" | "zk_pull" | "usb" => "universal",
        "cloud" => "cloud-webhook",
        _ => "agent",
    }
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct FingerDevice {
    pub id: i32,
    pub name: String,
    pub brand: String,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub protocol: String,
    pub driver: String,
    pub endpoint: Option<String>,
    pub location_id: Option<i32>,
    pub active: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct FingerDeviceInput {
    pub name: String,
    pub brand: String,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub protocol: String,
    pub driver: Option<String>,
    pub endpoint: Option<String>,
    pub location_id: Option<i32>,
    pub active: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct FingerEnroll {
    pub id: i32,
    pub device_id: i32,
    pub employee_id: i32,
    pub device_pin: String,
    pub employee_name: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct FingerLog {
    pub id: i32,
    pub device_id: i32,
    pub device_name: String,
    pub device_pin: String,
    pub event_time: String,
    pub verify_mode: Option<String>,
    pub employee_id: Option<i32>,
    pub processed: bool,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DeviceProbe {
    pub host: String,
    pub zk_4370_open: bool,
    pub adms_hint: String,
}

fn opt_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.clone()),
        _ => None,
    }
}

fn opt_dto(v: &Value, f: &str) -> Result<Option<i32>, String> {
    match v {
        Value::Null => Ok(None),
        _ => Ok(Some(to_dto_int(value_i64(v).unwrap_or(0), f)?)),
    }
}

fn map_device(r: &[Value]) -> Result<FingerDevice, String> {
    let protocol = value_to_string(&r[5]);
    let driver = opt_text(&r[9]).unwrap_or_else(|| default_driver(&protocol).to_string());
    Ok(FingerDevice {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fp.dev.id")?,
        name: value_to_string(&r[1]),
        brand: value_to_string(&r[2]),
        model: opt_text(&r[3]),
        serial: opt_text(&r[4]),
        protocol: protocol.clone(),
        driver,
        endpoint: opt_text(&r[6]),
        location_id: opt_dto(&r[7], "fp.dev.loc")?,
        active: value_i64(&r[8]).unwrap_or(0) != 0,
    })
}

pub async fn device_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<FingerDevice>, String> {
    let rows = q_all(
        db,
        "SELECT id, name, brand, model, serial, protocol, endpoint, location_id, is_active, driver FROM fingerprint_devices WHERE deleted_at IS NULL ORDER BY id".to_string(),
        vec![],
        10,
        "fp.dev.list",
    )
    .await?;
    rows.iter().map(|r| map_device(r)).collect()
}

pub async fn device_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &FingerDeviceInput,
) -> Result<i32, String> {
    let name = input.name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err("Nama perangkat wajib diisi sampai 120 karakter.".to_string());
    }
    let brand = input.brand.trim();
    if brand.is_empty() || brand.len() > 60 {
        return Err("Merek perangkat wajib diisi sampai 60 karakter.".to_string());
    }
    if !PROTOCOLS.contains(&input.protocol.as_str()) {
        return Err("Protokol tidak dikenal.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let loc = input.location_id.map(|v| Value::Int(v as i64)).unwrap_or(Value::Null);
    let driver = input.driver.clone().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| default_driver(&input.protocol).to_string());
    let new_id = match id {
        Some(x) => {
            let n = exec(
                db,
                "UPDATE fingerprint_devices SET name = ?1, brand = ?2, model = ?3, serial = ?4, protocol = ?5, driver = ?6, endpoint = ?7, location_id = ?8, is_active = ?9, updated_at = ?10 WHERE id = ?11 AND deleted_at IS NULL".to_string(),
                vec![
                    Value::Text(name.to_string()),
                    Value::Text(brand.to_string()),
                    input.model.clone().map(|s| Value::Text(s)).unwrap_or(Value::Null),
                    input.serial.clone().map(|s| Value::Text(s)).unwrap_or(Value::Null),
                    Value::Text(input.protocol.clone()),
                    Value::Text(driver.clone()),
                    input.endpoint.clone().map(|s| Value::Text(s)).unwrap_or(Value::Null),
                    loc,
                    Value::Int(if input.active { 1 } else { 0 }),
                    Value::Text(now),
                    Value::Int(x),
                ],
                "fp.dev.upd",
            )
            .await?;
            if n == 0 {
                return Err("Perangkat tidak ditemukan.".to_string());
            }
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO fingerprint_devices (name, brand, model, serial, protocol, driver, endpoint, location_id, is_active, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)".to_string(),
                vec![
                    Value::Text(name.to_string()),
                    Value::Text(brand.to_string()),
                    input.model.clone().map(|s| Value::Text(s)).unwrap_or(Value::Null),
                    input.serial.clone().map(|s| Value::Text(s)).unwrap_or(Value::Null),
                    Value::Text(input.protocol.clone()),
                    Value::Text(driver.clone()),
                    input.endpoint.clone().map(|s| Value::Text(s)).unwrap_or(Value::Null),
                    loc,
                    Value::Int(if input.active { 1 } else { 0 }),
                    Value::Text(now.clone()),
                    Value::Text(now),
                ],
                "fp.dev.ins",
            )
            .await?
        }
    };
    super::audit::log_sea(
        db,
        Some(actor_id),
        if id.is_some() { "UPDATE" } else { "CREATE" },
        "attendance.fingerprint.device",
        Some(&new_id.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(new_id, "fp.dev.id")
}

pub async fn device_delete_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let n = exec(
        db,
        "UPDATE fingerprint_devices SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL".to_string(),
        vec![Value::Text(now), Value::Int(id)],
        "fp.dev.del",
    )
    .await?;
    if n == 0 {
        return Err("Perangkat tidak ditemukan.".to_string());
    }
    super::audit::log_sea(db, Some(actor_id), "DELETE", "attendance.fingerprint.device", Some(&id.to_string()), None, None, None).await?;
    Ok(())
}

pub async fn enroll_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    employee_id: i64,
    device_pin: &str,
    department_id: Option<i64>,
) -> Result<i32, String> {
    let pin = device_pin.trim();
    if pin.is_empty() || pin.len() > 32 {
        return Err("PIN perangkat wajib diisi sampai 32 karakter.".to_string());
    }
    for (tbl, label) in [("fingerprint_devices", "Perangkat"), ("employees", "Karyawan")] {
        let row = q_one(
            db,
            format!("SELECT id FROM {tbl} WHERE id = ?1 AND deleted_at IS NULL"),
            vec![Value::Int(if tbl == "fingerprint_devices" { device_id } else { employee_id })],
            1,
            "fp.enroll.cek",
        )
        .await?;
        if row.is_none() {
            return Err(format!("{label} tidak ditemukan."));
        }
    }
    if let Some(did) = department_id {
        let dep = q_one(
            db,
            "SELECT department_id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
            vec![Value::Int(employee_id)],
            1,
            "fp.enroll.scope",
        )
        .await?;
        if dep.and_then(|r| value_i64(&r[0])) != Some(did) {
            return Err("Karyawan di luar departemen Anda.".to_string());
        }
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rid = exec_insert(
        db,
        "INSERT INTO fingerprint_enrollments (device_id, employee_id, device_pin, created_at) VALUES (?1, ?2, ?3, ?4)".to_string(),
        vec![Value::Int(device_id), Value::Int(employee_id), Value::Text(pin.to_string()), Value::Text(now)],
        "fp.enroll.ins",
    )
    .await
    .map_err(|e| {
        if e.contains("UNIQUE") {
            "PIN sudah terdaftar di perangkat ini.".to_string()
        } else {
            e
        }
    })?;
    super::audit::log_sea(db, Some(actor_id), "CREATE", "attendance.fingerprint.enroll", Some(&rid.to_string()), None, None, None).await?;
    to_dto_int(rid, "fp.enroll.id")
}

pub async fn enroll_list_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    department_id: Option<i64>,
) -> Result<Vec<FingerEnroll>, String> {
    let mut sql = "SELECT e.id, e.device_id, e.employee_id, e.device_pin, TRIM(emp.first_name || ' ' || COALESCE(emp.last_name, '')) FROM fingerprint_enrollments e JOIN employees emp ON emp.id = e.employee_id WHERE e.device_id = ?1".to_string();
    let mut vals = vec![Value::Int(device_id)];
    if let Some(did) = department_id {
        sql.push_str(" AND emp.department_id = ?2");
        vals.push(Value::Int(did));
    }
    sql.push_str(" ORDER BY e.id");
    let rows = q_all(db, sql, vals, 5, "fp.enroll.list").await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(FingerEnroll {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fp.enroll.id")?,
            device_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "fp.enroll.dev")?,
            employee_id: to_dto_int(value_i64(&r[2]).unwrap_or(0), "fp.enroll.emp")?,
            device_pin: value_to_string(&r[3]),
            employee_name: value_to_string(&r[4]),
        });
    }
    Ok(out)
}

/// Terima satu event mentah dari mesin/agent/USB ke antrean.
pub async fn ingest_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    device_pin: &str,
    event_time: &str,
    verify_mode: Option<&str>,
) -> Result<i32, String> {
    let pin = device_pin.trim();
    if pin.is_empty() || pin.len() > 32 {
        return Err("PIN perangkat wajib diisi.".to_string());
    }
    chrono::NaiveDateTime::parse_from_str(event_time.trim(), "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "Waktu event harus valid (YYYY-MM-DD HH:MM:SS).".to_string())?;
    let dev = q_one(
        db,
        "SELECT id FROM fingerprint_devices WHERE id = ?1 AND deleted_at IS NULL AND is_active = 1".to_string(),
        vec![Value::Int(device_id)],
        1,
        "fp.ing.dev",
    )
    .await?;
    if dev.is_none() {
        return Err("Perangkat tidak aktif.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rid = exec_insert(
        db,
        "INSERT INTO fingerprint_logs (device_id, device_pin, event_time, verify_mode, processed, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5)".to_string(),
        vec![
            Value::Int(device_id),
            Value::Text(pin.to_string()),
            Value::Text(event_time.trim().to_string()),
            verify_mode.map(|s| Value::Text(s.to_string())).unwrap_or(Value::Null),
            Value::Text(now),
        ],
        "fp.ing.ins",
    )
    .await?;
    to_dto_int(rid, "fp.ing.id")
}

fn map_log(r: &[Value], dev_name: &str) -> Result<FingerLog, String> {
    Ok(FingerLog {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fp.log.id")?,
        device_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "fp.log.dev")?,
        device_name: dev_name.to_string(),
        device_pin: value_to_string(&r[2]),
        event_time: value_to_string(&r[3]),
        verify_mode: opt_text(&r[4]),
        employee_id: opt_dto(&r[6], "fp.log.emp")?,
        processed: value_i64(&r[5]).unwrap_or(0) != 0,
    })
}

pub async fn log_list_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: Option<i64>,
    only_pending: bool,
    limit: i64,
    department_id: Option<i64>,
) -> Result<Vec<FingerLog>, String> {
    let lim = limit.clamp(1, 500);
    let mut sql = "SELECT l.id, l.device_id, l.device_pin, l.event_time, l.verify_mode, l.processed, l.employee_id, d.name FROM fingerprint_logs l JOIN fingerprint_devices d ON d.id = l.device_id LEFT JOIN employees emp ON emp.id = l.employee_id WHERE 1 = 1".to_string();
    let mut vals = Vec::new();
    let mut n = 0u32;
    if let Some(d) = device_id {
        n += 1;
        sql.push_str(&format!(" AND l.device_id = ?{n}"));
        vals.push(Value::Int(d));
    }
    if only_pending {
        sql.push_str(" AND l.processed = 0");
    }
    if let Some(did) = department_id {
        n += 1;
        sql.push_str(&format!(" AND (l.employee_id IS NULL OR emp.department_id = ?{n})"));
        vals.push(Value::Int(did));
    }
    n += 1;
    sql.push_str(&format!(" ORDER BY l.id DESC LIMIT ?{n}"));
    vals.push(Value::Int(lim));
    let rows = q_all(db, sql, vals, 8, "fp.log.list").await?;
    rows.iter().map(|r| map_log(r, &value_to_string(&r[7]))).collect()
}

/// Probing pasif: cek port ZK 4370 terbuka + petunjuk endpoint ADMS.
pub async fn probe_sea(
    db: &sea_orm::DatabaseConnection,
    host: &str,
    timeout_ms: u64,
) -> Result<DeviceProbe, String> {
    use std::net::{SocketAddr, ToSocketAddrs};
    use std::time::Duration;
    let h = host.trim();
    if h.is_empty() || h.len() > 253 {
        return Err("Host tidak valid.".to_string());
    }
    let addr: SocketAddr = format!("{h}:4370")
        .to_socket_addrs()
        .map_err(|_| "Host tidak dapat diurai.".to_string())?
        .next()
        .ok_or_else(|| "Host tidak dapat diurai.".to_string())?;
    let ms = timeout_ms.clamp(200, 5000);
    let open = std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(ms)).is_ok();
    let _ = db;
    Ok(DeviceProbe {
        host: h.to_string(),
        zk_4370_open: open,
        adms_hint: "Arahkan ADMS mesin ke http://<host-peoplex>:<port>/iclock/cdata bila didukung.".to_string(),
    })
}

/// Proses antrean: petakan PIN ke karyawan, catat clock-in/out, tandai diproses.
/// Mengembalikan jumlah baris antrean yang diproses.
pub async fn process_queue_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: Option<i64>,
    limit: i64,
    department_id: Option<i64>,
) -> Result<i32, String> {
    let lim = limit.clamp(1, 500);
    let mut sql = "SELECT l.id, l.device_id, l.device_pin, l.event_time FROM fingerprint_logs l WHERE l.processed = 0".to_string();
    let mut vals: Vec<Value> = Vec::new();
    let mut n = 0u32;
    if let Some(d) = device_id {
        n += 1;
        sql.push_str(&format!(" AND l.device_id = ?{n}"));
        vals.push(Value::Int(d));
    }
    n += 1;
    sql.push_str(&format!(" ORDER BY l.id ASC LIMIT ?{n}"));
    vals.push(Value::Int(lim));
    let rows = q_all(db, sql, vals, 4, "fp.q.pick").await?;
    let mut done = 0i64;
    for r in rows {
        let lid = value_i64(&r[0]).unwrap_or(0);
        let dev = value_i64(&r[1]).unwrap_or(0);
        let pin = value_to_string(&r[2]);
        let when = value_to_string(&r[3]);
        let emp = q_one(
            db,
            "SELECT employee_id FROM fingerprint_enrollments WHERE device_id = ?1 AND device_pin = ?2".to_string(),
            vec![Value::Int(dev), Value::Text(pin.clone())],
            1,
            "fp.q.map",
        )
        .await?
        .and_then(|x| value_i64(&x[0]));
        let Some(eid) = emp else {
            exec(
                db,
                "UPDATE fingerprint_logs SET processed = 2 WHERE id = ?1".to_string(),
                vec![Value::Int(lid)],
                "fp.q.skip",
            )
            .await?;
            done += 1;
            continue;
        };
        if let Some(did) = department_id {
            let dep = q_one(
                db,
                "SELECT department_id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
                vec![Value::Int(eid)],
                1,
                "fp.q.scope",
            )
            .await?;
            if dep.and_then(|r| value_i64(&r[0])) != Some(did) {
                continue;
            }
        };
        let day = when.get(..10).unwrap_or("").to_string();
        let ada = q_one(
            db,
            "SELECT id, clock_out FROM attendances WHERE employee_id = ?1 AND date = ?2".to_string(),
            vec![Value::Int(eid), Value::Text(day.clone())],
            2,
            "fp.q.cek",
        )
        .await?;
        match ada {
            None => {
                exec(
                    db,
                    "INSERT INTO attendances (employee_id, date, clock_in, status, late_minutes, early_minutes, work_minutes) VALUES (?1, ?2, ?3, 'present', 0, 0, 0)".to_string(),
                    vec![Value::Int(eid), Value::Text(day), Value::Text(when.clone())],
                    "fp.q.in",
                )
                .await?;
            }
            Some(a) if opt_text(&a[1]).is_none() => {
                let aid = value_i64(&a[0]).unwrap_or(0);
                exec(
                    db,
                    "UPDATE attendances SET clock_out = ?1 WHERE id = ?2".to_string(),
                    vec![Value::Text(when.clone()), Value::Int(aid)],
                    "fp.q.out",
                )
                .await?;
            }
            _ => {}
        }
        exec(
            db,
            "UPDATE fingerprint_logs SET processed = 1, employee_id = ?1 WHERE id = ?2".to_string(),
            vec![Value::Int(eid), Value::Int(lid)],
            "fp.q.done",
        )
        .await?;
        done += 1;
    }
    super::audit::log_sea(db, Some(actor_id), "PROCESS", "attendance.fingerprint.queue", None, None, None, Some(&format!("{done} baris diproses"))).await?;
    to_dto_int(done, "fp.q.done")
}

/// Parser baris CSV/USB generik: kembalikan (pin, event_time, verify_mode).
/// Format: `pin,waktu[,mode]`; waktu `YYYY-MM-DD HH:MM:SS`.
pub fn parse_usb_row(line: &str) -> Result<(String, String, Option<String>), String> {
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err("Baris harus berisi pin,waktu[,mode].".to_string());
    }
    chrono::NaiveDateTime::parse_from_str(parts[1], "%Y-%m-%d %H:%M:%S")
        .map_err(|_| "Waktu baris tidak valid (YYYY-MM-DD HH:MM:SS).".to_string())?;
    let mode = parts.get(2).filter(|s| !s.is_empty()).map(|s| s.to_string());
    Ok((parts[0].to_string(), parts[1].to_string(), mode))
}

/// Parser payload ADMS cdata: baris `PIN<TAB>waktu<TAB>...`; kolom mode opsional.
pub fn parse_adms_rows(body: &str) -> Vec<(String, String, Option<String>)> {
    let mut out = Vec::new();
    for line in body.lines() {
        let cols: Vec<&str> = line.split('\t').map(str::trim).collect();
        if cols.len() < 2 || cols[0].is_empty() || cols[1].is_empty() {
            continue;
        }
        if chrono::NaiveDateTime::parse_from_str(cols[1], "%Y-%m-%d %H:%M:%S").is_err() {
            continue;
        }
        let mode = cols.get(3).filter(|s| !s.is_empty()).map(|s| s.to_string());
        out.push((cols[0].to_string(), cols[1].to_string(), mode));
    }
    out
}

const CRED_TYPES: &[&str] = &["pin", "card"];
const CMD_OPS: &[&str] = &["enroll_pin", "enroll_card", "delete_pin", "delete_card", "sync_time", "reboot", "clear_logs"];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct FpCredential {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub cred_type: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DeviceCommand {
    pub id: i32,
    pub device_id: i32,
    pub op: String,
    pub payload: String,
    pub status: String,
    pub attempts: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DeviceRosterRow {
    pub device_id: i32,
    pub device_name: String,
    pub employee_id: i32,
    pub employee_name: String,
    pub cred_type: String,
    pub status: String,
}

fn cred_row(r: &[Value]) -> Result<FpCredential, String> {
    Ok(FpCredential {
        id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fp.cred.id")?,
        employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "fp.cred.emp")?,
        employee_name: value_to_string(&r[2]),
        cred_type: value_to_string(&r[3]),
    })
}

/// Kredensial sentral: satu PIN/kartu per karyawan, didorong ke semua device aktif.
pub async fn cred_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    cred_type: &str,
    cred_value: &str,
) -> Result<i32, String> {
    let kind = cred_type.trim();
    if !CRED_TYPES.contains(&kind) {
        return Err("Tipe kredensial harus pin atau card.".to_string());
    }
    let val = cred_value.trim();
    if val.is_empty() || val.len() > 64 {
        return Err("Nilai kredensial wajib diisi sampai 64 karakter.".to_string());
    }
    let emp = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(employee_id)],
        1,
        "fp.cred.emp",
    )
    .await?;
    if emp.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let ada = q_one(
        db,
        "SELECT id FROM fp_credentials WHERE employee_id = ?1 AND cred_type = ?2".to_string(),
        vec![Value::Int(employee_id), Value::Text(kind.to_string())],
        1,
        "fp.cred.cek",
    )
    .await?;
    let rid = match ada {
        Some(r) => {
            let x = value_i64(&r[0]).unwrap_or(0);
            exec(
                db,
                "UPDATE fp_credentials SET cred_value = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
                vec![Value::Text(val.to_string()), Value::Text(now.clone()), Value::Int(x)],
                "fp.cred.upd",
            )
            .await?;
            super::audit::log_sea(db, Some(actor_id), "UPDATE", "attendance.fingerprint.cred", Some(&x.to_string()), None, None, None).await?;
            x
        }
        None => {
            let x = exec_insert(
                db,
                "INSERT INTO fp_credentials (employee_id, cred_type, cred_value, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
                vec![Value::Int(employee_id), Value::Text(kind.to_string()), Value::Text(val.to_string()), Value::Text(now.clone()), Value::Text(now.clone())],
                "fp.cred.ins",
            )
            .await?;
            super::audit::log_sea(db, Some(actor_id), "CREATE", "attendance.fingerprint.cred", Some(&x.to_string()), None, None, None).await?;
            x
        }
    };
    let op = if kind == "pin" { "enroll_pin" } else { "enroll_card" };
    let devs = q_all(
        db,
        "SELECT id FROM fingerprint_devices WHERE deleted_at IS NULL AND is_active = 1".to_string(),
        vec![],
        1,
        "fp.cred.devs",
    )
    .await?;
    for d in devs {
        let did = value_i64(&d[0]).unwrap_or(0);
        exec(
            db,
            format!("INSERT INTO device_commands (device_id, op, payload, status, created_at, updated_at) VALUES (?1, '{op}', ?2, 'pending', ?3, ?3)"),
            vec![
                Value::Int(did),
                Value::Text(format!("{{\"employee_id\":{employee_id},\"value\":\"{val}\"}}")),
                Value::Text(now.clone()),
            ],
            "fp.cred.push",
        )
        .await?;
        exec(
            db,
            "INSERT OR IGNORE INTO device_roster (device_id, employee_id, cred_type, status) VALUES (?1, ?2, ?3, 'pending')".to_string(),
            vec![Value::Int(did), Value::Int(employee_id), Value::Text(kind.to_string())],
            "fp.cred.roster",
        )
        .await?;
    }
    to_dto_int(rid, "fp.cred.id")
}

pub async fn cred_list_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: Option<i64>,
) -> Result<Vec<FpCredential>, String> {
    let mut sql = "SELECT c.id, c.employee_id, TRIM(e.first_name || ' ' || COALESCE(e.last_name, '')), c.cred_type FROM fp_credentials c JOIN employees e ON e.id = c.employee_id WHERE 1 = 1".to_string();
    let mut vals = Vec::new();
    if let Some(e) = employee_id {
        sql.push_str(" AND c.employee_id = ?1");
        vals.push(Value::Int(e));
    }
    sql.push_str(" ORDER BY c.id");
    let rows = q_all(db, sql, vals, 4, "fp.cred.list").await?;
    rows.iter().map(|r| cred_row(r)).collect()
}

/// Antrekan perintah keluar manual (sync_time/reboot/clear_logs/hapus).
pub async fn cmd_enqueue_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    op: &str,
    payload_json: &str,
) -> Result<i32, String> {
    let o = op.trim();
    if !CMD_OPS.contains(&o) {
        return Err("Operasi perintah tidak dikenal.".to_string());
    }
    let p = payload_json.trim();
    if p.len() > 2000 {
        return Err("Payload maksimal 2000 karakter.".to_string());
    }
    let dev = q_one(
        db,
        "SELECT id FROM fingerprint_devices WHERE id = ?1 AND deleted_at IS NULL AND is_active = 1".to_string(),
        vec![Value::Int(device_id)],
        1,
        "fp.cmd.dev",
    )
    .await?;
    if dev.is_none() {
        return Err("Perangkat tidak aktif.".to_string());
    }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rid = exec_insert(
        db,
        "INSERT INTO device_commands (device_id, op, payload, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'pending', ?4, ?4)".to_string(),
        vec![Value::Int(device_id), Value::Text(o.to_string()), Value::Text(p.to_string()), Value::Text(now)],
        "fp.cmd.ins",
    )
    .await?;
    super::audit::log_sea(db, Some(actor_id), "CREATE", "attendance.fingerprint.cmd", Some(&rid.to_string()), None, None, None).await?;
    to_dto_int(rid, "fp.cmd.id")
}

/// Tarikan agent: perintah pending per device.
pub async fn cmd_pending_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    limit: i64,
) -> Result<Vec<DeviceCommand>, String> {
    let lim = limit.clamp(1, 100);
    let rows = q_all(
        db,
        "SELECT id, device_id, op, payload, status, attempts FROM device_commands WHERE device_id = ?1 AND status = 'pending' ORDER BY id ASC LIMIT ?2".to_string(),
        vec![Value::Int(device_id), Value::Int(lim)],
        6,
        "fp.cmd.pend",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(DeviceCommand {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fp.cmd.id")?,
            device_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "fp.cmd.dev")?,
            op: value_to_string(&r[2]),
            payload: value_to_string(&r[3]),
            status: value_to_string(&r[4]),
            attempts: value_i64(&r[5]).unwrap_or(0) as i32,
        });
    }
    Ok(out)
}

/// Konfirmasi agent: tandai acked/failed; enroll yang acked menandai roster synced.
pub async fn cmd_ack_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    cmd_id: i64,
    ok: bool,
    error: Option<&str>,
) -> Result<(), String> {
    let row = q_one(
        db,
        "SELECT op, payload FROM device_commands WHERE id = ?1 AND device_id = ?2 AND status = 'pending'".to_string(),
        vec![Value::Int(cmd_id), Value::Int(device_id)],
        2,
        "fp.cmd.get",
    )
    .await?
    .ok_or_else(|| "Perintah pending tidak ditemukan.".to_string())?;
    let op = value_to_string(&row[0]);
    let payload = value_to_string(&row[1]);
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let status = if ok { "acked" } else { "failed" };
    exec(
        db,
        "UPDATE device_commands SET status = ?1, attempts = attempts + 1, last_error = ?2, updated_at = ?3 WHERE id = ?4".to_string(),
        vec![
            Value::Text(status.to_string()),
            error.filter(|s| !s.trim().is_empty()).map(|s| Value::Text(s.to_string())).unwrap_or(Value::Null),
            Value::Text(now.clone()),
            Value::Int(cmd_id),
        ],
        "fp.cmd.ack",
    )
    .await?;
    if ok && (op == "enroll_pin" || op == "enroll_card") {
        let kind = if op == "enroll_pin" { "pin" } else { "card" };
        if let Some(eid) = employee_of_payload(&payload) {
            exec(
                db,
                "UPDATE device_roster SET status = 'synced', pushed_at = ?1, updated_at = ?1 WHERE device_id = ?2 AND employee_id = ?3 AND cred_type = ?4".to_string(),
                vec![Value::Text(now), Value::Int(device_id), Value::Int(eid), Value::Text(kind.to_string())],
                "fp.cmd.roster",
            )
            .await?;
        }
    }
    Ok(())
}

fn employee_of_payload(payload: &str) -> Option<i64> {
    let key = "\"employee_id\":";
    payload.find(key).and_then(|i| {
        payload[i + key.len()..]
            .trim_start()
            .split(|c: char| !c.is_ascii_digit())
            .next()
            .and_then(|n| n.parse::<i64>().ok())
    })
}

/// Cermin status pendaftaran per device.
pub async fn roster_sea(
    db: &sea_orm::DatabaseConnection,
    device_id: Option<i64>,
) -> Result<Vec<DeviceRosterRow>, String> {
    let mut sql = "SELECT r.device_id, d.name, r.employee_id, TRIM(e.first_name || ' ' || COALESCE(e.last_name, '')), r.cred_type, r.status FROM device_roster r JOIN fingerprint_devices d ON d.id = r.device_id JOIN employees e ON e.id = r.employee_id WHERE 1 = 1".to_string();
    let mut vals = Vec::new();
    if let Some(d) = device_id {
        sql.push_str(" AND r.device_id = ?1");
        vals.push(Value::Int(d));
    }
    sql.push_str(" ORDER BY r.device_id, r.employee_id");
    let rows = q_all(db, sql, vals, 6, "fp.roster.list").await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(DeviceRosterRow {
            device_id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "fp.ros.dev")?,
            device_name: value_to_string(&r[1]),
            employee_id: to_dto_int(value_i64(&r[2]).unwrap_or(0), "fp.ros.emp")?,
            employee_name: value_to_string(&r[3]),
            cred_type: value_to_string(&r[4]),
            status: value_to_string(&r[5]),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sea_raw::{exec, q_one};

    async fn state() -> (tempfile::TempDir, sea_orm::DatabaseConnection) {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = app.sea.clone();
        (dir, db)
    }

    async fn actor(db: &sea_orm::DatabaseConnection) -> i64 {
        q_one(db, "SELECT id FROM users WHERE username = 'admin'".to_string(), vec![], 1, "t.admin")
            .await.expect("admin").expect("ada")
            .first().and_then(value_i64).expect("id")
    }

    async fn emp(db: &sea_orm::DatabaseConnection) -> i64 {
        exec(db, "INSERT INTO employees (employee_number, first_name, company_id, join_date, employment_status, employment_type) VALUES ('EMP-FP1', 'Fp', 1, '2026-01-05', 'active', 'permanent')".to_string(), vec![], "t.emp")
            .await.expect("emp");
        q_one(db, "SELECT last_insert_rowid()".to_string(), vec![], 1, "t.rid")
            .await.expect("rid").expect("ada")
            .first().and_then(value_i64).expect("id")
    }

    fn dev_input() -> FingerDeviceInput {
        FingerDeviceInput {
            name: "Lobi Utama".to_string(),
            brand: "ZKTeco".to_string(),
            model: Some("M1".to_string()),
            serial: Some("SN001".to_string()),
            protocol: "adms".to_string(),
            driver: None,
            endpoint: None,
            location_id: None,
            active: true,
        }
    }

    #[tokio::test]
    async fn device_crud_berjalan() {
        let (_d, db) = state().await;
        let a = actor(&db).await;
        assert!(device_save_sea(&db, a, None, &FingerDeviceInput { protocol: "x".to_string(), ..dev_input() }).await.is_err());
        let id = device_save_sea(&db, a, None, &dev_input()).await.expect("simpan") as i64;
        assert_eq!(device_list_sea(&db).await.expect("list").len(), 1);
        device_save_sea(&db, a, Some(id), &FingerDeviceInput { name: "Lobi 2".to_string(), ..dev_input() }).await.expect("ubah");
        device_delete_sea(&db, a, id).await.expect("hapus");
        assert!(device_list_sea(&db).await.expect("list2").is_empty());
    }

    #[tokio::test]
    async fn enroll_ingest_process_clock() {
        let (_d, db) = state().await;
        let a = actor(&db).await;
        let e = emp(&db).await;
        let d = device_save_sea(&db, a, None, &dev_input()).await.expect("dev") as i64;
        enroll_save_sea(&db, a, d, e, "  7 ", None).await.expect("enroll");
        assert!(enroll_save_sea(&db, a, d, e, "7", None).await.is_err());
        ingest_sea(&db, d, "7", "2026-09-10 08:01:00", Some("fp")).await.expect("in");
        ingest_sea(&db, d, "7", "2026-09-10 17:02:00", None).await.expect("out");
        assert_eq!(process_queue_sea(&db, a, Some(d), 100, None).await.expect("proses"), 2);
        let row = q_one(&db, "SELECT clock_in, clock_out FROM attendances WHERE employee_id = ?1 AND date = '2026-09-10'".to_string(), vec![Value::Int(e)], 2, "t.att").await.expect("att").expect("ada");
        assert_eq!(value_to_string(&row[0]), "2026-09-10 08:01:00");
        assert_eq!(value_to_string(&row[1]), "2026-09-10 17:02:00");
        assert_eq!(process_queue_sea(&db, a, Some(d), 100, None).await.expect("proses2"), 0);
        assert_eq!(log_list_sea(&db, Some(d), false, 10, None).await.expect("logs").len(), 2);
    }

    #[tokio::test]
    async fn ingest_tolak_dan_skip_tanpa_enroll() {
        let (_d, db) = state().await;
        let a = actor(&db).await;
        let d = device_save_sea(&db, a, None, &dev_input()).await.expect("dev") as i64;
        assert!(ingest_sea(&db, d, "9", "waktu-salah", None).await.is_err());
        assert!(ingest_sea(&db, 999999, "9", "2026-09-10 08:00:00", None).await.is_err());
        ingest_sea(&db, d, "99", "2026-09-10 08:00:00", None).await.expect("antre");
        assert_eq!(process_queue_sea(&db, a, Some(d), 100, None).await.expect("proses"), 1);
        let logs = log_list_sea(&db, Some(d), false, 10, None).await.expect("logs");
        assert_eq!(logs.len(), 1);
        assert!(logs[0].processed);
    }

    #[tokio::test]
    async fn scoping_dept_filter_dan_tolak_lintas_dept() {
        let (_d, db) = state().await;
        let a = actor(&db).await;
        let d = device_save_sea(&db, a, None, &dev_input()).await.expect("dev") as i64;
        let hr = q_one(&db, "SELECT id FROM departments WHERE code = 'HRD'".to_string(), vec![], 1, "t.hrd")
            .await.expect("hrd").expect("ada");
        let hr = value_i64(&hr[0]).expect("id");
        let it = q_one(&db, "SELECT id FROM departments WHERE code = 'IT'".to_string(), vec![], 1, "t.it")
            .await.expect("it").expect("ada");
        let it = value_i64(&it[0]).expect("id");
        exec(&db, format!("UPDATE employees SET department_id = {hr} WHERE employee_number = 'EMP-0001'"), vec![], "t.dep")
            .await.expect("dep");
        exec(&db, format!("INSERT INTO employees (employee_number, first_name, company_id, department_id, join_date, employment_status, employment_type) VALUES ('EMP-FP2', 'Fp2', 1, {it}, '2026-01-05', 'active', 'permanent')").to_string(), vec![], "t.emp2")
            .await.expect("emp2");
        let e2 = q_one(&db, "SELECT last_insert_rowid()".to_string(), vec![], 1, "t.rid2")
            .await.expect("rid2").expect("ada");
        let e2 = value_i64(&e2[0]).expect("id2");
        let e1 = q_one(&db, "SELECT id FROM employees WHERE employee_number = 'EMP-0001'".to_string(), vec![], 1, "t.e1")
            .await.expect("e1").expect("ada");
        let e1 = value_i64(&e1[0]).expect("id1");
        enroll_save_sea(&db, a, d, e1, "11", None).await.expect("enroll1");
        enroll_save_sea(&db, a, d, e2, "22", None).await.expect("enroll2");
        assert_eq!(enroll_list_sea(&db, d, Some(hr)).await.expect("l1").len(), 1);
        assert_eq!(enroll_list_sea(&db, d, Some(it)).await.expect("l2").len(), 1);
        assert_eq!(enroll_list_sea(&db, d, None).await.expect("l3").len(), 2);
        let e = enroll_save_sea(&db, a, d, e2, "33", Some(hr)).await.expect_err("tolak");
        assert!(e.contains("luar departemen"));
        ingest_sea(&db, d, "11", "2026-09-11 08:00:00", None).await.expect("in1");
        ingest_sea(&db, d, "22", "2026-09-11 08:01:00", None).await.expect("in2");
        assert_eq!(process_queue_sea(&db, a, Some(d), 100, Some(hr)).await.expect("p1"), 1);
        assert_eq!(process_queue_sea(&db, a, Some(d), 100, Some(it)).await.expect("p2"), 1);
        assert_eq!(process_queue_sea(&db, a, Some(d), 100, None).await.expect("p3"), 0);
        assert_eq!(log_list_sea(&db, Some(d), false, 10, Some(hr)).await.expect("g1").len(), 1);
        assert_eq!(log_list_sea(&db, Some(d), false, 10, None).await.expect("g2").len(), 2);
    }

    #[tokio::test]
    async fn parser_usb_dan_adms() {
        let (p, w, m) = parse_usb_row("7,2026-09-10 08:01:00,fp").expect("usb");
        assert_eq!((p.as_str(), w.as_str(), m.as_deref()), ("7", "2026-09-10 08:01:00", Some("fp")));
        assert!(parse_usb_row("salah").is_err());
        let rows = parse_adms_rows("7\t2026-09-10 08:01:00\t1\t15\n8\t2026-09-10 08:02:00\n\njelek");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].2.as_deref(), Some("15"));
        assert!(rows[1].2.is_none());
    }

    #[tokio::test]
    async fn kredensial_dorong_outbox_dan_ack_sinkron_roster() {
        let (_d, db) = state().await;
        let a = actor(&db).await;
        let e = emp(&db).await;
        let d = device_save_sea(&db, a, None, &dev_input()).await.expect("dev") as i64;
        let rid = cred_save_sea(&db, a, e, "pin", " 1234 ").await.expect("cred");
        assert!(rid > 0);
        assert!(cred_save_sea(&db, a, e, "sidik", "x").await.is_err());
        let creds = cred_list_sea(&db, Some(e)).await.expect("list");
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].cred_type, "pin");
        let pend = cmd_pending_sea(&db, d, 10).await.expect("pend");
        assert_eq!(pend.len(), 1);
        assert_eq!(pend[0].op, "enroll_pin");
        let ros = roster_sea(&db, Some(d)).await.expect("roster");
        assert_eq!(ros.len(), 1);
        assert_eq!(ros[0].status, "pending");
        let cid = pend[0].id as i64;
        cmd_ack_sea(&db, d, cid, true, None).await.expect("ack");
        assert!(cmd_pending_sea(&db, d, 10).await.expect("pend2").is_empty());
        let ros2 = roster_sea(&db, Some(d)).await.expect("roster2");
        assert_eq!(ros2[0].status, "synced");
    }

    #[tokio::test]
    async fn perintah_manual_dan_ack_gagal() {
        let (_d, db) = state().await;
        let a = actor(&db).await;
        let d = device_save_sea(&db, a, None, &dev_input()).await.expect("dev") as i64;
        assert!(cmd_enqueue_sea(&db, a, d, "hapus-semua", "{}").await.is_err());
        let cid = cmd_enqueue_sea(&db, a, d, "sync_time", "{}").await.expect("cmd") as i64;
        cmd_ack_sea(&db, d, cid, false, Some("jam sibuk")).await.expect("nack");
        let row = q_one(&db, "SELECT status, attempts FROM device_commands WHERE id = ?1".to_string(), vec![Value::Int(cid)], 2, "t.cmd")
            .await.expect("cmd").expect("ada");
        assert_eq!(value_to_string(&row[0]), "failed");
        assert_eq!(value_i64(&row[1]), Some(1));
        assert!(cmd_ack_sea(&db, d, cid, true, None).await.is_err());
    }
}
