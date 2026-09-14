//! Pembantu query dinamis di atas koneksi SeaORM.
//!
//! Dipakai modul yang SQL-nya dirakit saat runtime dari konfigurasi
//! (misalnya satu engine untuk banyak entitas) sehingga tidak bisa memakai
//! metode entity statis. Query tetap jalan di pool SeaORM yang sama lewat
//! `get_sqlite_connection_pool`, bukan koneksi terpisah. Untuk query statis,
//! pakai entity SeaORM langsung.

use sea_orm::sqlx::{AssertSqlSafe, Row};

/// Nilai sel generik untuk engine dinamis.
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

fn pool_of(db: &sea_orm::DatabaseConnection) -> &sea_orm::sqlx::SqlitePool {
    db.get_sqlite_connection_pool()
}

/// Jalankan SQL mentah dan baca tiap baris sebagai daftar Value per indeks kolom.
pub async fn q_all(
    db: &sea_orm::DatabaseConnection,
    sql: String,
    vals: Vec<Value>,
    ncols: usize,
    label: &str,
) -> Result<Vec<Vec<Value>>, String> {
    let pool = pool_of(db);
    let mut q = sea_orm::sqlx::query(AssertSqlSafe(sql));
    for v in vals {
        q = match v {
            Value::Null => q.bind(None::<String>),
            Value::Int(i) => q.bind(i),
            Value::Float(f) => q.bind(f),
            Value::Text(s) => q.bind(s),
        };
    }
    let rows = q
        .fetch_all(pool)
        .await
        .map_err(|e| format!("gagal {label}: {e}"))?;
    let mut out = Vec::new();
    for r in rows {
        let mut v = Vec::with_capacity(ncols);
        for i in 0..ncols {
            v.push(cell_of(&r, i));
        }
        out.push(v);
    }
    Ok(out)
}

fn cell_of(row: &sea_orm::sqlx::sqlite::SqliteRow, i: usize) -> Value {
    if let Ok(Some(v)) = row.try_get::<Option<i64>, _>(i) {
        return Value::Int(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<f64>, _>(i) {
        return Value::Float(v);
    }
    if let Ok(Some(v)) = row.try_get::<Option<String>, _>(i) {
        return Value::Text(v);
    }
    Value::Null
}

pub async fn q_one(
    db: &sea_orm::DatabaseConnection,
    sql: String,
    vals: Vec<Value>,
    ncols: usize,
    label: &str,
) -> Result<Option<Vec<Value>>, String> {
    let mut rows = q_all(db, sql, vals, ncols, label).await?;
    Ok(rows.pop())
}

pub async fn exec(
    db: &sea_orm::DatabaseConnection,
    sql: String,
    vals: Vec<Value>,
    label: &str,
) -> Result<u64, String> {
    let pool = pool_of(db);
    let mut q = sea_orm::sqlx::query(AssertSqlSafe(sql));
    for v in vals {
        q = match v {
            Value::Null => q.bind(None::<String>),
            Value::Int(i) => q.bind(i),
            Value::Float(f) => q.bind(f),
            Value::Text(s) => q.bind(s),
        };
    }
    q.execute(pool)
        .await
        .map_err(|e| format!("gagal {label}: {e}"))
        .map(|r| r.rows_affected())
}

pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        Value::Text(s) => s.clone(),
    }
}

pub fn value_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) => Some(*i),
        Value::Text(s) => s.parse().ok(),
        _ => None,
    }
}
