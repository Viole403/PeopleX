//! Laporan: tabel siap tampil dan ekspor CSV.

use rusqlite::{params, Connection};

const FULL: &str = "TRIM(e.first_name || ' ' || COALESCE(e.last_name,''))";

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReportTable {
    pub title: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct DeptStat {
    pub department: String,
    pub employees: i32,
    pub open_vacancies: i32,
    pub active_trainings: i32,
    pub avg_performance: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ExportFile {
    pub filename: String,
    pub csv: String,
}

fn h(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v:.2}")
    }
}

fn valid_month(m: &str) -> bool {
    if m.len() != 7 {
        return false;
    }
    let b = m.as_bytes();
    if b[4] != b'-' {
        return false;
    }
    let y = &m[0..4];
    let mo = &m[5..7];
    match (y.parse::<i32>(), mo.parse::<u32>()) {
        (Ok(y), Ok(mo)) => (2000..=2100).contains(&y) && (1..=12).contains(&mo),
        _ => false,
    }
}

fn valid_date(d: &str) -> bool {
    chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok()
}

fn csv_escape(s: &str) -> String {
    if s.contains([';', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn to_csv(t: &ReportTable) -> String {
    let mut out = String::new();
    out.push_str(
        &t.headers
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(";"),
    );
    out.push('\n');
    for row in &t.rows {
        out.push_str(
            &row.iter()
                .map(|c| csv_escape(c))
                .collect::<Vec<_>>()
                .join(";"),
        );
        out.push('\n');
    }
    out
}

pub fn employees(
    conn: &Connection,
    search: Option<&str>,
    department_id: Option<i64>,
    status: Option<&str>,
) -> Result<ReportTable, String> {
    let mut sql = format!("SELECT e.employee_number, {FULL}, e.gender, d.name, p.name, e.employment_type, e.employment_status, e.join_date FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN positions p ON p.id = e.position_id WHERE e.deleted_at IS NULL");
    if search.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        sql.push_str(" AND (e.employee_number LIKE '%' || ? || '%' OR e.first_name LIKE '%' || ? || '%' OR e.last_name LIKE '%' || ? || '%')");
    }
    if department_id.is_some() {
        sql.push_str(" AND e.department_id = ?");
    }
    if status.map(|s| !s.trim().is_empty()).unwrap_or(false) {
        sql.push_str(" AND e.employment_status = ?");
    }
    sql.push_str(" ORDER BY e.first_name LIMIT 2000");
    let s = search.unwrap_or("").trim().to_string();
    let st = status.unwrap_or("").trim().to_string();
    let mut boxed: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if !s.is_empty() {
        boxed.push(Box::new(s.clone()));
        boxed.push(Box::new(s.clone()));
        boxed.push(Box::new(s));
    }
    if let Some(d) = department_id {
        boxed.push(Box::new(d));
    }
    if !st.is_empty() {
        boxed.push(Box::new(st));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = boxed.iter().map(|b| b.as_ref()).collect();
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("gagal menyiapkan laporan karyawan: {e}"))?;
    let rows = stmt
        .query_map(refs.as_slice(), |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "-".to_string()),
                r.get::<_, Option<String>>(4)?
                    .unwrap_or_else(|| "-".to_string()),
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
            ])
        })
        .map_err(|e| format!("gagal membaca laporan karyawan: {e}"))?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|e| format!("gagal membaca laporan: {e}"))?;
    Ok(ReportTable {
        title: "Laporan Karyawan".to_string(),
        headers: h(&[
            "NIP",
            "Nama",
            "Gender",
            "Departemen",
            "Jabatan",
            "Tipe",
            "Status",
            "Tgl Masuk",
        ]),
        rows,
    })
}

