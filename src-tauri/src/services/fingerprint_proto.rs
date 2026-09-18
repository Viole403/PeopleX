//! Protokol Grup A (T2): ZK-pull biner port 4370 + server ADMS/iClock.
//!
//! Clean-room dari spec publik (bukan vendor/GPL): framing TCP
//! `50 50 82 7D` + len u32 LE + payload (cmd/checksum/session/reply u16 LE
//! + data), checksum gaya node-zklib, record ATTLOG 40-byte, dan endpoint
//! ADMS `/iclock/cdata|getrequest|devicecmd` berbalas teks `OK`.
//! Tarikan log masuk antrean via `ingest_sea`; perintah keluar dibaca dari
//! `cmd_pending_sea` dan ack via `cmd_ack_sea`.

use super::audit;
use super::fingerprint::{cmd_ack_sea, cmd_pending_sea, ingest_sea, verify_mode_label};
use super::sea_raw::{q_one, value_i64, value_to_string, Value};


pub const ZK_TCP_MAGIC: [u8; 4] = [0x50, 0x50, 0x82, 0x7D];
pub const CMD_ATTLOG_RRQ: u16 = 13;
pub const CMD_CLEAR_ATTLOG: u16 = 15;
pub const CMD_EXIT: u16 = 1001;
pub const CMD_ENABLEDEVICE: u16 = 1002;
pub const CMD_DISABLEDEVICE: u16 = 1003;
pub const CMD_CONNECT: u16 = 1000;
pub const CMD_ACK_OK: u16 = 2000;
pub const CMD_ACK_ERROR: u16 = 2001;
pub const CMD_ACK_DATA: u16 = 2002;
pub const ZK_DEFAULT_PORT: u16 = 4370;

/// Satu tap hasil tarikan ZK.
#[derive(Clone, Debug, PartialEq)]
pub struct ZkTap {
    pub pin: String,
    pub time: String,
    pub verify: u8,
    pub status: u8,
}


/// Checksum gaya node-zklib: jumlah u16 LE per 2 byte (ganjil tambah 0),
/// lalu `0xFFFF - (sum % 0xFFFF) - 1`.
pub fn zk_checksum(payload: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i < payload.len() {
        let lo = payload[i] as u32;
        let hi = if i + 1 < payload.len() {
            payload[i + 1] as u32
        } else {
            0
        };
        sum += lo | (hi << 8);
        i += 2;
    }
    sum %= 0xFFFF;
    (0xFFFF - sum - 1) as u16
}

fn put_u16_le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Bangun paket TCP lengkap: magic + len + payload ter-checksum.
/// Field checksum dihitung dengan slot checksum = 0.
pub fn zk_build(cmd: u16, session: u16, reply: u16, data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + data.len());
    put_u16_le(&mut payload, cmd);
    put_u16_le(&mut payload, 0);
    put_u16_le(&mut payload, session);
    put_u16_le(&mut payload, reply);
    payload.extend_from_slice(data);
    let chk = zk_checksum(&payload);
    payload[2] = (chk & 0xFF) as u8;
    payload[3] = (chk >> 8) as u8;
    let mut pkt = Vec::with_capacity(8 + payload.len());
    pkt.extend_from_slice(&ZK_TCP_MAGIC);
    pkt.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    pkt.extend_from_slice(&payload);
    pkt
}

fn get_u16_le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

/// Validasi paket TCP: magic + panjang + checksum (slot dinolkan dulu).
pub fn zk_validate(pkt: &[u8]) -> Result<(u16, u16, u16, Vec<u8>), String> {
    if pkt.len() < 16 {
        return Err("Paket ZK terlalu pendek.".to_string());
    }
    if pkt[0..4] != ZK_TCP_MAGIC {
        return Err("Magic ZK tidak cocok.".to_string());
    }
    let len = u32::from_le_bytes([pkt[4], pkt[5], pkt[6], pkt[7]]) as usize;
    if pkt.len() < 8 + len {
        return Err("Paket ZK terpotong.".to_string());
    }
    let mut payload = pkt[8..8 + len].to_vec();
    let want = get_u16_le(&payload[2..4]);
    payload[2] = 0;
    payload[3] = 0;
    if zk_checksum(&payload) != want {
        return Err("Checksum ZK tidak valid.".to_string());
    }
    let cmd = get_u16_le(&payload[0..2]);
    let session = get_u16_le(&payload[4..6]);
    let reply = get_u16_le(&payload[6..8]);
    Ok((cmd, session, reply, payload[8..].to_vec()))
}


