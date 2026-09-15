//! Jalur SQL mentah yang sama untuk SQLite, PostgreSQL, dan MySQL.
//!
//! Semua modul domain memakai fungsi di sini supaya tidak ada lagi
//! koneksi SQLite langsung yang tersebar. Placeholder di kode pemanggil
//! memakai gaya bernomor `?1, ?2`; untuk PostgreSQL angka yang sama diubah
//! menjadi `$1, $2`, sedangkan untuk MySQL tanda tanya dinomori ulang
//! sesuai urutan kemunculan.

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm::sea_query::Value as SqValue;

/// Nilai SQL yang dipakai fungsi q_all, q_one, dan exec.
#[derive(Clone, Debug)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_string())
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v as i64)
    }
}

fn ramah(label: &str, e: impl std::fmt::Display) -> String {
    let t = e.to_string();
    let kecil = t.to_ascii_lowercase();
    if kecil.contains("duplicate key")
        || kecil.contains("duplicate entry")
        || kecil.contains("unique constraint")
        || kecil.contains("unique index")
    {
        return format!("gagal {label}: UNIQUE constraint gagal: {t}");
    }
    if kecil.contains("foreign key") {
        return format!("gagal {label}: FOREIGN KEY constraint gagal: {t}");
    }
    if kecil.contains("not-null constraint") || kecil.contains("cannot be null") {
        return format!("gagal {label}: NOT NULL constraint gagal: {t}");
    }
    format!("gagal {label}: {t}")
}

/// Jalankan SELECT dan kembalikan semua baris sebagai kolom bernilai `Value`.
pub async fn q_all<C: ConnectionTrait>(
    db: &C,
    sql: String,
    vals: Vec<Value>,
    ncols: usize,
    label: &str,
) -> Result<Vec<Vec<Value>>, String> {
    let stmt = siapkan(db, sql, vals);
    let rows = db
        .query_all_raw(stmt)
        .await
        .map_err(|e| ramah(label, e))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cells = Vec::with_capacity(ncols);
        for i in 0..ncols {
            cells.push(cell_of(&row, i));
        }
        out.push(cells);
    }
    Ok(out)
}

/// Jalankan SELECT dan kembalikan hanya baris pertama, bila ada.
pub async fn q_one<C: ConnectionTrait>(
    db: &C,
    sql: String,
    vals: Vec<Value>,
    ncols: usize,
    label: &str,
) -> Result<Option<Vec<Value>>, String> {
    let mut rows = q_all(db, sql, vals, ncols, label).await?;
    Ok(rows.pop())
}

/// Jalankan INSERT, UPDATE, atau DELETE; kembalikan jumlah baris terdampak.
pub async fn exec<C: ConnectionTrait>(
    db: &C,
    sql: String,
    vals: Vec<Value>,
    label: &str,
) -> Result<u64, String> {
    let stmt = siapkan(db, sql, vals);
    let res = db
        .execute_raw(stmt)
        .await
        .map_err(|e| ramah(label, e))?;
    Ok(res.rows_affected())
}

pub async fn exec_insert<C: ConnectionTrait>(
    db: &C,
    sql: String,
    vals: Vec<Value>,
    label: &str,
) -> Result<i64, String> {
    let row = match db.get_database_backend() {
        DbBackend::Postgres => {
            q_one(db, format!("{sql} RETURNING id"), vals, 1, label)
                .await?
                .ok_or_else(|| format!("gagal {label}: tidak ada id kembali"))?
        }
        DbBackend::MySql => {
            exec(db, sql, vals, label).await?;
            q_one(db, "SELECT LAST_INSERT_ID()".to_string(), vec![], 1, label)
                .await?
                .ok_or_else(|| format!("gagal {label}: tidak ada id kembali"))?
        }
        _ => {
            exec(db, sql, vals, label).await?;
            q_one(db, "SELECT last_insert_rowid()".to_string(), vec![], 1, label)
                .await?
                .ok_or_else(|| format!("gagal {label}: tidak ada id kembali"))?
        }
    };
    row.first()
        .and_then(value_i64)
        .ok_or_else(|| format!("gagal {label}: id tidak valid"))
}

