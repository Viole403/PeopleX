use chrono::Local;

use super::audit;
use super::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};
use crate::to_dto_int;

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn opt_text(v: &Value) -> Option<String> {
    match v {
        Value::Text(s) => Some(s.clone()),
        _ => None,
}

}

fn opt_i(v: &Value) -> Option<i32> {
    match v {
        Value::Int(i) => Some(*i as i32),
        _ => None,
    }
}

fn teks(v: &Value) -> String {
    value_to_string(v)
}

fn angka(v: &Value) -> i64 {
    value_i64(v).unwrap_or(0)
}

async fn cek_karyawan(db: &sea_orm::DatabaseConnection, id: i64) -> Result<(), String> {
    let row = q_one(
        db,
        "SELECT id FROM employees WHERE id = ?1 AND deleted_at IS NULL".to_string(),
        vec![Value::Int(id)],
        1,
        "engagement.emp",
    )
    .await
    .map_err(|e| format!("gagal memeriksa karyawan: {e}"))?;
    if row.is_none() {
        return Err("Karyawan tidak ditemukan.".to_string());
    }
    Ok(())
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct Kudos {
    pub id: i32,
    pub from_employee_id: i32,
    pub from_name: String,
    pub to_employee_id: i32,
    pub to_name: String,
    pub message: String,
    pub badge: Option<String>,
    pub created_at: String,
}

pub async fn kudos_send_sea(
    db: &sea_orm::DatabaseConnection,
    from_employee_id: i64,
    to_employee_id: i64,
    message: &str,
    badge: Option<&str>,
) -> Result<i32, String> {
    if from_employee_id == to_employee_id {
        return Err("Pilih rekan tujuan.".to_string());
    }
    let pesan = message.trim();
    if pesan.is_empty() {
        return Err("Pesan wajib diisi.".to_string());
    }
    if pesan.chars().count() > 280 {
        return Err("Pesan terlalu panjang.".to_string());
    }
    cek_karyawan(db, from_employee_id).await?;
    cek_karyawan(db, to_employee_id).await?;
    let sekarang = now_str();
    let rid = exec_insert(
        db,
        "INSERT INTO kudos (from_employee_id, to_employee_id, message, badge, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)".to_string(),
        vec![
            Value::Int(from_employee_id),
            Value::Int(to_employee_id),
            Value::Text(pesan.to_string()),
            badge.map(|b| Value::Text(b.to_string())).unwrap_or(Value::Null),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang),
        ],
        "kudos.insert",
    )
    .await
    .map_err(|e| format!("gagal menyimpan kudos: {e}"))?;
    audit::log_sea(
        db,
        None,
        "CREATE",
        "engagement.kudos",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "kudos.id")
}

fn kudos_row(r: &[Value]) -> Kudos {
    Kudos {
        id: opt_i(&r[0]).unwrap_or(0),
        from_employee_id: opt_i(&r[1]).unwrap_or(0),
        to_employee_id: opt_i(&r[2]).unwrap_or(0),
        from_name: teks(&r[3]),
        to_name: teks(&r[4]),
        message: teks(&r[5]),
        badge: opt_text(&r[6]),
        created_at: teks(&r[7]),
    }
}

