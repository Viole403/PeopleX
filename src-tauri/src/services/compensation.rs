use super::audit;
use crate::services::sea_raw::{exec, exec_insert, q_all, q_one, value_i64, value_to_string, Value};
use crate::to_dto_int;
use chrono::{Datelike, Local, NaiveDate};

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn sea_f64(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        Value::Text(t) => t.parse().unwrap_or(0.0),
        Value::Null => 0.0,
    }
}

fn teks_opsional(v: &Value) -> Option<String> {
    match v {
        Value::Text(t) => Some(t.clone()),
        _ => None,
    }
}

fn bulat(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct BonusScheme {
    pub id: i32,
    pub name: String,
    pub kind: String,
    pub amount: f64,
    pub percent: f64,
    pub threshold: f64,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct BonusInput {
    pub name: String,
    pub kind: String,
    pub amount: f64,
    pub percent: f64,
    pub threshold: f64,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct BonusRow {
    pub id: i32,
    pub scheme_id: i32,
    pub scheme_name: String,
    pub employee_id: i32,
    pub employee_name: String,
    pub period: String,
    pub amount: f64,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct Benefit {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub cost: f64,
    pub category: Option<String>,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct BenefitInput {
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub cost: f64,
    pub status: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EnrolledBenefit {
    pub id: i32,
    pub benefit_id: i32,
    pub benefit_name: String,
    pub year: i32,
    pub amount: f64,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct MeritRow {
    pub department_id: i32,
    pub department_name: String,
    pub employees: i32,
    pub total_current: f64,
    pub total_next: f64,
    pub increase: f64,
}

pub async fn scheme_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &BonusInput,
) -> Result<i32, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Nama skema wajib diisi.".to_string());
    }
    if !["fixed", "percent_of_salary", "attendance"].contains(&input.kind.as_str()) {
        return Err("Jenis skema tidak valid.".to_string());
    }
    if input.amount < 0.0 || input.percent < 0.0 || input.threshold < 0.0 {
        return Err("Nilai tidak boleh negatif.".to_string());
    }
    if input.status != "active" && input.status != "inactive" {
        return Err("Status tidak valid.".to_string());
    }
    let dup = q_one(
        db,
        "SELECT id FROM bonus_schemes WHERE name = ?1".to_string(),
        vec![Value::Text(name.to_string())],
        1,
        "compensation.scheme.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa skema: {e}"))?;
    if let Some(r) = dup {
        let found = value_i64(&r[0]).unwrap_or(0);
        if Some(found) != id {
            return Err("Nama skema sudah dipakai.".to_string());
        }
    }
    let ts = now_str();
    match id {
        Some(rid) => {
            let ada = q_one(
                db,
                "SELECT id FROM bonus_schemes WHERE id = ?1".to_string(),
                vec![Value::Int(rid)],
                1,
                "compensation.scheme.ada",
            )
            .await
            .map_err(|e| format!("gagal memeriksa skema: {e}"))?;
            if ada.is_none() {
                return Err("Skema tidak ditemukan.".to_string());
            }
            exec(
                db,
                "UPDATE bonus_schemes SET name = ?1, kind = ?2, amount = ?3, percent = ?4, threshold = ?5, status = ?6, updated_at = ?7 WHERE id = ?8".to_string(),
                vec![
                    Value::Text(name.to_string()),
                    Value::Text(input.kind.clone()),
                    Value::Float(input.amount),
                    Value::Float(input.percent),
                    Value::Float(input.threshold),
                    Value::Text(input.status.clone()),
                    Value::Text(ts),
                    Value::Int(rid),
                ],
                "compensation.scheme.upd",
            )
            .await
            .map_err(|e| format!("gagal memperbarui skema: {e}"))?;
            audit::log_sea(
                db,
                Some(actor_id),
                "UPDATE",
                "compensation.scheme",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await;
            to_dto_int(rid, "id")
        }
        None => {
            let rid = exec_insert(
                db,
                "INSERT INTO bonus_schemes (name, kind, amount, percent, threshold, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)".to_string(),
                vec![
                    Value::Text(name.to_string()),
                    Value::Text(input.kind.clone()),
                    Value::Float(input.amount),
                    Value::Float(input.percent),
                    Value::Float(input.threshold),
                    Value::Text(input.status.clone()),
                    Value::Text(ts.clone()),
                    Value::Text(ts),
                ],
                "compensation.scheme.ins",
            )
            .await
            .map_err(|e| format!("gagal menyimpan skema: {e}"))?;
            audit::log_sea(
                db,
                Some(actor_id),
                "CREATE",
                "compensation.scheme",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await;
            to_dto_int(rid, "id")
        }
    }
}

pub async fn scheme_list_sea(
    db: &sea_orm::DatabaseConnection,
) -> Result<Vec<BonusScheme>, String> {
    let rows = q_all(
        db,
        "SELECT id, name, kind, amount, percent, threshold, status FROM bonus_schemes ORDER BY id DESC".to_string(),
        vec![],
        7,
        "compensation.scheme.list",
    )
    .await
    .map_err(|e| format!("gagal membaca skema: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| BonusScheme {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "id").unwrap_or(0),
            name: value_to_string(&r[1]),
            kind: value_to_string(&r[2]),
            amount: sea_f64(&r[3]),
            percent: sea_f64(&r[4]),
            threshold: sea_f64(&r[5]),
            status: value_to_string(&r[6]),
        })
        .collect())
}

pub async fn bonus_run_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    scheme_id: i64,
    period: &str,
) -> Result<i32, String> {
    if period.len() != 7 || &period[4..5] != "-" {
        return Err("Format periode harus YYYY-MM.".to_string());
    }
    let sc = q_one(
        db,
        "SELECT kind, amount, percent, threshold, status FROM bonus_schemes WHERE id = ?1".to_string(),
        vec![Value::Int(scheme_id)],
        5,
        "compensation.bonus.scheme",
    )
    .await
    .map_err(|e| format!("gagal membaca skema: {e}"))?;
    let sc = sc.ok_or_else(|| "Skema tidak ditemukan.".to_string())?;
    let kind = value_to_string(&sc[0]);
    if value_to_string(&sc[4]) != "active" {
        return Err("Skema tidak aktif.".to_string());
    }
    let emps = q_all(
        db,
        "SELECT e.id, s.basic_salary FROM employees e INNER JOIN employee_salaries s ON s.employee_id = e.id AND s.is_active = 1 WHERE e.deleted_at IS NULL AND e.employment_status IN ('active','probation') ORDER BY e.id".to_string(),
        vec![],
        2,
        "compensation.bonus.employees",
    )
    .await
    .map_err(|e| format!("gagal membaca karyawan: {e}"))?;
    let ts = now_str();
    let mut dibuat = 0i64;
    for r in &emps {
        let eid = value_i64(&r[0]).unwrap_or(0);
        let basic = sea_f64(&r[1]);
        let amount = match kind.as_str() {
            "fixed" => sea_f64(&sc[1]),
            "percent_of_salary" => bulat(basic * sea_f64(&sc[2]) / 100.0),
            _ => {
                let total = q_one(
                    db,
                    "SELECT COUNT(*) FROM attendances WHERE employee_id = ?1 AND date LIKE ?2".to_string(),
                    vec![Value::Int(eid), Value::Text(format!("{}%", period))],
                    1,
                    "compensation.bonus.total",
                )
                .await
                .map_err(|e| format!("gagal membaca kehadiran: {e}"))?;
                let had = q_one(
                    db,
                    "SELECT COALESCE(SUM(CASE WHEN status IN ('present','late') THEN 1 ELSE 0 END), 0) FROM attendances WHERE employee_id = ?1 AND date LIKE ?2".to_string(),
                    vec![Value::Int(eid), Value::Text(format!("{}%", period))],
                    1,
                    "compensation.bonus.hadir",
                )
                .await
                .map_err(|e| format!("gagal membaca kehadiran: {e}"))?;
                let t = total.and_then(|x| value_i64(&x[0])).unwrap_or(0) as f64;
                let h = had.and_then(|x| value_i64(&x[0])).unwrap_or(0) as f64;
                let ratio = if t > 0.0 { h * 100.0 / t } else { 0.0 };
                if ratio >= sea_f64(&sc[3]) {
                    sea_f64(&sc[1])
                } else {
                    0.0
                }
            }
        };
        let n = exec(
            db,
            "INSERT OR IGNORE INTO bonuses (scheme_id, employee_id, period, amount, status, created_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'draft', ?5, ?6, ?7)".to_string(),
            vec![
                Value::Int(scheme_id),
                Value::Int(eid),
                Value::Text(period.to_string()),
                Value::Float(bulat(amount)),
                Value::Int(actor_id),
                Value::Text(ts.clone()),
                Value::Text(ts.clone()),
            ],
            "compensation.bonus.ins",
        )
        .await
        .map_err(|e| format!("gagal membuat bonus: {e}"))?;
        dibuat += n as i64;
    }
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "compensation.bonus",
        Some(&scheme_id.to_string()),
        None,
        None,
        None,
    )
    .await;
    to_dto_int(dibuat, "bonus")
}

pub async fn bonus_list_sea(
    db: &sea_orm::DatabaseConnection,
    period: Option<&str>,
) -> Result<Vec<BonusRow>, String> {
    let rows = match period {
        Some(p) => {
            q_all(
                db,
                "SELECT b.id, b.scheme_id, s.name, b.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), b.period, b.amount, b.status FROM bonuses b INNER JOIN bonus_schemes s ON s.id = b.scheme_id INNER JOIN employees e ON e.id = b.employee_id WHERE b.period = ?1 ORDER BY b.id DESC".to_string(),
                vec![Value::Text(p.to_string())],
                8,
                "compensation.bonus.list",
            )
            .await
        }
        None => {
            q_all(
                db,
                "SELECT b.id, b.scheme_id, s.name, b.employee_id, e.first_name || ' ' || COALESCE(e.last_name, ''), b.period, b.amount, b.status FROM bonuses b INNER JOIN bonus_schemes s ON s.id = b.scheme_id INNER JOIN employees e ON e.id = b.employee_id ORDER BY b.id DESC".to_string(),
                vec![],
                8,
                "compensation.bonus.list",
            )
            .await
        }
    }
    .map_err(|e| format!("gagal membaca bonus: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| BonusRow {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "id").unwrap_or(0),
            scheme_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "id").unwrap_or(0),
            scheme_name: value_to_string(&r[2]),
            employee_id: to_dto_int(value_i64(&r[3]).unwrap_or(0), "id").unwrap_or(0),
            employee_name: value_to_string(&r[4]),
            period: value_to_string(&r[5]),
            amount: sea_f64(&r[6]),
            status: value_to_string(&r[7]),
        })
        .collect())
}

