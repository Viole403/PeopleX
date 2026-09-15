ALTER TABLE attendances ADD COLUMN clock_in_photo TEXT NULL;
CREATE TABLE IF NOT EXISTS liveness_challenges (
    nonce VARCHAR(64) PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    instruction TEXT NOT NULL,
    issued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    used INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);
CREATE INDEX IF NOT EXISTS idx_liveness_employee ON liveness_challenges(employee_id);