/// Waktu ZK (detik sejak 2000-01-01) ke `YYYY-MM-DD HH:MM:SS`.
pub fn zk_time_to_string(secs: u32) -> String {
    let base = chrono::NaiveDate::from_ymd_opt(2000, 1, 1)
        .expect("tanggal dasar")
        .and_hms_opt(0, 0, 0)
        .expect("jam dasar");
    (base + chrono::Duration::seconds(secs as i64))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

/// Parse satu record ATTLOG 40-byte: uid u16 @0, PIN ascii 9B @2,
/// verify u8 @26, waktu u32 @27, status u8 @31.
pub fn zk_parse_record(rec: &[u8]) -> Result<ZkTap, String> {
    if rec.len() < 40 {
        return Err("Record ATTLOG kurang dari 40 byte.".to_string());
    }
    let pin_raw = &rec[2..11];
    let end = pin_raw.iter().position(|&b| b == 0).unwrap_or(9);
    let pin = String::from_utf8_lossy(&pin_raw[..end]).trim().to_string();
    if pin.is_empty() {
        return Err("PIN record kosong.".to_string());
    }
    let secs = u32::from_le_bytes([rec[27], rec[28], rec[29], rec[30]]);
    Ok(ZkTap {
        pin,
        time: zk_time_to_string(secs),
        verify: rec[26],
        status: rec[31],
    })
}

/// Label verify numerik ZK ke label teks log.
pub fn zk_verify_label(v: u8) -> String {
    verify_mode_label(&v.to_string())
}


fn split_host_port(endpoint: &str) -> (String, u16) {
    let e = endpoint.trim();
    if let Some(i) = e.rfind(':') {
        if let Ok(p) = e[i + 1..].parse::<u16>() {
            return (e[..i].to_string(), p);
        }
    }
    (e.to_string(), ZK_DEFAULT_PORT)
}

async fn zk_exchange(
    stream: &mut tokio::net::TcpStream,
    cmd: u16,
    session: u16,
    reply: u16,
    data: &[u8],
    timeout_ms: u64,
) -> Result<(u16, u16, Vec<u8>), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let pkt = zk_build(cmd, session, reply, data);
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        stream.write_all(&pkt),
    )
    .await
    .map_err(|_| "Batas waktu kirim ZK.".to_string())?
    .map_err(|e| format!("gagal kirim ZK: {e}"))?;
    let mut head = [0u8; 8];
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        stream.read_exact(&mut head),
    )
    .await
    .map_err(|_| "Batas waktu baca ZK.".to_string())?
    .map_err(|e| format!("gagal baca ZK: {e}"))?;
    if head[0..4] != ZK_TCP_MAGIC {
        return Err("Magic balasan ZK tidak cocok.".to_string());
    }
    let len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
    if len > 1024 * 1024 {
        return Err("Balasan ZK terlalu besar.".to_string());
    }
    let mut body = vec![0u8; len];
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        stream.read_exact(&mut body),
    )
    .await
    .map_err(|_| "Batas waktu baca isi ZK.".to_string())?
    .map_err(|e| format!("gagal baca isi ZK: {e}"))?;
    let mut full = head.to_vec();
    full.extend_from_slice(&body);
    let (rcmd, rsession, _, data) = zk_validate(&full)?;
    Ok((rcmd, rsession, data))
}