pub async fn bonus_decide_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: i64,
    decision: &str,
) -> Result<(), String> {
    if decision != "approved" && decision != "paid" {
        return Err("Keputusan tidak valid.".to_string());
    }
    let row = q_one(
        db,
        "SELECT status FROM bonuses WHERE id = ?1".to_string(),
        vec![Value::Int(id)],
        1,
        "compensation.bonus.ada",
    )
    .await
    .map_err(|e| format!("gagal membaca bonus: {e}"))?;
    let row = row.ok_or_else(|| "Bonus tidak ditemukan.".to_string())?;
    let status = value_to_string(&row[0]);
    if decision == "approved" && status != "draft" {
        return Err("Status bonus tidak bisa diubah.".to_string());
    }
    if decision == "paid" && status != "approved" {
        return Err("Status bonus tidak bisa diubah.".to_string());
    }
    exec(
        db,
        "UPDATE bonuses SET status = ?1, updated_at = ?2 WHERE id = ?3".to_string(),
        vec![
            Value::Text(decision.to_string()),
            Value::Text(now_str()),
            Value::Int(id),
        ],
        "compensation.bonus.upd",
    )
    .await
    .map_err(|e| format!("gagal memutus bonus: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "UPDATE",
        "compensation.bonus",
        Some(&id.to_string()),
        None,
        None,
        None,
    )
    .await;
    Ok(())
}

pub async fn benefit_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &BenefitInput,
) -> Result<i32, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Nama benefit wajib diisi.".to_string());
    }
    if input.cost < 0.0 {
        return Err("Biaya tidak boleh negatif.".to_string());
    }
    if input.status != "active" && input.status != "inactive" {
        return Err("Status tidak valid.".to_string());
    }
    let dup = q_one(
        db,
        "SELECT id FROM benefits WHERE name = ?1".to_string(),
        vec![Value::Text(name.to_string())],
        1,
        "compensation.benefit.dup",
    )
    .await
    .map_err(|e| format!("gagal memeriksa benefit: {e}"))?;
    if let Some(r) = dup {
        let found = value_i64(&r[0]).unwrap_or(0);
        if Some(found) != id {
            return Err("Nama benefit sudah dipakai.".to_string());
        }
    }
    let ts = now_str();
    let desc = input
        .description
        .as_ref()
        .map(|s| Value::Text(s.clone()))
        .unwrap_or(Value::Null);
    let cat = input
        .category
        .as_ref()
        .map(|s| Value::Text(s.clone()))
        .unwrap_or(Value::Null);
    match id {
        Some(rid) => {
            let ada = q_one(
                db,
                "SELECT id FROM benefits WHERE id = ?1".to_string(),
                vec![Value::Int(rid)],
                1,
                "compensation.benefit.ada",
            )
            .await
            .map_err(|e| format!("gagal memeriksa benefit: {e}"))?;
            if ada.is_none() {
                return Err("Benefit tidak ditemukan.".to_string());
            }
            exec(
                db,
                "UPDATE benefits SET name = ?1, description = ?2, cost = ?3, category = ?4, status = ?5, updated_at = ?6 WHERE id = ?7".to_string(),
                vec![
                    Value::Text(name.to_string()),
                    desc,
                    Value::Float(input.cost),
                    cat,
                    Value::Text(input.status.clone()),
                    Value::Text(ts),
                    Value::Int(rid),
                ],
                "compensation.benefit.upd",
            )
            .await
            .map_err(|e| format!("gagal memperbarui benefit: {e}"))?;
            audit::log_sea(
                db,
                Some(actor_id),
                "UPDATE",
                "compensation.benefit",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await;
            to_dto_int(rid, "id")
        }
        None => {
            let rid = exec_insert(
                db,
                "INSERT INTO benefits (name, description, cost, category, status, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)".to_string(),
                vec![
                    Value::Text(name.to_string()),
                    desc,
                    Value::Float(input.cost),
                    cat,
                    Value::Text(input.status.clone()),
                    Value::Text(ts.clone()),
                    Value::Text(ts),
                ],
                "compensation.benefit.ins",
            )
            .await
            .map_err(|e| format!("gagal menyimpan benefit: {e}"))?;
            audit::log_sea(
                db,
                Some(actor_id),
                "CREATE",
                "compensation.benefit",
                Some(&rid.to_string()),
                None,
                None,
                None,
            )
            .await;
            to_dto_int(rid, "id")
        }
    }
}

