CREATE TABLE IF NOT EXISTS pulse_surveys (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    recurrence TEXT NOT NULL DEFAULT 'none',
    period_start TEXT NULL,
    period_end TEXT NULL,
    created_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    closed_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS pulse_questions (
    id INTEGER PRIMARY KEY,
    survey_id INTEGER NOT NULL REFERENCES pulse_surveys(id) ON DELETE CASCADE,
    question TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'scale',
    position INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS pulse_responses (
    id INTEGER PRIMARY KEY,
    survey_id INTEGER NOT NULL REFERENCES pulse_surveys(id) ON DELETE CASCADE,
    question_id INTEGER NOT NULL REFERENCES pulse_questions(id) ON DELETE CASCADE,
    employee_id INTEGER NOT NULL,
    score INTEGER NULL,
    answer TEXT NULL,
    submitted_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (survey_id, question_id, employee_id)
);

CREATE INDEX IF NOT EXISTS idx_pulse_questions_survey ON pulse_questions(survey_id);
CREATE INDEX IF NOT EXISTS idx_pulse_responses_survey ON pulse_responses(survey_id);
