//! Survei denyut: buat, terbitkan, jawab, rekap, dan tutup.

use chrono::Local;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use crate::entities::{
    department, employee, notification, pulse_question, pulse_response, pulse_survey, user,
};
use crate::to_dto_int;

const KINDS: &[&str] = &["scale", "text"];
const RECURRENCES: &[&str] = &["none", "weekly", "monthly"];

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PulseSurvey {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub recurrence: String,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub question_count: i32,
    pub respondents: i32,
    pub created_at: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PulseQuestion {
    pub id: i32,
    pub question: String,
    pub kind: String,
    pub position: i32,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PulseDetail {
    pub survey: PulseSurvey,
    pub questions: Vec<PulseQuestion>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct QuestionInput {
    pub question: String,
    pub kind: String,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct PulseInput {
    pub title: String,
    pub description: Option<String>,
    pub recurrence: String,
    pub questions: Vec<QuestionInput>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug, Default)]
pub struct AnswerInput {
    pub question_id: i32,
    pub score: Option<i32>,
    pub answer: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct QuestionResult {
    pub question_id: i32,
    pub question: String,
    pub kind: String,
    pub respondents: i32,
    pub avg_score: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct PulseResults {
    pub respondents: i32,
    pub items: Vec<QuestionResult>,
}

fn now_str() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

async fn employee_of(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
) -> Result<Option<i32>, String> {
    user::Entity::find_by_id(user_id as i32)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat akun: {e}"))
        .map(|u| u.and_then(|u| u.employee_id))
}

async fn find_survey(
    db: &sea_orm::DatabaseConnection,
    id: i32,
) -> Result<pulse_survey::Model, String> {
    pulse_survey::Entity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| format!("gagal memuat survei: {e}"))?
        .ok_or_else(|| "Survei tidak ditemukan.".to_string())
}

async fn questions_of(
    db: &sea_orm::DatabaseConnection,
    survey_id: i32,
) -> Result<Vec<pulse_question::Model>, String> {
    pulse_question::Entity::find()
        .filter(pulse_question::Column::SurveyId.eq(survey_id))
        .order_by_asc(pulse_question::Column::Position)
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat pertanyaan: {e}"))
}

fn map_question(q: &pulse_question::Model) -> Result<PulseQuestion, String> {
    Ok(PulseQuestion {
        id: to_dto_int(q.id as i64, "pertanyaan")?,
        question: q.question.clone(),
        kind: q.kind.clone(),
        position: q.position,
    })
}

async fn map_survey(
    db: &sea_orm::DatabaseConnection,
    s: &pulse_survey::Model,
) -> Result<PulseSurvey, String> {
    let questions = questions_of(db, s.id).await?;
    let responses = pulse_response::Entity::find()
        .filter(pulse_response::Column::SurveyId.eq(s.id))
        .all(db)
        .await
        .map_err(|e| format!("gagal menghitung jawaban: {e}"))?;
    let mut seen = std::collections::HashSet::new();
    for r in &responses {
        seen.insert(r.employee_id);
    }
    Ok(PulseSurvey {
        id: to_dto_int(s.id as i64, "survei")?,
        title: s.title.clone(),
        description: s.description.clone(),
        status: s.status.clone(),
        recurrence: s.recurrence.clone(),
        period_start: s.period_start.clone(),
        period_end: s.period_end.clone(),
        question_count: questions.len() as i32,
        respondents: seen.len() as i32,
        created_at: s.created_at.clone(),
    })
}

pub async fn list(db: &sea_orm::DatabaseConnection) -> Result<Vec<PulseSurvey>, String> {
    let rows = pulse_survey::Entity::find()
        .order_by_desc(pulse_survey::Column::Id)
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat survei: {e}"))?;
    let mut out = Vec::with_capacity(rows.len());
    for s in &rows {
        out.push(map_survey(db, s).await?);
    }
    Ok(out)
}

pub async fn get(
    db: &sea_orm::DatabaseConnection,
    id: i32,
) -> Result<PulseDetail, String> {
    let s = find_survey(db, id).await?;
    let questions = questions_of(db, id).await?;
    let mut items = Vec::with_capacity(questions.len());
    for q in &questions {
        items.push(map_question(q)?);
    }
    Ok(PulseDetail {
        survey: map_survey(db, &s).await?,
        questions: items,
    })
}

pub async fn create(
    db: &sea_orm::DatabaseConnection,
    actor: i64,
    input: &PulseInput,
) -> Result<i32, String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err("Judul survei wajib diisi.".to_string());
    }
    if !RECURRENCES.contains(&input.recurrence.as_str()) {
        return Err("Pola pengulangan tidak valid.".to_string());
    }
    if input.questions.is_empty() {
        return Err("Survei membutuhkan minimal satu pertanyaan.".to_string());
    }
    for q in &input.questions {
        if q.question.trim().is_empty() {
            return Err("Pertanyaan tidak boleh kosong.".to_string());
        }
        if !KINDS.contains(&q.kind.as_str()) {
            return Err("Jenis pertanyaan tidak valid.".to_string());
        }
    }
    let res = pulse_survey::ActiveModel {
        title: Set(title.to_string()),
        description: Set(input.description.clone()),
        status: Set("draft".to_string()),
        recurrence: Set(input.recurrence.clone()),
        created_by: Set(Some(actor as i32)),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("gagal menyimpan survei: {e}"))?;
    for (i, q) in input.questions.iter().enumerate() {
        pulse_question::ActiveModel {
            survey_id: Set(res.id),
            question: Set(q.question.trim().to_string()),
            kind: Set(q.kind.clone()),
            position: Set(i as i32),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal menyimpan pertanyaan: {e}"))?;
    }
    to_dto_int(res.id as i64, "survei")
}

pub async fn publish(db: &sea_orm::DatabaseConnection, id: i32) -> Result<(), String> {
    let s = find_survey(db, id).await?;
    if s.status != "draft" {
        return Err("Hanya survei draf yang dapat diterbitkan.".to_string());
    }
    let mut am = s.clone().into_active_model();
    am.status = Set("active".to_string());
    if s.period_start.is_none() {
        am.period_start = Set(Some(today_str()));
    }
    am.update(db)
        .await
        .map_err(|e| format!("gagal menerbitkan survei: {e}"))?;
    let users = user::Entity::find()
        .filter(user::Column::EmployeeId.is_not_null())
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat penerima: {e}"))?;
    for u in &users {
        notification::ActiveModel {
            user_id: Set(u.id),
            kind: Set("pulse".to_string()),
            title: Set(format!("Survei baru: {}", s.title)),
            message: Set(s
                .description
                .clone()
                .or_else(|| Some("Mohon isi survei sebelum periode berakhir.".to_string()))),
            link: Set(None),
            is_read: Set(0),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal mengirim pemberitahuan: {e}"))?;
    }
    Ok(())
}

pub async fn answer(
    db: &sea_orm::DatabaseConnection,
    user_id: i64,
    survey_id: i32,
    items: &[AnswerInput],
) -> Result<(), String> {
    let s = find_survey(db, survey_id).await?;
    if s.status != "active" {
        return Err("Survei tidak aktif.".to_string());
    }
    let employee = employee_of(db, user_id)
        .await?
        .ok_or_else(|| "Akun belum terhubung ke data karyawan.".to_string())?;
    if items.is_empty() {
        return Err("Jawaban tidak boleh kosong.".to_string());
    }
    let questions = questions_of(db, survey_id).await?;
    for item in items {
        let q = questions
            .iter()
            .find(|q| q.id == item.question_id)
            .ok_or_else(|| "Pertanyaan tidak termasuk survei ini.".to_string())?;
        if q.kind == "scale" {
            match item.score {
                Some(v) if (1..=5).contains(&v) => {}
                _ => return Err("Skor wajib diisi 1 sampai 5.".to_string()),
            }
        } else if item.answer.as_deref().unwrap_or("").trim().is_empty() {
            return Err("Jawaban wajib diisi.".to_string());
        }
        let exists = pulse_response::Entity::find()
            .filter(pulse_response::Column::SurveyId.eq(survey_id))
            .filter(pulse_response::Column::QuestionId.eq(item.question_id))
            .filter(pulse_response::Column::EmployeeId.eq(employee))
            .one(db)
            .await
            .map_err(|e| format!("gagal memeriksa jawaban: {e}"))?;
        if exists.is_some() {
            return Err("Anda sudah menjawab survei ini.".to_string());
        }
        pulse_response::ActiveModel {
            survey_id: Set(survey_id),
            question_id: Set(item.question_id),
            employee_id: Set(employee),
            score: Set(item.score),
            answer: Set(item.answer.clone()),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal menyimpan jawaban: {e}"))?;
    }
    Ok(())
}

pub async fn results(
    db: &sea_orm::DatabaseConnection,
    id: i32,
) -> Result<PulseResults, String> {
    find_survey(db, id).await?;
    let questions = questions_of(db, id).await?;
    let responses = pulse_response::Entity::find()
        .filter(pulse_response::Column::SurveyId.eq(id))
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat jawaban: {e}"))?;
    let mut people = std::collections::HashSet::new();
    for r in &responses {
        people.insert(r.employee_id);
    }
    let mut items = Vec::with_capacity(questions.len());
    for q in &questions {
        let rows: Vec<_> = responses.iter().filter(|r| r.question_id == q.id).collect();
        let avg = if q.kind == "scale" && !rows.is_empty() {
            let sum: i64 = rows.iter().filter_map(|r| r.score.map(|v| v as i64)).sum();
            let n = rows.iter().filter(|r| r.score.is_some()).count().max(1) as f64;
            Some((sum as f64 / n * 10.0).round() / 10.0)
        } else {
            None
        };
        items.push(QuestionResult {
            question_id: to_dto_int(q.id as i64, "pertanyaan")?,
            question: q.question.clone(),
            kind: q.kind.clone(),
            respondents: rows.len() as i32,
            avg_score: avg,
        });
    }
    Ok(PulseResults {
        respondents: people.len() as i32,
        items,
    })
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Clone, Debug)]
pub struct EnpsRow {
    pub department_id: i32,
    pub department: String,
    pub promoters: i32,
    pub passives: i32,
    pub detractors: i32,
    pub total: i32,
    pub score: f64,
}

pub async fn enps_by_department(
    db: &sea_orm::DatabaseConnection,
    id: i32,
) -> Result<Vec<EnpsRow>, String> {
    find_survey(db, id).await?;
    let scale: Vec<i32> = questions_of(db, id)
        .await?
        .into_iter()
        .filter(|q| q.kind == "scale")
        .map(|q| q.id)
        .collect();
    if scale.is_empty() {
        return Ok(Vec::new());
    }
    let answers = pulse_response::Entity::find()
        .filter(pulse_response::Column::SurveyId.eq(id))
        .filter(pulse_response::Column::Score.is_not_null())
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat jawaban: {e}"))?;
    let scored: Vec<&pulse_response::Model> = answers
        .iter()
        .filter(|r| scale.contains(&r.question_id))
        .collect();
    if scored.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<i32> = scored
        .iter()
        .map(|r| r.employee_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let emps = employee::Entity::find()
        .filter(employee::Column::Id.is_in(ids))
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat karyawan: {e}"))?;
    let dept_of: std::collections::HashMap<i32, Option<i32>> =
        emps.into_iter().map(|e| (e.id, e.department_id)).collect();
    let need: Vec<i32> = dept_of
        .values()
        .filter_map(|d| *d)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let names: std::collections::HashMap<i32, String> = department::Entity::find()
        .filter(department::Column::Id.is_in(need))
        .all(db)
        .await
        .map_err(|e| format!("gagal memuat departemen: {e}"))?
        .into_iter()
        .map(|d| (d.id, d.name))
        .collect();
    let mut agg: std::collections::HashMap<i32, (i32, i32, i32)> =
        std::collections::HashMap::new();
    for r in &scored {
        let dept = dept_of.get(&r.employee_id).and_then(|d| *d).unwrap_or(0);
        let e = agg.entry(dept).or_insert((0, 0, 0));
        match r.score.unwrap_or(0) {
            5 => e.0 += 1,
            4 => e.1 += 1,
            _ => e.2 += 1,
        }
    }
    let mut rows: Vec<EnpsRow> = agg
        .into_iter()
        .map(|(dept, (prom, pas, det))| {
            let total = prom + pas + det;
            let score = (prom - det) as f64 / total.max(1) as f64 * 100.0;
            EnpsRow {
                department_id: dept,
                department: names
                    .get(&dept)
                    .cloned()
                    .unwrap_or_else(|| "Tanpa departemen".to_string()),
                promoters: prom,
                passives: pas,
                detractors: det,
                total,
                score: (score * 10.0).round() / 10.0,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.department.cmp(&b.department));
    Ok(rows)
}

pub async fn close(
    db: &sea_orm::DatabaseConnection,
    id: i32,
) -> Result<Option<i32>, String> {
    let s = find_survey(db, id).await?;
    if s.status != "active" {
        return Err("Hanya survei aktif yang dapat ditutup.".to_string());
    }
    let mut am = s.clone().into_active_model();
    am.status = Set("closed".to_string());
    am.closed_at = Set(Some(now_str()));
    am.update(db)
        .await
        .map_err(|e| format!("gagal menutup survei: {e}"))?;
    if s.recurrence == "none" {
        return Ok(None);
    }
    let next = pulse_survey::ActiveModel {
        title: Set(s.title.clone()),
        description: Set(s.description.clone()),
        status: Set("draft".to_string()),
        recurrence: Set(s.recurrence.clone()),
        created_by: Set(s.created_by),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("gagal membuat survei lanjutan: {e}"))?;
    let questions = questions_of(db, id).await?;
    for q in &questions {
        pulse_question::ActiveModel {
            survey_id: Set(next.id),
            question: Set(q.question.clone()),
            kind: Set(q.kind.clone()),
            position: Set(q.position),
            ..Default::default()
        }
        .insert(db)
        .await
        .map_err(|e| format!("gagal menyalin pertanyaan: {e}"))?;
    }
    Ok(Some(to_dto_int(next.id as i64, "survei")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn alur_pulse_dari_draf_sampai_tutup() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::init_state(dir.path().to_path_buf()).expect("init_state");
        let db = &state.sea;
        let sid = create(
            db,
            1,
            &PulseInput {
                title: "Pulse Q3".to_string(),
                description: None,
                recurrence: "none".to_string(),
                questions: vec![
                    QuestionInput {
                        question: "Seberapa puas minggu ini?".to_string(),
                        kind: "scale".to_string(),
                    },
                    QuestionInput {
                        question: "Saran perbaikan?".to_string(),
                        kind: "text".to_string(),
                    },
                ],
            },
        )
        .await
        .unwrap();
        let detail = get(db, sid).await.unwrap();
        assert_eq!(detail.questions.len(), 2);
        let q_scale = detail.questions[0].id;
        let q_text = detail.questions[1].id;
        let err = answer(
            db,
            1,
            sid,
            &[AnswerInput {
                question_id: q_scale,
                score: Some(4),
                answer: None,
            }],
        )
        .await
        .unwrap_err();
        assert!(err.contains("aktif"), "pesan: {err}");
        publish(db, sid).await.unwrap();
        let notif = notification::Entity::find()
            .filter(notification::Column::Kind.eq("pulse"))
            .all(db)
            .await
            .unwrap();
        assert!(!notif.is_empty());
        let err = answer(
            db,
            1,
            sid,
            &[AnswerInput {
                question_id: q_scale,
                score: Some(6),
                answer: None,
            }],
        )
        .await
        .unwrap_err();
        assert!(err.contains("Skor"), "pesan: {err}");
        answer(
            db,
            1,
            sid,
            &[
                AnswerInput {
                    question_id: q_scale,
                    score: Some(4),
                    answer: None,
                },
                AnswerInput {
                    question_id: q_text,
                    score: None,
                    answer: Some("Baik".to_string()),
                },
            ],
        )
        .await
        .unwrap();
        let err = answer(
            db,
            1,
            sid,
            &[AnswerInput {
                question_id: q_scale,
                score: Some(5),
                answer: None,
            }],
        )
        .await
        .unwrap_err();
        assert!(err.contains("sudah menjawab"), "pesan: {err}");
        let r = results(db, sid).await.unwrap();
        assert_eq!(r.respondents, 1);
        assert_eq!(r.items[0].avg_score, Some(4.0));
        assert_eq!(r.items[1].avg_score, None);
        let next = close(db, sid).await.unwrap();
        assert!(next.is_none());
        let d = get(db, sid).await.unwrap();
        assert_eq!(d.survey.status, "closed");
    }

    #[tokio::test]
    async fn tutup_berkala_membuat_draf_lanjutan() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::init_state(dir.path().to_path_buf()).expect("init_state");
        let db = &state.sea;
        let sid = create(
            db,
            1,
            &PulseInput {
                title: "Pulse Bulanan".to_string(),
                description: None,
                recurrence: "monthly".to_string(),
                questions: vec![QuestionInput {
                    question: "Semangat kerja?".to_string(),
                    kind: "scale".to_string(),
                }],
            },
        )
        .await
        .unwrap();
        publish(db, sid).await.unwrap();
        let next = close(db, sid).await.unwrap().expect("draf lanjutan");
        let d = get(db, next).await.unwrap();
        assert_eq!(d.survey.status, "draft");
        assert_eq!(d.survey.recurrence, "monthly");
        assert_eq!(d.questions.len(), 1);
        let all = list(db).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn enps_mengelompokkan_per_departemen() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::init_state(dir.path().to_path_buf()).expect("init_state");
        let db = &state.sea;
        let mut depts = department::Entity::find()
            .order_by_asc(department::Column::Id)
            .all(db)
            .await
            .unwrap();
        assert!(depts.len() >= 2, "seed wajib punya 2 departemen");
        let b = depts.remove(1);
        let a = depts.remove(0);
        let admin_emp = employee_of(db, 1).await.unwrap().expect("pegawai admin");
        let base = employee::Entity::find_by_id(admin_emp)
            .one(db)
            .await
            .unwrap()
            .expect("baris pegawai");
        let (comp, join) = (base.company_id, base.join_date.clone());
        let mut am = base.into_active_model();
        am.department_id = Set(Some(a.id));
        am.update(db).await.unwrap();
        let sid = create(
            db,
            1,
            &PulseInput {
                title: "Pulse eNPS".to_string(),
                description: None,
                recurrence: "none".to_string(),
                questions: vec![QuestionInput {
                    question: "Rekomendasi?".to_string(),
                    kind: "scale".to_string(),
                }],
            },
        )
        .await
        .unwrap();
        publish(db, sid).await.unwrap();
        let qid = get(db, sid).await.unwrap().questions[0].id;
        answer(
            db,
            1,
            sid,
            &[AnswerInput {
                question_id: qid,
                score: Some(5),
                answer: None,
            }],
        )
        .await
        .unwrap();
        let e2 = employee::ActiveModel {
            employee_number: Set("EMP-ENPS".to_string()),
            first_name: Set("Sari".to_string()),
            company_id: Set(comp),
            join_date: Set(join),
            department_id: Set(Some(b.id)),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
        let u2 = user::ActiveModel {
            username: Set("enps".to_string()),
            email: Set("enps@uji.test".to_string()),
            password: Set("sandi".to_string()),
            employee_id: Set(Some(e2.id)),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
        answer(
            db,
            u2.id as i64,
            sid,
            &[AnswerInput {
                question_id: qid,
                score: Some(2),
                answer: None,
            }],
        )
        .await
        .unwrap();
        let rows = enps_by_department(db, sid).await.unwrap();
        assert_eq!(rows.len(), 2);
        let ra = rows.iter().find(|r| r.department_id == a.id).expect("baris a");
        assert_eq!((ra.promoters, ra.passives, ra.detractors, ra.total), (1, 0, 0, 1));
        assert_eq!(ra.score, 100.0);
        let rb = rows.iter().find(|r| r.department_id == b.id).expect("baris b");
        assert_eq!((rb.promoters, rb.passives, rb.detractors, rb.total), (0, 0, 1, 1));
        assert_eq!(rb.score, -100.0);
    }

    #[test]
    fn validasi_input_ditolak() {
        assert!(RECURRENCES.contains(&"weekly"));
        assert!(!RECURRENCES.contains(&"harian"));
        assert!(KINDS.contains(&"text"));
        assert!(!KINDS.contains(&"angka"));
    }
}
