//! Slip gaji PDF (A4, font bawaan, tanpa aset eksternal).

use printpdf::{
    BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Pt, TextItem,
};
use std::path::Path;

use super::payroll;

/// Berkas slip untuk unduh.
#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PayslipFile {
    pub mime: String,
    pub name: String,
    pub bytes: Vec<u8>,
}

fn rupiah(v: f64) -> String {
    let n = v.round() as i64;
    let neg = n < 0;
    let mut s = n.abs().to_string();
    let mut out = String::new();
    while s.len() > 3 {
        out = format!(".{}{}", &s[s.len() - 3..], out);
        s.truncate(s.len() - 3);
    }
    out = format!("{s}{out}");
    format!("{}Rp {out}", if neg { "-" } else { "" })
}

fn line(ops: &mut Vec<Op>, label: &str, value: &str) {
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(format!("{label:<28} {value:>22}"))],
    });
    ops.push(Op::AddLineBreak);
}

fn blank(ops: &mut Vec<Op>) {
    ops.push(Op::AddLineBreak);
}

// ---------------- Varian SeaORM ----------------

use super::sea_raw::{exec, q_one, value_i64, value_to_string, Value};

/// Render slip untuk satu payroll, simpan ke exports, kembalikan path relatif.
pub async fn render_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    payroll_id: i64,
) -> Result<String, String> {
    let det = payroll::payroll_detail_sea(db, payroll_id)
        .await?
        .ok_or("Payroll tidak ditemukan.".to_string())?;
    let slip = q_one(
        db,
        "SELECT id, payslip_number FROM payslips WHERE payroll_id = ?1".to_string(),
        vec![Value::Int(payroll_id)],
        2,
        "payslip.slip",
    )
    .await
    .map_err(|e| format!("gagal memuat slip: {e}"))?;
    let Some(slip) = slip else {
        return Err("Slip belum tersedia untuk payroll ini.".to_string());
    };
    let (slip_id, number) = (value_i64(&slip[0]).unwrap_or(0), value_to_string(&slip[1]));
    let company_row = q_one(
        db,
        "SELECT name FROM companies ORDER BY id LIMIT 1".to_string(),
        vec![],
        1,
        "payslip.company",
    )
    .await
    .map_err(|e| format!("gagal memuat perusahaan: {e}"))?;
    let company = company_row
        .as_ref()
        .map(|r| value_to_string(&r[0]))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Perusahaan".to_string());

    let mut ops = vec![
        Op::StartTextSection,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::HelveticaBold),
            size: Pt(16.0),
        },
        Op::SetLineHeight { lh: Pt(20.0) },
        Op::ShowText {
            items: vec![TextItem::Text(company.clone())],
        },
        Op::AddLineBreak,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Helvetica),
            size: Pt(12.0),
        },
        Op::SetLineHeight { lh: Pt(15.0) },
        Op::ShowText {
            items: vec![TextItem::Text("SLIP GAJI".to_string())],
        },
        Op::AddLineBreak,
        Op::SetFont {
            font: PdfFontHandle::Builtin(BuiltinFont::Courier),
            size: Pt(10.0),
        },
        Op::SetLineHeight { lh: Pt(13.0) },
    ];
    line(&mut ops, "Nomor", &number);
    line(
        &mut ops,
        "Periode",
        &format!(
            "{} ({} s.d. {})",
            det.period_name, det.start_date, det.end_date
        ),
    );
    blank(&mut ops);
    line(
        &mut ops,
        "Karyawan",
        &format!("{} - {}", det.employee_number, det.name),
    );
    line(
        &mut ops,
        "Departemen",
        det.department_name.as_deref().unwrap_or("-"),
    );
    line(
        &mut ops,
        "Jabatan",
        det.position_name.as_deref().unwrap_or("-"),
    );
    blank(&mut ops);
    ops.push(Op::ShowText {
        items: vec![TextItem::Text("PENDAPATAN".to_string())],
    });
    ops.push(Op::AddLineBreak);
    for l in det.lines.iter().filter(|l| l.line_type == "income") {
        line(&mut ops, &l.component_name, &rupiah(l.amount));
    }
    line(&mut ops, "Total pendapatan", &rupiah(det.total_income));
    blank(&mut ops);
    ops.push(Op::ShowText {
        items: vec![TextItem::Text("POTONGAN".to_string())],
    });
    ops.push(Op::AddLineBreak);
    for l in det.lines.iter().filter(|l| l.line_type == "deduction") {
        line(&mut ops, &l.component_name, &rupiah(l.amount));
    }
    line(&mut ops, "Total potongan", &rupiah(det.total_deduction));
    blank(&mut ops);
    ops.push(Op::SetFont {
        font: PdfFontHandle::Builtin(BuiltinFont::CourierBold),
        size: Pt(11.0),
    });
    line(&mut ops, "GAJI BERSIH", &rupiah(det.net_salary));
    ops.push(Op::SetFont {
        font: PdfFontHandle::Builtin(BuiltinFont::CourierOblique),
        size: Pt(8.0),
    });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(
            "PPh 21 pada slip ini adalah estimasi, bukan perhitungan pajak resmi.".to_string(),
        )],
    });
    ops.push(Op::AddLineBreak);
    ops.push(Op::EndTextSection);

    let page = PdfPage::new(Mm(210.0), Mm(297.0), ops);
    let mut warnings = Vec::new();
    let bytes = PdfDocument::new("Slip Gaji")
        .with_pages(vec![page])
        .save(&PdfSaveOptions::default(), &mut warnings);
    if bytes.is_empty() {
        return Err("Gagal merender PDF.".to_string());
    }
    let dir = files.join("exports/payslips");
    std::fs::create_dir_all(&dir).map_err(|e| format!("gagal membuat folder slip: {e}"))?;
    let rel = format!("exports/payslips/{number}.pdf");
    std::fs::write(files.join(&rel), &bytes).map_err(|e| format!("gagal menyimpan PDF: {e}"))?;
    exec(
        db,
        "UPDATE payslips SET pdf_path = ?1 WHERE id = ?2".to_string(),
        vec![Value::Text(rel.clone()), Value::Int(slip_id)],
        "payslip.record",
    )
    .await
    .map_err(|e| format!("gagal mencatat PDF: {e}"))?;
    Ok(rel)
}

