//! Transpilasi naskah skema SQL SQLite menjadi dialek PostgreSQL/MySQL.
//!
//! Satu sumber kebenaran skema adalah berkas `.sql` bergaya SQLite. Sebelum
//! dijalankan pada backend lain, setiap pernyataan dilewatkan [transpile]:
//! - `id INTEGER PRIMARY KEY` → identitas per backend (IDENTITAS PG / AUTO_INCREMENT MySQL)
//! - `INTEGER` polos → `BIGINT` agar kolom FK setipe dengan `id`
//! - `TEXT` yang diindeks/di-UNIQUE → `VARCHAR(255)` (MySQL tidak mengizinkan indeks TEXT penuh)
//! - `DEFAULT CURRENT_TIMESTAMP` pada kolom TEXT → ekspresi default bertipe teks
//! - kata-kunci MySQL diberi backtick; `CREATE INDEX IF NOT EXISTS` disederhanakan
//! - tabel diberi `ENGINE=InnoDB DEFAULT CHARSET=utf8mb4` (MySQL)
//!
//! Naskah diasumsikan ASCII (dijaga oleh test) sehingga indeks char == indeks byte.

use std::collections::HashSet;

const MYSQL_RESERVED: [&str; 5] = ["action", "date", "level", "status", "type"];

/// Pasangan (tabel, kolom) bertipe TEXT yang menjadi target UNIQUE atau indeks.
pub fn collect_indexed_text(sql_all: &str) -> HashSet<(String, String)> {
    let mut set = HashSet::new();
    let lower = sql_all.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();

    let mut pos = 0usize;
    while let Some(p) = find_word(&chars, pos, "table") {
        pos = p + 5;
        let Some((tname, body)) = parse_table(&lower, p) else {
            continue;
        };
        for seg in split_top_level(&body) {
            let t = seg.trim();
            let toks: Vec<&str> = t.split_whitespace().collect();
            if toks.is_empty() {
                continue;
            }
            if toks[0] == "unique" {
                if let Some(inner) = between_parens(t) {
                    for c in inner.split(',') {
                        let col = c.trim().split_whitespace().next().unwrap_or("");
                        if col_type(&body, col) == Some("text") {
                            set.insert((tname.clone(), col.to_string()));
                        }
                    }
                }
                continue;
            }
            if toks.len() >= 2
                && toks[1] == "text"
                && t.contains("unique")
                && !is_constraint_kw(toks[0])
            {
                set.insert((tname.clone(), toks[0].to_string()));
            }
        }
    }

    let mut pos = 0usize;
    while let Some(p) = find_word(&chars, pos, "index") {
        pos = p + 5;
        let Some(onp) = find_word(&chars, p, "on") else {
            continue;
        };
        let tbl = ident_after(&lower, onp + 2);
        let rest = &lower[onp + 2 + tbl.len()..];
        let Some(inner) = between_parens(rest) else {
            continue;
        };
        for c in inner.split(',') {
            let col = c.trim().split_whitespace().next().unwrap_or("");
            if col_type_of_table(&lower, &tbl, col) == Some("text") {
                set.insert((tbl.clone(), col.to_string()));
            }
        }
    }
    set
}

/// Tipe kolom (`text`/`integer`/`numeric`) dari badan CREATE TABLE, bila ada.
fn col_type(body: &str, col: &str) -> Option<&'static str> {
    for seg in split_top_level(body) {
        let t = seg.trim();
        let toks: Vec<&str> = t.split_whitespace().collect();
        if toks.len() >= 2 && toks[0] == col {
            return match toks[1] {
                "text" => Some("text"),
                "integer" => Some("integer"),
                "numeric" => Some("numeric"),
                _ => Some("lain"),
            };
        }
    }
    None
}

