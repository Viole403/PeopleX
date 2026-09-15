-- Fase 2 (G11): kepatuhan ketenagakerjaan per wilayah
CREATE TABLE IF NOT EXISTS compliance_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    work_location_id INTEGER NULL,
    item TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'ok', 'violation')),
    notes TEXT NULL,
    checked_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(work_location_id) REFERENCES work_locations(id)
);

CREATE INDEX IF NOT EXISTS idx_compliance_checks_location ON compliance_checks(work_location_id);