pub async fn kudos_list_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: Option<i64>,
) -> Result<Vec<Kudos>, String> {
    let (sql, vals) = match employee_id {
        Some(id) => (
            "SELECT k.id, k.from_employee_id, k.to_employee_id, f.first_name || ' ' || COALESCE(f.last_name, ''), t.first_name || ' ' || COALESCE(t.last_name, ''), k.message, k.badge, k.created_at FROM kudos k INNER JOIN employees f ON f.id = k.from_employee_id INNER JOIN employees t ON t.id = k.to_employee_id WHERE k.from_employee_id = ?1 OR k.to_employee_id = ?1 ORDER BY k.id DESC LIMIT 50".to_string(),
            vec![Value::Int(id)],
        ),
        None => (
            "SELECT k.id, k.from_employee_id, k.to_employee_id, f.first_name || ' ' || COALESCE(f.last_name, ''), t.first_name || ' ' || COALESCE(t.last_name, ''), k.message, k.badge, k.created_at FROM kudos k INNER JOIN employees f ON f.id = k.from_employee_id INNER JOIN employees t ON t.id = k.to_employee_id ORDER BY k.id DESC LIMIT 50".to_string(),
            vec![],
        ),
    };
    let rows = q_all(db, sql, vals, 8, "kudos.list")
        .await
        .map_err(|e| format!("gagal membaca kudos: {e}"))?;
    Ok(rows.iter().map(|r| kudos_row(r)).collect())
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct PollOption {
    pub id: i32,
    pub label: String,
    pub votes: i32,
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct Poll {
    pub id: i32,
    pub question: String,
    pub status: String,
    pub total_votes: i32,
    pub options: Vec<PollOption>,
}

pub async fn poll_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    question: &str,
    options: &[String],
) -> Result<i32, String> {
    let tanya = question.trim();
    if tanya.is_empty() {
        return Err("Pertanyaan wajib diisi.".to_string());
    }
    if options.len() < 2 {
        return Err("Minimal dua pilihan.".to_string());
    }
    for o in options {
        if o.trim().is_empty() {
            return Err("Pilihan tidak boleh kosong.".to_string());
        }
    }
    let sekarang = now_str();
    if let Some(pid) = id {
        let ada = q_one(
            db,
            "SELECT status FROM polls WHERE id = ?1".to_string(),
            vec![Value::Int(pid)],
            1,
            "poll.ada",
        )
        .await
        .map_err(|e| format!("gagal memeriksa polling: {e}"))?;
        match ada {
            None => return Err("Polling tidak ditemukan.".to_string()),
            Some(r) if teks(&r[0]) != "open" => {
                return Err("Polling sudah ditutup.".to_string())
            }
            Some(_) => {}
        }
        exec(
            db,
            "UPDATE polls SET question = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
            vec![Value::Text(tanya.to_string()), Value::Text(sekarang), Value::Int(pid)],
            "poll.upd",
        )
        .await
        .map_err(|e| format!("gagal memperbarui polling: {e}"))?;
        exec(
            db,
            "DELETE FROM poll_options WHERE poll_id = ?1".to_string(),
            vec![Value::Int(pid)],
            "poll.hapus",
        )
        .await
        .map_err(|e| format!("gagal menyegarkan pilihan: {e}"))?;
        for o in options {
            exec(
                db,
                "INSERT INTO poll_options (poll_id, label, created_at) VALUES (?1, ?2, ?3)".to_string(),
                vec![
                    Value::Int(pid),
                    Value::Text(o.trim().to_string()),
                    Value::Text(now_str()),
                ],
                "poll.ops",
            )
            .await
            .map_err(|e| format!("gagal menyimpan pilihan: {e}"))?;
        }
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "engagement.poll",
            Some(&pid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        return to_dto_int(pid, "poll.id");
    }
    let rid = exec_insert(
        db,
        "INSERT INTO polls (question, status, created_by, created_at, updated_at) VALUES (?1, 'open', ?2, ?3, ?4)".to_string(),
        vec![
            Value::Text(tanya.to_string()),
            Value::Int(actor_id),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang),
        ],
        "poll.insert",
    )
    .await
    .map_err(|e| format!("gagal menyimpan polling: {e}"))?;
    for o in options {
        exec(
            db,
            "INSERT INTO poll_options (poll_id, label, created_at) VALUES (?1, ?2, ?3)".to_string(),
            vec![
                Value::Int(rid),
                Value::Text(o.trim().to_string()),
                Value::Text(now_str()),
            ],
            "poll.ops",
        )
        .await
        .map_err(|e| format!("gagal menyimpan pilihan: {e}"))?;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "engagement.poll",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "poll.id")
}

pub async fn poll_close_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    poll_id: i64,
) -> Result<(), String> {
    let ada = q_one(
        db,
        "SELECT status FROM polls WHERE id = ?1".to_string(),
        vec![Value::Int(poll_id)],
        1,
        "poll.cari",
    )
    .await
    .map_err(|e| format!("gagal memeriksa polling: {e}"))?;
    match ada {
        None => return Err("Polling tidak ditemukan.".to_string()),
        Some(r) if teks(&r[0]) != "open" => return Err("Polling sudah ditutup.".to_string()),
        Some(_) => {}
    }
    let sekarang = now_str();
    exec(
        db,
        "UPDATE polls SET status = 'closed', closed_at = ?1, updated_at = ?1 WHERE id = ?2".to_string(),
        vec![Value::Text(sekarang), Value::Int(poll_id)],
        "poll.tutup",
    )
    .await
    .map_err(|e| format!("gagal menutup polling: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "engagement.poll",
        Some(&poll_id.to_string()),
        None,
        None,
        None,
    )
    .await
}

pub async fn poll_vote_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    poll_id: i64,
    option_id: i64,
) -> Result<i32, String> {
    cek_karyawan(db, employee_id).await?;
    let poll = q_one(
        db,
        "SELECT status FROM polls WHERE id = ?1".to_string(),
        vec![Value::Int(poll_id)],
        1,
        "poll.v.cari",
    )
    .await
    .map_err(|e| format!("gagal memeriksa polling: {e}"))?;
    match poll {
        None => return Err("Polling tidak ditemukan.".to_string()),
        Some(r) if teks(&r[0]) != "open" => return Err("Polling sudah ditutup.".to_string()),
        Some(_) => {}
    }
    let opsi = q_one(
        db,
        "SELECT id FROM poll_options WHERE id = ?1 AND poll_id = ?2".to_string(),
        vec![Value::Int(option_id), Value::Int(poll_id)],
        1,
        "poll.v.opsi",
    )
    .await
    .map_err(|e| format!("gagal memeriksa pilihan: {e}"))?;
    if opsi.is_none() {
        return Err("Pilihan tidak valid.".to_string());
    }
    let dulu = q_one(
        db,
        "SELECT id FROM poll_votes WHERE poll_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(poll_id), Value::Int(employee_id)],
        1,
        "poll.v.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa suara: {e}"))?;
    if let Some(v) = dulu {
        return Ok(opt_i(&v[0]).unwrap_or(0));
    }
    let rid = exec_insert(
        db,
        "INSERT INTO poll_votes (poll_id, option_id, employee_id, created_at) VALUES (?1, ?2, ?3, ?4)".to_string(),
        vec![
            Value::Int(poll_id),
            Value::Int(option_id),
            Value::Int(employee_id),
            Value::Text(now_str()),
        ],
        "poll.v.ins",
    )
    .await
    .map_err(|e| format!("gagal menyimpan suara: {e}"))?;
    to_dto_int(rid, "vote.id")
}

