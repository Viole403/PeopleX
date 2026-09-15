//! Laporan: tabel siap tampil dan ekspor CSV, XLSX, dan PDF.

use printpdf::{BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Pt};
use rust_xlsxwriter::{Format, Workbook};

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
    pub mime: String,
    pub bytes: Vec<u8>,
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

fn to_xlsx(t: &ReportTable) -> Result<Vec<u8>, String> {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet
        .set_name("Laporan")
        .map_err(|e| format!("gagal menyiapkan sheet: {e}"))?;
    let bold = Format::new().set_bold();
    for (c, h) in t.headers.iter().enumerate() {
        sheet
            .write_with_format(0, c as u16, h, &bold)
            .map_err(|e| format!("gagal menulis sel: {e}"))?;
    }
    for (r, row) in t.rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            sheet
                .write_string((r + 1) as u32, c as u16, cell)
                .map_err(|e| format!("gagal menulis sel: {e}"))?;
        }
    }
    for c in 0..t.headers.len() as u16 {
        sheet
            .set_column_width(c, 24)
            .map_err(|e| format!("gagal mengatur lebar kolom: {e}"))?;
    }
    book.save_to_buffer()
        .map_err(|e| format!("gagal menyusun XLSX: {e}"))
}

fn pdf_line(ops: &mut Vec<Op>, text: &str) {
    let mut s: String = text.chars().take(104).collect();
    if s.len() < text.len() {
        s.push_str("...");
    }
    ops.push(Op::ShowText {
        items: vec![printpdf::TextItem::Text(s)],
    });
    ops.push(Op::AddLineBreak);
}

fn to_pdf(t: &ReportTable, when: &str) -> Result<Vec<u8>, String> {
    let head = || {
        vec![
            Op::StartTextSection,
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
                size: Pt(13.0),
            },
            Op::SetLineHeight { lh: Pt(16.0) },
            Op::ShowText {
                items: vec![printpdf::TextItem::Text(t.title.clone())],
            },
            Op::AddLineBreak,
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
                size: Pt(9.0),
            },
            Op::SetLineHeight { lh: Pt(12.0) },
            Op::ShowText {
                items: vec![printpdf::TextItem::Text(format!("Diunduh {when}"))],
            },
            Op::AddLineBreak,
            Op::SetFont {
                font: PdfFontHandle::Builtin(BuiltinFont::CourierBold),
                size: Pt(8.0),
            },
            Op::SetLineHeight { lh: Pt(11.0) },
        ]
    };
    let mut pages: Vec<PdfPage> = Vec::new();
    let mut ops = head();
    let header_line = t.headers.join(" | ");
    pdf_line(&mut ops, &header_line);
    let mut count = 0usize;
    for row in &t.rows {
        if count >= 40 {
            ops.push(Op::EndTextSection);
            pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
            ops = head();
            pdf_line(&mut ops, &header_line);
            count = 0;
        }
        pdf_line(&mut ops, &row.join(" | "));
        count += 1;
    }
    if t.rows.is_empty() {
        pdf_line(&mut ops, "Tidak ada data.");
    }
    ops.push(Op::EndTextSection);
    pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
    let mut warnings = Vec::new();
    let bytes = PdfDocument::new(&t.title)
        .with_pages(pages)
        .save(&PdfSaveOptions::default(), &mut warnings);
    if bytes.is_empty() {
        return Err("Gagal merender PDF.".to_string());
    }
    Ok(bytes)
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{q_all, q_one, value_i64, value_to_string, Value};

const FULL_SEA: &str = "TRIM(e.first_name || ' ' || COALESCE(e.last_name,''))";

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReportField {
    pub id: String,
    pub label: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct ReportFilter {
    pub field: String,
    pub op: String,
    pub value: String,
}

fn custom_fields() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("nip", "NIP", "e.employee_number"),
        ("nama", "Nama", FULL_SEA),
        ("gender", "Gender", "e.gender"),
        ("departemen", "Departemen", "d.name"),
        ("jabatan", "Jabatan", "p.name"),
        ("tipe", "Tipe", "e.employment_type"),
        ("status", "Status", "e.employment_status"),
        ("tgl_masuk", "Tgl Masuk", "e.join_date"),
        ("email", "Email", "e.personal_email"),
        ("telepon", "Telepon", "e.phone"),
        ("lokasi", "Lokasi", "wl.name"),
        ("nik", "NIK", "e.nik"),
    ]
}

