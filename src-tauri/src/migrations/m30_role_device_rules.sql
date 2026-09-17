-- T3: graph akses role -> device fingerprint.
-- Satu baris = satu role boleh memakai satu device. Resolver: role -> devices aktif,
-- role -> anggota (via user_roles + users.employee_id). enroll_bulk memakai PIN
-- sentral (fp_credentials) tiap anggota, enqueue enroll_pin, catat roster.
CREATE TABLE IF NOT EXISTS role_device_rules (
    id INTEGER PRIMARY KEY,
    role_id INTEGER NOT NULL,
    device_id INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(role_id, device_id),
    FOREIGN KEY(role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY(device_id) REFERENCES fingerprint_devices(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_role_device_role ON role_device_rules(role_id);
CREATE INDEX IF NOT EXISTS idx_role_device_device ON role_device_rules(device_id);