async fn susun_poll(db: &sea_orm::DatabaseConnection, id: i64) -> Result<Option<Poll>, String> {
    let header = q_one(
        db,
        "SELECT question, status FROM polls WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        2,
        "poll.hasil",
    )
    .await
    .map_err(|e| format!("gagal membaca polling: {e}"))?;
    let h = match header {
        None => return Ok(None),
        Some(h) => h,
    };
    let opsi = q_all(
        db,
        "SELECT o.id, o.label, (SELECT COUNT(*) FROM poll_votes v WHERE v.option_id = o.id) FROM poll_options o WHERE o.poll_id = ?1 ORDER BY o.id".to_string(),
        vec![Value::Int(id)],
        3,
        "poll.opsi",
    )
    .await
    .map_err(|e| format!("gagal membaca pilihan: {e}"))?;
    let mut total = 0i64;
    let mut opsi_out = Vec::new();
    for r in &opsi {
        let v = angka(&r[2]);
        total += v;
        opsi_out.push(PollOption {
            id: opt_i(&r[0]).unwrap_or(0),
            label: teks(&r[1]),
            votes: v as i32,
        });
    }
    Ok(Some(Poll {
        id: opt_i(&Value::Int(id)).unwrap_or(0),
        question: teks(&h[0]),
        status: teks(&h[1]),
        total_votes: total as i32,
        options: opsi_out,
    }))
}

pub async fn poll_results_sea(
    db: &sea_orm::DatabaseConnection,
    poll_id: i64,
) -> Result<Poll, String> {
    susun_poll(db, poll_id)
        .await?
        .ok_or_else(|| "Polling tidak ditemukan.".to_string())
}