/// Tarik log dari device ZK: connect → disable → minta ATTLOG →
/// parse record → enable → exit. Tanpa hardware = belum teruji lapangan.
pub async fn zk_pull_logs(endpoint: &str, timeout_ms: u64) -> Result<Vec<ZkTap>, String> {
    let (host, port) = split_host_port(endpoint);
    let addr = format!("{host}:{port}");
    let mut stream = tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.max(1000)),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("batas waktu sambung ke {addr}"))?
    .map_err(|e| format!("gagal sambung ke {addr}: {e}"))?;
    let (cmd, session, _) =
        zk_exchange(&mut stream, CMD_CONNECT, 0, 0, &[], timeout_ms).await?;
    if cmd != CMD_ACK_OK {
        return Err("Device menolak koneksi ZK.".to_string());
    }
    let mut reply: u16 = 1;
    let mut next = || {
        let v = reply;
        reply = reply.wrapping_add(1);
        v
    };
    zk_exchange(
        &mut stream,
        CMD_DISABLEDEVICE,
        session,
        next(),
        &[0, 0, 0, 0],
        timeout_ms,
    )
    .await?;
    let req_data = [1u8, CMD_ATTLOG_RRQ as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let (_, _, mut data) = zk_exchange(
        &mut stream,
        CMD_ATTLOG_RRQ,
        session,
        next(),
        &req_data,
        timeout_ms,
    )
    .await?;
    let mut out = Vec::new();
    let mut guard = 0;
    while !data.is_empty() && guard < 512 {
        guard += 1;
        let mut i = 0;
        while i + 40 <= data.len() {
            if let Ok(t) = zk_parse_record(&data[i..i + 40]) {
                out.push(t);
            }
            i += 40;
        }
        let (more_cmd, _, more) =
            zk_exchange(&mut stream, CMD_ATTLOG_RRQ, session, next(), &[], timeout_ms).await?;
        if more_cmd == CMD_ACK_OK || more.is_empty() {
            break;
        }
        data = more;
    }
    let _ = zk_exchange(&mut stream, CMD_ENABLEDEVICE, session, next(), &[], timeout_ms).await;
    let _ = zk_exchange(&mut stream, CMD_EXIT, session, next(), &[], timeout_ms).await;
    Ok(out)
}


/// Satu tap dari body ATTLOG: `PIN\ttime\tstatus\tverify\tworkcode`.
#[derive(Clone, Debug, PartialEq)]
pub struct AdmsTap {
    pub pin: String,
    pub time: String,
    pub status: String,
    pub verify: String,
}

/// Parse body ATTLOG tab-separated, satu baris per tap.
pub fn parse_attlog_body(body: &str) -> Vec<AdmsTap> {
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 || cols[0].trim().is_empty() {
            continue;
        }
        let time = cols[1].trim().to_string();
        if chrono::NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M:%S").is_err()
            && time.parse::<i64>().is_err()
        {
            continue;
        }
        out.push(AdmsTap {
            pin: cols[0].trim().to_string(),
            time,
            status: cols.get(2).map(|s| s.trim().to_string()).unwrap_or_default(),
            verify: cols.get(3).map(|s| s.trim().to_string()).unwrap_or_default(),
        });
    }
    out
}

/// Parse baris USER: `USER PIN=1\tName=...\tCard=...`.
pub fn parse_user_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    if !line.starts_with("USER") {
        return None;
    }
    let mut pin = String::new();
    let mut name = String::new();
    let mut card = String::new();
    for part in line["USER".len()..].split(['\t', ' ']) {
        let part = part.trim();
        if let Some(v) = part.strip_prefix("PIN=") {
            pin = v.to_string();
        } else if let Some(v) = part.strip_prefix("Name=") {
            name = v.to_string();
        } else if let Some(v) = part.strip_prefix("Card=") {
            card = v.to_string();
        }
    }
    if pin.is_empty() {
        return None;
    }
    Some((pin, name, card))
}

/// Jawaban teks ADMS: selalu `OK`, device retry bila bukan OK.
pub fn adms_ok(count: usize) -> String {
    if count > 0 {
        format!("OK: {count}")
    } else {
        "OK".to_string()
    }
}

/// Cari device aktif berdasarkan serial (SN ADMS).
pub async fn device_by_serial(
    db: &sea_orm::DatabaseConnection,
    serial: &str,
) -> Result<Option<i64>, String> {
    let row = q_one(
        db,
        "SELECT id FROM fingerprint_devices WHERE serial = ?1 AND deleted_at IS NULL AND is_active = 1".to_string(),
        vec![Value::Text(serial.trim().to_string())],
        1,
        "fp.proto.seri",
    )
    .await?;
    Ok(row.and_then(|r| value_i64(&r[0])))
}

