ALTER TABLE employee_contracts ADD COLUMN signature_name TEXT NULL;
ALTER TABLE employee_contracts ADD COLUMN signature_hash TEXT NULL;
ALTER TABLE employee_contracts ADD COLUMN signed_at TEXT NULL;
CREATE TABLE IF NOT EXISTS background_checks (
    id INTEGER PRIMARY KEY,
    candidate_id INTEGER NOT NULL,
    kind TEXT NOT NULL DEFAULT 'reference' CHECK (kind IN ('criminal', 'education', 'employment', 'reference')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'clear', 'flagged')),
    result TEXT NULL,
    checked_by INTEGER NULL,
    checked_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (candidate_id) REFERENCES candidates(id),
    FOREIGN KEY (checked_by) REFERENCES users(id)
);
CREATE INDEX IF NOT EXISTS idx_background_checks_candidate ON background_checks(candidate_id);
