-- Fase 3 (T7): blocklist kartu RFID hilang + label verify mode di sisi kode.
-- Kartu adalah atribut user (fp_credentials cred_type card) dan blocklist per nomor kartu global.
CREATE TABLE IF NOT EXISTS card_blocklist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    card_number TEXT NOT NULL UNIQUE,
    reason TEXT NULL,
    blocked_by INTEGER NULL,
    blocked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(blocked_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_card_blocklist_number ON card_blocklist(card_number);