pub async fn poll_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Poll>, String> {
    let rows = q_all(
        db,
        "SELECT id FROM polls ORDER BY id DESC LIMIT 20".to_string(),
        vec![],
        1,
        "poll.list",
    )
    .await
    .map_err(|e| format!("gagal membaca daftar polling: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        if let Some(id) = opt_i(&r[0]) {
            if let Some(p) = susun_poll(db, id as i64).await? {
                out.push(p);
            }
        }
    }
    Ok(out)
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct Whistleblow {
    pub id: i32,
    pub topic: String,
    pub status: String,
    pub response: Option<String>,
    pub created_at: String,
    pub closed_at: Option<String>,
}

fn wbc_row(r: &[Value]) -> Whistleblow {
    Whistleblow {
        id: opt_i(&r[0]).unwrap_or(0),
        topic: teks(&r[1]),
        status: teks(&r[2]),
        response: opt_text(&r[3]),
        created_at: teks(&r[4]),
        closed_at: opt_text(&r[5]),
    }
}

pub async fn whistleblow_report_sea(
    db: &sea_orm::DatabaseConnection,
    topic: &str,
    message: &str,
) -> Result<String, String> {
    let top = topic.trim();
    let isi = message.trim();
    if top.is_empty() || isi.is_empty() {
        return Err("Topik dan isi laporan wajib diisi.".to_string());
    }
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let sekarang = now_str();
    exec_insert(
        db,
        "INSERT INTO whistleblows (topic, message, token, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'open', ?4, ?5)".to_string(),
        vec![
            Value::Text(top.to_string()),
            Value::Text(isi.to_string()),
            Value::Text(token.clone()),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang),
        ],
        "wbc.ins",
    )
    .await
    .map_err(|e| format!("gagal menyimpan laporan: {e}"))?;
    Ok(token)
}

pub async fn whistleblow_status_sea(
    db: &sea_orm::DatabaseConnection,
    token: &str,
) -> Result<Option<Whistleblow>, String> {
    let rows = q_all(
        db,
        "SELECT id, topic, status, response, created_at, closed_at FROM whistleblows WHERE token = ?1".to_string(),
        vec![Value::Text(token.to_string())],
        6,
        "wbc.cari",
    )
    .await
    .map_err(|e| format!("gagal membaca laporan: {e}"))?;
    Ok(rows.first().map(|r| wbc_row(r)))
}

pub async fn whistleblow_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<Whistleblow>, String> {
    let rows = q_all(
        db,
        "SELECT id, topic, status, response, created_at, closed_at FROM whistleblows ORDER BY id DESC LIMIT 50".to_string(),
        vec![],
        6,
        "wbc.list",
    )
    .await
    .map_err(|e| format!("gagal membaca laporan: {e}"))?;
    Ok(rows.iter().map(|r| wbc_row(r)).collect())
}

