CREATE TABLE IF NOT EXISTS offer_letters (
    id INTEGER PRIMARY KEY,
    candidate_id INTEGER NOT NULL REFERENCES candidates(id),
    title TEXT NOT NULL,
    message TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','sent','signed','declined')),
    recipient_name TEXT NOT NULL DEFAULT '',
    signature_name TEXT,
    signature_hash TEXT,
    sent_at TEXT,
    signed_at TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_offer_letters_candidate ON offer_letters(candidate_id);