fn col_type_of_table<'a>(lower_all: &'a str, tname: &str, col: &str) -> Option<&'static str> {
    let chars: Vec<char> = lower_all.chars().collect();
    let mut pos = 0usize;
    while let Some(p) = find_word(&chars, pos, "table") {
        pos = p + 5;
        if let Some((t, body)) = parse_table(lower_all, p) {
            if t == tname {
                return col_type(&body, col);
            }
        }
    }
    None
}

/// Parse badan `(...)` dari `CREATE TABLE [IF NOT EXISTS] nama` mulai kata "table".
fn parse_table(lower_all: &str, word_pos: usize) -> Option<(String, String)> {
    let after = lower_all[word_pos + 5..].trim_start();
    let after = after
        .strip_prefix("if not exists")
        .map(str::trim_start)
        .unwrap_or(after);
    let end = after
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.' || c == '"'))
        .unwrap_or(after.len());
    let tname = after[..end].trim_matches('"').to_string();
    let abs_after = lower_all.len() - after.len();
    let paren_rel = after.find('(')?;
    let abs_paren = abs_after + paren_rel;
    let close = matching_paren(lower_all, abs_paren)?;
    Some((tname, lower_all[abs_paren + 1..close].to_string()))
}

fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let cs: Vec<char> = s.chars().collect();
    let mut depth = 0usize;
    for i in open..cs.len() {
        match cs[i] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Pecah per koma pada kedalaman kurung 0, abaikan koma dalam literal '...'.
fn split_top_level(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut in_str = false;
    for c in s.chars() {
        if c == '\'' {
            in_str = !in_str;
            cur.push(c);
        } else if !in_str && c == '(' {
            depth += 1;
            cur.push(c);
        } else if !in_str && c == ')' {
            depth -= 1;
            cur.push(c);
        } else if !in_str && c == ',' && depth == 0 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn find_word(chars: &[char], from: usize, word: &str) -> Option<usize> {
    let w: Vec<char> = word.chars().collect();
    if from >= chars.len() {
        return None;
    }
    'outer: for i in from..chars.len() {
        if chars[i] != w[0] {
            continue;
        }
        if i + w.len() > chars.len() {
            continue;
        }
        if i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            continue;
        }
        let after = chars.get(i + w.len());
        if after.is_some_and(|c| c.is_alphanumeric() || *c == '_') {
            continue;
        }
        for k in 1..w.len() {
            if chars[i + k] != w[k] {
                continue 'outer;
            }
        }
        return Some(i);
    }
    None
}

fn ident_after(lower_all: &str, at: usize) -> String {
    let rest = lower_all[at..].trim_start();
    let end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(0);
    rest[..end].to_string()
}

fn between_parens(s: &str) -> Option<&str> {
    let open = s.find('(')?;
    let close = s.rfind(')')?;
    if close > open {
        Some(&s[open + 1..close])
    } else {
        None
    }
}

fn is_constraint_kw(t: &str) -> bool {
    matches!(t, "unique" | "primary" | "foreign" | "check" | "constraint")
}

/// Terjemahkan satu pernyataan dari dialek SQLite ke backend tujuan.
pub fn transpile(
    backend: sea_orm::DbBackend,
    stmt: &str,
    indexed_text: &HashSet<(String, String)>,
) -> String {
    use sea_orm::DbBackend;
    match backend {
        DbBackend::Postgres => transpile_pg(stmt),
        DbBackend::MySql => transpile_mysql(stmt, indexed_text),
        _ => stmt.to_string(),
    }
}

fn is_create_table(stmt: &str) -> bool {
    stmt.trim_start().to_lowercase().starts_with("create table")
}

fn table_name_of(stmt: &str) -> String {
    let l = stmt.to_lowercase();
    let Some(chars0) = l.as_str().get(..) else {
        return String::new();
    };
    let cs: Vec<char> = chars0.chars().collect();
    let Some(p) = find_word(&cs, 0, "table") else {
        return String::new();
    };
    let after = l[p + 5..].trim_start();
    let after = after
        .strip_prefix("if not exists")
        .map(str::trim_start)
        .unwrap_or(after);
    let end = after
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(0);
    after[..end].to_string()
}

fn transpile_pg(stmt: &str) -> String {
    let mut out = replace_ci(
        stmt,
        "id INTEGER PRIMARY KEY",
        "id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY",
    );
    out = map_ct_lines(&out, |line| {
        replace_ci(
            line,
            "DEFAULT CURRENT_TIMESTAMP",
            "DEFAULT (to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS'))",
        )
    });
    out = integer_to_bigint(&out);
    out
}

fn transpile_mysql(stmt: &str, indexed_text: &HashSet<(String, String)>) -> String {
    let mut out = stmt.to_string();
    if is_create_table(&out) {
        let tname = table_name_of(&out);
        let cols: Vec<String> = indexed_text
            .iter()
            .filter(|(t, _)| t == &tname)
            .map(|(_, c)| c.clone())
            .collect();
        for col in cols {
            out = text_col_to_varchar(&out, &col);
        }
    }
    out = replace_ci(
        &out,
        "id INTEGER PRIMARY KEY",
        "id BIGINT AUTO_INCREMENT PRIMARY KEY",
    );
    out = map_ct_lines(&out, |line| {
        if line.to_lowercase().contains("varchar(255)") {
            replace_ci(
                line,
                "DEFAULT CURRENT_TIMESTAMP",
                "DEFAULT (CAST(CURRENT_TIMESTAMP AS CHAR))",
            )
        } else {
            let mut s = replace_ci(line, "DEFAULT CURRENT_TIMESTAMP", "");
            while s.contains("  ") {
                s = s.replace("  ", " ");
            }
            s
        }
    });
    out = integer_to_bigint(&out);
    out = replace_word_ci(&out, "numeric", "DECIMAL(20,6)");
    out = replace_ci(&out, "CREATE INDEX IF NOT EXISTS", "CREATE INDEX");
    out = quote_mysql_reserved(&out);
    if is_create_table(&out) {
        if let Some(pos) = out.rfind(')') {
            out.insert_str(pos + 1, " ENGINE=InnoDB DEFAULT CHARSET=utf8mb4");
        }
    }
    out
}

/// Ubah baris yang memuat TEXT + DEFAULT CURRENT_TIMESTAMP (baris definisi kolom).
fn map_ct_lines<F: Fn(&str) -> String>(stmt: &str, f: F) -> String {
    stmt.lines()
        .map(|line| {
            let l = line.to_lowercase();
            if (l.contains("text") || l.contains("varchar"))
                && l.contains("default current_timestamp")
            {
                f(line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn integer_to_bigint(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let cs: Vec<char> = s.chars().collect();
    let cl: Vec<char> = s.to_lowercase().chars().collect();
    let w: Vec<char> = "integer".chars().collect();
    let mut i = 0usize;
    while i < cs.len() {
        if i + w.len() <= cl.len()
            && &cl[i..i + w.len()] == w.as_slice()
            && is_word_start(&cl, i)
            && is_word_end(&cl, i + w.len())
        {
            out.push_str("BIGINT");
            i += w.len();
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

/// `col TEXT` → `col VARCHAR(255)` untuk satu kolom tertentu.
fn text_col_to_varchar(s: &str, col: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let cs: Vec<char> = s.chars().collect();
    let cl: Vec<char> = s.to_lowercase().chars().collect();
    let cw: Vec<char> = col.chars().collect();
    let mut i = 0usize;
    while i < cs.len() {
        if i + cw.len() + 5 <= cl.len()
            && &cl[i..i + cw.len()] == cw.as_slice()
            && is_word_start(&cl, i)
            && is_word_end(&cl, i + cw.len())
        {
            let mut j = i + cw.len();
            while j < cl.len() && (cl[j] == ' ' || cl[j] == '\t') {
                j += 1;
            }
            if j + 4 <= cl.len()
                && &cl[j..j + 4] == ['t', 'e', 'x', 't'].as_slice()
                && is_word_end(&cl, j + 4)
            {
                out.extend(cs[i..j].iter());
                out.push_str("VARCHAR(255)");
                i = j + 4;
                continue;
            }
        }
        out.push(cs[i]);
        i += 1;
    }
    out
}

fn is_word_start(cl: &[char], i: usize) -> bool {
    i == 0 || !(cl[i - 1].is_alphanumeric() || cl[i - 1] == '_')
}

fn is_word_end(cl: &[char], i: usize) -> bool {
    i >= cl.len() || !(cl[i].is_alphanumeric() || cl[i] == '_')
}

/// Backtick kata kunci MySQL pada posisi identifier (di luar literal '...').
fn quote_mysql_reserved(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let mut in_str = false;
    while i < cs.len() {
        let c = cs[i];
        if c == '\'' {
            in_str = !in_str;
            out.push(c);
            i += 1;
            continue;
        }
        if !in_str && (c.is_alphabetic() || c == '_') {
            let mut j = i;
            while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                j += 1;
            }
            let word: String = cs[i..j].iter().collect();
            let lw = word.to_lowercase();
            let prev = if i > 0 { cs[i - 1] } else { ' ' };
            let already = prev == '`' || prev == '.';
            if MYSQL_RESERVED.contains(&lw.as_str()) && !already {
                out.push('`');
                out.push_str(&word);
                out.push('`');
                i = j;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn replace_ci(hay: &str, needle_ci: &str, repl: &str) -> String {
    let hl = hay.to_lowercase();
    let nl = needle_ci.to_lowercase();
    let mut out = String::with_capacity(hay.len());
    let mut prev = 0usize;
    let mut search = 0usize;
    while let Some(pos) = hl[search..].find(&nl) {
        let abs = search + pos;
        let end = abs + nl.len();
        let b = hay.as_bytes();
        let before_ok = abs == 0 || !(b[abs - 1].is_ascii_alphanumeric() || b[abs - 1] == b'_');
        let after_ok = end >= hay.len() || !(b[end].is_ascii_alphanumeric() || b[end] == b'_');
        if before_ok && after_ok {
            out.push_str(&hay[prev..abs]);
            out.push_str(repl);
            prev = end;
        }
        search = end;
    }
    out.push_str(&hay[prev..]);
    out
}

fn replace_word_ci(hay: &str, word_ci: &str, repl: &str) -> String {
    let mut out = String::with_capacity(hay.len());
    let cs: Vec<char> = hay.chars().collect();
    let cl: Vec<char> = hay.to_lowercase().chars().collect();
    let w: Vec<char> = word_ci.chars().collect();
    let mut i = 0usize;
    while i < cs.len() {
        if i + w.len() <= cl.len()
            && &cl[i..i + w.len()] == w.as_slice()
            && is_word_start(&cl, i)
            && is_word_end(&cl, i + w.len())
        {
            out.push_str(repl);
            i += w.len();
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

/// SQL hitung jumlah tabel user per backend (status DB + test migrasi).
pub fn count_tables_sql(backend: sea_orm::DbBackend) -> String {
    use sea_orm::DbBackend;
    match backend {
        DbBackend::Postgres => "SELECT COUNT(*) AS n FROM information_schema.tables WHERE table_schema = current_schema()".to_string(),
        DbBackend::MySql => "SELECT COUNT(*) AS n FROM information_schema.tables WHERE table_schema = DATABASE()".to_string(),
        _ => "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::DbBackend;

    const SAMPLE: &str = "CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    email TEXT UNIQUE,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    date TEXT,
    amount NUMERIC,
    level INTEGER,
    user_id INTEGER REFERENCES companies(id)
)";

    fn idx_sample() -> HashSet<(String, String)> {
        let mut s = HashSet::new();
        s.insert(("users".to_string(), "username".to_string()));
        s.insert(("users".to_string(), "email".to_string()));
        s
    }

    #[test]
    fn sqlite_dilewatkan_utuh() {
        assert_eq!(transpile(DbBackend::Sqlite, SAMPLE, &idx_sample()), SAMPLE);
    }

    #[test]
    fn postgres_memakai_identity_dan_to_char() {
        let out = transpile(DbBackend::Postgres, SAMPLE, &idx_sample());
        assert!(out.contains("id BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY"));
        assert!(out.contains("to_char(CURRENT_TIMESTAMP"));
        assert!(out.contains("user_id BIGINT"));
        assert!(out.contains("level BIGINT"));
        assert!(!out.contains("INTEGER"));
    }

    #[test]
    fn mysql_memakai_auto_increment_varchar_dan_engine() {
        let out = transpile(DbBackend::MySql, SAMPLE, &idx_sample());
        assert!(out.contains("id BIGINT AUTO_INCREMENT PRIMARY KEY"));
        assert!(out.contains("username VARCHAR(255) NOT NULL UNIQUE"));
        assert!(!out.contains("CURRENT_TIMESTAMP"));
        assert!(out.contains("amount DECIMAL(20,6)"));
        assert!(out.contains("user_id BIGINT"));
        assert!(out.contains("`status`"));
        assert!(out.contains("`date`"));
        assert!(out.ends_with("ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"));
        // literal 'active' tidak boleh ikut di-backtick
        assert!(out.contains("DEFAULT 'active'"));
    }

    #[test]
    fn mysql_varchar_terindeks_menyimpan_default_cast() {
        let stmt = "CREATE TABLE IF NOT EXISTS t (
    id INTEGER PRIMARY KEY,
    posted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (posted_at)
)";
        let mut idx = HashSet::new();
        idx.insert(("t".to_string(), "posted_at".to_string()));
        let out = transpile(DbBackend::MySql, stmt, &idx);
        assert!(out
            .contains("posted_at VARCHAR(255) NOT NULL DEFAULT (CAST(CURRENT_TIMESTAMP AS CHAR))"));
    }

    #[test]
    fn collect_menangkap_unique_inline_dan_daftar() {
        let script = "CREATE TABLE IF NOT EXISTS companies (
    id INTEGER PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT,
    notes TEXT
);
CREATE TABLE IF NOT EXISTS branches (
    id INTEGER PRIMARY KEY,
    company_id INTEGER REFERENCES companies(id),
    code TEXT NOT NULL,
    UNIQUE (company_id, code)
);
CREATE INDEX IF NOT EXISTS idx_companies_name ON companies (name);";
        let set = collect_indexed_text(script);
        assert!(set.contains(&("companies".to_string(), "code".to_string())));
        assert!(set.contains(&("companies".to_string(), "name".to_string())));
        assert!(set.contains(&("branches".to_string(), "code".to_string())));
        assert!(set.contains(&("branches".to_string(), "company_id".to_string())) == false || true); // company_id INTEGER, bukan TEXT
        assert!(!set.contains(&("branches".to_string(), "company_id".to_string())));
        assert!(!set.contains(&("companies".to_string(), "notes".to_string())));
    }

    #[test]
    fn naskah_skema_asli_tertranspil_tanpan_panic() {
        let sqlite = crate::db::migration_scripts();
        let joined: String = sqlite.join("\n");
        let idx = collect_indexed_text(&joined);
        assert!(!idx.is_empty());
        for script in sqlite {
            for part in script.split(';') {
                let stmt = part.trim();
                if stmt.is_empty() {
                    continue;
                }
                let _ = transpile(DbBackend::Postgres, stmt, &idx);
                let _ = transpile(DbBackend::MySql, stmt, &idx);
            }
        }
    }
}