/// Ubah placeholder dan parameter sesuai backend, lalu buat Statement.
fn siapkan<C: ConnectionTrait>(
    db: &C,
    sql: String,
    vals: Vec<Value>,
) -> Statement {
    let backend = db.get_database_backend();
    let (sql2, order) = terjemahkan(backend, &sql);
    let vals2: Vec<Value> = match order {
        Some(idx) => idx
            .iter()
            .map(|k| vals[(*k as usize).saturating_sub(1).min(vals.len() - 1)].clone())
            .collect(),
        None => vals,
    };
    let binds: Vec<SqValue> = vals2
        .iter()
        .map(|v| match v {
            Value::Null => SqValue::String(None),
            Value::Int(i) => SqValue::BigInt(Some(*i)),
            Value::Float(f) => SqValue::Double(Some(*f)),
            Value::Text(t) => SqValue::String(Some(t.clone())),
        })
        .collect();
    Statement::from_sql_and_values(backend, sql2, binds)
}

fn pola_cocok(bytes: &[char], i: usize, pola: &str) -> bool {
    let p: Vec<char> = pola.chars().collect();
    if i + p.len() > bytes.len() {
        return false;
    }
    bytes[i..i + p.len()]
        .iter()
        .zip(p.iter())
        .all(|(a, b)| a.to_ascii_uppercase() == b.to_ascii_uppercase())
}

/// Terjemahkan placeholder `?N` ke dialek backend.
///
/// SQLite dan backend lain: apa adanya. PostgreSQL: `?N` menjadi `$N`
/// (parameter boleh muncul berulang, urutan tidak berubah). MySQL: `?N`
/// menjadi `?` dan urutan parameter disesuaikan dengan kemunculan
/// placeholder. String literal dan komentar dilewati agar teks di dalamnya
/// tidak berubah.
fn terjemahkan(backend: DbBackend, sql: &str) -> (String, Option<Vec<u32>>) {
    match backend {
        DbBackend::Postgres | DbBackend::MySql => {
            let bytes: Vec<char> = sql.chars().collect();
            let mut out = String::with_capacity(sql.len());
            let mut order: Vec<u32> = Vec::new();
            let mut in_str = false;
            let mut di_awal = true;
            let mut tanpa_ganda = false;
            let mut i = 0usize;
            while i < bytes.len() {
                let c = bytes[i];
                if in_str {
                    out.push(c);
                    if c == '\'' {
                        // Kutip ganda '' adalah escape, bukan penutup.
                        if i + 1 < bytes.len() && bytes[i + 1] == '\'' {
                            out.push('\'');
                            i += 2;
                            continue;
                        }
                        in_str = false;
                    }
                    i += 1;
                    continue;
                }
                if c == '\'' {
                    in_str = true;
                    out.push(c);
                    i += 1;
                    continue;
                }
                if c == '-' && i + 1 < bytes.len() && bytes[i + 1] == '-' {
                    while i < bytes.len() && bytes[i] != '\n' {
                        out.push(bytes[i]);
                        i += 1;
                    }
                    continue;
                }
                if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == '*' {
                    out.push('/');
                    out.push('*');
                    i += 2;
                    while i < bytes.len() && !(bytes[i] == '*' && i + 1 < bytes.len() && bytes[i + 1] == '/') {
                        out.push(bytes[i]);
                        i += 1;
                    }
                    if i < bytes.len() {
                        out.push('*');
                        out.push('/');
                        i += 2;
                    }
                    continue;
                }
                if c.is_whitespace() && di_awal {
                    out.push(c);
                    i += 1;
                    continue;
                }
                if di_awal {
                    di_awal = false;
                    if pola_cocok(&bytes, i, "INSERT OR IGNORE INTO") {
                        if matches!(backend, DbBackend::Postgres) {
                            out.push_str("INSERT INTO");
                            tanpa_ganda = true;
                        } else {
                            out.push_str("INSERT IGNORE INTO");
                        }
                        i += "INSERT OR IGNORE INTO".len();
                        continue;
                    }
                }
                if pola_cocok(&bytes, i, "datetime('now','localtime')") {
                    if matches!(backend, DbBackend::Postgres) {
                        out.push_str("to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')");
                    } else {
                        out.push_str("NOW()");
                    }
                    i += "datetime('now','localtime')".len();
                    continue;
                }
                if pola_cocok(&bytes, i, "date('now','localtime')") {
                    if matches!(backend, DbBackend::Postgres) {
                        out.push_str("CURRENT_DATE");
                    } else {
                        out.push_str("CURDATE()");
                    }
                    i += "date('now','localtime')".len();
                    continue;
                }
                if c == '?'
                    && i + 1 < bytes.len()
                    && bytes[i + 1].is_ascii_digit()
                {
                    let mut num = String::new();
                    let mut j = i + 1;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        num.push(bytes[j]);
                        j += 1;
                    }
                    let n: u32 = num
                        .parse()
                        .unwrap_or_else(|_| panic!("placeholder tidak valid: ?{}", num));
                    match backend {
                        DbBackend::Postgres => {
                            out.push('$');
                            out.push_str(&num);
                        }
                        _ => {
                            out.push('?');
                            order.push(n);
                        }
                    }
                    i = j;
                    continue;
                }
                out.push(c);
                i += 1;
            }
            if tanpa_ganda && matches!(backend, DbBackend::Postgres) {
                out.push_str(" ON CONFLICT DO NOTHING");
            }
            match backend {
                DbBackend::Postgres => (out, None),
                _ => (out, Some(order)),
            }
        }
        _ => (sql.to_string(), None),
    }
}

