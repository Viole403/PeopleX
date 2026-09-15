-- Fase 3: kurs multi-currency + kontraktor lintas negara
CREATE TABLE IF NOT EXISTS currency_rates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL,
    rate_to_idr NUMERIC NOT NULL,
    as_of TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(code, as_of)
);

CREATE TABLE IF NOT EXISTS contractors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    country TEXT NOT NULL DEFAULT 'ID',
    currency TEXT NOT NULL DEFAULT 'IDR',
    email TEXT NULL,
    phone TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS contractor_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    contractor_id INTEGER NOT NULL,
    period TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    currency TEXT NOT NULL DEFAULT 'IDR',
    amount_idr NUMERIC NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','paid')),
    paid_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(contractor_id) REFERENCES contractors(id)
);

CREATE INDEX IF NOT EXISTS idx_contractor_payments_contractor ON contractor_payments(contractor_id);