pub fn headcount(conn: &Connection) -> Result<ReportTable, String> {
    let mut stmt = conn
        .prepare("SELECT COALESCE(d.name,'(Tanpa departemen)'), SUM(CASE WHEN e.employment_type = 'permanent' THEN 1 ELSE 0 END), SUM(CASE WHEN e.employment_type != 'permanent' THEN 1 ELSE 0 END), COUNT(*) FROM employees e LEFT JOIN departments d ON d.id = e.department_id WHERE e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY d.name ORDER BY COUNT(*) DESC")
        .map_err(|e| format!("gagal menyiapkan headcount: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?.to_string(),
                r.get::<_, i64>(2)?.to_string(),
                r.get::<_, i64>(3)?.to_string(),
            ])
        })
        .map_err(|e| format!("gagal membaca headcount: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: "Headcount per Departemen".to_string(),
        headers: h(&["Departemen", "Tetap", "Non-Tetap", "Total"]),
        rows: out,
    })
}

fn att_row(r: &rusqlite::Row<'_>) -> Result<Vec<String>, rusqlite::Error> {
    Ok(vec![
        r.get::<_, String>(0)?,
        r.get::<_, String>(1)?,
        r.get::<_, String>(2)?,
        r.get::<_, i64>(3)?.to_string(),
        r.get::<_, i64>(4)?.to_string(),
        r.get::<_, i64>(5)?.to_string(),
    ])
}

pub fn attendance(
    conn: &Connection,
    month: &str,
    department_id: Option<i64>,
) -> Result<ReportTable, String> {
    if !valid_month(month) {
        return Err("Bulan tidak valid (format YYYY-MM).".to_string());
    }
    let like = format!("{month}%");
    let mut sql = format!("SELECT e.employee_number, {FULL}, COALESCE(d.name,'-'), SUM(CASE WHEN a.status IN ('present','wfh','business_trip') THEN 1 ELSE 0 END), SUM(CASE WHEN a.status = 'late' THEN 1 ELSE 0 END), COALESCE(SUM(a.work_minutes),0) FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN attendances a ON a.employee_id = e.id AND a.date LIKE ? WHERE e.deleted_at IS NULL");
    if department_id.is_some() {
        sql.push_str(" AND e.department_id = ?");
    }
    sql.push_str(" GROUP BY e.id ORDER BY e.first_name LIMIT 2000");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("gagal menyiapkan laporan absensi: {e}"))?;
    let rows = match department_id {
        Some(d) => stmt
            .query_map(params![like, d], att_row)
            .map_err(|e| format!("gagal membaca absensi: {e}"))?,
        None => stmt
            .query_map(params![like], att_row)
            .map_err(|e| format!("gagal membaca absensi: {e}"))?,
    };
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: format!("Laporan Absensi {month}"),
        headers: h(&[
            "NIP",
            "Nama",
            "Departemen",
            "Hadir",
            "Terlambat",
            "Menit Kerja",
        ]),
        rows: out,
    })
}

pub fn leave(conn: &Connection, year: i32) -> Result<ReportTable, String> {
    if !(2000..=2100).contains(&year) {
        return Err("Tahun tidak valid.".to_string());
    }
    let mut stmt = conn
        .prepare(&format!("SELECT e.employee_number, {FULL}, t.name, b.allocated_days, b.used_days FROM leave_balances b INNER JOIN employees e ON e.id = b.employee_id INNER JOIN leave_types t ON t.id = b.leave_type_id WHERE b.year = ?1 ORDER BY e.first_name, t.name LIMIT 2000"))
        .map_err(|e| format!("gagal menyiapkan laporan cuti: {e}"))?;
    let rows = stmt
        .query_map(params![year as i64], |r| {
            let alloc: f64 = r.get(3)?;
            let used: f64 = r.get(4)?;
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                num(alloc),
                num(used),
                num((alloc - used).max(0.0)),
            ])
        })
        .map_err(|e| format!("gagal membaca cuti: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: format!("Laporan Cuti {year}"),
        headers: h(&["NIP", "Nama", "Jenis Cuti", "Alokasi", "Terpakai", "Sisa"]),
        rows: out,
    })
}

