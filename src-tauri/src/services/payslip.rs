//! Slip gaji PDF (A4, font bawaan, tanpa aset eksternal).

use printpdf::{
    BuiltinFont, Mm, Op, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions, Pt, TextItem,
};
use rusqlite::{params, Connection, OptionalExtension};
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

/// Render slip untuk satu payroll, simpan ke exports, kembalikan path relatif.
pub fn render(conn: &Connection, files: &Path, payroll_id: i64) -> Result<String, String> {
    let det =
        payroll::payroll_detail(conn, payroll_id)?.ok_or("Payroll tidak ditemukan.".to_string())?;
    let slip: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, payslip_number FROM payslips WHERE payroll_id = ?1",
            params![payroll_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat slip: {e}"))?;
    let Some((slip_id, number)) = slip else {
        return Err("Slip belum tersedia untuk payroll ini.".to_string());
    };
    let company: String = conn
        .query_row("SELECT name FROM companies ORDER BY id LIMIT 1", [], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|e| format!("gagal memuat perusahaan: {e}"))?
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
    conn.execute(
        "UPDATE payslips SET pdf_path = ?1 WHERE id = ?2",
        params![rel, slip_id],
    )
    .map_err(|e| format!("gagal mencatat PDF: {e}"))?;
    Ok(rel)
}

/// Baca berkas slip untuk unduh (milik sendiri atau izin payroll.view).
pub fn read_file(conn: &Connection, files: &Path, payroll_id: i64) -> Result<PayslipFile, String> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT pdf_path, payslip_number FROM payslips WHERE payroll_id = ?1",
            params![payroll_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| format!("gagal memuat slip: {e}"))?;
    let Some((rel, number)) = row else {
        return Err("Slip belum tersedia.".to_string());
    };
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
    use crate::{db, seed};

    #[test]
    fn slip_terrender_menjadi_pdf_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let files = tempfile::tempdir().expect("files");
        let pool = db::init_pool(&dir.path().join("t.db")).expect("pool");
        let mut c = pool.get().expect("get");
        db::migrate(&mut c).expect("migrate");
        seed::seed(&mut c).expect("seed");
        let actor: i64 = c
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |r| {
                r.get(0)
            })
            .unwrap();
        c.execute(
            "INSERT INTO employees (employee_number, first_name, gender, marital_status, company_id, join_date, employment_status, employment_type) VALUES ('EMP-P1', 'Tes', 'male', 'single', 1, '2026-01-01', 'active', 'permanent')",
            [],
        )
        .unwrap();
        let eid = c.last_insert_rowid();
        c.execute(
            "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, 4000000, '2026-01-01', 1)",
            rusqlite::params![eid],
        )
        .unwrap();
        let pid = crate::services::payroll::period_create(
            &c,
            actor,
            actor,
            &crate::services::payroll::PeriodInput {
                name: "Mar 2026".to_string(),
                start_date: "2026-03-01".to_string(),
                end_date: "2026-03-31".to_string(),
                payment_date: None,
            },
        )
        .expect("periode") as i64;
        drop(c);
        let mut c2 = pool.get().expect("get");
        crate::services::payroll::generate(&mut c2, actor, pid).expect("generate");
        crate::services::payroll::approve_period(&c2, actor, pid).expect("approve");
        crate::services::payroll::mark_paid(&c2, actor, pid).expect("pay");
        let payroll_id: i64 = c2
            .query_row(
                "SELECT id FROM payrolls WHERE payroll_period_id = ?1 AND employee_id = ?2",
                rusqlite::params![pid, eid],
                |r| r.get(0),
            )
            .unwrap();
        let rel = render(&c2, files.path(), payroll_id).expect("render");
        assert!(rel.ends_with(".pdf"));
        let got = read_file(&c2, files.path(), payroll_id).expect("baca");
        assert_eq!(got.mime, "application/pdf");
        assert!(got.bytes.starts_with(b"%PDF"));
        assert!(got.bytes.len() > 1000);
    }
}
