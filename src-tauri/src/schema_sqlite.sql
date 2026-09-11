-- Skema awal PeopleX (SQLite).
-- Konvensi: ENUM -> TEXT + CHECK, tanggal/waktu -> TEXT (ISO8601),
-- desimal -> NUMERIC. Kolom updated_at ditulis oleh application layer.
CREATE TABLE IF NOT EXISTS companies (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    legal_name TEXT NULL,
    address TEXT NULL,
    city TEXT NULL,
    province TEXT NULL,
    postal_code TEXT NULL,
    phone TEXT NULL,
    email TEXT NULL,
    logo TEXT NULL,
    npwp TEXT NULL,
    established_date TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS branches (
    id INTEGER PRIMARY KEY,
    company_id INTEGER NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    address TEXT NULL,
    city TEXT NULL,
    phone TEXT NULL,
    is_head_office INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    UNIQUE (company_id, code),
    FOREIGN KEY (company_id) REFERENCES companies(id)
);

CREATE TABLE IF NOT EXISTS departments (
    id INTEGER PRIMARY KEY,
    company_id INTEGER NOT NULL,
    branch_id INTEGER NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    head_employee_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    UNIQUE (company_id, code),
    FOREIGN KEY (company_id) REFERENCES companies(id),
    FOREIGN KEY (branch_id) REFERENCES branches(id)
);

CREATE TABLE IF NOT EXISTS divisions (
    id INTEGER PRIMARY KEY,
    department_id INTEGER NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    head_employee_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    UNIQUE (department_id, code),
    FOREIGN KEY (department_id) REFERENCES departments(id)
);

CREATE TABLE IF NOT EXISTS sections (
    id INTEGER PRIMARY KEY,
    division_id INTEGER NOT NULL,
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    head_employee_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    UNIQUE (division_id, code),
    FOREIGN KEY (division_id) REFERENCES divisions(id)
);

CREATE TABLE IF NOT EXISTS job_levels (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    level_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS job_grades (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    grade_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS positions (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    department_id INTEGER NULL,
    job_level_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (department_id) REFERENCES departments(id),
    FOREIGN KEY (job_level_id) REFERENCES job_levels(id)
);

CREATE TABLE IF NOT EXISTS work_locations (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    address TEXT NULL,
    latitude NUMERIC NULL,
    longitude NUMERIC NULL,
    radius_meter INTEGER NOT NULL DEFAULT 100,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS cost_centers (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    department_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (department_id) REFERENCES departments(id)
);

CREATE TABLE IF NOT EXISTS employees (
    id INTEGER PRIMARY KEY,
    employee_number TEXT NOT NULL UNIQUE,
    nik TEXT NULL UNIQUE,
    first_name TEXT NOT NULL,
    last_name TEXT NULL,
    photo TEXT NULL,
    birth_place TEXT NULL,
    birth_date TEXT NULL,
    gender TEXT NOT NULL DEFAULT 'male' CHECK (gender IN ('male','female')),
    religion TEXT NULL,
    marital_status TEXT NOT NULL DEFAULT 'single' CHECK (marital_status IN ('single','married','divorced','widowed')),
    phone TEXT NULL,
    personal_email TEXT NULL,
    company_id INTEGER NOT NULL,
    branch_id INTEGER NULL,
    department_id INTEGER NULL,
    division_id INTEGER NULL,
    section_id INTEGER NULL,
    position_id INTEGER NULL,
    job_level_id INTEGER NULL,
    job_grade_id INTEGER NULL,
    work_location_id INTEGER NULL,
    cost_center_id INTEGER NULL,
    supervisor_id INTEGER NULL,
    manager_id INTEGER NULL,
    join_date TEXT NOT NULL,
    appointment_date TEXT NULL,
    resign_date TEXT NULL,
    employment_status TEXT NOT NULL DEFAULT 'probation' CHECK (employment_status IN ('active','probation','resigned','terminated')),
    employment_type TEXT NOT NULL DEFAULT 'contract' CHECK (employment_type IN ('permanent','contract','intern','daily','freelance')),
    bank_name TEXT NULL,
    bank_account_number TEXT NULL,
    bank_account_holder TEXT NULL,
    npwp TEXT NULL,
    ptkp_status TEXT NULL,
    bpjs_health_number TEXT NULL,
    bpjs_employment_number TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (company_id) REFERENCES companies(id),
    FOREIGN KEY (branch_id) REFERENCES branches(id),
    FOREIGN KEY (department_id) REFERENCES departments(id),
    FOREIGN KEY (division_id) REFERENCES divisions(id),
    FOREIGN KEY (section_id) REFERENCES sections(id),
    FOREIGN KEY (position_id) REFERENCES positions(id),
    FOREIGN KEY (job_level_id) REFERENCES job_levels(id),
    FOREIGN KEY (job_grade_id) REFERENCES job_grades(id),
    FOREIGN KEY (work_location_id) REFERENCES work_locations(id),
    FOREIGN KEY (cost_center_id) REFERENCES cost_centers(id),
    FOREIGN KEY (supervisor_id) REFERENCES employees(id),
    FOREIGN KEY (manager_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS employee_addresses (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('ktp','domicile')),
    address TEXT NULL,
    city TEXT NULL,
    province TEXT NULL,
    postal_code TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, type),
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS employee_contacts (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    relationship TEXT NULL,
    phone TEXT NULL,
    address TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS employee_families (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    relationship TEXT NOT NULL DEFAULT 'child' CHECK (relationship IN ('spouse','child','other')),
    birth_date TEXT NULL,
    occupation TEXT NULL,
    is_dependent INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS employee_educations (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    level TEXT NOT NULL CHECK (level IN ('sd','smp','sma','d3','s1','s2','s3')),
    school_name TEXT NOT NULL,
    major TEXT NULL,
    graduation_year INTEGER NULL,
    gpa NUMERIC NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS employee_experiences (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    company_name TEXT NOT NULL,
    position TEXT NULL,
    start_date TEXT NULL,
    end_date TEXT NULL,
    description TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS employee_documents (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    category TEXT NOT NULL CHECK (category IN ('ktp','kk','npwp','bpjs','ijazah','certificate','cv','contract','appointment_letter','promotion_letter','mutation_letter','other')),
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER NULL,
    mime_type TEXT NULL,
    expiry_date TEXT NULL,
    uploaded_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (employee_id) REFERENCES employees(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS employee_contracts (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    contract_number TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL DEFAULT 'pkwt' CHECK (type IN ('probation','pkwt','pkwtt')),
    start_date TEXT NOT NULL,
    end_date TEXT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','expired','terminated','renewed')),
    notes TEXT NULL,
    document_id INTEGER NULL,
    created_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (document_id) REFERENCES employee_documents(id)
);

CREATE TABLE IF NOT EXISTS employee_career_histories (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('promotion','mutation','transfer','demotion','position_change','department_change','salary_change','supervisor_change')),
    effective_date TEXT NOT NULL,
    from_position_id INTEGER NULL,
    to_position_id INTEGER NULL,
    from_department_id INTEGER NULL,
    to_department_id INTEGER NULL,
    from_salary NUMERIC NULL,
    to_salary NUMERIC NULL,
    notes TEXT NULL,
    created_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (from_position_id) REFERENCES positions(id),
    FOREIGN KEY (to_position_id) REFERENCES positions(id),
    FOREIGN KEY (from_department_id) REFERENCES departments(id),
    FOREIGN KEY (to_department_id) REFERENCES departments(id)
);

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NULL UNIQUE,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active','inactive')),
    failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until TEXT NULL,
    remember_token TEXT NULL,
    password_reset_token TEXT NULL,
    password_reset_expires_at TEXT NULL,
    must_change_password INTEGER NOT NULL DEFAULT 0,
    last_login_at TEXT NULL,
    last_login_ip TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    description TEXT NULL,
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS permissions (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    module TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS role_permissions (
    id INTEGER PRIMARY KEY,
    role_id INTEGER NOT NULL,
    permission_id INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (role_id, permission_id),
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
    FOREIGN KEY (permission_id) REFERENCES permissions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_roles (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (user_id, role_id),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS login_activities (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NULL,
    username_attempt TEXT NULL,
    ip_address TEXT NULL,
    user_agent TEXT NULL,
    status TEXT NOT NULL CHECK (status IN ('success','failed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS employee_salaries (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    basic_salary NUMERIC NOT NULL DEFAULT 0,
    effective_date TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS shifts (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    break_start TEXT NULL,
    break_end TEXT NULL,
    grace_period_minutes INTEGER NOT NULL DEFAULT 0,
    is_overnight INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS work_schedules (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS work_schedule_days (
    id INTEGER PRIMARY KEY,
    work_schedule_id INTEGER NOT NULL,
    day_of_week INTEGER NOT NULL,
    shift_id INTEGER NULL,
    is_working_day INTEGER NOT NULL DEFAULT 1,
    UNIQUE (work_schedule_id, day_of_week),
    FOREIGN KEY (work_schedule_id) REFERENCES work_schedules(id) ON DELETE CASCADE,
    FOREIGN KEY (shift_id) REFERENCES shifts(id)
);

CREATE TABLE IF NOT EXISTS shift_assignments (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    work_schedule_id INTEGER NULL,
    shift_id INTEGER NULL,
    date TEXT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (work_schedule_id) REFERENCES work_schedules(id),
    FOREIGN KEY (shift_id) REFERENCES shifts(id)
);

CREATE TABLE IF NOT EXISTS holidays (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'national' CHECK (type IN ('national','company','collective_leave','custom')),
    description TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (date, name)
);

CREATE TABLE IF NOT EXISTS attendances (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    clock_in TEXT NULL,
    clock_out TEXT NULL,
    clock_in_lat NUMERIC NULL,
    clock_in_lng NUMERIC NULL,
    clock_out_lat NUMERIC NULL,
    clock_out_lng NUMERIC NULL,
    clock_in_ip TEXT NULL,
    clock_out_ip TEXT NULL,
    clock_in_device TEXT NULL,
    clock_out_device TEXT NULL,
    shift_id INTEGER NULL,
    status TEXT NOT NULL DEFAULT 'present' CHECK (status IN ('present','late','absent','sick','permission','leave','wfh','business_trip','early_checkout')),
    late_minutes INTEGER NOT NULL DEFAULT 0,
    early_minutes INTEGER NOT NULL DEFAULT 0,
    work_minutes INTEGER NOT NULL DEFAULT 0,
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, date),
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (shift_id) REFERENCES shifts(id)
);

CREATE TABLE IF NOT EXISTS attendance_corrections (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    attendance_id INTEGER NULL,
    date TEXT NOT NULL,
    requested_clock_in TEXT NULL,
    requested_clock_out TEXT NULL,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    approved_by INTEGER NULL,
    approved_at TEXT NULL,
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (attendance_id) REFERENCES attendances(id),
    FOREIGN KEY (approved_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS leave_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    default_days_per_year INTEGER NOT NULL DEFAULT 0,
    is_paid INTEGER NOT NULL DEFAULT 1,
    carry_forward INTEGER NOT NULL DEFAULT 0,
    carry_forward_max_days INTEGER NOT NULL DEFAULT 0,
    requires_attachment INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS leave_balances (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    leave_type_id INTEGER NOT NULL,
    year INTEGER NOT NULL,
    allocated_days NUMERIC NOT NULL DEFAULT 0,
    used_days NUMERIC NOT NULL DEFAULT 0,
    carried_days NUMERIC NOT NULL DEFAULT 0,
    adjustment_days NUMERIC NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, leave_type_id, year),
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (leave_type_id) REFERENCES leave_types(id)
);

CREATE TABLE IF NOT EXISTS leave_requests (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    leave_type_id INTEGER NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    total_days NUMERIC NOT NULL,
    reason TEXT NULL,
    attachment_path TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','cancelled')),
    current_step INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (leave_type_id) REFERENCES leave_types(id)
);

CREATE TABLE IF NOT EXISTS leave_approvals (
    id INTEGER PRIMARY KEY,
    leave_request_id INTEGER NOT NULL,
    approver_id INTEGER NOT NULL,
    step_order INTEGER NOT NULL,
    step_role TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    notes TEXT NULL,
    acted_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (leave_request_id) REFERENCES leave_requests(id) ON DELETE CASCADE,
    FOREIGN KEY (approver_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS permission_types (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS permission_requests (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    permission_type_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    start_time TEXT NULL,
    end_time TEXT NULL,
    reason TEXT NOT NULL,
    attachment_path TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    approved_by INTEGER NULL,
    approved_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (permission_type_id) REFERENCES permission_types(id),
    FOREIGN KEY (approved_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS overtime_requests (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    duration_minutes INTEGER NOT NULL,
    reason TEXT NULL,
    rate_multiplier NUMERIC NOT NULL DEFAULT 1.50,
    amount NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    current_step INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS overtime_approvals (
    id INTEGER PRIMARY KEY,
    overtime_request_id INTEGER NOT NULL,
    approver_id INTEGER NOT NULL,
    step_order INTEGER NOT NULL,
    step_role TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected')),
    notes TEXT NULL,
    acted_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (overtime_request_id) REFERENCES overtime_requests(id) ON DELETE CASCADE,
    FOREIGN KEY (approver_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS salary_components (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL CHECK (type IN ('income','deduction')),
    calculation_type TEXT NOT NULL DEFAULT 'fixed' CHECK (calculation_type IN ('fixed','percentage','formula')),
    is_taxable INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS employee_salary_components (
    id INTEGER PRIMARY KEY,
    employee_salary_id INTEGER NOT NULL,
    salary_component_id INTEGER NOT NULL,
    amount NUMERIC NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_salary_id, salary_component_id),
    FOREIGN KEY (employee_salary_id) REFERENCES employee_salaries(id) ON DELETE CASCADE,
    FOREIGN KEY (salary_component_id) REFERENCES salary_components(id)
);

CREATE TABLE IF NOT EXISTS payroll_periods (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    payment_date TEXT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','processing','review','approved','paid','locked')),
    created_by INTEGER NULL,
    approved_by INTEGER NULL,
    approved_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (start_date, end_date),
    FOREIGN KEY (created_by) REFERENCES users(id),
    FOREIGN KEY (approved_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS payrolls (
    id INTEGER PRIMARY KEY,
    payroll_period_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    basic_salary NUMERIC NOT NULL DEFAULT 0,
    total_income NUMERIC NOT NULL DEFAULT 0,
    gross_salary NUMERIC NOT NULL DEFAULT 0,
    total_deduction NUMERIC NOT NULL DEFAULT 0,
    net_salary NUMERIC NOT NULL DEFAULT 0,
    total_overtime_amount NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','processing','review','approved','paid','locked')),
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (payroll_period_id, employee_id),
    FOREIGN KEY (payroll_period_id) REFERENCES payroll_periods(id),
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS payroll_details (
    id INTEGER PRIMARY KEY,
    payroll_id INTEGER NOT NULL,
    salary_component_id INTEGER NULL,
    component_name TEXT NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('income','deduction')),
    amount NUMERIC NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (payroll_id) REFERENCES payrolls(id) ON DELETE CASCADE,
    FOREIGN KEY (salary_component_id) REFERENCES salary_components(id)
);

CREATE TABLE IF NOT EXISTS payroll_deductions (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    payroll_period_id INTEGER NULL,
    type TEXT NOT NULL CHECK (type IN ('loan','kasbon','other')),
    description TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    installment_no INTEGER NULL,
    total_installments INTEGER NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','processed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (payroll_period_id) REFERENCES payroll_periods(id)
);

CREATE TABLE IF NOT EXISTS payslips (
    id INTEGER PRIMARY KEY,
    payroll_id INTEGER NOT NULL UNIQUE,
    payslip_number TEXT NOT NULL UNIQUE,
    generated_at TEXT NOT NULL,
    pdf_path TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (payroll_id) REFERENCES payrolls(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS vacancies (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    department_id INTEGER NULL,
    position_id INTEGER NULL,
    employment_type TEXT NOT NULL DEFAULT 'contract' CHECK (employment_type IN ('permanent','contract','intern','daily','freelance')),
    description TEXT NULL,
    requirements TEXT NULL,
    quota INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','closed','on_hold')),
    posted_date TEXT NULL,
    closing_date TEXT NULL,
    created_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (department_id) REFERENCES departments(id),
    FOREIGN KEY (position_id) REFERENCES positions(id),
    FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS candidates (
    id INTEGER PRIMARY KEY,
    vacancy_id INTEGER NOT NULL,
    full_name TEXT NOT NULL,
    email TEXT NULL,
    phone TEXT NULL,
    birth_date TEXT NULL,
    gender TEXT NULL CHECK (gender IN ('male','female')),
    address TEXT NULL,
    cv_path TEXT NULL,
    source TEXT NULL,
    stage TEXT NOT NULL DEFAULT 'applied' CHECK (stage IN ('applied','screening','interview','test','hr_interview','offering','hired','rejected')),
    rating NUMERIC NULL,
    notes TEXT NULL,
    employee_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (vacancy_id) REFERENCES vacancies(id),
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS candidate_documents (
    id INTEGER PRIMARY KEY,
    candidate_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    category TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (candidate_id) REFERENCES candidates(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS recruitment_stages (
    id INTEGER PRIMARY KEY,
    candidate_id INTEGER NOT NULL,
    stage TEXT NOT NULL,
    notes TEXT NULL,
    changed_by INTEGER NULL,
    changed_at TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (candidate_id) REFERENCES candidates(id) ON DELETE CASCADE,
    FOREIGN KEY (changed_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS interviews (
    id INTEGER PRIMARY KEY,
    candidate_id INTEGER NOT NULL,
    interviewer_id INTEGER NULL,
    schedule_at TEXT NOT NULL,
    location TEXT NULL,
    type TEXT NOT NULL DEFAULT 'hr' CHECK (type IN ('hr','user','technical')),
    result TEXT NOT NULL DEFAULT 'pending' CHECK (result IN ('pending','pass','fail')),
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (candidate_id) REFERENCES candidates(id) ON DELETE CASCADE,
    FOREIGN KEY (interviewer_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS candidate_assessments (
    id INTEGER PRIMARY KEY,
    candidate_id INTEGER NOT NULL,
    assessment_name TEXT NOT NULL,
    score NUMERIC NULL,
    notes TEXT NULL,
    assessed_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (candidate_id) REFERENCES candidates(id) ON DELETE CASCADE,
    FOREIGN KEY (assessed_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS onboarding (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL UNIQUE,
    candidate_id INTEGER NULL,
    start_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'in_progress' CHECK (status IN ('in_progress','completed')),
    progress_percent INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (candidate_id) REFERENCES candidates(id)
);

CREATE TABLE IF NOT EXISTS onboarding_tasks (
    id INTEGER PRIMARY KEY,
    onboarding_id INTEGER NOT NULL,
    task_name TEXT NOT NULL,
    is_completed INTEGER NOT NULL DEFAULT 0,
    completed_at TEXT NULL,
    completed_by INTEGER NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (onboarding_id) REFERENCES onboarding(id) ON DELETE CASCADE,
    FOREIGN KEY (completed_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS offboarding (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    resignation_date TEXT NOT NULL,
    last_working_date TEXT NOT NULL,
    reason TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','supervisor_approved','hr_approved','finance_approved','completed','rejected')),
    current_step INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS exit_interviews (
    id INTEGER PRIMARY KEY,
    offboarding_id INTEGER NOT NULL,
    conducted_by INTEGER NULL,
    feedback TEXT NULL,
    reason_category TEXT NULL,
    would_recommend INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (offboarding_id) REFERENCES offboarding(id) ON DELETE CASCADE,
    FOREIGN KEY (conducted_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS clearance_items (
    id INTEGER PRIMARY KEY,
    offboarding_id INTEGER NOT NULL,
    item_name TEXT NOT NULL,
    department TEXT NULL,
    is_cleared INTEGER NOT NULL DEFAULT 0,
    cleared_by INTEGER NULL,
    cleared_at TEXT NULL,
    notes TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (offboarding_id) REFERENCES offboarding(id) ON DELETE CASCADE,
    FOREIGN KEY (cleared_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS performance_periods (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL DEFAULT 'quarterly' CHECK (type IN ('monthly','quarterly','semester','annual')),
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','closed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS kpis (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NULL,
    department_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (department_id) REFERENCES departments(id)
);

CREATE TABLE IF NOT EXISTS employee_kpis (
    id INTEGER PRIMARY KEY,
    performance_period_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    kpi_id INTEGER NOT NULL,
    target NUMERIC NOT NULL DEFAULT 0,
    weight NUMERIC NOT NULL DEFAULT 0,
    actual NUMERIC NULL,
    score NUMERIC NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (performance_period_id) REFERENCES performance_periods(id),
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (kpi_id) REFERENCES kpis(id)
);

CREATE TABLE IF NOT EXISTS performance_reviews (
    id INTEGER PRIMARY KEY,
    performance_period_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    self_score NUMERIC NULL,
    supervisor_score NUMERIC NULL,
    manager_score NUMERIC NULL,
    hr_score NUMERIC NULL,
    final_score NUMERIC NULL,
    final_rating INTEGER NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','self_review','supervisor_review','manager_review','hr_review','completed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (performance_period_id, employee_id),
    FOREIGN KEY (performance_period_id) REFERENCES performance_periods(id),
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS performance_details (
    id INTEGER PRIMARY KEY,
    performance_review_id INTEGER NOT NULL,
    reviewer_role TEXT NOT NULL CHECK (reviewer_role IN ('self','supervisor','manager','hr')),
    reviewer_id INTEGER NULL,
    comments TEXT NULL,
    rating INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (performance_review_id) REFERENCES performance_reviews(id) ON DELETE CASCADE,
    FOREIGN KEY (reviewer_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS trainings (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NULL,
    trainer_name TEXT NULL,
    trainer_employee_id INTEGER NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    location TEXT NULL,
    cost NUMERIC NOT NULL DEFAULT 0,
    quota INTEGER NULL,
    status TEXT NOT NULL DEFAULT 'scheduled' CHECK (status IN ('scheduled','ongoing','completed','cancelled')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (trainer_employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS training_participants (
    id INTEGER PRIMARY KEY,
    training_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'registered' CHECK (status IN ('registered','attended','absent','completed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (training_id, employee_id),
    FOREIGN KEY (training_id) REFERENCES trainings(id) ON DELETE CASCADE,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS training_attendance (
    id INTEGER PRIMARY KEY,
    training_participant_id INTEGER NOT NULL,
    date TEXT NOT NULL,
    attended INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (training_participant_id, date),
    FOREIGN KEY (training_participant_id) REFERENCES training_participants(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS certifications (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    training_id INTEGER NULL,
    name TEXT NOT NULL,
    issuer TEXT NULL,
    certificate_number TEXT NULL,
    issued_date TEXT NULL,
    expiry_date TEXT NULL,
    file_path TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (training_id) REFERENCES trainings(id)
);

CREATE TABLE IF NOT EXISTS employee_skills (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    skill_name TEXT NOT NULL,
    level INTEGER NOT NULL DEFAULT 1,
    assessed_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (employee_id, skill_name),
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS asset_categories (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL
);

CREATE TABLE IF NOT EXISTS assets (
    id INTEGER PRIMARY KEY,
    asset_code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    asset_category_id INTEGER NOT NULL,
    brand TEXT NULL,
    serial_number TEXT NULL,
    purchase_date TEXT NULL,
    purchase_price NUMERIC NULL,
    condition_status TEXT NOT NULL DEFAULT 'good' CHECK (condition_status IN ('new','good','fair','damaged','lost')),
    status TEXT NOT NULL DEFAULT 'available' CHECK (status IN ('available','assigned','maintenance','disposed')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (asset_category_id) REFERENCES asset_categories(id)
);

CREATE TABLE IF NOT EXISTS asset_assignments (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    assigned_date TEXT NOT NULL,
    returned_date TEXT NULL,
    condition_on_assign TEXT NULL,
    condition_on_return TEXT NULL,
    notes TEXT NULL,
    verified_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (asset_id) REFERENCES assets(id),
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (verified_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS asset_maintenance (
    id INTEGER PRIMARY KEY,
    asset_id INTEGER NOT NULL,
    maintenance_date TEXT NOT NULL,
    description TEXT NOT NULL,
    cost NUMERIC NOT NULL DEFAULT 0,
    performed_by TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (asset_id) REFERENCES assets(id)
);

CREATE TABLE IF NOT EXISTS business_trips (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    destination TEXT NOT NULL,
    purpose TEXT NOT NULL,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    transportation TEXT NULL,
    hotel TEXT NULL,
    budget NUMERIC NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','completed','settled')),
    current_step INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS business_trip_expenses (
    id INTEGER PRIMARY KEY,
    business_trip_id INTEGER NOT NULL,
    category TEXT NOT NULL,
    description TEXT NULL,
    amount NUMERIC NOT NULL,
    receipt_path TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (business_trip_id) REFERENCES business_trips(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS reimbursement_categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    code TEXT NOT NULL UNIQUE,
    max_amount NUMERIC NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS reimbursements (
    id INTEGER PRIMARY KEY,
    employee_id INTEGER NOT NULL,
    reimbursement_category_id INTEGER NOT NULL,
    amount NUMERIC NOT NULL,
    description TEXT NULL,
    receipt_path TEXT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','manager_approved','finance_verified','paid','rejected')),
    current_step INTEGER NOT NULL DEFAULT 1,
    paid_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (employee_id) REFERENCES employees(id),
    FOREIGN KEY (reimbursement_category_id) REFERENCES reimbursement_categories(id)
);

CREATE TABLE IF NOT EXISTS announcements (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    target_type TEXT NOT NULL DEFAULT 'all' CHECK (target_type IN ('all','department','branch','role','selected')),
    publish_at TEXT NULL,
    expire_at TEXT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft','published','expired')),
    created_by INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    deleted_at TEXT NULL,
    FOREIGN KEY (created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS announcement_targets (
    id INTEGER PRIMARY KEY,
    announcement_id INTEGER NOT NULL,
    target_type TEXT NOT NULL CHECK (target_type IN ('department','branch','role','employee')),
    target_id INTEGER NOT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (announcement_id) REFERENCES announcements(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS announcement_reads (
    id INTEGER PRIMARY KEY,
    announcement_id INTEGER NOT NULL,
    employee_id INTEGER NOT NULL,
    read_at TEXT NOT NULL,
    UNIQUE (announcement_id, employee_id),
    FOREIGN KEY (announcement_id) REFERENCES announcements(id) ON DELETE CASCADE,
    FOREIGN KEY (employee_id) REFERENCES employees(id)
);

CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    message TEXT NULL,
    link TEXT NULL,
    is_read INTEGER NOT NULL DEFAULT 0,
    read_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS approval_workflows (
    id INTEGER PRIMARY KEY,
    module TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS approval_steps (
    id INTEGER PRIMARY KEY,
    approval_workflow_id INTEGER NOT NULL,
    step_order INTEGER NOT NULL,
    approver_type TEXT NOT NULL CHECK (approver_type IN ('supervisor','manager','role','specific_user','department_head')),
    role_id INTEGER NULL,
    user_id INTEGER NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (approval_workflow_id, step_order),
    FOREIGN KEY (approval_workflow_id) REFERENCES approval_workflows(id) ON DELETE CASCADE,
    FOREIGN KEY (role_id) REFERENCES roles(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS approval_requests (
    id INTEGER PRIMARY KEY,
    approval_workflow_id INTEGER NOT NULL,
    reference_type TEXT NOT NULL,
    reference_id INTEGER NOT NULL,
    requested_by INTEGER NOT NULL,
    current_step INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','rejected','cancelled')),
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (approval_workflow_id) REFERENCES approval_workflows(id),
    FOREIGN KEY (requested_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS approval_histories (
    id INTEGER PRIMARY KEY,
    approval_request_id INTEGER NOT NULL,
    step_order INTEGER NOT NULL,
    approver_id INTEGER NULL,
    action TEXT NULL CHECK (action IN ('approved','rejected','delegated')),
    notes TEXT NULL,
    acted_at TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (approval_request_id) REFERENCES approval_requests(id) ON DELETE CASCADE,
    FOREIGN KEY (approver_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NULL,
    action TEXT NOT NULL,
    module TEXT NOT NULL,
    record_id TEXT NULL,
    description TEXT NULL,
    before_data TEXT NULL,
    after_data TEXT NULL,
    ip_address TEXT NULL,
    user_agent TEXT NULL,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS system_settings (
    id INTEGER PRIMARY KEY,
    setting_key TEXT NOT NULL UNIQUE,
    setting_value TEXT NULL,
    setting_group TEXT NOT NULL DEFAULT 'general',
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_employee_department ON employees (department_id);
CREATE INDEX IF NOT EXISTS idx_employee_status ON employees (employment_status);
CREATE INDEX IF NOT EXISTS idx_employee_supervisor ON employees (supervisor_id);
CREATE INDEX IF NOT EXISTS idx_document_expiry ON employee_documents (expiry_date);
CREATE INDEX IF NOT EXISTS idx_contract_end ON employee_contracts (end_date);
CREATE INDEX IF NOT EXISTS idx_salary_employee_active ON employee_salaries (employee_id, is_active);
CREATE INDEX IF NOT EXISTS idx_shiftassign_employee ON shift_assignments (employee_id, start_date, end_date);
CREATE INDEX IF NOT EXISTS idx_attendance_date ON attendances (date);
CREATE INDEX IF NOT EXISTS idx_leave_status ON leave_requests (status);
CREATE INDEX IF NOT EXISTS idx_overtime_status ON overtime_requests (status);
CREATE INDEX IF NOT EXISTS idx_notification_user_read ON notifications (user_id, is_read);
CREATE INDEX IF NOT EXISTS idx_approvalrequest_reference ON approval_requests (reference_type, reference_id);
CREATE INDEX IF NOT EXISTS idx_audit_module ON audit_logs (module);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_logs (created_at);