/// Tulis hasil devicecmd: baris `ID=<id>&Return=<0|...>`.
pub async fn apply_devicecmd(
    db: &sea_orm::DatabaseConnection,
    device_id: i64,
    body: &str,
) -> Result<usize, String> {
    let mut n = 0;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut id: Option<i64> = None;
        let mut ret: Option<i64> = None;
        for kv in line.split('&') {
            if let Some(v) = kv.strip_prefix("ID=") {
                id = v.trim().parse().ok();
            } else if let Some(v) = kv.strip_prefix("Return=") {
                ret = v.trim().parse().ok();
            }
        }
        if let (Some(cid), Some(r)) = (id, ret) {
            cmd_ack_sea(db, device_id, cid, r == 0, None).await?;
            n += 1;
        }
    }
    Ok(n)
}


#[derive(Clone)]
pub struct AdmsState {
    pub db: sea_orm::DatabaseConnection,
}

#[derive(serde::Deserialize)]
pub struct PushQuery {
    #[serde(rename = "SN")]
    pub sn: Option<String>,
    pub table: Option<String>,
    #[serde(rename = "Stamp")]
    pub stamp: Option<String>,
}

async fn handle_cdata(
    axum::extract::State(st): axum::extract::State<AdmsState>,
    axum::extract::Query(q): axum::extract::Query<PushQuery>,
    body: String,
) -> String {
    let sn = q.sn.clone().unwrap_or_default();
    let dev = match device_by_serial(&st.db, &sn).await {
        Ok(Some(id)) => id,
        _ => return adms_ok(0),
    };
    let table = q.table.clone().unwrap_or_default().to_uppercase();
    if table == "ATTLOG" {
        let taps = parse_attlog_body(&body);
        let mut n = 0;
        for t in taps {
            let time = if let Ok(secs) = t.time.parse::<i64>() {
                let since2000 = secs.saturating_sub(946_684_800);
                zk_time_to_string(since2000.max(0) as u32)
            } else {
                t.time.clone()
            };
            let verify = if t.verify.is_empty() {
                None
            } else {
                Some(verify_mode_label(&t.verify))
            };
            if ingest_sea(&st.db, dev, &t.pin, &time, verify.as_deref())
                .await
                .is_ok()
            {
                n += 1;
            }
        }
        return adms_ok(n);
    }
    adms_ok(0)
}

async fn handle_getrequest(
    axum::extract::State(st): axum::extract::State<AdmsState>,
    axum::extract::Query(q): axum::extract::Query<PushQuery>,
) -> String {
    let sn = q.sn.clone().unwrap_or_default();
    let dev = match device_by_serial(&st.db, &sn).await {
        Ok(Some(id)) => id,
        _ => return adms_ok(0),
    };
    let cmds = cmd_pending_sea(&st.db, dev, 20).await.unwrap_or_default();
    if cmds.is_empty() {
        return adms_ok(0);
    }
    let mut out = String::new();
    for c in cmds {
        out.push_str(&format!("C:{}:{}\n", c.id, c.payload));
    }
    out
}

async fn handle_devicecmd(
    axum::extract::State(st): axum::extract::State<AdmsState>,
    axum::extract::Query(q): axum::extract::Query<PushQuery>,
    body: String,
) -> String {
    let sn = q.sn.clone().unwrap_or_default();
    let dev = match device_by_serial(&st.db, &sn).await {
        Ok(Some(id)) => id,
        _ => return adms_ok(0),
    };
    let n = apply_devicecmd(&st.db, dev, &body).await.unwrap_or(0);
    adms_ok(n)
}

/// Router ADMS: cdata (push log), getrequest (poll perintah),
/// devicecmd (hasil perintah). Selalu balas teks OK.
pub fn adms_router(db: sea_orm::DatabaseConnection) -> axum::Router {
    let st = AdmsState { db };
    axum::Router::new()
        .route("/iclock/cdata", axum::routing::get(handle_cdata).post(handle_cdata))
        .route("/iclock/getrequest", axum::routing::get(handle_getrequest))
        .route("/iclock/devicecmd", axum::routing::post(handle_devicecmd))
        .with_state(st)
}

/// Jalankan listener ADMS di port LAN; tugas detached, mati ikut proses.
pub async fn adms_serve(db: sea_orm::DatabaseConnection, port: i64) -> Result<String, String> {
    if !(1..=65535).contains(&port) {
        return Err("Port harus 1 sampai 65535.".to_string());
    }
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("gagal bind {addr}: {e}"))?;
    let app = adms_router(db);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(addr)
}

