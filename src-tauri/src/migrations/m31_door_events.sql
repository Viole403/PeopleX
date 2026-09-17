-- M31: event pintu (T5) + tambah op open_door ke antrean perintah.
-- SQLite tidak bisa ALTER CHECK: bangun ulang device_commands dengan data disalin.
CREATE TABLE device_commands_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    op TEXT NOT NULL CHECK(op IN ('enroll_pin','enroll_card','delete_pin','delete_card','sync_time','reboot','clear_logs','open_door')),
    payload TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','acked','failed')),
    attempts INTEGER NOT NULL DEFAULT 0,
    last_error TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE
);
INSERT INTO device_commands_new
    (id, device_id, op, payload, status, attempts, last_error, created_at, updated_at)
    SELECT id, device_id, op, payload, status, attempts, last_error, created_at, updated_at
    FROM device_commands;
DROP TABLE device_commands;
ALTER TABLE device_commands_new RENAME TO device_commands;

CREATE TABLE IF NOT EXISTS door_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id INTEGER NOT NULL,
    employee_id INTEGER NULL,
    method TEXT NOT NULL CHECK(method IN ('card','finger','face','palm','password','combo','remote','unknown')),
    granted INTEGER NOT NULL DEFAULT 1,
    event_time TEXT NOT NULL,
    raw_line TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE,
    FOREIGN KEY(employee_id) REFERENCES employees(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_door_events_device_time ON door_events(device_id, event_time);
CREATE INDEX IF NOT EXISTS idx_door_events_employee ON door_events(employee_id);