pub async fn custom_fields_sea() -> Result<Vec<ReportField>, String> {
    Ok(custom_fields()
        .iter()
        .map(|(id, label, _)| ReportField {
            id: id.to_string(),
            label: label.to_string(),
        })
        .collect())
}

pub async fn custom_sea(
    db: &sea_orm::DatabaseConnection,
    fields: Vec<String>,
    filters: Vec<ReportFilter>,
) -> Result<ReportTable, String> {
    let defs = custom_fields();
    let mut cols: Vec<(&str, &str, &str)> = Vec::new();
    for f in &fields {
        match defs.iter().find(|(id, _, _)| id == f) {
            Some((id, label, expr)) => cols.push((id, label, expr)),
            None => return Err(format!("Field tidak dikenal: {f}.")),
        }
    }
    if cols.is_empty() {
        return Err("Pilih minimal satu field.".to_string());
    }
    if cols.len() > 12 {
        return Err("Maksimal 12 field.".to_string());
    }
    let select = cols
        .iter()
        .map(|(_, _, expr)| *expr)
        .collect::<Vec<_>>()
        .join(", ");
    let mut sql = format!("SELECT {select} FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN positions p ON p.id = e.position_id LEFT JOIN work_locations wl ON wl.id = e.work_location_id WHERE e.deleted_at IS NULL");
    let mut vals: Vec<Value> = Vec::new();
    let mut n = 0usize;
    for flt in &filters {
        let expr = match defs.iter().find(|(id, _, _)| id == &flt.field) {
            Some((_, _, expr)) => *expr,
            None => return Err(format!("Field filter tidak dikenal: {}.", flt.field)),
        };
        n += 1;
        let ph = format!("?{n}");
        match flt.op.as_str() {
            "eq" => {
                sql.push_str(&format!(" AND {expr} = {ph}"));
                vals.push(Value::Text(flt.value.clone()));
            }
            "like" => {
                sql.push_str(&format!(" AND {expr} LIKE '%' || {ph} || '%'"));
                vals.push(Value::Text(flt.value.clone()));
            }
            "gte" => {
                sql.push_str(&format!(" AND {expr} >= {ph}"));
                vals.push(Value::Text(flt.value.clone()));
            }
            "lte" => {
                sql.push_str(&format!(" AND {expr} <= {ph}"));
                vals.push(Value::Text(flt.value.clone()));
            }
            _ => return Err(format!("Operator tidak dikenal: {}.", flt.op)),
        }
    }
    sql.push_str(" ORDER BY e.first_name LIMIT 2000");
    let rows = q_all(db, sql, vals, cols.len(), "report.custom")
        .await
        .map_err(|e| format!("gagal membaca laporan kustom: {e}"))?;
    let headers = cols.iter().map(|(_, label, _)| label.to_string()).collect();
    let out = rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|v| match v {
                    Value::Null => "-".to_string(),
                    _ => value_to_string(v),
                })
                .collect()
        })
        .collect();
    Ok(ReportTable {
        title: "Laporan Kustom".to_string(),
        headers,
        rows: out,
    })
}

fn rnum(v: &Value) -> String {
    num(match v {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        Value::Text(s) => s.parse().unwrap_or(0.0),
        Value::Null => 0.0,
    })
}

fn rf64(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        Value::Text(s) => s.parse().unwrap_or(0.0),
        Value::Null => 0.0,
    }
}

