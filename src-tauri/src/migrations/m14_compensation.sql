CREATE TABLE IF NOT EXISTS bonus_schemes (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL DEFAULT 'fixed' CHECK (kind IN ('fixed', 'percent_of_salary', 'attendance')),
    amount NUMERIC NOT NULL DEFAULT 0,
    percent NUMERIC NOT NULL DEFAULT 0,
    threshold NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS bonuses (
    id INTEGER PRIMARY KEY,
    scheme_id INTEGER NOT NULL REFERENCES bonus_schemes(id),
    employee_id INTEGER NOT NULL REFERENCES employees(id),
    period TEXT NOT NULL,
    amount NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'approved', 'paid')),
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (scheme_id, employee_id, period)
);
CREATE TABLE IF NOT EXISTS benefits (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    cost NUMERIC NOT NULL DEFAULT 0,
    category TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS benefit_enrollments (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL REFERENCES employees(id),
    benefit_id INTEGER NOT NULL REFERENCES benefits(id),
    year INTEGER NOT NULL,
    amount NUMERIC NOT NULL DEFAULT 0,
    elected_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, benefit_id, year)
);
CREATE INDEX IF NOT EXISTS idx_bonuses_scheme ON bonuses (scheme_id);
CREATE INDEX IF NOT EXISTS idx_benefit_enrollments_employee ON benefit_enrollments (employee_id);