pub fn payroll(conn: &Connection, period_id: i64) -> Result<ReportTable, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT e.employee_number, {FULL}, p.basic_salary, p.total_income, p.gross_salary, p.total_deduction, p.net_salary FROM payrolls p INNER JOIN employees e ON e.id = p.employee_id WHERE p.payroll_period_id = ?1 ORDER BY e.first_name LIMIT 2000"))
        .map_err(|e| format!("gagal menyiapkan laporan payroll: {e}"))?;
    let rows = stmt
        .query_map(params![period_id], |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                num(r.get::<_, f64>(2)?),
                num(r.get::<_, f64>(3)?),
                num(r.get::<_, f64>(4)?),
                num(r.get::<_, f64>(5)?),
                num(r.get::<_, f64>(6)?),
            ])
        })
        .map_err(|e| format!("gagal membaca payroll: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: "Laporan Payroll".to_string(),
        headers: h(&[
            "NIP",
            "Nama",
            "Gaji Pokok",
            "Total Pendapatan",
            "Bruto",
            "Potongan",
            "Bersih",
        ]),
        rows: out,
    })
}

pub fn recruitment(conn: &Connection) -> Result<ReportTable, String> {
    let mut stmt = conn
        .prepare("SELECT v.title, COALESCE(d.name,'-'), v.quota, COUNT(c.id), SUM(CASE WHEN c.stage IN ('interview','hr_interview','test') THEN 1 ELSE 0 END), SUM(CASE WHEN c.stage = 'hired' THEN 1 ELSE 0 END), SUM(CASE WHEN c.stage = 'rejected' THEN 1 ELSE 0 END) FROM vacancies v LEFT JOIN departments d ON d.id = v.department_id LEFT JOIN candidates c ON c.vacancy_id = v.id AND c.deleted_at IS NULL WHERE v.deleted_at IS NULL GROUP BY v.id ORDER BY v.id DESC LIMIT 500")
        .map_err(|e| format!("gagal menyiapkan laporan rekrutmen: {e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?.to_string(),
                r.get::<_, i64>(3)?.to_string(),
                r.get::<_, Option<i64>>(4)?.unwrap_or(0).to_string(),
                r.get::<_, Option<i64>>(5)?.unwrap_or(0).to_string(),
                r.get::<_, Option<i64>>(6)?.unwrap_or(0).to_string(),
            ])
        })
        .map_err(|e| format!("gagal membaca rekrutmen: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: "Laporan Rekrutmen".to_string(),
        headers: h(&[
            "Lowongan",
            "Departemen",
            "Kuota",
            "Pelamar",
            "Seleksi",
            "Direkrut",
            "Ditolak",
        ]),
        rows: out,
    })
}

pub fn performance(conn: &Connection, period_id: i64) -> Result<ReportTable, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT e.employee_number, {FULL}, r.final_score, r.status FROM performance_reviews r INNER JOIN employees e ON e.id = r.employee_id WHERE r.performance_period_id = ?1 ORDER BY r.final_score DESC LIMIT 2000"))
        .map_err(|e| format!("gagal menyiapkan laporan kinerja: {e}"))?;
    let rows = stmt
        .query_map(params![period_id], |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<f64>>(2)?
                    .map(num)
                    .unwrap_or_else(|| "-".to_string()),
                r.get::<_, String>(3)?,
            ])
        })
        .map_err(|e| format!("gagal membaca kinerja: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: "Laporan Kinerja".to_string(),
        headers: h(&["NIP", "Nama", "Skor Akhir", "Status"]),
        rows: out,
    })
}

