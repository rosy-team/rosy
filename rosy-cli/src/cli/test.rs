use anyhow::{Context, Result, anyhow};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use super::compile::compile_source;
use super::{BOLD, DIM, GREEN, RED, RESET};

struct CaseFile {
    source: String,
    expect: Option<String>,
    fox: Option<String>,
}

fn parse_case(text: &str) -> CaseFile {
    let mut source = String::new();
    let mut expect = None;
    let mut fox = None;
    let mut dest = 0u8; // 0 source, 1 expect, 2 fox, 3 other
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("======= ") {
            dest = match rest.trim() {
                "expect" => {
                    expect = Some(String::new());
                    1
                }
                "fox" => {
                    fox = Some(String::new());
                    2
                }
                _ => 3,
            };
            continue;
        }
        let buf = match dest {
            1 => expect.as_mut().unwrap(),
            2 => fox.as_mut().unwrap(),
            3 => continue,
            _ => &mut source,
        };
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
    }
    CaseFile {
        source,
        expect,
        fox,
    }
}

fn norm_out(s: &str) -> String {
    s.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn write_blessed(path: &Path, case: &CaseFile, stdout: &str) -> Result<()> {
    let mut out = case.source.trim_end().to_string();
    out.push_str("\n\n======= expect\n");
    out.push_str(stdout.trim_end());
    out.push('\n');
    if let Some(fox) = &case.fox {
        out.push_str("\n======= fox\n");
        out.push_str(fox.trim_end());
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

fn discover_case_files(base: &Path) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    discover_case_files_recursive(base, base, &mut results);
    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

fn discover_case_files_recursive(root: &Path, dir: &Path, results: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            discover_case_files_recursive(root, &path, results);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("rosy") | Some("fox")
        ) {
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            results.push((name, path));
        }
    }
}

/// Result of a single construct test.
#[derive(Debug)]
struct TestResult {
    label: String,
    ok: bool,
    elapsed_secs: f64,
    failure_msg: Option<String>,
}

fn run_single_test(
    name: &str,
    case_path: &Path,
    build_dir: &Path,
    release: bool,
    bless: bool,
) -> TestResult {
    let t = Instant::now();
    let raw = match fs::read_to_string(case_path) {
        Ok(s) => s,
        Err(e) => {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("read failed: {e}")),
            };
        }
    };
    let case = parse_case(&raw);

    let binary = match compile_source(
        &case.source,
        Some(case_path),
        build_dir.to_path_buf(),
        release,
        false,
        true,
    ) {
        Ok(p) => p,
        Err(e) => {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("{e:#}")),
            };
        }
    };

    let output = match Command::new(&binary)
        .current_dir(case_path.parent().unwrap_or(case_path))
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("failed to spawn: {e}")),
            };
        }
    };
    if !output.status.success() {
        return TestResult {
            label: name.to_string(),
            ok: false,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: Some(format!(
                "runtime failed\n{}",
                String::from_utf8_lossy(&output.stderr)
            )),
        };
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if bless {
        if let Err(e) = write_blessed(case_path, &case, &stdout) {
            return TestResult {
                label: name.to_string(),
                ok: false,
                elapsed_secs: t.elapsed().as_secs_f64(),
                failure_msg: Some(format!("bless failed: {e}")),
            };
        }
        return TestResult {
            label: name.to_string(),
            ok: true,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: None,
        };
    }

    let Some(expect) = case.expect else {
        return TestResult {
            label: name.to_string(),
            ok: false,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: Some("missing `======= expect` (rosy test --bless)".into()),
        };
    };
    if norm_out(&stdout) != norm_out(&expect) {
        return TestResult {
            label: name.to_string(),
            ok: false,
            elapsed_secs: t.elapsed().as_secs_f64(),
            failure_msg: Some(format!(
                "stdout mismatch\n--- expected ---\n{}\n--- got ---\n{}",
                expect.trim_end(),
                stdout.trim_end()
            )),
        };
    }

    TestResult {
        label: name.to_string(),
        ok: true,
        elapsed_secs: t.elapsed().as_secs_f64(),
        failure_msg: None,
    }
}

pub(crate) fn run_construct_tests(filter: Option<&str>, release: bool, bless: bool) -> Result<()> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cases_dir = crate_root.join("../rosy-compiler/tests/constructs");
    let mut all_tests = discover_case_files(&cases_dir);

    if let Some(f) = filter {
        all_tests.retain(|(name, _)| name.contains(f));
    }

    let total = all_tests.len();
    if total == 0 {
        eprintln!(
            "No tests found{}",
            filter
                .map(|f| format!(" matching '{f}'"))
                .unwrap_or_default()
        );
        return Ok(());
    }

    eprintln!(
        "{BOLD}        Rosy{RESET} v{} — testing {total} construct{}",
        env!("CARGO_PKG_VERSION"),
        if total == 1 { "" } else { "s" }
    );
    if release {
        eprintln!("        Mode: release");
    }
    if bless {
        eprintln!("        Bless: rewriting expect sections");
    }

    let build_dir = std::env::temp_dir().join(format!("rosy_test_{}", std::process::id()));
    fs::create_dir_all(&build_dir).context("Failed to create build directory")?;
    eprintln!("  Build dir: {}\n", build_dir.display());

    let total_start = Instant::now();
    let mut results = Vec::with_capacity(total);

    for (i, (name, path)) in all_tests.iter().enumerate() {
        let result = run_single_test(name, path, &build_dir, release, bless);
        let n = i + 1;
        if result.ok {
            eprintln!(
                "{DIM}[{n:>3}/{total}]{RESET} {}... {GREEN}ok{RESET} {DIM}({:.1}s){RESET}",
                result.label, result.elapsed_secs
            );
        } else {
            eprintln!(
                "{DIM}[{n:>3}/{total}]{RESET} {}... {RED}FAIL{RESET} {DIM}({:.1}s){RESET}",
                result.label, result.elapsed_secs
            );
        }
        results.push(result);
    }

    let _ = fs::remove_dir_all(&build_dir);

    let passed = results.iter().filter(|r| r.ok).count();
    let failed = results.iter().filter(|r| !r.ok).count();
    let total_secs = total_start.elapsed().as_secs_f64();

    eprintln!();
    let failures: Vec<&TestResult> = results.iter().filter(|r| !r.ok).collect();
    if !failures.is_empty() {
        eprintln!("{BOLD}{RED}failures:{RESET}\n");
        for f in &failures {
            eprintln!(
                "  {}: {}\n",
                f.label,
                f.failure_msg.as_deref().unwrap_or("unknown")
            );
        }
    }

    eprintln!(
        "{BOLD}test result:{RESET} {} passed, {} failed ({:.1}s)",
        passed, failed, total_secs
    );

    if failed > 0 {
        Err(anyhow!("{} test(s) failed", failed))
    } else {
        Ok(())
    }
}