/// Baca berkas slip untuk unduh (milik sendiri atau izin payroll.view).
pub async fn read_file_sea(
    db: &sea_orm::DatabaseConnection,
    files: &Path,
    payroll_id: i64,
) -> Result<PayslipFile, String> {
    let row = q_one(
        db,
        "SELECT pdf_path, payslip_number FROM payslips WHERE payroll_id = ?1".to_string(),
        vec![Value::Int(payroll_id)],
        2,
        "payslip.read",
    )
    .await
    .map_err(|e| format!("gagal memuat slip: {e}"))?;
    let Some(row) = row else {
        return Err("Slip belum tersedia.".to_string());
    };
    let (rel, number) = (value_to_string(&row[0]), value_to_string(&row[1]));
    if rel.trim().is_empty() {
        return Err("PDF belum dibuat. Minta HRD membuatkan.".to_string());
    }
    let path = files.join(&rel);
    if !path.starts_with(files) {
        return Err("Path slip tidak valid.".to_string());
    }
    let bytes =
        std::fs::read(&path).map_err(|_| "Berkas PDF hilang dari penyimpanan.".to_string())?;
    Ok(PayslipFile {
        mime: "application/pdf".to_string(),
        name: format!("{number}.pdf"),
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sea_raw::{exec, q_one, value_i64, Value};

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

    async fn mkemp(db: &sea_orm::DatabaseConnection, number: &str, basic: f64) -> i64 {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type) VALUES (?1, 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent')".to_string(),
            vec![Value::Text(number.to_string())],
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
            "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, ?2, '2026-01-01', 1)".to_string(),
            vec![Value::Int(eid), Value::Float(basic)],
            "test.mksal",
        )
        .await
        .expect("salary");
        eid
    }

    async fn payroll_id_for(
        db: &sea_orm::DatabaseConnection,
        period_id: i64,
        employee_id: i64,
    ) -> i64 {
        q_one(
            db,
            "SELECT id FROM payrolls WHERE payroll_period_id = ?1 AND employee_id = ?2".to_string(),
            vec![Value::Int(period_id), Value::Int(employee_id)],
            1,
            "test.payroll",
        )
        .await
        .expect("payroll")
        .and_then(|r| value_i64(&r[0]))
        .expect("payroll id")
    }

    #[tokio::test]
    async fn slip_sea_terrender_menjadi_pdf_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin = admin_id(db).await;
        let eid = mkemp(db, "EMP-S1", 4_000_000.0).await;
        let pid = crate::services::payroll::period_create_sea(
            db,
            admin,
            admin,
            &crate::services::payroll::PeriodInput {
                name: "Mar 2026".to_string(),
                start_date: "2026-03-01".to_string(),
                end_date: "2026-03-31".to_string(),
                payment_date: None,
                company_id: None,
            },
        )
        .await
        .expect("periode") as i64;
        crate::services::payroll::generate_sea(db, admin, pid)
            .await
            .expect("generate");
        crate::services::payroll::approve_period_sea(db, admin, pid)
            .await
            .expect("approve");
        crate::services::payroll::mark_paid_sea(db, admin, pid)
            .await
            .expect("pay");
        let payroll_id = payroll_id_for(db, pid, eid).await;
        let det = crate::services::payroll::payroll_detail_sea(db, payroll_id)
            .await
            .expect("det")
            .expect("ada");
        assert_eq!(det.basic_salary, 4_000_000.0);
        assert!(det.total_income >= det.basic_salary);
        assert!(det.net_salary > 0.0);
        assert!((det.total_income - det.total_deduction - det.net_salary).abs() < 1.0);
        let rel = render_sea(db, files.path(), payroll_id)
            .await
            .expect("render");
        assert!(rel.ends_with(".pdf"));
        let recorded = q_one(
            db,
            "SELECT pdf_path FROM payslips WHERE payroll_id = ?1".to_string(),
            vec![Value::Int(payroll_id)],
            1,
            "test.pdfpath",
        )
        .await
        .expect("pdfpath")
        .map(|r| value_to_string(&r[0]))
        .expect("ada");
        assert_eq!(recorded, rel);
        let got = read_file_sea(db, files.path(), payroll_id)
            .await
            .expect("baca");
        assert_eq!(got.mime, "application/pdf");
        assert!(got.name.ends_with(".pdf"));
        assert!(got.bytes.starts_with(b"%PDF"));
        assert!(got.bytes.len() > 1000);
    }

    #[tokio::test]
    async fn slip_sea_gagal_jelas_tanpa_data() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let admin = admin_id(db).await;
        let e = render_sea(db, files.path(), 999_999).await.expect_err("twd");
        assert_eq!(e, "Payroll tidak ditemukan.".to_string());
        let e = read_file_sea(db, files.path(), 999_999)
            .await
            .expect_err("twd");
        assert_eq!(e, "Slip belum tersedia.".to_string());
        let eid = mkemp(db, "EMP-S2", 4_000_000.0).await;
        let pid = crate::services::payroll::period_create_sea(
            db,
            admin,
            admin,
            &crate::services::payroll::PeriodInput {
                name: "Apr 2026".to_string(),
                start_date: "2026-04-01".to_string(),
                end_date: "2026-04-30".to_string(),
                payment_date: None,
                company_id: None,
            },
        )
        .await
        .expect("periode") as i64;
        crate::services::payroll::generate_sea(db, admin, pid)
            .await
            .expect("generate");
        let payroll_id = payroll_id_for(db, pid, eid).await;
        let e = render_sea(db, files.path(), payroll_id)
            .await
            .expect_err("belum bayar");
        assert_eq!(e, "Slip belum tersedia untuk payroll ini.".to_string());
        let e = read_file_sea(db, files.path(), payroll_id)
            .await
            .expect_err("belum bayar");
        assert_eq!(e, "Slip belum tersedia.".to_string());
    }

    #[test]
    fn rupiah_memformat_id() {
        assert_eq!(rupiah(1_500_000.0), "Rp 1.500.000");
        assert_eq!(rupiah(4_000_000.0), "Rp 4.000.000");
        assert_eq!(rupiah(0.0), "Rp 0");
        assert_eq!(rupiah(-500.0), "-Rp 500");
        assert_eq!(rupiah(1_500_000.6), "Rp 1.500.001");
    }
}
