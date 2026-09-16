-- Fase 3 (T1): kredensial sentral, outbox perintah, roster cermin per device.
-- device_commands = antrean push keluar;
-- device_roster cermin status pendaftaran per device (sidik/wajah per device, PIN/kartu lintas device).
CREATE TABLE IF NOT EXISTS fp_credentials (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    employee_id INTEGER NOT NULL,
    cred_type TEXT NOT NULL CHECK(cred_type IN ('pin','card')),
    cred_value TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, cred_type),
    FOREIGN KEY(employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS device_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    op TEXT NOT NULL CHECK(op IN ('enroll_pin','enroll_card','delete_pin','delete_card','sync_time','reboot','clear_logs')),
    payload TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','acked','failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS device_roster (
    device_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    cred_type TEXT NOT NULL CHECK(cred_type IN ('pin','card','finger','face')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','synced','failed')),
    pushed_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (device_id, employee_id, cred_type),
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE,
    FOREIGN KEY(employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

ALTER TABLE fingerprint_devices ADD COLUMN driver TEXT NULL;
ALTER TABLE fingerprint_devices ADD COLUMN conn_params TEXT NULL DEFAULT '{}';

CREATE INDEX IF NOT EXISTS idx_fp_credentials_employee ON fp_credentials(employee_id);
CREATE INDEX IF NOT EXISTS idx_device_commands_status ON device_commands(device_id, status);
CREATE INDEX IF NOT EXISTS idx_device_roster_device ON device_roster(device_id);
