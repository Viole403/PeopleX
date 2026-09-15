-- Fase 3: OKR cascading, 360 feedback, kalibrasi, succession
CREATE TABLE IF NOT EXISTS goals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    performance_period_id INTEGER NOT NULL,
    parent_id INTEGER NULL,
    level TEXT NOT NULL DEFAULT 'company' CHECK(level IN ('company','department','individual')),
    title TEXT NOT NULL,
    owner_employee_id INTEGER NULL,
    department_id INTEGER NULL,
    target NUMERIC NOT NULL DEFAULT 0,
    actual NUMERIC NULL,
    weight NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','closed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(performance_period_id) REFERENCES performance_periods(id),
    FOREIGN KEY(parent_id) REFERENCES goals(id),
    FOREIGN KEY(owner_employee_id) REFERENCES employees(id),
    FOREIGN KEY(department_id) REFERENCES departments(id)
);

CREATE TABLE IF NOT EXISTS feedback_360 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    performance_period_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    reviewer_employee_id INTEGER NOT NULL,
    relation TEXT NOT NULL DEFAULT 'peer' CHECK(relation IN ('manager','peer','subordinate','self')),
    score NUMERIC NOT NULL,
    comments TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(performance_period_id, employee_id, reviewer_employee_id),
    FOREIGN KEY(performance_period_id) REFERENCES performance_periods(id),
    FOREIGN KEY(employee_id) REFERENCES employees(id),
    FOREIGN KEY(reviewer_employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS calibrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    performance_period_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    initial_score NUMERIC NOT NULL,
    final_score NUMERIC NOT NULL,
    decided_by INTEGER NOT NULL,
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(performance_period_id, employee_id),
    FOREIGN KEY(performance_period_id) REFERENCES performance_periods(id),
    FOREIGN KEY(employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS successions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    position_id INTEGER NOT NULL,
    successor_employee_id INTEGER NOT NULL,
    readiness TEXT NOT NULL DEFAULT 'developing' CHECK(readiness IN ('ready','developing','not_ready')),
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(position_id, successor_employee_id),
    FOREIGN KEY(position_id) REFERENCES positions(id),
    FOREIGN KEY(successor_employee_id) REFERENCES employees(id)
);

CREATE INDEX IF NOT EXISTS idx_goals_parent ON goals(parent_id);
CREATE INDEX IF NOT EXISTS idx_goals_period ON goals(performance_period_id);
CREATE INDEX IF NOT EXISTS idx_fb360_employee ON feedback_360(employee_id);
