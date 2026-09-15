CREATE TABLE IF NOT EXISTS kudos (
    id INTEGER PRIMARY KEY,
    from_employee_id INTEGER NOT NULL,
    to_employee_id INTEGER NOT NULL,
    message TEXT NOT NULL,
    badge TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (from_employee_id) REFERENCES employees(id),
    FOREIGN KEY (to_employee_id) REFERENCES employees(id)
);
CREATE TABLE IF NOT EXISTS polls (
    id INTEGER PRIMARY KEY,
    question TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'closed')),
    created_by INTEGER NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TEXT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS poll_options (
    id INTEGER PRIMARY KEY,
    poll_id INTEGER NOT NULL,
    label TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (poll_id) REFERENCES polls(id) ON DELETE CASCADE
);
CREATE TABLE IF NOT EXISTS poll_votes (
    id INTEGER PRIMARY KEY,
    poll_id INTEGER NOT NULL,
    option_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (poll_id, employee_id),
    FOREIGN KEY (poll_id) REFERENCES polls(id) ON DELETE CASCADE,
    FOREIGN KEY (option_id) REFERENCES poll_options(id),
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);
CREATE INDEX IF NOT EXISTS idx_kudos_to_employee ON kudos(to_employee_id);
CREATE INDEX IF NOT EXISTS idx_poll_votes_poll ON poll_votes(poll_id);
CREATE TABLE IF NOT EXISTS whistleblows (
    id INTEGER PRIMARY KEY,
    topic TEXT NOT NULL,
    message TEXT NOT NULL,
    token TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'investigating', 'closed')),
    response TEXT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TEXT NULL
);
CREATE TABLE IF NOT EXISTS policies (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    version TEXT NOT NULL,
    content TEXT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published', 'retired')),
    created_by INTEGER NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    published_at TEXT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);
CREATE TABLE IF NOT EXISTS policy_acks (
    id INTEGER PRIMARY KEY,
    policy_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    acked_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (policy_id, employee_id),
    FOREIGN KEY (policy_id) REFERENCES policies(id) ON DELETE CASCADE,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);
CREATE TABLE IF NOT EXISTS consents (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    purpose TEXT NOT NULL,
    granted INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, purpose),
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);
CREATE INDEX IF NOT EXISTS idx_whistleblows_status ON whistleblows(status);
CREATE INDEX IF NOT EXISTS idx_consents_employee ON consents(employee_id);