pub async fn benefit_list_sea(
    db: &sea_orm::DatabaseConnection,
    active_only: bool,
) -> Result<Vec<Benefit>, String> {
    let sql = if active_only {
        "SELECT id, name, description, cost, category, status FROM benefits WHERE status = 'active' ORDER BY id"
    } else {
        "SELECT id, name, description, cost, category, status FROM benefits ORDER BY id"
    };
    let rows = q_all(
        db,
        sql.to_string(),
        vec![],
        6,
        "compensation.benefit.list",
    )
    .await
    .map_err(|e| format!("gagal membaca benefit: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| Benefit {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "id").unwrap_or(0),
            name: value_to_string(&r[1]),
            description: teks_opsional(&r[2]),
            cost: sea_f64(&r[3]),
            category: teks_opsional(&r[4]),
            status: value_to_string(&r[5]),
        })
        .collect())
}

pub async fn benefit_enroll_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    employee_id: i64,
    benefit_id: i64,
    year: i32,
) -> Result<i32, String> {
    if !(1990..=2100).contains(&year) {
        return Err("Tahun harus antara 1990 dan 2100.".to_string());
    }
    let b = q_one(
        db,
        "SELECT cost, status FROM benefits WHERE id = ?1".to_string(),
        vec![Value::Int(benefit_id)],
        2,
        "compensation.enroll.benefit",
    )
    .await
    .map_err(|e| format!("gagal membaca benefit: {e}"))?;
    let b = b.ok_or_else(|| "Benefit tidak ditemukan.".to_string())?;
    if value_to_string(&b[1]) != "active" {
        return Err("Benefit tidak aktif.".to_string());
    }
    let exist = q_one(
        db,
        "SELECT id FROM benefit_enrollments WHERE employee_id = ?1 AND benefit_id = ?2 AND year = ?3".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(benefit_id),
            Value::Int(year as i64),
        ],
        1,
        "compensation.enroll.ada",
    )
    .await
    .map_err(|e| format!("gagal memeriksa enrolment: {e}"))?;
    if let Some(r) = exist {
        return to_dto_int(value_i64(&r[0]).unwrap_or(0), "id");
    }
    let ts = now_str();
    let rid = exec_insert(
        db,
        "INSERT INTO benefit_enrollments (employee_id, benefit_id, year, amount, elected_by, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)".to_string(),
        vec![
            Value::Int(employee_id),
            Value::Int(benefit_id),
            Value::Int(year as i64),
            Value::Float(sea_f64(&b[0])),
            Value::Int(actor_id),
            Value::Text(ts.clone()),
            Value::Text(ts),
        ],
        "compensation.enroll.ins",
    )
    .await
    .map_err(|e| format!("gagal menyimpan enrolment: {e}"))?;
    audit::log_sea(
        db,
        Some(actor_id),
        "CREATE",
        "compensation.enrollment",
        Some(&employee_id.to_string()),
        None,
        None,
        None,
    )
    .await;
    to_dto_int(rid, "id")
}

