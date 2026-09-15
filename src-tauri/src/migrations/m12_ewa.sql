CREATE TABLE IF NOT EXISTS ewa_withdrawals (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    amount NUMERIC NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','paid')),
    requested_at TEXT DEFAULT CURRENT_TIMESTAMP,
    decided_by INTEGER NULL,
    decided_at TEXT NULL,
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (decided_by) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_ewa_status ON ewa_withdrawals(status);
CREATE INDEX IF NOT EXISTS idx_ewa_employee ON ewa_withdrawals(employee_id);