pub async fn whistleblow_decide_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
    status: &str,
    response: Option<&str>,
) -> Result<(), String> {
    if status != "investigating" && status != "closed" {
        return Err("Status tidak valid.".to_string());
    }
    let ada = q_one(
        db,
        "SELECT status FROM whistleblows WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "wbc.ada",
    )
    .await
    .map_err(|e| format!("gagal memeriksa laporan: {e}"))?;
    match ada {
        None => return Err("Laporan tidak ditemukan.".to_string()),
        Some(r) if teks(&r[0]) == "closed" => {
            return Err("Laporan sudah ditutup.".to_string())
        }
        Some(_) => {}
    }
    let sekarang = now_str();
    let closed = if status == "closed" {
        Value::Text(sekarang.clone())
    } else {
        Value::Null
    };
    exec(
        db,
        "UPDATE whistleblows SET status = ?1, response = ?2, closed_at = ?3, updated_at = ?4 WHERE id = ?5".to_string(),
        vec![
            Value::Text(status.to_string()),
            response.map(|s| Value::Text(s.to_string())).unwrap_or(Value::Null),
            closed,
            Value::Text(sekarang),
            Value::Int(id),
        ],
        "wbc.upd",
    )
    .await
    .map_err(|e| format!("gagal memperbarui laporan: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "engagement.whistleblow",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct Policy {
    pub id: i32,
    pub title: String,
    pub version: String,
    pub content: Option<String>,
    pub status: String,
    pub published_at: Option<String>,
}

fn policy_row(r: &[Value]) -> Policy {
    Policy {
        id: opt_i(&r[0]).unwrap_or(0),
        title: teks(&r[1]),
        version: teks(&r[2]),
        content: opt_text(&r[3]),
        status: teks(&r[4]),
        published_at: opt_text(&r[5]),
    }
}

pub async fn policy_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    title: &str,
    version: &str,
    content: Option<&str>,
) -> Result<i32, String> {
    let judul = title.trim();
    let versi = version.trim();
    if judul.is_empty() {
        return Err("Judul wajib diisi.".to_string());
    }
    if versi.is_empty() {
        return Err("Versi wajib diisi.".to_string());
    }
    let sekarang = now_str();
    if let Some(pid) = id {
        let ada = q_one(
            db,
            "SELECT status FROM policies WHERE id = ?1".to_string(),
            vec![Value::Int(pid)],
            1,
            "pol.ada",
        )
        .await
        .map_err(|e| format!("gagal memeriksa kebijakan: {e}"))?;
        match ada {
            None => return Err("Kebijakan tidak ditemukan.".to_string()),
            Some(r) if teks(&r[0]) == "published" => {
                return Err("Kebijakan yang terbit tidak bisa diubah.".to_string())
            }
            Some(_) => {}
        }
        exec(
            db,
            "UPDATE policies SET title = ?1, version = ?2, content = ?3, updated_at = ?4 WHERE id = ?5".to_string(),
            vec![
                Value::Text(judul.to_string()),
                Value::Text(versi.to_string()),
                content.map(|c| Value::Text(c.to_string())).unwrap_or(Value::Null),
                Value::Text(sekarang),
                Value::Int(pid),
            ],
            "pol.upd",
        )
        .await
        .map_err(|e| format!("gagal memperbarui kebijakan: {e}"))?;
        audit::log_sea(
            db,
            Some(actor_id),
            "UPDATE",
            "engagement.policy",
            Some(&pid.to_string()),
            None,
            None,
            None,
        )
        .await?;
        return to_dto_int(pid, "policy.id");
    }
    let rid = exec_insert(
        db,
        "INSERT INTO policies (title, version, content, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, 'draft', ?4, ?5, ?6)".to_string(),
        vec![
            Value::Text(judul.to_string()),
            Value::Text(versi.to_string()),
            content.map(|c| Value::Text(c.to_string())).unwrap_or(Value::Null),
            Value::Int(actor_id),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang),
        ],
        "pol.ins",
    )
    .await
    .map_err(|e| format!("gagal menyimpan kebijakan: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "engagement.policy",
        Some(&rid.to_string()),
        None,
        None,
        None,
    )
    .await?;
    to_dto_int(rid, "policy.id")
}

pub async fn policy_publish_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
) -> Result<(), String> {
    let ada = q_one(
        db,
        "SELECT status FROM policies WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "pol.pub",
    )
    .await
    .map_err(|e| format!("gagal memeriksa kebijakan: {e}"))?;
    match ada {
        None => return Err("Kebijakan tidak ditemukan.".to_string()),
        Some(r) if teks(&r[0]) != "draft" => {
            return Err("Hanya draf yang bisa diterbitkan.".to_string())
        }
        Some(_) => {}
    }
    let sekarang = now_str();
    exec(
        db,
        "UPDATE policies SET status = 'published', published_at = ?1, updated_at = ?1 WHERE id = ?2".to_string(),
        vec![Value::Text(sekarang), Value::Int(id)],
        "pol.terbit",
    )
    .await
    .map_err(|e| format!("gagal menerbitkan kebijakan: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "engagement.policy",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await
}

pub async fn policy_list_sea(
    db: &sea_orm::DatabaseConnection,
    published_only: bool,
) -> Result<Vec<Policy>, String> {
    let sql = if published_only {
        "SELECT id, title, version, content, status, published_at FROM policies WHERE status = 'published' ORDER BY id DESC"
    } else {
        "SELECT id, title, version, content, status, published_at FROM policies ORDER BY id DESC"
    };
    let rows = q_all(db, sql.to_string(), vec![], 6, "pol.list")
        .await
        .map_err(|e| format!("gagal membaca kebijakan: {e}"))?;
    Ok(rows.iter().map(|r| policy_row(r)).collect())
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct PolicyAck {
    pub employee_id: i32,
    pub employee_name: String,
    pub acked_at: String,
}

pub async fn policy_ack_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    policy_id: i64,
) -> Result<i32, String> {
    cek_karyawan(db, employee_id).await?;
    let pol = q_one(
        db,
        "SELECT status FROM policies WHERE id = ?1".to_string(),
        vec![Value::Int(policy_id)],
        1,
        "ack.pol",
    )
    .await
    .map_err(|e| format!("gagal memeriksa kebijakan: {e}"))?;
    match pol {
        None => return Err("Kebijakan tidak ditemukan.".to_string()),
        Some(r) if teks(&r[0]) != "published" => {
            return Err("Kebijakan belum dipublikasikan.".to_string())
        }
        Some(_) => {}
    }
    let dulu = q_one(
        db,
        "SELECT id FROM policy_acks WHERE policy_id = ?1 AND employee_id = ?2".to_string(),
        vec![Value::Int(policy_id), Value::Int(employee_id)],
        1,
        "ack.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa pengakuan: {e}"))?;
    if let Some(a) = dulu {
        return Ok(opt_i(&a[0]).unwrap_or(0));
    }
    let rid = exec_insert(
        db,
        "INSERT INTO policy_acks (policy_id, employee_id, acked_at) VALUES (?1, ?2, ?3)".to_string(),
        vec![
            Value::Int(policy_id),
            Value::Int(employee_id),
            Value::Text(now_str()),
        ],
        "ack.ins",
    )
    .await
    .map_err(|e| format!("gagal menyimpan pengakuan: {e}"))?;
    to_dto_int(rid, "ack.id")
}

pub async fn policy_ack_list_sea(
    db: &sea_orm::DatabaseConnection,
    policy_id: i64,
) -> Result<Vec<PolicyAck>, String> {
    let rows = q_all(
        db,
        "SELECT a.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), a.acked_at FROM policy_acks a INNER JOIN employees e ON e.id = a.employee_id WHERE a.policy_id = ?1 ORDER BY a.id".to_string(),
        vec![Value::Int(policy_id)],
        3,
        "ack.list",
    )
    .await
    .map_err(|e| format!("gagal membaca pengakuan: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| PolicyAck {
            employee_id: opt_i(&r[0]).unwrap_or(0),
            employee_name: teks(&r[1]),
            acked_at: teks(&r[2]),
        })
        .collect())
}