pub async fn my_benefits_sea(
    db: &sea_orm::DatabaseConnection,
    employee_id: i64,
    year: i32,
) -> Result<Vec<EnrolledBenefit>, String> {
    let rows = q_all(
        db,
        "SELECT er.id, er.benefit_id, b.name, er.year, er.amount FROM benefit_enrollments er INNER JOIN benefits b ON b.id = er.benefit_id WHERE er.employee_id = ?1 AND er.year = ?2 ORDER BY er.id".to_string(),
        vec![Value::Int(employee_id), Value::Int(year as i64)],
        5,
        "compensation.enroll.mine",
    )
    .await
    .map_err(|e| format!("gagal membaca enrolment: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| EnrolledBenefit {
            id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "id").unwrap_or(0),
            benefit_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "id").unwrap_or(0),
            benefit_name: value_to_string(&r[2]),
            year: to_dto_int(value_i64(&r[3]).unwrap_or(0), "y").unwrap_or(0),
            amount: sea_f64(&r[4]),
        })
        .collect())
}

pub async fn merit_model_sea(
    db: &sea_orm::DatabaseConnection,
    percent: f64,
) -> Result<Vec<MeritRow>, String> {
    if !(0.0..=100.0).contains(&percent) {
        return Err("Persentase kenaikan tidak valid.".to_string());
    }
    let rows = q_all(
        db,
        "SELECT COALESCE(e.department_id, 0), d.name, SUM(s.basic_salary * 12), COUNT(*) FROM employees e INNER JOIN employee_salaries s ON s.employee_id = e.id AND s.is_active = 1 LEFT JOIN departments d ON d.id = e.department_id WHERE e.deleted_at IS NULL AND e.employment_status IN ('active','probation') GROUP BY COALESCE(e.department_id, 0), d.name ORDER BY COALESCE(d.name, 'Tanpa departemen')".to_string(),
        vec![],
        4,
        "compensation.merit",
    )
    .await
    .map_err(|e| format!("gagal memodelkan merit: {e}"))?;
    Ok(rows
        .iter()
        .map(|r| {
            let current = sea_f64(&r[2]);
            let next = bulat(current * (1.0 + percent / 100.0));
            MeritRow {
                department_id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "id").unwrap_or(0),
                department_name: teks_opsional(&r[1]).unwrap_or_else(|| "Tanpa departemen".to_string()),
                employees: to_dto_int(value_i64(&r[3]).unwrap_or(0), "n").unwrap_or(0),
                total_current: current,
                total_next: next,
                increase: bulat(next - current),
            }
        })
        .collect())
}


#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct CompReviewRow {
    pub employee_id: i32,
    pub employee_name: String,
    pub basic_salary: f64,
    pub score: f64,
    pub recommendation_percent: f64,
}

