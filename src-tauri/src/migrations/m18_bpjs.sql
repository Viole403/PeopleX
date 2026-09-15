-- Fase 2 (G11): tanggungan BPJS keluarga
CREATE TABLE IF NOT EXISTS bpjs_dependents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    employee_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    relation TEXT NOT NULL,
    birth_date TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(employee_id, name),
    FOREIGN KEY(employee_id) REFERENCES employees(id)
);

CREATE INDEX IF NOT EXISTS idx_bpjs_dependents_employee ON bpjs_dependents(employee_id);
