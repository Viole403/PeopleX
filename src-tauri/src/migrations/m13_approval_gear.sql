CREATE TABLE IF NOT EXISTS leave_carryovers (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    leave_type_id INTEGER NOT NULL,
    from_year INTEGER NOT NULL,
    to_year INTEGER NOT NULL,
    days_carried NUMERIC NOT NULL,
    days_expired NUMERIC NOT NULL,
    processed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(employee_id, leave_type_id, from_year),
    FOREIGN KEY(employee_id) REFERENCES employees(id),
    FOREIGN KEY(leave_type_id) REFERENCES leave_types(id)
);

CREATE TABLE IF NOT EXISTS approval_delegations (
    id INTEGER PRIMARY KEY,
    from_user INTEGER NOT NULL,
    to_user INTEGER NOT NULL,
    module TEXT NOT NULL DEFAULT 'leave',
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','cancelled')),
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(from_user) REFERENCES users(id),
    FOREIGN KEY(to_user) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_approval_delegations_from_user ON approval_delegations(from_user);

CREATE TABLE IF NOT EXISTS leave_type_levels (
    id INTEGER PRIMARY KEY,
    leave_type_id INTEGER NOT NULL,
    department_id INTEGER NOT NULL DEFAULT 0,
    step_order INTEGER NOT NULL,
    approver_user_id INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(leave_type_id, department_id, step_order),
    FOREIGN KEY(leave_type_id) REFERENCES leave_types(id) ON DELETE CASCADE,
    FOREIGN KEY(approver_user_id) REFERENCES users(id)
);