pub async fn compensation_review_list_sea(
    db: &sea_orm::DatabaseConnection,
    year: i64,
) -> Result<Vec<CompReviewRow>, String> {
    let y = year.to_string();
    let rows = q_all(
        db,
        "SELECT e.id, TRIM(e.first_name || ' ' || COALESCE(e.last_name, '')) FROM employees e WHERE e.deleted_at IS NULL AND e.employment_status = 'active' ORDER BY e.id".to_string(),
        vec![],
        2,
        "comp.review.emp",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let emp = value_i64(&r[0]).unwrap_or(0);
        let g = q_one(
            db,
            "SELECT s.basic_salary FROM employee_salaries s WHERE s.employee_id = ?1 AND s.is_active = 1 ORDER BY s.effective_date DESC LIMIT 1".to_string(),
            vec![Value::Int(emp)],
            1,
            "comp.review.sal",
        )
        .await?;
        let basic = g.map(|r_| match &r_[0] {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f64,
            _ => 0.0,
        }).unwrap_or(0.0);
        let kpis = q_all(
            db,
            "SELECT ek.actual, ek.target FROM employee_kpis ek INNER JOIN performance_periods pp ON pp.id = ek.performance_period_id WHERE ek.employee_id = ?1 AND substr(pp.start_date, 1, 4) = ?2 AND ek.actual IS NOT NULL AND ek.target IS NOT NULL".to_string(),
            vec![Value::Int(emp), Value::Text(y.clone())],
            2,
            "comp.review.kpi",
        )
        .await?;
        let mut tot = 0.0;
        let mut cnt = 0i64;
        for k in kpis {
            let actual = match &k[0] {
                Value::Float(v) => *v,
                Value::Int(v) => *v as f64,
                _ => continue,
            };
            let target = match &k[1] {
                Value::Float(v) => *v,
                Value::Int(v) => *v as f64,
                _ => continue,
            };
            if target <= 0.0 {
                continue;
            }
            tot += (actual / target * 100.0).min(120.0);
            cnt += 1;
        }
        let score = if cnt > 0 { tot / cnt as f64 } else { 0.0 };
        let rec = if score >= 90.0 {
            7.0
        } else if score >= 75.0 {
            5.0
        } else if score >= 60.0 {
            3.0
        } else if score >= 40.0 {
            1.0
        } else {
            0.0
        };
        out.push(CompReviewRow {
            employee_id: to_dto_int(emp, "comp.review.emp")?,
            employee_name: value_to_string(&r[1]),
            basic_salary: basic,
            score: (score * 100.0).round() / 100.0,
            recommendation_percent: rec,
        });
    }
    Ok(out)
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct BenchmarkInput {
    pub position_id: Option<i32>,
    pub department_id: Option<i32>,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub source: String,
    pub period: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct BenchmarkGapRow {
    pub employee_id: i32,
    pub employee_name: String,
    pub salary: f64,
    pub p50: f64,
    pub ratio: f64,
}

fn bench_period_ok(period: &str) -> bool {
    let b = period.as_bytes();
    b.len() == 7 && b[4] == b'-' && b[..4].iter().all(|c| c.is_ascii_digit()) && b[5..].iter().all(|c| c.is_ascii_digit())
}

pub async fn benchmark_save_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    id: Option<i64>,
    input: &BenchmarkInput,
) -> Result<i32, String> {
    if !(input.p25 > 0.0 && input.p25 <= input.p50 && input.p50 <= input.p75) {
        return Err("Rentang benchmark tidak valid.".to_string());
    }
    if !bench_period_ok(&input.period) {
        return Err("Format periode tidak valid.".to_string());
    }
    let mm: i32 = input.period[5..7].parse().map_err(|_| "Format periode tidak valid.".to_string())?;
    if mm < 1 || mm > 12 {
        return Err("Format periode tidak valid.".to_string());
    }
    let pos = match input.position_id {
        Some(v) => Value::Int(v as i64),
        None => Value::Int(0),
    };
    let dep = match input.department_id {
        Some(v) => Value::Int(v as i64),
        None => Value::Int(0),
    };
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let new_id = match id {
        Some(x) => {
            let n = exec(
                db,
                "UPDATE salary_benchmarks SET position_id = ?1, department_id = ?2, p25 = ?3, p50 = ?4, p75 = ?5, source = ?6, period = ?7, updated_at = ?8 WHERE id = ?9".to_string(),
                vec![
                    pos, dep,
                    Value::Float(input.p25),
                    Value::Float(input.p50),
                    Value::Float(input.p75),
                    Value::Text(input.source.trim().to_string()),
                    Value::Text(input.period.trim().to_string()),
                    Value::Text(now),
                    Value::Int(x),
                ],
                "cmp.bench.upd",
            )
            .await?;
            if n == 0 {
                return Err("Benchmark tidak ditemukan.".to_string());
            }
            x
        }
        None => {
            exec_insert(
                db,
                "INSERT INTO salary_benchmarks (position_id, department_id, p25, p50, p75, source, period, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)".to_string(),
                vec![
                    pos, dep,
                    Value::Float(input.p25),
                    Value::Float(input.p50),
                    Value::Float(input.p75),
                    Value::Text(input.source.trim().to_string()),
                    Value::Text(input.period.trim().to_string()),
                    Value::Text(now.clone()),
                    Value::Text(now),
                ],
                "cmp.bench.ins",
            )
            .await?
        }
    };
    audit::log_sea(
        db,
        Some(actor_id),
        if id.is_some() { "UPDATE" } else { "CREATE" },
        "compensation.benchmark",
        Some(&new_id.to_string()),
        None,
        None,
        Some(&format!("Benchmark {} periode {}", input.position_id.map_or("-".to_string(), |v| v.to_string()), input.period.trim())),
    )
    .await?;
    to_dto_int(new_id, "cmp.bench.id")
}

pub async fn benchmark_compare_sea(
    db: &sea_orm::DatabaseConnection,
    period: &str,
) -> Result<Vec<BenchmarkGapRow>, String> {
    if !bench_period_ok(period) {
        return Err("Format periode tidak valid.".to_string());
    }
    let rows = q_all(
        db,
        "SELECT e.id, TRIM(e.first_name || ' ' || COALESCE(e.last_name, '')) FROM employees e WHERE e.deleted_at IS NULL AND e.employment_status = 'active' ORDER BY e.id".to_string(),
        vec![],
        2,
        "cmp.gap.emp",
    )
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let emp = value_i64(&r[0]).unwrap_or(0);
        let g = q_one(
            db,
            "SELECT s.basic_salary, e.position_id, e.department_id FROM employee_salaries s INNER JOIN employees e ON e.id = s.employee_id WHERE s.employee_id = ?1 AND s.is_active = 1 ORDER BY s.effective_date DESC LIMIT 1".to_string(),
            vec![Value::Int(emp)],
            3,
            "cmp.gap.sal",
        )
        .await?;
        let (sal, pos, dep) = match &g {
            Some(g_) => (
                match &g_[0] {
                    Value::Float(v) => *v,
                    Value::Int(v) => *v as f64,
                    _ => 0.0,
                },
                g_[1].clone(),
                g_[2].clone(),
            ),
            None => (0.0, Value::Null, Value::Null),
        };
        let b = q_one(
            db,
            "SELECT p50 FROM salary_benchmarks WHERE period = ?1 AND COALESCE(position_id, 0) = COALESCE(?2, 0) AND COALESCE(department_id, 0) = COALESCE(?3, 0)".to_string(),
            vec![Value::Text(period.to_string()), pos, dep],
            1,
            "cmp.gap.bench",
        )
        .await?;
        if let Some(b_) = b {
            let p50 = match &b_[0] {
                Value::Float(v) => *v,
                Value::Int(v) => *v as f64,
                _ => 0.0,
            };
            if p50 > 0.0 {
                out.push(BenchmarkGapRow {
                    employee_id: to_dto_int(emp, "cmp.gap.emp")?,
                    employee_name: value_to_string(&r[1]),
                    salary: sal,
                    p50,
                    ratio: (sal / p50 * 10000.0).round() / 100.0,
                });
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::sea_raw::{exec, q_one, value_i64};

    async fn admin_id(db: &sea_orm::DatabaseConnection) -> i64 {
        let r = q_one(
            db,
            "SELECT id FROM users WHERE username = 'admin'".to_string(),
            vec![],
            1,
            "c.admin",
        )
        .await
        .expect("admin")
        .expect("ada");
        value_i64(&r[0]).unwrap_or(0)
    }

    async fn mkemp(db: &sea_orm::DatabaseConnection, number: &str, basic: f64) -> i64 {
        exec(
            db,
            "INSERT INTO employees (employee_number, first_name, company_id, join_date, employment_status, employment_type) VALUES (?1, 'Boni', 1, '2026-01-05', 'active', 'permanent')".to_string(),
            vec![Value::Text(number.to_string())],
            "c.emp",
        )
        .await
        .expect("karyawan");
        let rid = q_one(
            db,
            "SELECT last_insert_rowid()".to_string(),
            vec![],
            1,
            "c.rid",
        )
        .await
        .expect("rid")
        .expect("ada");
        let eid = value_i64(&rid[0]).unwrap_or(0);
        exec(
            db,
            "INSERT INTO employee_salaries (employee_id, basic_salary, effective_date, is_active) VALUES (?1, ?2, '2026-01-01', 1)".to_string(),
            vec![Value::Int(eid), Value::Float(basic)],
            "c.sal",
        )
        .await
        .expect("gaji");
        eid
    }

    fn skema(name: &str, kind: &str, amount: f64, percent: f64) -> BonusInput {
        BonusInput {
            name: name.to_string(),
            kind: kind.to_string(),
            amount,
            percent,
            threshold: 0.0,
            status: "active".to_string(),
        }
    }

    #[tokio::test]
    async fn bonus_skema_persen_dan_idempoten() {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin_id(db).await;
        let emp = mkemp(db, "EMP-CB1", 5_000_000.0).await;
        let sid = scheme_save_sea(db, actor, None, &skema("Bonus Kinerja", "percent_of_salary", 0.0, 10.0))
            .await
            .expect("skema") as i64;
        let err = scheme_save_sea(db, actor, None, &skema("Bonus Kinerja", "fixed", 100.0, 0.0))
            .await
            .expect_err("dup");
        assert!(err.contains("Nama skema sudah dipakai"));
        assert!(bonus_run_sea(db, actor, sid, "x").await.is_err());
        let dibuat = bonus_run_sea(db, actor, sid, "2026-07").await.expect("jalankan");
        assert!(dibuat >= 1);
        assert_eq!(bonus_run_sea(db, actor, sid, "2026-07").await.expect("idempoten"), 0);
        let list = bonus_list_sea(db, Some("2026-07")).await.expect("list");
        let baris = list.iter().find(|b| b.employee_id as i64 == emp).expect("bonus");
        assert!((baris.amount - 500_000.0).abs() < 1e-6);
        assert_eq!(baris.status, "draft");
        bonus_decide_sea(db, actor, baris.id as i64, "approved")
            .await
            .expect("setujui");
        let err = bonus_decide_sea(db, actor, baris.id as i64, "approved")
            .await
            .expect_err("ganda");
        assert!(err.contains("tidak bisa diubah"));
        scheme_save_sea(
            db,
            actor,
            Some(sid),
            &BonusInput {
                status: "inactive".to_string(),
                ..skema("Bonus Kinerja", "percent_of_salary", 0.0, 10.0)
            },
        )
        .await
        .expect("nonaktif");
        let err = bonus_run_sea(db, actor, sid, "2026-08").await.expect_err("aktif");
        assert!(err.contains("tidak aktif"));
    }

    #[tokio::test]
    async fn benefit_enrol_idempoten() {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin_id(db).await;
        let emp = mkemp(db, "EMP-CB2", 4_000_000.0).await;
        let bid = benefit_save_sea(
            db,
            actor,
            None,
            &BenefitInput {
                name: "Asuransi".to_string(),
                description: Some("Asuransi tambahan".to_string()),
                category: Some("kesehatan".to_string()),
                cost: 250_000.0,
                status: "active".to_string(),
            },
        )
        .await
        .expect("benefit") as i64;
        let err = benefit_save_sea(
            db,
            actor,
            None,
            &BenefitInput {
                name: "Asuransi".to_string(),
                status: "active".to_string(),
                ..Default::default()
            },
        )
        .await
        .expect_err("dup");
        assert!(err.contains("sudah dipakai"));
        let i1 = benefit_enroll_sea(db, actor, emp, bid, 2026)
            .await
            .expect("enrol");
        let i2 = benefit_enroll_sea(db, actor, emp, bid, 2026)
            .await
            .expect("enrol lagi");
        assert_eq!(i1, i2);
        assert!(benefit_enroll_sea(db, actor, emp, bid, 1980).await.is_err());
        let mine = my_benefits_sea(db, emp, 2026).await.expect("mine");
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].benefit_name, "Asuransi");
        assert!((mine[0].amount - 250_000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn merit_model_konsisten() {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        mkemp(db, "EMP-CB3", 10_000_000.0).await;
        let rows = merit_model_sea(db, 10.0).await.expect("model");
        assert!(!rows.is_empty());
        let tot: f64 = rows.iter().map(|r| r.total_current).sum();
        let next: f64 = rows.iter().map(|r| r.total_next).sum();
        assert!(tot >= 10_000_000.0 * 12.0);
        assert!((next - tot * 1.1).abs() < 1.0);
        let err = merit_model_sea(db, 150.0).await.expect_err("batas");
        assert!(err.contains("tidak valid"));
    }

    #[tokio::test]
    async fn compensation_review_tautkan_goal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &state.sea;
        let actor = admin_id(db).await;
        let emp = mkemp(db, "EMP-REV", 5_000_000.0).await;
        let kid = exec_insert(
            db,
            "INSERT INTO kpis (name, description) VALUES ('Produksi', NULL)".to_string(),
            vec![],
            "t.kpi",
        )
        .await
        .expect("kpi");
        let pidf = exec_insert(
            db,
            "INSERT INTO performance_periods (name, type, start_date, end_date, status) VALUES ('2026', 'annual', '2026-01-01', '2026-12-31', 'open')".to_string(),
            vec![],
            "t.per",
        )
        .await
        .expect("periode");
        exec_insert(
            db,
            "INSERT INTO employee_kpis (performance_period_id, employee_id, kpi_id, target, weight, actual, score) VALUES (?1, ?2, ?3, 100, 100, 95, 95)".to_string(),
            vec![Value::Int(pidf), Value::Int(emp), Value::Int(kid)],
            "t.ek",
        )
        .await
        .expect("ek");
        let rows = compensation_review_list_sea(db, 2026).await.expect("review");
        assert_eq!(rows.len(), 2);
        let boni = rows
            .iter()
            .find(|r| r.employee_name.trim() == "Boni")
            .expect("baris boni");
        assert_eq!(boni.basic_salary, 5_000_000.0);
        assert!((boni.score - 95.0).abs() < 1e-6);
        assert_eq!(boni.recommendation_percent, 7.0);
        let pos_bench = exec_insert(
            db,
            "INSERT INTO positions (code, name) VALUES ('BENCH', 'Penguji')".to_string(),
            vec![],
            "t.pos",
        )
        .await
        .expect("posisi");
        let bid = benchmark_save_sea(
            db,
            actor,
            None,
            &BenchmarkInput {
                position_id: Some(pos_bench as i32),
                department_id: None,
                p25: 4_000_000.0,
                p50: 6_000_000.0,
                p75: 9_000_000.0,
                source: "Survei 2026".to_string(),
                period: "2026-06".to_string(),
            },
        )
        .await
        .expect("benchmark");
        assert!(bid > 0);
        let err = benchmark_save_sea(
            db,
            actor,
            None,
            &BenchmarkInput {
                position_id: None,
                department_id: None,
                p25: 9_000_000.0,
                p50: 6_000_000.0,
                p75: 9_000_000.0,
                source: "X".to_string(),
                period: "2026-06".to_string(),
            },
        )
        .await
        .expect_err("rentang");
        assert!(err.contains("Rentang benchmark tidak valid."));
        let err = benchmark_save_sea(
            db,
            actor,
            None,
            &BenchmarkInput {
                position_id: None,
                department_id: None,
                p25: 4_000_000.0,
                p50: 6_000_000.0,
                p75: 9_000_000.0,
                source: "X".to_string(),
                period: "2026-13".to_string(),
            },
        )
        .await
        .expect_err("periode");
        assert!(err.contains("Format periode tidak valid."));
        let gap = benchmark_compare_sea(db, "2026-06").await.expect("banding");
        assert!(gap.is_empty());
        exec(
            db,
            "UPDATE employees SET position_id = ?1 WHERE id = ?2".to_string(),
            vec![Value::Int(pos_bench), Value::Int(emp)],
            "t.pos2",
        )
        .await
        .expect("posisi");
        let gap2 = benchmark_compare_sea(db, "2026-06").await.expect("banding2");
        assert_eq!(gap2.len(), 1);
        assert!((gap2[0].ratio - 5_000_000.0 / 6_000_000.0 * 100.0).abs() < 1.0);
    }

    #[tokio::test]
    async fn esop_vesting_cliff_dan_laporan() {
        let dir = tempfile::tempdir().expect("dir");
        let app = crate::init_state(dir.path().to_path_buf()).expect("state");
        let db = &app.sea;
        let actor = admin_id(db).await;
        let emp = mkemp(db, "EMP-ESOP", 5_000_000.0).await;
        assert!(esop_grant_sea(db, actor, &EsopGrantInput { employee_id: emp as i32, total_shares: 0, grant_date: "2026-01-01".to_string(), vest_months: 12, cliff_months: 3 }).await.is_err());
        assert!(esop_grant_sea(db, actor, &EsopGrantInput { employee_id: emp as i32, total_shares: 1200, grant_date: "bukan-tanggal".to_string(), vest_months: 12, cliff_months: 3 }).await.is_err());
        let gid = esop_grant_sea(db, actor, &EsopGrantInput { employee_id: emp as i32, total_shares: 1200, grant_date: "2026-01-01".to_string(), vest_months: 12, cliff_months: 3 }).await.expect("grant");
        assert!(gid > 0);
        let naik = esop_vest_run_sea(db, actor, "2026-06-01").await.expect("vest");
        assert_eq!(naik, 2);
        let daftar = esop_list_sea(db).await.expect("list");
        let baris = daftar.iter().find(|g| g.id == gid).expect("baris");
        assert_eq!(baris.vested_shares + baris.scheduled_shares, 1200);
        assert_eq!(baris.vested_shares, 200);
        let naik2 = esop_vest_run_sea(db, actor, "2027-06-01").await.expect("vest2");
        assert_eq!(naik2, 7);
        let daftar2 = esop_list_sea(db).await.expect("list2");
        let baris2 = daftar2.iter().find(|g| g.id == gid).expect("baris2");
        assert_eq!(baris2.vested_shares, 1200);
        assert_eq!(baris2.scheduled_shares, 0);
    }
}

// ---------------- ESOP vesting + kepemilikan ----------------

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EsopGrant {
    pub id: i32,
    pub employee_id: i32,
    pub employee_name: String,
    pub total_shares: i32,
    pub grant_date: String,
    pub vest_months: i32,
    pub cliff_months: i32,
    pub vested_shares: i32,
    pub scheduled_shares: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct EsopGrantInput {
    pub employee_id: i32,
    pub total_shares: i32,
    pub grant_date: String,
    pub vest_months: i32,
    pub cliff_months: i32,
}

fn tambah_bulan(tgl: NaiveDate, n: i32) -> NaiveDate {
    let total = tgl.month0() + n as u32;
    let tahun = tgl.year() + (total / 12) as i32;
    let bulan = total % 12 + 1;
    let batas = match bulan {
        2 => if tahun % 4 == 0 && (tahun % 100 != 0 || tahun % 400 == 0) { 29 } else { 28 },
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    NaiveDate::from_ymd_opt(tahun, bulan, tgl.day().min(batas)).unwrap_or(tgl)
}

pub async fn esop_grant_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    input: &EsopGrantInput,
) -> Result<i32, String> {
    if input.total_shares < 1 {
        return Err("Jumlah saham minimal 1.".to_string());
    }
    if input.vest_months < 1 || input.cliff_months < 0 || input.cliff_months > input.vest_months {
        return Err("Tenor vesting tidak valid.".to_string());
    }
    let grant = NaiveDate::parse_from_str(input.grant_date.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal grant tidak valid.".to_string())?;
    let now = now_str();
    let gid = exec_insert(
        db,
        "INSERT INTO esop_grants (employee_id, total_shares, grant_date, vest_months, cliff_months, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)".to_string(),
        vec![Value::Int(input.employee_id as i64), Value::Int(input.total_shares as i64), Value::Text(input.grant_date.trim().to_string()), Value::Int(input.vest_months as i64), Value::Int(input.cliff_months as i64), Value::Text(now)],
        "esop.grant",
    )
    .await?;
    let per_bulan = input.total_shares / input.vest_months;
    let sisa = input.total_shares % input.vest_months;
    for i in input.cliff_months..input.vest_months {
        let lembar = if i == input.vest_months - 1 { per_bulan + sisa + per_bulan * input.cliff_months } else { per_bulan };
        let tgl = tambah_bulan(grant, i + 1).format("%Y-%m-%d").to_string();
        exec(
            db,
            "INSERT OR IGNORE INTO esop_vestings (grant_id, vest_date, shares, status) VALUES (?1, ?2, ?3, 'scheduled')".to_string(),
            vec![Value::Int(gid), Value::Text(tgl), Value::Int(lembar as i64)],
            "esop.jadwal",
        )
        .await?;
    }
    audit::log_sea(db, Some(actor_id), "CREATE", "compensation.esop", Some(&gid.to_string()), None, None, None).await?;
    to_dto_int(gid, "esop.id")
}

pub async fn esop_vest_run_sea(
    db: &sea_orm::DatabaseConnection,
    actor_id: i64,
    as_of: &str,
) -> Result<i32, String> {
    NaiveDate::parse_from_str(as_of.trim(), "%Y-%m-%d")
        .map_err(|_| "Tanggal tidak valid.".to_string())?;
    let n = exec(
        db,
        "UPDATE esop_vestings SET status = 'vested' WHERE status = 'scheduled' AND vest_date <= ?1".to_string(),
        vec![Value::Text(as_of.trim().to_string())],
        "esop.vest",
    )
    .await?;
    audit::log_sea(db, Some(actor_id), "UPDATE", "compensation.esop.vest", None, None, None, None).await?;
    to_dto_int(n as i64, "esop.vested")
}

pub async fn esop_list_sea(db: &sea_orm::DatabaseConnection) -> Result<Vec<EsopGrant>, String> {
    let rows = q_all(
        db,
        "SELECT g.id, g.employee_id, TRIM(e.first_name || ' ' || COALESCE(e.last_name, '')), g.total_shares, g.grant_date, g.vest_months, g.cliff_months, COALESCE((SELECT SUM(shares) FROM esop_vestings v WHERE v.grant_id = g.id AND v.status = 'vested'), 0), COALESCE((SELECT SUM(shares) FROM esop_vestings v WHERE v.grant_id = g.id AND v.status = 'scheduled'), 0) FROM esop_grants g INNER JOIN employees e ON e.id = g.employee_id ORDER BY g.id".to_string(),
        vec![],
        9,
        "esop.list",
    )
    .await
    .map_err(|e| format!("gagal membaca ESOP: {e}"))?;
    rows.iter()
        .map(|r| {
            Ok(EsopGrant {
                id: to_dto_int(value_i64(&r[0]).unwrap_or(0), "esop.id")?,
                employee_id: to_dto_int(value_i64(&r[1]).unwrap_or(0), "esop.emp")?,
                employee_name: value_to_string(&r[2]),
                total_shares: to_dto_int(value_i64(&r[3]).unwrap_or(0), "esop.total")?,
                grant_date: value_to_string(&r[4]),
                vest_months: to_dto_int(value_i64(&r[5]).unwrap_or(0), "esop.vest")?,
                cliff_months: to_dto_int(value_i64(&r[6]).unwrap_or(0), "esop.cliff")?,
                vested_shares: to_dto_int(value_i64(&r[7]).unwrap_or(0), "esop.vested")?,
                scheduled_shares: to_dto_int(value_i64(&r[8]).unwrap_or(0), "esop.sched")?,
            })
        })
        .collect()
}

