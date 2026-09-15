-- Fase 2 (2.27): sinkronisasi mesin fingerprint ke absensi
-- Grup A (ZK/ADMS: ZKTeco, Solution, eSSL, dsb): push via webhook + pull port 4370
-- Grup B (Fingerspot Revo, Deli): cloud webhook generik + impor USB/CSV
CREATE TABLE IF NOT EXISTS fingerprint_devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    brand TEXT NOT NULL,
    model TEXT NULL,
    serial TEXT NULL,
    protocol TEXT NOT NULL DEFAULT 'adms' CHECK(protocol IN ('adms','zk_pull','usb','cloud','agent')),
    endpoint TEXT NULL,
    location_id INTEGER NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS fingerprint_enrollments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    device_pin TEXT NOT NULL,
    employee_id INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(device_id, device_pin),
    UNIQUE(device_id, employee_id),
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE,
    FOREIGN KEY(employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS fingerprint_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    device_pin TEXT NOT NULL,
    employee_id INTEGER NULL,
    event_time TEXT NOT NULL,
    verify_mode TEXT NULL,
    processed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE,
    FOREIGN KEY(employee_id) REFERENCES employees(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_fp_logs_device_time ON fingerprint_logs(device_id, event_time);
CREATE INDEX IF NOT EXISTS idx_fp_logs_processed ON fingerprint_logs(processed);
