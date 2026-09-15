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
        .map_err(|e| format!("gagal {}: {}", label, e))?;
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
        .map_err(|e| format!("gagal {}: {}", label, e))?;
    Ok(res.rows_affected())
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
                if c == '?' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
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