/// Cek listener hidup via sambung TCP singkat.
pub async fn adms_alive(port: i64, timeout_ms: u64) -> bool {
    if !(1..=65535).contains(&port) {
        return false;
    }
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.max(200)),
        tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")),
    )
    .await
    .map(|r| r.is_ok())
    .unwrap_or(false)
}

/// Tarik log via ZK-pull dari device terdaftar lalu masukkan antrean
/// via `ingest_sea` per tap. Kembalikan jumlah tap termuat.
pub async fn fp_pull_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    device_id: i64,
    timeout_ms: u64,
) -> Result<i32, String> {
    let row = q_one(
        db,
        "SELECT endpoint FROM fingerprint_devices WHERE id = ?1 AND deleted_at IS NULL AND is_active = 1".to_string(),
        vec![Value::Int(device_id)],
        1,
        "fp.proto.endpoint",
    )
    .await?
    .ok_or_else(|| "Device tidak ditemukan atau nonaktif.".to_string())?;
    let endpoint = value_to_string(&row[0]);
    if endpoint.trim().is_empty() {
        return Err("Device belum punya endpoint host.".to_string());
    }
    let taps = zk_pull_logs(&endpoint, timeout_ms).await?;
    let mut n = 0i64;
    for t in taps {
        if ingest_sea(db, device_id, &t.pin, &t.time, Some(&zk_verify_label(t.verify)))
            .await
            .is_ok()
        {
            n += 1;
        }
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "PULL",
        "fingerprint.pull",
        Some(&device_id.to_string()),
        None,
        None,
        Some(&format!("{n} tap termuat via ZK-pull.")),
    )
    .await?;
    crate::to_dto_int(n, "fp.pull")
}

#[cfg(test)]
mod tests {    use super::*;

    #[test]
    fn checksum_connect_vector() {
        let pkt = zk_build(CMD_CONNECT, 0, 0, &[]);
        assert_eq!(&pkt[0..4], &ZK_TCP_MAGIC);
        assert_eq!(u32::from_le_bytes([pkt[4], pkt[5], pkt[6], pkt[7]]), 8);
        assert_eq!(get_u16_le(&pkt[10..12]), 0xFC16);
        let (cmd, session, reply, data) = zk_validate(&pkt).expect("valid");
        assert_eq!((cmd, session, reply), (CMD_CONNECT, 0, 0));
        assert!(data.is_empty());
    }

    #[test]
    fn checksum_tolak_rusak() {
        let mut pkt = zk_build(CMD_CONNECT, 0, 0, &[]);
        pkt[12] ^= 0xFF;
        assert!(zk_validate(&pkt).is_err());
    }

    #[test]
    fn record_40byte_terparse() {
        let mut rec = vec![0u8; 40];
        rec[0] = 7;
        rec[2..7].copy_from_slice(b"101  ");
        rec[26] = 1;
        let secs: u32 = 800_000_000;
        rec[27..31].copy_from_slice(&secs.to_le_bytes());
        rec[31] = 0;
        let t = zk_parse_record(&rec).expect("parse");
        assert_eq!(t.pin, "101");
        assert_eq!(t.verify, 1);
        assert_eq!(t.time, zk_time_to_string(secs));
    }

    #[test]
    fn attlog_tab_terparse_dan_baris_rusak_dilewati() {
        let body = "2\t2020-07-10 16:34:02\t255\t15\t0\n\nsampah\n3\t2020-07-10 16:35:00\t0\t2\n";
        let taps = parse_attlog_body(body);
        assert_eq!(taps.len(), 2);
        assert_eq!(taps[0].pin, "2");
        assert_eq!(taps[0].verify, "15");
        assert_eq!(taps[1].status, "0");
    }

    #[test]
    fn user_line_terparse() {
        let (pin, name, card) =
            parse_user_line("USER PIN=12\tName=Budi\tCard=987").expect("user");
        assert_eq!((pin.as_str(), name.as_str(), card.as_str()), ("12", "Budi", "987"));
        assert!(parse_user_line("OPLOG 30 0").is_none());
    }

    #[test]
    fn split_endpoint_default_port() {
        assert_eq!(split_host_port("10.0.0.5"), ("10.0.0.5".to_string(), 4370));
        assert_eq!(split_host_port("10.0.0.5:4371"), ("10.0.0.5".to_string(), 4371));
    }
}
