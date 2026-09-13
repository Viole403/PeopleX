-- M06: materi e-learning per training.
CREATE TABLE training_materials (
    id INTEGER PRIMARY KEY,
    training_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'tautan' CHECK (kind IN ('tautan','dokumen','teks')),
    url TEXT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (training_id) REFERENCES trainings(id) ON DELETE CASCADE
);