pub async fn employees_sea(
    db: &sea_orm::DatabaseConnection,
    search: Option<&str>,
    department_id: Option<i64>,
    status: Option<&str>,
) -> Result<ReportTable, String> {
    let mut sql = format!("SELECT e.employee_number, {FULL_SEA}, e.gender, d.name, p.name, e.employment_type, e.employment_status, e.join_date FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN positions p ON p.id = e.position_id WHERE e.deleted_at IS NULL");
    let s = search.unwrap_or("").trim().to_string();
    let st = status.unwrap_or("").trim().to_string();
    let mut vals: Vec<Value> = Vec::new();
    if !s.is_empty() {
        sql.push_str(" AND (e.employee_number LIKE '%' || ? || '%' OR e.first_name LIKE '%' || ? || '%' OR e.last_name LIKE '%' || ? || '%')");
        vals.push(Value::Text(s.clone()));
        vals.push(Value::Text(s.clone()));
        vals.push(Value::Text(s));
    }
    if let Some(d) = department_id {
        sql.push_str(" AND e.department_id = ?");
        vals.push(Value::Int(d));
    }
    if !st.is_empty() {
        sql.push_str(" AND e.employment_status = ?");
        vals.push(Value::Text(st));
    }
    sql.push_str(" ORDER BY e.first_name LIMIT 2000");
    let ncols = 8;
    let rows = q_all(db, sql, vals, ncols, "report.employees")
        .await
        .map_err(|e| format!("gagal membaca laporan karyawan: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            match &r[3] {
                Value::Null => "-".to_string(),
                _ => value_to_string(&r[3]),
            },
            match &r[4] {
                Value::Null => "-".to_string(),
                _ => value_to_string(&r[4]),
            },
            value_to_string(&r[5]),
            value_to_string(&r[6]),
            value_to_string(&r[7]),
        ]);
    }
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
        rows: out,
    })
}

pub async fn headcount_sea(db: &sea_orm::DatabaseConnection) -> Result<ReportTable, String> {
    let rows = q_all(
        db,
        "SELECT COALESCE(d.name,'(Tanpa departemen)'), SUM(CASE WHEN e.employment_type = 'permanent' THEN 1 ELSE 0 END), SUM(CASE WHEN e.employment_type != 'permanent' THEN 1 ELSE 0 END), COUNT(*) FROM employees e LEFT JOIN departments d ON d.id = e.department_id WHERE e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY d.name ORDER BY COUNT(*) DESC".to_string(),
        vec![],
        4,
        "report.headcount",
    )
    .await
    .map_err(|e| format!("gagal membaca headcount: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            value_to_string(&r[3]),
        ]);
    }
    Ok(ReportTable {
        title: "Headcount per Departemen".to_string(),
        headers: h(&["Departemen", "Tetap", "Non-Tetap", "Total"]),
        rows: out,
    })
}

pub async fn attendance_sea(
    db: &sea_orm::DatabaseConnection,
    month: &str,
    department_id: Option<i64>,
) -> Result<ReportTable, String> {
    if !valid_month(month) {
        return Err("Bulan tidak valid (format YYYY-MM).".to_string());
    }
    let like = format!("{month}%");
    let mut sql = format!("SELECT e.employee_number, {FULL_SEA}, COALESCE(d.name,'-'), SUM(CASE WHEN a.status IN ('present','wfh','business_trip') THEN 1 ELSE 0 END), SUM(CASE WHEN a.status = 'late' THEN 1 ELSE 0 END), COALESCE(SUM(a.work_minutes),0) FROM employees e LEFT JOIN departments d ON d.id = e.department_id LEFT JOIN attendances a ON a.employee_id = e.id AND a.date LIKE ? WHERE e.deleted_at IS NULL");
    let mut vals = vec![Value::Text(like)];
    if let Some(d) = department_id {
        sql.push_str(" AND e.department_id = ?");
        vals.push(Value::Int(d));
    }
    sql.push_str(" GROUP BY e.id ORDER BY e.first_name LIMIT 2000");
    let rows = q_all(db, sql, vals, 6, "report.attendance")
        .await
        .map_err(|e| format!("gagal membaca absensi: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            value_to_string(&r[3]),
            value_to_string(&r[4]),
            value_to_string(&r[5]),
        ]);
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

pub async fn leave_sea(
    db: &sea_orm::DatabaseConnection,
    year: i32,
) -> Result<ReportTable, String> {
    if !(2000..=2100).contains(&year) {
        return Err("Tahun tidak valid.".to_string());
    }
    let rows = q_all(
        db,
        format!("SELECT e.employee_number, {FULL_SEA}, t.name, b.allocated_days, b.used_days FROM leave_balances b INNER JOIN employees e ON e.id = b.employee_id INNER JOIN leave_types t ON t.id = b.leave_type_id WHERE b.year = ?1 ORDER BY e.first_name, t.name LIMIT 2000"),
        vec![Value::Int(year as i64)],
        5,
        "report.leave",
    )
    .await
    .map_err(|e| format!("gagal membaca cuti: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let (alloc, used) = (rf64(&r[3]), rf64(&r[4]));
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            num(alloc),
            num(used),
            num((alloc - used).max(0.0)),
        ]);
    }
    Ok(ReportTable {
        title: format!("Laporan Cuti {year}"),
        headers: h(&["NIP", "Nama", "Jenis Cuti", "Alokasi", "Terpakai", "Sisa"]),
        rows: out,
    })
}

