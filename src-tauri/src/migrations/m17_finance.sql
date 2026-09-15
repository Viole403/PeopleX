-- Fase 2 (G9): jurnal umum, requisition lowongan, depresiasi aset
ALTER TABLE assets ADD COLUMN useful_life_months INTEGER NULL;
ALTER TABLE assets ADD COLUMN salvage_value NUMERIC NULL;

CREATE TABLE IF NOT EXISTS gl_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    period TEXT NOT NULL,
    account TEXT NOT NULL,
    debit NUMERIC NOT NULL DEFAULT 0,
    credit NUMERIC NOT NULL DEFAULT 0,
    description TEXT NULL,
    source TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS requisitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    requested_by INTEGER NOT NULL,
    position_id INTEGER NULL,
    department_id INTEGER NULL,
    headcount INTEGER NOT NULL DEFAULT 1,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected')),
    approved_by INTEGER NULL,
    approved_at TEXT NULL,
    vacancy_id INTEGER NULL,
    notes TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (requested_by) REFERENCES users (id),
    FOREIGN KEY (position_id) REFERENCES positions (id),
    FOREIGN KEY (department_id) REFERENCES departments (id),
    FOREIGN KEY (approved_by) REFERENCES users (id),
    FOREIGN KEY (vacancy_id) REFERENCES vacancies (id)
);

CREATE TABLE IF NOT EXISTS asset_depreciations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    asset_id INTEGER NOT NULL,
    period TEXT NOT NULL,
    opening NUMERIC NOT NULL,
    depreciation NUMERIC NOT NULL,
    closing NUMERIC NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (asset_id, period),
    FOREIGN KEY (asset_id) REFERENCES assets (id)
);

CREATE INDEX IF NOT EXISTS idx_gl_entries_period ON gl_entries (period);
CREATE INDEX IF NOT EXISTS idx_requisitions_status ON requisitions (status);
