-- M33: tambah protokol easylink ke CHECK fingerprint_devices.
-- SQLite/MySQL bangun ulang tabel karena CHECK tak bisa ALTER dan FK dimatikan saat swap.
-- Postgres lepas lalu pasang ulang constraint (nama baku tabel_kolom_check).
-- only: sqlite
PRAGMA foreign_keys=OFF;
-- only: sqlite
CREATE TABLE fingerprint_devices_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    brand TEXT NOT NULL,
    model TEXT NULL,
    serial TEXT NULL,
    protocol TEXT NOT NULL DEFAULT 'adms' CHECK(protocol IN ('adms','zk_pull','usb','cloud','agent','easylink')),
    endpoint TEXT NULL,
    location_id INTEGER NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    driver TEXT NULL,
    conn_params TEXT NULL DEFAULT '{}'
);
-- only: sqlite
INSERT INTO fingerprint_devices_new
    (id, name, brand, model, serial, protocol, endpoint, location_id, is_active, created_at, updated_at, deleted_at, driver, conn_params)
    SELECT id, name, brand, model, serial, protocol, endpoint, location_id, is_active, created_at, updated_at, deleted_at, driver, conn_params
    FROM fingerprint_devices;
-- only: sqlite
DROP TABLE fingerprint_devices;
-- only: sqlite
ALTER TABLE fingerprint_devices_new RENAME TO fingerprint_devices;
-- only: sqlite
PRAGMA foreign_keys=ON;
-- only: postgres
ALTER TABLE fingerprint_devices DROP CONSTRAINT fingerprint_devices_protocol_check;
-- only: postgres
ALTER TABLE fingerprint_devices ADD CONSTRAINT fingerprint_devices_protocol_check CHECK (protocol IN ('adms','zk_pull','usb','cloud','agent','easylink'));
-- only: mysql
ALTER TABLE fingerprint_devices DROP CHECK fingerprint_devices_chk_1;
-- only: mysql
ALTER TABLE fingerprint_devices ADD CHECK (protocol IN ('adms','zk_pull','usb','cloud','agent','easylink'));