pub async fn payroll_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
) -> Result<ReportTable, String> {
    let rows = q_all(
        db,
        format!("SELECT e.employee_number, {FULL_SEA}, p.basic_salary, p.total_income, p.gross_salary, p.total_deduction, p.net_salary FROM payrolls p INNER JOIN employees e ON e.id = p.employee_id WHERE p.payroll_period_id = ?1 ORDER BY e.first_name LIMIT 2000"),
        vec![Value::Int(period_id)],
        7,
        "report.payroll",
    )
    .await
    .map_err(|e| format!("gagal membaca payroll: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            rnum(&r[2]),
            rnum(&r[3]),
            rnum(&r[4]),
            rnum(&r[5]),
            rnum(&r[6]),
        ]);
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

/// Rekap PPh 21 setahun per karyawan (bahan form 1721-A1):
/// bruto dari payroll periode final, PPh dari rincian estimasi.
pub async fn pph21_annual_sea(
    db: &sea_orm::DatabaseConnection,
    year: i32,
) -> Result<ReportTable, String> {
    if !(2000..=2100).contains(&year) {
        return Err("Tahun harus 2000 sampai 2100.".to_string());
    }
    let y = year.to_string();
    let rows = q_all(
        db,
        format!("SELECT e.employee_number, {FULL_SEA}, COALESCE(e.npwp,'-'), COALESCE(e.ptkp_status,'TK/0'), SUM(p.total_income), COALESCE((SELECT SUM(d.amount) FROM payroll_details d INNER JOIN payrolls q ON q.id = d.payroll_id INNER JOIN payroll_periods qp ON qp.id = q.payroll_period_id WHERE q.employee_id = e.id AND d.component_name = 'PPh 21 (estimasi)' AND substr(qp.start_date,1,4) = ?1 AND qp.status IN ('approved','paid','locked')), 0) FROM payrolls p INNER JOIN employees e ON e.id = p.employee_id INNER JOIN payroll_periods pp ON pp.id = p.payroll_period_id WHERE substr(pp.start_date,1,4) = ?1 AND pp.status IN ('approved','paid','locked') GROUP BY e.id ORDER BY e.first_name LIMIT 2000"),
        vec![Value::Text(y)],
        6,
        "report.pph21",
    )
    .await
    .map_err(|e| format!("gagal membaca PPh tahunan: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        let bruto = rf64(&r[4]);
        let dipotong = rf64(&r[5]);
        let ptkp_status = value_to_string(&r[3]);
        let terutang =
            crate::services::payroll::pph21_monthly(bruto / 12.0, &ptkp_status) * 12.0;
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            ptkp_status,
            num(bruto),
            num(dipotong),
            num(terutang),
            num(terutang - dipotong),
        ]);
    }
    Ok(ReportTable {
        title: "Laporan PPh 21 Tahunan".to_string(),
        headers: h(&[
            "NIP",
            "Nama",
            "NPWP",
            "PTKP",
            "Bruto Setahun",
            "PPh 21 Setahun",
            "PPh Terutang Setahun",
            "Selisih",
        ]),
        rows: out,
    })
}

