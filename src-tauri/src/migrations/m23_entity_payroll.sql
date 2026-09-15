-- Fase 3: payroll lintas entitas
ALTER TABLE payroll_periods ADD COLUMN company_id INTEGER NULL;
ALTER TABLE payrolls ADD COLUMN company_id INTEGER NULL;