#[derive(serde::Serialize, specta::Type, Clone, Debug)]
pub struct Consent {
    pub id: i32,
    pub purpose: String,
    pub granted: bool,
    pub updated_at: String,
}

pub async fn consent_set_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    purpose: &str,
    granted: bool,
) -> Result<i32, String> {
    let tujuan = purpose.trim();
    if tujuan.is_empty() {
        return Err("Tujuan wajib diisi.".to_string());
    }
    cek_karyawan(db, employee_id).await?;
    let sekarang = now_str();
    let dulu = q_one(
        db,
        "SELECT id FROM consents WHERE employee_id = ?1 AND purpose = ?2".to_string(),
        vec![Value::Int(employee_id), Value::Text(tujuan.to_string())],
        1,
        "cons.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa persetujuan: {e}"))?;
    if let Some(c) = dulu {
        let cid = opt_i(&c[0]).unwrap_or(0);
        exec(
            db,
            "UPDATE consents SET granted = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
            vec![
                Value::Int(granted as i64),
                Value::Text(sekarang),
                Value::Int(cid as i64),
            ],
            "cons.upd",
        )
        .await
        .map_err(|e| format!("gagal memperbarui persetujuan: {e}"))?;
        return Ok(cid);
    }
    let rid = exec_insert(
        db,
        "INSERT INTO consents (employee_id, purpose, granted, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Text(tujuan.to_string()),
            Value::Int(granted as i64),
            Value::Text(sekarang.clone()),
            Value::Text(sekarang),
        ],
        "cons.ins",
    )
    .await
    .map_err(|e| format!("gagal menyimpan persetujuan: {e}"))?;
    to_dto_int(rid, "consent.id")
}