pub async fn recruitment_sea(db: &sea_orm::DatabaseConnection) -> Result<ReportTable, String> {
    let rows = q_all(
        db,
        "SELECT v.title, COALESCE(d.name,'-'), v.quota, COUNT(c.id), SUM(CASE WHEN c.stage IN ('interview','hr_interview','test') THEN 1 ELSE 0 END), SUM(CASE WHEN c.stage = 'hired' THEN 1 ELSE 0 END), SUM(CASE WHEN c.stage = 'rejected' THEN 1 ELSE 0 END) FROM vacancies v LEFT JOIN departments d ON d.id = v.department_id LEFT JOIN candidates c ON c.vacancy_id = v.id AND c.deleted_at IS NULL WHERE v.deleted_at IS NULL GROUP BY v.id ORDER BY v.id DESC LIMIT 500".to_string(),
        vec![],
        7,
        "report.recruitment",
    )
    .await
    .map_err(|e| format!("gagal membaca rekrutmen: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            value_to_string(&r[3]),
            value_to_string(&r[4]),
            value_to_string(&r[5]),
            value_to_string(&r[6]),
        ]);
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

pub async fn performance_sea(
    db: &sea_orm::DatabaseConnection,
    period_id: i64,
) -> Result<ReportTable, String> {
    let rows = q_all(
        db,
        format!("SELECT e.employee_number, {FULL_SEA}, r.final_score, r.status FROM performance_reviews r INNER JOIN employees e ON e.id = r.employee_id WHERE r.performance_period_id = ?1 ORDER BY r.final_score DESC LIMIT 2000"),
        vec![Value::Int(period_id)],
        4,
        "report.performance",
    )
    .await
    .map_err(|e| format!("gagal membaca kinerja: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            match &r[2] {
                Value::Null => "-".to_string(),
                _ => rnum(&r[2]),
            },
            value_to_string(&r[3]),
        ]);
    }
    Ok(ReportTable {
        title: "Laporan Kinerja".to_string(),
        headers: h(&["NIP", "Nama", "Skor Akhir", "Status"]),
        rows: out,
    })
}

pub async fn contracts_sea(
    db: &sea_orm::DatabaseConnection,
    before: &str,
) -> Result<ReportTable, String> {
    if !valid_date(before) {
        return Err("Tanggal tidak valid (format YYYY-MM-DD).".to_string());
    }
    let rows = q_all(
        db,
        format!("SELECT c.contract_number, {FULL_SEA}, c.type, c.start_date, COALESCE(c.end_date,'-'), c.status FROM employee_contracts c INNER JOIN employees e ON e.id = c.employee_id WHERE c.deleted_at IS NULL AND (c.end_date IS NULL OR c.end_date <= ?1) ORDER BY c.end_date LIMIT 2000"),
        vec![Value::Text(before.to_string())],
        6,
        "report.contracts",
    )
    .await
    .map_err(|e| format!("gagal membaca kontrak: {e}"))?;
    let mut out = Vec::new();
    for r in &rows {
        out.push(vec![
            value_to_string(&r[0]),
            value_to_string(&r[1]),
            value_to_string(&r[2]),
            value_to_string(&r[3]),
            value_to_string(&r[4]),
            value_to_string(&r[5]),
        ]);
    }
    Ok(ReportTable {
        title: format!("Kontrak Berakhir s.d. {before}"),
        headers: h(&["No. Kontrak", "Nama", "Tipe", "Mulai", "Selesai", "Status"]),
        rows: out,
    })
}

pub async fn analytics_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<DeptStat>, String> {
    let depts = q_all(
        db,
        "SELECT d.id, d.name FROM departments d WHERE d.deleted_at IS NULL ORDER BY d.name".to_string(),
        vec![],
        2,
        "report.depts",
    )
    .await
    .map_err(|e| format!("gagal membaca departemen: {e}"))?;
    let mut out = Vec::new();
    for d in &depts {
        let id = value_i64(&d[0]).unwrap_or(0);
        let emp = q_one(
            db,
            "SELECT COUNT(*) FROM employees WHERE department_id = ?1 AND deleted_at IS NULL AND employment_status IN ('active','probation')".to_string(),
            vec![Value::Int(id)],
            1,
            "report.emp",
        )
        .await
        .map_err(|e| format!("gagal menghitung karyawan: {e}"))?;
        let vac = q_one(
            db,
            "SELECT COUNT(*) FROM vacancies WHERE department_id = ?1 AND status = 'open' AND deleted_at IS NULL".to_string(),
            vec![Value::Int(id)],
            1,
            "report.vac",
        )
        .await
        .map_err(|e| format!("gagal menghitung lowongan: {e}"))?;
        let avg_row = q_one(
            db,
            "SELECT AVG(r.final_score) FROM performance_reviews r INNER JOIN employees e ON e.id = r.employee_id WHERE e.department_id = ?1 AND r.final_score IS NOT NULL".to_string(),
            vec![Value::Int(id)],
            1,
            "report.avg",
        )
        .await
        .map_err(|e| format!("gagal menghitung rata-rata: {e}"))?;
        out.push(DeptStat {
            department: value_to_string(&d[1]),
            employees: crate::to_dto_int(
                emp.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0),
                "report.emp",
            )?,
            open_vacancies: crate::to_dto_int(
                vac.as_ref().and_then(|r| value_i64(&r[0])).unwrap_or(0),
                "report.vac",
            )?,
            active_trainings: 0,
            avg_performance: avg_row.as_ref().and_then(|r| match &r[0] {
                Value::Float(f) => Some(*f),
                Value::Int(i) => Some(*i as f64),
                _ => None,
            }),
        });
    }
    Ok(out)
}

