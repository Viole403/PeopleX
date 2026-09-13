-- M07: nilai kuis peserta training (0-100).
ALTER TABLE training_participants ADD COLUMN quiz_score NUMERIC NULL;
