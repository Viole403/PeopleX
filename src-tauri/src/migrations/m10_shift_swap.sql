-- Tukar shift antar karyawan dengan persetujuan supervisor/HR.
CREATE TABLE IF NOT EXISTS shift_swaps (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    target_employee_id INTEGER NOT NULL,
    shift_date TEXT NOT NULL,
    from_shift_id INTEGER NULL,
    to_shift_id INTEGER NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    requested_at TEXT DEFAULT CURRENT_TIMESTAMP,
    decided_by INTEGER NULL,
    decided_at TEXT NULL,
    notes TEXT NULL,
    CHECK (status IN ('pending','approved','rejected','cancelled')),
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (target_employee_id) REFERENCES employees(id),
    FOREIGN KEY (from_shift_id) REFERENCES shifts(id),
    FOREIGN KEY (to_shift_id) REFERENCES shifts(id)
);

CREATE INDEX IF NOT EXISTS idx_shift_swaps_status ON shift_swaps(status);
CREATE INDEX IF NOT EXISTS idx_shift_swaps_employee ON shift_swaps(employee_id);
