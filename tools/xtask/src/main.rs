//! Repository checks for o1.
//!
//! `cargo run -p xtask -- unsafe-inventory` scans every `.rs` file under `crates/`,
//! `tools/`, `o1-custodian/`, `o1-machine-profile/` and `o1-predict-lab/` for the `unsafe` keyword in code (comments and string literals are skipped;
//! doc-comment examples are therefore not counted) and checks that:
//!
//! 1. every `unsafe { … }` block and `unsafe impl` carries an `[unsafe:<id>]` tag in the
//!    comment/attribute lines directly above it (or on the same line);
//! 2. every `unsafe fn` has a `# Safety` section in the doc comment above it; its id is
//!    `fn:<file>:<name>`;
//! 3. ids are unique;
//! 4. the set of ids equals the set listed in `docs/UNSAFE_INVENTORY.md` (first table cell,
//!    in backticks).
//!
//! A new unsafe site without an inventory entry, or an entry without a site, fails the check.
//! `--list` prints every site.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug)]
struct Site {
    file: String,
    line: usize,
    kind: &'static str,
    id: Option<String>,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("unsafe-inventory") => unsafe_inventory(args.iter().any(|a| a == "--list")),
        _ => {
            eprintln!("usage: cargo run -p xtask -- unsafe-inventory [--list]");
            ExitCode::from(2)
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tools/xtask is two levels below the root")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Replaces comments and string/char literal contents with spaces, keeping line structure,
/// so keyword search only sees code.
fn code_only(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    let blank = |c: u8| if c == b'\n' { b'\n' } else { b' ' };
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                out.push(b' ');
                i += 1;
            }
        } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == b'/' && b.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    out.extend_from_slice(b"  ");
                    i += 2;
                } else if b[i] == b'*' && b.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    out.extend_from_slice(b"  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(b[i]));
                    i += 1;
                }
            }
        } else if c == b'r'
            && (b.get(i + 1) == Some(&b'"') || b.get(i + 1) == Some(&b'#'))
            && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_'))
        {
            // raw string r"…" / r#"…"#
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&b'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) != Some(&b'"') {
                out.push(c);
                i += 1;
                continue;
            }
            j += 1;
            while j < b.len() {
                if b[j] == b'"'
                    && b[j + 1..]
                        .iter()
                        .take(hashes)
                        .filter(|&&h| h == b'#')
                        .count()
                        == hashes
                {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            for &x in &b[i..j.min(b.len())] {
                out.push(blank(x));
            }
            i = j;
        } else if c == b'"' {
            out.push(b' ');
            i += 1;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' {
                    out.push(b' ');
                    i += 1;
                }
                if i < b.len() {
                    out.push(blank(b[i]));
                    i += 1;
                }
            }
            out.push(b' ');
            i += 1;
        } else if c == b'\'' {
            // char literal 'x' or '\…'; otherwise a lifetime.
            if b.get(i + 1) == Some(&b'\\') {
                let mut j = i + 2;
                while j < b.len() && b[j] != b'\'' {
                    j += 1;
                }
                out.extend(std::iter::repeat_n(b' ', j + 1 - i));
                i = j + 1;
            } else if b.get(i + 2) == Some(&b'\'') {
                out.extend_from_slice(b"   ");
                i += 3;
            } else {
                out.push(c);
                i += 1;
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn find_tag(s: &str) -> Option<String> {
    let start = s.find("[unsafe:")? + "[unsafe:".len();
    let end = s[start..].find(']')? + start;
    Some(s[start..end].to_string())
}

fn scan(root: &Path, path: &Path) -> Vec<Site> {
    let src = fs::read_to_string(path).expect("readable source");
    let code = code_only(&src);
    let raw_lines: Vec<&str> = src.lines().collect();
    let code_lines: Vec<&str> = code.lines().collect();
    let rel = path.strip_prefix(root).unwrap().display().to_string();
    let mut sites = Vec::new();
    for (n, line) in code_lines.iter().enumerate() {
        let mut rest: &str = line;
        let mut offset = 0;
        while let Some(pos) = rest.find("unsafe") {
            let abs = offset + pos;
            let before = line[..abs].chars().next_back();
            let after_kw = &line[abs + 6..];
            let boundary_ok = !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                && !after_kw.starts_with(|c: char| c.is_alphanumeric() || c == '_');
            offset = abs + 6;
            rest = &line[offset..];
            if !boundary_ok {
                continue;
            }
            let next = after_kw.trim_start();
            let kind = if next.starts_with('{') {
                "block"
            } else if next.starts_with("fn") {
                "fn"
            } else if next.starts_with("impl") {
                "impl"
            } else if next.starts_with("extern") {
                "extern"
            } else if next.starts_with("trait") {
                "trait"
            } else if next.starts_with('(') {
                "attribute"
            } else {
                "other"
            };
            // Comment/attribute lines directly above, plus the site line itself.
            let mut ctx = vec![raw_lines[n]];
            let mut k = n;
            while k > 0 {
                let t = raw_lines[k - 1].trim_start();
                if t.starts_with("//") || t.starts_with("#[") || t.starts_with("#![") {
                    ctx.push(raw_lines[k - 1]);
                    k -= 1;
                } else {
                    break;
                }
            }
            let joined = ctx.join("\n");
            let id = if kind == "fn" {
                if joined.contains("# Safety") {
                    let name = next[2..]
                        .trim_start()
                        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                        .next()
                        .unwrap_or("?");
                    Some(format!("fn:{rel}:{name}"))
                } else {
                    None
                }
            } else {
                find_tag(&joined)
            };
            sites.push(Site {
                file: rel.clone(),
                line: n + 1,
                kind,
                id,
            });
        }
    }
    sites
}

fn inventory_ids(doc: &str) -> BTreeSet<String> {
    doc.lines()
        .filter_map(|l| {
            let l = l.trim();
            let cell = l.strip_prefix("| `")?;
            let end = cell.find('`')?;
            Some(cell[..end].to_string())
        })
        .collect()
}

fn unsafe_inventory(list: bool) -> ExitCode {
    let root = repo_root();
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    rust_files(&root.join("tools"), &mut files);
    // The Custodian verification adapter (its own workspace, outside the root one).
    rust_files(&root.join("o1-custodian"), &mut files);
    // The machine-profile calibration tool (its own workspace, outside the root one).
    rust_files(&root.join("o1-machine-profile"), &mut files);
    // The Unit 5 prediction lab (its own workspace, outside the root one).
    rust_files(&root.join("o1-predict-lab"), &mut files);
    let sites: Vec<Site> = files.iter().flat_map(|f| scan(&root, f)).collect();

    let mut ok = true;
    let mut seen = BTreeMap::new();
    let mut per_crate: BTreeMap<String, usize> = BTreeMap::new();
    for s in &sites {
        let krate = s.file.split('/').take(2).collect::<Vec<_>>().join("/");
        *per_crate.entry(krate).or_default() += 1;
        if list {
            println!(
                "{:<10} {}:{}  {}",
                s.kind,
                s.file,
                s.line,
                s.id.as_deref().unwrap_or("<untagged>")
            );
        }
        match &s.id {
            None => {
                ok = false;
                eprintln!(
                    "ERROR {}:{} unsafe {} without {}",
                    s.file,
                    s.line,
                    s.kind,
                    if s.kind == "fn" {
                        "a `# Safety` doc section"
                    } else {
                        "an [unsafe:<id>] tag"
                    }
                );
            }
            Some(id) => {
                if let Some(prev) = seen.insert(id.clone(), format!("{}:{}", s.file, s.line)) {
                    ok = false;
                    eprintln!(
                        "ERROR duplicate id `{id}` at {}:{} and {prev}",
                        s.file, s.line
                    );
                }
            }
        }
    }

    let doc_path = root.join("docs/UNSAFE_INVENTORY.md");
    let documented = fs::read_to_string(&doc_path)
        .map(|d| inventory_ids(&d))
        .unwrap_or_default();
    let in_code: BTreeSet<String> = seen.keys().cloned().collect();
    for id in in_code.difference(&documented) {
        ok = false;
        eprintln!(
            "ERROR `{id}` ({}) is not in docs/UNSAFE_INVENTORY.md",
            seen[id]
        );
    }
    for id in documented.difference(&in_code) {
        ok = false;
        eprintln!("ERROR docs/UNSAFE_INVENTORY.md lists `{id}`, which no longer exists in code");
    }

    println!("unsafe sites: {} total", sites.len());
    for (k, n) in &per_crate {
        println!("  {k}: {n}");
    }
    if ok {
        println!(
            "unsafe inventory: OK ({} ids match docs/UNSAFE_INVENTORY.md)",
            in_code.len()
        );
        ExitCode::SUCCESS
    } else {
        println!("unsafe inventory: FAILED");
        ExitCode::FAILURE
    }
}
