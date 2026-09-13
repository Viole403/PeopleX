-- M05: band gaji per grade.
ALTER TABLE job_grades ADD COLUMN min_salary NUMERIC NULL;
ALTER TABLE job_grades ADD COLUMN max_salary NUMERIC NULL;
