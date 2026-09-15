-- Fase 3: ESOP vesting schedule + kepemilikan
CREATE TABLE IF NOT EXISTS esop_grants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    employee_id INTEGER NOT NULL,
    total_shares INTEGER NOT NULL,
    grant_date TEXT NOT NULL,
    vest_months INTEGER NOT NULL DEFAULT 48,
    cliff_months INTEGER NOT NULL DEFAULT 12,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS esop_vestings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    grant_id INTEGER NOT NULL,
    vest_date TEXT NOT NULL,
    shares INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'scheduled' CHECK(status IN ('scheduled','vested','cancelled')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(grant_id, vest_date),
    FOREIGN KEY(grant_id) REFERENCES esop_grants(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_esop_grants_employee ON esop_grants(employee_id);