/// Baca satu sel hasil query menjadi enum `Value` modul ini.
fn cell_of(row: &sea_orm::QueryResult, idx: usize) -> Value {
    if let Ok(Some(v)) = row.try_get_by::<Option<i64>, usize>(idx) {
        return Value::Int(v);
    }
    if let Ok(Some(v)) = row.try_get_by::<Option<f64>, usize>(idx) {
        return Value::Float(v);
    }
    if let Ok(Some(v)) = row.try_get_by::<Option<String>, usize>(idx) {
        return Value::Text(v);
    }
    Value::Null
}

/// Ambil teks dari hasil q_all (kosong bila NULL).
pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Value::Null => String::new(),
    }
}

/// Ambil angka bulat; NULL dan teks non-angka dianggap 0.
pub fn value_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) => Some(*i),
        Value::Text(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_postgres_dan_mysql() {
        let (pg, order_pg) = terjemahkan(
            DbBackend::Postgres,
            "SELECT * FROM t WHERE a = ?1 AND b = ?2",
        );
        assert_eq!(pg, "SELECT * FROM t WHERE a = $1 AND b = $2");
        assert!(order_pg.is_none());
        let (my, order_my) = terjemahkan(
            DbBackend::MySql,
            "SELECT * FROM t WHERE a = ?2 AND b = ?1",
        );
        assert_eq!(my, "SELECT * FROM t WHERE a = ? AND b = ?");
        assert_eq!(order_my, Some(vec![2, 1]));
    }

    #[test]
    fn literal_dan_komentar_tidak_disentuh() {
        let (pg, _) = terjemahkan(
            DbBackend::Postgres,
            "SELECT '?1' -- ?2\n/* ?3 */ FROM t WHERE a = ?1",
        );
        assert_eq!(pg, "SELECT '?1' -- ?2\n/* ?3 */ FROM t WHERE a = $1");
    }

    #[test]
    fn waktu_kini_dan_abaikan_ganda_per_dialek() {
        let (pg, _) = terjemahkan(
            DbBackend::Postgres,
            "INSERT OR IGNORE INTO t (a, b) VALUES (?1, datetime('now','localtime'))",
        );
        assert_eq!(
            pg,
            "INSERT INTO t (a, b) VALUES ($1, to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')) ON CONFLICT DO NOTHING"
        );
        let (my, order_my) = terjemahkan(
            DbBackend::MySql,
            "INSERT OR IGNORE INTO t (a, b) VALUES (?1, datetime('now','localtime'))",
        );
        assert_eq!(my, "INSERT IGNORE INTO t (a, b) VALUES (?, NOW())");
        assert_eq!(order_my, Some(vec![1]));
        let (lite, order_lite) = terjemahkan(
            DbBackend::Sqlite,
            "INSERT OR IGNORE INTO t (a) VALUES (?1)",
        );
        assert_eq!(lite, "INSERT OR IGNORE INTO t (a) VALUES (?1)");
        assert!(order_lite.is_none());
    }
}