pub async fn consent_list_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
) -> Result<Vec<Consent>, String> {
    let rows = q_all(
        db,
        "SELECT id, purpose, granted, updated_at FROM consents WHERE employee_id = ?1 ORDER BY id".to_string(),
        vec![Value::Int(employee_id)],
        4,
        "cons.list",
    )
    .await
    .map_err(|e| format!("gagal membaca persetujuan: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| Consent {
            id: opt_i(&r[0]).unwrap_or(0),
            purpose: teks(&r[1]),
            granted: angka(&r[2]) == 1,
            updated_at: teks(&r[3]),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = state.sea.clone();
        let emp = q_one(
            &db,
            "SELECT id FROM employees WHERE employee_number = 'EMP-0001'".to_string(),
            vec![],
            1,
            "t.emp",
        )
        .await
        .expect("emp")
        .and_then(|r| value_i64(&r[0]))
        .expect("id");
        for (n, nama) in [("EMP-G8A", "Ayu"), ("EMP-G8B", "Bima")] {
            exec(
                &db,
                "INSERT INTO employees (employee_number, first_name, company_id, join_date, employment_status, employment_type) VALUES (?1, ?2, 1, '2026-01-05', 'active', 'permanent')".to_string(),
                vec![Value::Text(n.to_string()), Value::Text(nama.to_string())],
                "t.ins",
            )
            .await
            .expect("sisip");
        }
        dir
    }

    async fn emp_by(db: &sea_orm::DatabaseConnection, number: &str) -> i64 {
        q_one(
            db,
            "SELECT id FROM employees WHERE employee_number = ?1".to_string(),
            vec![Value::Text(number.to_string())],
            1,
            "t.cari",
        )
        .await
        .expect("cari")
        .and_then(|r| value_i64(&r[0]))
        .expect("id")
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
        .and_then(|r| value_i64(&r[0]))
        .expect("id")
    }

    #[tokio::test]
    async fn kudos_apresiasi_tercatat_dan_bersyarat() {
        let dir = setup().await;
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let satu = emp_by(db, "EMP-0001").await;
        let dua = emp_by(db, "EMP-G8A").await;
        assert!(kudos_send_sea(db, satu, satu, "Hebat", None)
            .await
            .expect_err("diri")
            .contains("Pilih rekan"));
        assert!(kudos_send_sea(db, satu, dua, "  ", None)
            .await
            .expect_err("kosong")
            .contains("Pesan wajib"));
        let id = kudos_send_sea(db, satu, dua, "Terima kasih atas bantuannya.", Some("teamwork"))
            .await
            .expect("kirim");
        assert!(id > 0);
        let daftar = kudos_list_sea(db, Some(dua)).await.expect("daftar");
        assert_eq!(daftar.len(), 1);
        assert_eq!(daftar[0].badge.as_deref(), Some("teamwork"));
        assert_eq!(daftar[0].from_name, daftar[0].from_name);
        assert!(kudos_send_sea(db, satu, 999999, "Hai", None)
            .await
            .expect_err("asing")
            .contains("Karyawan tidak ditemukan"));
    }

    #[tokio::test]
    async fn polling_vote_dan_hasil() {
        let dir = setup().await;
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let satu = emp_by(db, "EMP-0001").await;
        let dua = emp_by(db, "EMP-G8A").await;
        let tiga = emp_by(db, "EMP-G8B").await;
        assert!(poll_save_sea(db, actor, None, "WFH?", &["Ya".to_string()])
            .await
            .expect_err("opsi")
            .contains("Minimal dua pilihan"));
        let pid = poll_save_sea(
            db,
            actor,
            None,
            "WFH?",
            &["Ya".to_string(), "Tidak".to_string()],
        )
        .await
        .expect("buat") as i64;
        let opsi = q_all(
            db,
            "SELECT id, label FROM poll_options WHERE poll_id = ?1 ORDER BY id".to_string(),
            vec![Value::Int(pid)],
            2,
            "t.opsi",
        )
        .await
        .expect("opsi");
        let ya = value_i64(&opsi[0][0]).expect("id opsi");
        let tidak = value_i64(&opsi[1][0]).expect("id opsi");
        poll_vote_sea(db, satu, pid, ya).await.expect("v1");
        poll_vote_sea(db, dua, pid, ya).await.expect("v2");
        let ulang = poll_vote_sea(db, satu, pid, tidak).await.expect("dup");
        let hasil = poll_results_sea(db, pid).await.expect("hasil");
        assert_eq!(hasil.total_votes, 2);
        assert_eq!(hasil.options[0].votes, 2);
        assert_eq!(hasil.options[1].votes, 0);
        assert_eq!(hasil.id as i64, pid);
        assert!(ulang > 0);
        poll_vote_sea(db, tiga, pid, 999999)
            .await
            .expect_err("asing");
        poll_close_sea(db, actor, pid).await.expect("tutup");
        assert!(poll_vote_sea(db, tiga, pid, ya)
            .await
            .expect_err("tutup")
            .contains("sudah ditutup"));
        assert!(poll_close_sea(db, actor, pid)
            .await
            .expect_err("lagi")
            .contains("sudah ditutup"));
    }

    #[tokio::test]
    async fn whistleblow_policy_konsen_tercatat() {
        let dir = setup().await;
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin(db).await;
        let satu = emp_by(db, "EMP-0001").await;
        let token = whistleblow_report_sea(db, "Integritas", "Ada pungutan liar.")
            .await
            .expect("lapor");
        assert_eq!(token.len(), 32);
        let cek = whistleblow_status_sea(db, &token)
            .await
            .expect("status")
            .expect("ada");
        assert_eq!(cek.status, "open");
        assert!(whistleblow_status_sea(db, "salah-token")
            .await
            .expect("kosong")
            .is_none());
        whistleblow_decide_sea(db, actor, cek.id as i64, "closed", Some("Sudah ditindak"))
            .await
            .expect("putus");
        assert!(whistleblow_decide_sea(db, actor, cek.id as i64, "closed", None)
            .await
            .expect_err("tutup lagi")
            .contains("sudah ditutup"));
        assert!(whistleblow_decide_sea(db, actor, cek.id as i64, "enteng", None)
            .await
            .expect_err("status")
            .contains("Status tidak valid"));
        assert!(whistleblow_report_sea(db, "  ", "isi")
            .await
            .expect_err("kosong")
            .contains("wajib diisi"));

        let pol = policy_save_sea(db, actor, None, "Kode Etik", "1.0", Some("Isi"))
            .await
            .expect("draf") as i64;
        assert!(policy_ack_sea(db, satu, pol)
            .await
            .expect_err("belum terbit")
            .contains("belum dipublikasikan"));
        policy_publish_sea(db, actor, pol).await.expect("terbit");
        assert!(policy_save_sea(db, actor, Some(pol), "Kode Etik", "1.1", None)
            .await
            .expect_err("ubah terbit")
            .contains("tidak bisa diubah"));
        let ack = policy_ack_sea(db, satu, pol).await.expect("akui");
        assert_eq!(policy_ack_sea(db, satu, pol).await.expect("idempoten"), ack);
        let daftar = policy_ack_list_sea(db, pol).await.expect("daftar ack");
        assert_eq!(daftar.len(), 1);
        assert_eq!(policy_list_sea(db, true).await.expect("list").len(), 1);
        assert!(policy_publish_sea(db, actor, pol)
            .await
            .expect_err("terbit lagi")
            .contains("Hanya draf"));

        let cid = consent_set_sea(db, satu, "pemrosesan_data_pribadi", true)
            .await
            .expect("consent");
        consent_set_sea(db, satu, "pemrosesan_data_pribadi", false)
            .await
            .expect("cabut");
        let list = consent_list_sea(db, satu).await.expect("list consent");
        assert_eq!(list.len(), 1);
        assert!(!list[0].granted);
        assert_eq!(cid, list[0].id);
    }
}