pub async fn export_sea(
    db: &sea_orm::DatabaseConnection,
    kind: &str,
    format: &str,
    arg1: Option<&str>,
    arg2: Option<i64>,
) -> Result<ExportFile, String> {
    let table = match kind {
        "employees" => employees_sea(db, None, None, None).await?,
        "headcount" => headcount_sea(db).await?,
        "attendance" => attendance_sea(db, arg1.unwrap_or(""), None).await?,
        "leave" => leave_sea(db, arg2.unwrap_or(0) as i32).await?,
        "payroll" => payroll_sea(db, arg2.unwrap_or(0)).await?,
        "recruitment" => recruitment_sea(db).await?,
        "performance" => performance_sea(db, arg2.unwrap_or(0)).await?,
        "contracts" => contracts_sea(db, arg1.unwrap_or("")).await?,
        "pph21annual" => pph21_annual_sea(db, arg2.unwrap_or(0) as i32).await?,
        _ => return Err("Jenis laporan tidak dikenal.".to_string()),
    };
    let today = chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%d")
        .to_string();
    let (ext, mime, bytes) = match format {
        "csv" => (
            "csv",
            "text/csv;charset=utf-8".to_string(),
            to_csv(&table).into_bytes(),
        ),
        "xlsx" => (
            "xlsx",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            to_xlsx(&table)?,
        ),
        "pdf" => (
            "pdf",
            "application/pdf".to_string(),
            to_pdf(&table, &today)?,
        ),
        _ => return Err("Format ekspor tidak dikenal.".to_string()),
    };
    Ok(ExportFile {
        filename: format!("laporan-{kind}-{today}.{ext}"),
        mime,
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::payroll as pay;
    use crate::services::sea_raw::exec;

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "test.admin",
        )
        .await
        .expect("admin")
        .and_then(|r| value_i64(&r[0]))
        .expect("admin id")
    }

    async fn seed_pph_employee(db: &sea_orm::DatabaseConnection) {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type, ptkp_status) VALUES ('EMP-PPH', 'Pajak', 'male', 'single', 1, '2026-01-01', 'active', 'permanent', 'TK/0')".to_string(),
            vec![],
            "test.mkemp",
        )
        .await
        .expect("emp");
        let eid = q_one(
            db,
            "SELECT last_insert_rowid()".to_string(),
            vec![],
            1,
            "test.rowid",
        )
        .await
        .expect("rowid")
        .and_then(|r| value_i64(&r[0]))
        .expect("eid");
        exec(
            db,
            "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, 20000000, '2026-01-01', 1)".to_string(),
            vec![Value::Int(eid)],
            "test.mksal",
        )
        .await
        .expect("sal");
    }

    async fn approve_jan_2026(db: &sea_orm::DatabaseConnection) {
        let actor = admin_id(db).await;
        let pid = pay::period_create_sea(
            db,
            actor,
            actor,
            &pay::PeriodInput {
                name: "Jan 2026".to_string(),
                start_date: "2026-01-01".to_string(),
                end_date: "2026-01-31".to_string(),
                payment_date: None,
            },
        )
        .await
        .expect("periode") as i64;
        pay::generate_sea(db, actor, pid).await.expect("generate");
        pay::approve_period_sea(db, actor, pid)
            .await
            .expect("approve");
    }

    #[tokio::test]
    async fn laporan_karyawan_dan_headcount_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let t = employees_sea(db, None, None, None).await.expect("emp");
        assert!(!t.rows.is_empty());
        assert_eq!(t.headers.len(), 8);
        let hc = headcount_sea(db).await.expect("hc");
        assert!(!hc.rows.is_empty());
        assert!(attendance_sea(db, "20XX-13", None).await.is_err());
        let a = attendance_sea(db, "2026-03", None).await.expect("abs");
        assert_eq!(a.headers.len(), 6);
        assert!(!a.rows.is_empty());
        let r = recruitment_sea(db).await.expect("rec");
        assert_eq!(r.headers.len(), 7);
        assert_eq!(r.title, "Laporan Rekrutmen");
        let n = analytics_sea(db).await.expect("ana");
        assert!(!n.is_empty());
        assert!(n.iter().all(|s| s.active_trainings == 0));
    }

    #[tokio::test]
    async fn pph21_tahunan_merangkum_periode_final_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        seed_pph_employee(db).await;
        approve_jan_2026(db).await;
        let t = pph21_annual_sea(db, 2026).await.expect("tahunan");
        assert_eq!(t.headers.len(), 8);
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0][4], "20000000");
        let terutang: f64 = t.rows[0][6].parse().unwrap();
        let dipotong: f64 = t.rows[0][5].parse().unwrap();
        let selisih: f64 = t.rows[0][7].parse().unwrap();
        assert_eq!(selisih, terutang - dipotong);
        assert!(pph21_annual_sea(db, 1999).await.is_err());
        let csv = export_sea(db, "pph21annual", "csv", None, Some(2026))
            .await
            .expect("csv tahunan");
        let text = String::from_utf8(csv.bytes).expect("utf8");
        assert!(text.contains("Bruto Setahun"));
        let xlsx = export_sea(db, "pph21annual", "xlsx", None, Some(2026))
            .await
            .expect("xlsx tahunan");
        assert_eq!(&xlsx.bytes[0..2], b"PK");
        let pdf = export_sea(db, "pph21annual", "pdf", None, Some(2026))
            .await
            .expect("pdf tahunan");
        assert!(pdf.bytes.starts_with(b"%PDF"));
    }

    #[tokio::test]
    async fn builder_kustom_field_dan_filter_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let fields = custom_fields_sea().await.expect("daftar field");
        assert!(fields.iter().any(|f| f.id == "nama"));
        let t = custom_sea(
            db,
            vec!["nip".to_string(), "nama".to_string(), "status".to_string()],
            vec![],
        )
        .await
        .expect("kustom");
        assert_eq!(t.headers, vec!["NIP", "Nama", "Status"]);
        assert!(!t.rows.is_empty());
        let saring = custom_sea(
            db,
            vec!["nip".to_string(), "nama".to_string()],
            vec![ReportFilter {
                field: "nip".to_string(),
                op: "eq".to_string(),
                value: "EMP-0001".to_string(),
            }],
        )
        .await
        .expect("saring eq");
        assert_eq!(saring.rows.len(), 1);
        assert_eq!(saring.rows[0][0], "EMP-0001");
        let cari = custom_sea(
            db,
            vec!["nama".to_string()],
            vec![ReportFilter {
                field: "nama".to_string(),
                op: "like".to_string(),
                value: "Super".to_string(),
            }],
        )
        .await
        .expect("saring like");
        assert_eq!(cari.rows.len(), 1);
        assert!(custom_sea(db, vec![], vec![]).await.is_err());
        assert!(custom_sea(db, vec!["ngawur".to_string()], vec![])
            .await
            .is_err());
        assert!(custom_sea(
            db,
            vec!["nama".to_string()],
            vec![ReportFilter {
                field: "nama".to_string(),
                op: "ngawur".to_string(),
                value: "x".to_string(),
            }],
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn ekspor_csv_xlsx_pdf_valid_sea() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let f = export_sea(db, "headcount", "csv", None, None)
            .await
            .expect("csv");
        assert!(f.filename.starts_with("laporan-headcount-"));
        assert!(f.filename.ends_with(".csv"));
        let text = String::from_utf8(f.bytes).expect("utf8");
        assert!(text.contains(';'));
        let x = export_sea(db, "headcount", "xlsx", None, None)
            .await
            .expect("xlsx");
        assert!(x.filename.ends_with(".xlsx"));
        assert_eq!(&x.bytes[0..2], b"PK");
        let p = export_sea(db, "headcount", "pdf", None, None)
            .await
            .expect("pdf");
        assert!(p.filename.ends_with(".pdf"));
        assert!(p.bytes.starts_with(b"%PDF"));
        assert!(export_sea(db, "keliru", "csv", None, None)
            .await
            .is_err());
        assert!(export_sea(db, "headcount", "keliru", None, None)
            .await
            .is_err());
    }
}
