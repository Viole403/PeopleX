-- Fase 3: inventaris lisensi software
CREATE TABLE IF NOT EXISTS software_licenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    vendor TEXT NULL,
    total_seats INTEGER NOT NULL DEFAULT 1,
    cost NUMERIC NOT NULL DEFAULT 0,
    expires_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS license_assignments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    license_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    assigned_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    revoked_at TEXT NULL,
    UNIQUE(license_id, employee_id),
    FOREIGN KEY(license_id) REFERENCES software_licenses(id),
    FOREIGN KEY(employee_id) REFERENCES employees(id)
);

CREATE INDEX IF NOT EXISTS idx_license_assignments_employee ON license_assignments(employee_id);