pub fn contracts(conn: &Connection, before: &str) -> Result<ReportTable, String> {
    if !valid_date(before) {
        return Err("Tanggal tidak valid (format YYYY-MM-DD).".to_string());
    }
    let mut stmt = conn
        .prepare(&format!("SELECT c.contract_number, {FULL}, c.type, c.start_date, COALESCE(c.end_date,'-'), c.status FROM employee_contracts c INNER JOIN employees e ON e.id = c.employee_id WHERE c.deleted_at IS NULL AND (c.end_date IS NULL OR c.end_date <= ?1) ORDER BY c.end_date LIMIT 2000"))
        .map_err(|e| format!("gagal menyiapkan laporan kontrak: {e}"))?;
    let rows = stmt
        .query_map(params![before], |r| {
            Ok(vec![
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ])
        })
        .map_err(|e| format!("gagal membaca kontrak: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("gagal membaca baris: {e}"))?);
    }
    Ok(ReportTable {
        title: format!("Kontrak Berakhir s.d. {before}"),
        headers: h(&["No. Kontrak", "Nama", "Tipe", "Mulai", "Selesai", "Status"]),
        rows: out,
    })
}

pub fn analytics(conn: &Connection) -> Result<Vec<DeptStat>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.name FROM departments d WHERE d.deleted_at IS NULL ORDER BY d.name",
        )
        .map_err(|e| format!("gagal menyiapkan analitik: {e}"))?;
    let depts = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| format!("gagal membaca departemen: {e}"))?;
    let mut out = Vec::new();
    for row in depts {
        let (id, name) = row.map_err(|e| format!("gagal membaca baris: {e}"))?;
        let emp: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM employees WHERE department_id = ?1 AND deleted_at IS NULL AND employment_status IN ('active','probation')",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("gagal menghitung karyawan: {e}"))?;
        let vac: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM vacancies WHERE department_id = ?1 AND status = 'open' AND deleted_at IS NULL",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("gagal menghitung lowongan: {e}"))?;
        let avg: Option<f64> = conn
            .query_row(
                "SELECT AVG(r.final_score) FROM performance_reviews r INNER JOIN employees e ON e.id = r.employee_id WHERE e.department_id = ?1 AND r.final_score IS NOT NULL",
                params![id],
                |r| r.get(0),
            )
            .map_err(|e| format!("gagal menghitung rata-rata: {e}"))?;
        out.push(DeptStat {
            department: name,
            employees: crate::to_dto_int(emp, "report.emp")?,
            open_vacancies: crate::to_dto_int(vac, "report.vac")?,
            active_trainings: 0,
            avg_performance: avg,
        });
    }
    Ok(out)
}

pub fn export(
    conn: &Connection,
    kind: &str,
    arg1: Option<&str>,
    arg2: Option<i64>,
) -> Result<ExportFile, String> {
    let table = match kind {
        "employees" => employees(conn, None, None, None)?,
        "headcount" => headcount(conn)?,
        "attendance" => attendance(conn, arg1.unwrap_or(""), None)?,
        "leave" => leave(conn, arg2.unwrap_or(0) as i32)?,
        "payroll" => payroll(conn, arg2.unwrap_or(0))?,
        "recruitment" => recruitment(conn)?,
        "performance" => performance(conn, arg2.unwrap_or(0))?,
        "contracts" => contracts(conn, arg1.unwrap_or(""))?,
        _ => return Err("Jenis laporan tidak dikenal.".to_string()),
    };
    let today = chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%d")
        .to_string();
    Ok(ExportFile {
        filename: format!("laporan-{kind}-{today}.csv"),
        csv: to_csv(&table),
    })
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

    #[test]
    fn laporan_karyawan_dan_headcount() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let t = employees(&conn, None, None, None).expect("emp");
        assert!(!t.rows.is_empty());
        assert_eq!(t.headers.len(), 8);
        let h = headcount(&conn).expect("hc");
        assert!(!h.rows.is_empty());
        assert!(attendance(&conn, "20XX-13", None).is_err());
    }

    #[test]
    fn ekspor_csv_memakai_titik_koma() {
        let (_d, pool) = live();
        let conn = pool.get().expect("get");
        let f = export(&conn, "headcount", None, None).expect("csv");
        assert!(f.filename.starts_with("laporan-headcount-"));
        assert!(f.csv.contains(';'));
        assert!(export(&conn, "keliru", None, None).is_err());
    }
}
