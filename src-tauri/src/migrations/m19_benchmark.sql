-- Fase 2 (G11): benchmark gaji eksternal
CREATE TABLE IF NOT EXISTS salary_benchmarks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    position_id INTEGER NOT NULL DEFAULT 0,
    department_id INTEGER NOT NULL DEFAULT 0,
    level TEXT NOT NULL DEFAULT '',
    p25 NUMERIC NOT NULL,
    p50 NUMERIC NOT NULL,
    p75 NUMERIC NOT NULL,
    source TEXT NOT NULL DEFAULT 'survei',
    period TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(period, position_id, level),
    FOREIGN KEY(position_id) REFERENCES positions(id)
);

CREATE INDEX IF NOT EXISTS idx_salary_benchmarks_period ON salary_benchmarks(period);
