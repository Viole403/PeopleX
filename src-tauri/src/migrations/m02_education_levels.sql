-- M02: perluas jenjang pendidikan (SMA/SMK pisah, D1-D4 lengkap).
-- SQLite tidak bisa ALTER CHECK: bangun ulang tabel dengan data disalin.
CREATE TABLE employee_educations_new (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('sd','smp','sma','smk','d1','d2','d3','d4','s1','s2','s3')),
    school_name TEXT NOT NULL,
    major TEXT NULL,
    graduation_year INTEGER NULL,
    gpa NUMERIC NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);
INSERT INTO employee_educations_new
    (id, employee_id, level, school_name, major, graduation_year, gpa, created_at, updated_at)
    SELECT id, employee_id, level, school_name, major, graduation_year, gpa, created_at, updated_at
    FROM employee_educations;
DROP TABLE employee_educations;
ALTER TABLE employee_educations_new RENAME TO employee_educations;
