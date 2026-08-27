use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    if Path::new("../rosy-lib").is_dir() {
        println!("cargo:warning=using local rosy-lib");
    }

    // Re-run if source changes
    println!("cargo:rerun-if-changed=src/program");
    println!("cargo:rerun-if-changed=src/compiler");
    println!("cargo:rerun-if-changed=../rosy-lib/src");
    println!("cargo:rerun-if-changed=assets/output_template/main.rs");

    let out_dir = std::env::var("OUT_DIR").unwrap();

    // ─── LSP: Generate keyword list and hover docs from grammar + modules ──
    let pest_path = Path::new("assets/rosy.pest");
    let registry_path = Path::new("../rosy-lib/src/registry.rs");
    let program_dir = Path::new("src/program");
    println!("cargo:rerun-if-changed={}", pest_path.display());
    println!("cargo:rerun-if-changed={}", registry_path.display());

    let intrinsics = if registry_path.exists() {
        let names = intrinsic_names_from_registry(registry_path);
        sync_pest_intrinsic_name(pest_path, &names);
        names
    } else {
        let pest = fs::read_to_string(pest_path).expect("Failed to read rosy.pest");
        intrinsic_names_from_pest(&pest)
    };

    let pest_source = fs::read_to_string(pest_path).expect("Failed to read rosy.pest");
    let keywords = extract_keywords(&pest_source);
    let module_docs = scan_module_docs(program_dir);
    let rule_to_keyword = build_rule_keyword_map(&pest_source);
    generate_keywords_file(&out_dir, &keywords, &module_docs);
    generate_hover_file(&out_dir, &module_docs, &rule_to_keyword);
    generate_editor_configs(&out_dir, &keywords);

    // ─── Tree-sitter: Generate grammar.js and highlights.scm from Pest ────
    let types = vec!["RE", "ST", "LO", "CM", "VE", "DA", "CD"];
    generate_tree_sitter_grammar(&out_dir, &keywords, &intrinsics, &types);
    generate_tree_sitter_highlights(&out_dir, &keywords, &intrinsics, &types);
}

fn intrinsic_names_from_pest(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        if !line.trim_start().starts_with("intrinsic_name") {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("^\"") {
            rest = &rest[start + 2..];
            if let Some(end) = rest.find('"') {
                names.push(rest[..end].to_ascii_uppercase());
                rest = &rest[end + 1..];
            } else {
                break;
            }
        }
    }
    names.sort();
    names.dedup();
    names
}

// ─── LSP: Keyword & Hover Doc Generation ───────────────────────────────────

const BASE_DOC_URL: &str = "https://rosy-team.github.io/rosy/rosy_compiler";

fn extract_keywords(source: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("keyword_raw")
            && let Some(start) = trimmed.find('{')
        {
            let body = &trimmed[start + 1..];
            let body = body.trim_end_matches('}').trim();
            for part in body.split('|') {
                if let Some(s) = extract_quoted(part.trim()) {
                    let upper = s.to_uppercase();
                    if !upper.is_empty()
                        && upper != "TRUE"
                        && upper != "FALSE"
                        && !upper.starts_with("ROSY_")
                    {
                        keywords.push(upper);
                    }
                }
            }
        }
    }
    keywords.sort();
    keywords.dedup();
    keywords
}

fn extract_quoted(s: &str) -> Option<String> {
    let s = s.trim().trim_start_matches('^');
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        Some(s[1..s.len() - 1].to_string())
    } else {
        None
    }
}

struct ModuleDoc {
    keyword: String,
    title: String,
    description: String,
    doc_url: String,
    kind: ModuleKind,
}

#[derive(Clone, Copy)]
enum ModuleKind {
    Statement,
    Expression,
}

fn scan_module_docs(program_dir: &Path) -> HashMap<String, ModuleDoc> {
    let mut docs = HashMap::new();
    for (subdir, kind) in [
        ("statements", ModuleKind::Statement),
        ("expressions", ModuleKind::Expression),
    ] {
        let dir = program_dir.join(subdir);
        if dir.is_dir() {
            scan_modules_recursive(&dir, &dir, kind, &mut docs);
        }
    }
    docs
}

fn scan_modules_recursive(
    base_dir: &Path,
    current_dir: &Path,
    kind: ModuleKind,
    docs: &mut HashMap<String, ModuleDoc>,
) {
    let Ok(entries) = fs::read_dir(current_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_modules_recursive(base_dir, &path, kind, docs);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
            && let Some(doc) = parse_module_doc(&path, &path.with_extension(""), base_dir, kind)
        {
            docs.insert(doc.keyword.clone(), doc);
        }
    }
}

fn parse_module_doc(
    mod_file: &Path,
    module_dir: &Path,
    base_dir: &Path,
    kind: ModuleKind,
) -> Option<ModuleDoc> {
    let content = fs::read_to_string(mod_file).ok()?;
    let dir_name = module_dir.file_name()?.to_str()?;
    let keyword = dir_name_to_keyword(dir_name)?;

    let mut doc_lines = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//!") {
            doc_lines.push(trimmed.trim_start_matches("//!").trim().to_string());
        } else if !trimmed.is_empty() {
            break;
        }
    }
    if doc_lines.is_empty() {
        return None;
    }

    let title = doc_lines[0].trim_start_matches('#').trim().to_string();
    let mut desc_parts = Vec::new();
    let mut started = false;
    for line in &doc_lines[1..] {
        if line.is_empty() {
            if started {
                break;
            }
            continue;
        }
        if line.starts_with('#') {
            break;
        }
        started = true;
        desc_parts.push(line.as_str());
    }
    let description = desc_parts.join(" ");

    let relative = module_dir.strip_prefix(base_dir).ok()?;
    let kind_prefix = match kind {
        ModuleKind::Statement => "program/statements",
        ModuleKind::Expression => "program/expressions",
    };
    let doc_url = format!(
        "{BASE_DOC_URL}/{kind_prefix}/{}/",
        relative.display().to_string().replace('\\', "/")
    );

    Some(ModuleDoc {
        keyword,
        title,
        description,
        doc_url,
        kind,
    })
}

fn dir_name_to_keyword(dir_name: &str) -> Option<String> {
    let keyword = match dir_name {
        "var_decl" => "VARIABLE",
        "var_expr" | "variable_identifier" | "assign" | "function_call" | "procedure_call" => {
            return None;
        }
        "while_loop" => "WHILE",
        "da_init" => "DAINI",
        "break" => "BREAK",
        "if" => "IF",
        "loop" => "LOOP",
        "ploop" => "PLOOP",
        "function" => "FUNCTION",
        "procedure" => "PROCEDURE",
        "quit" => "QUIT",
        "os_call" => "OS",
        "cos_fn" | "cos" => "COS",
        "sin" => "SIN",
        "tan_fn" | "tan" => "TAN",
        "asin_fn" | "asin" => "ASIN",
        "acos_fn" | "acos" => "ACOS",
        "atan_fn" | "atan" => "ATAN",
        "sinh_fn" | "sinh" => "SINH",
        "cosh_fn" | "cosh" => "COSH",
        "tanh_fn" | "tanh" => "TANH",
        "sqrt_fn" | "sqrt" => "SQRT",
        "exp_fn" | "exp" => "EXP",
        "log_fn" | "log" => "LOG",
        "abs_fn" | "abs" => "ABS",
        "norm_fn" | "norm" => "NORM",
        "cons_fn" | "cons" => "CONS",
        "int_fn" => "INT",
        "nint" => "NINT",
        "type_fn" => "TYPE",
        "real_fn" => "REAL",
        "imag_fn" => "IMAG",
        "re_convert" => "RE",
        "string_convert" => "ST",
        "logical_convert" => "LO",
        "complex_convert" => "CM",
        "ve_convert" => "VE",
        "erf" => "ERF",
        "werf" => "WERF",
        "isrt" => "ISRT",
        "isrt3" => "ISRT3",
        "cmplx" => "CMPLX",
        "conj" => "CONJ",
        "trim" => "TRIM",
        "ltrim" => "LTRIM",
        "length" => "LENGTH",
        "varmem" => "VARMEM",
        "varpoi" => "VARPOI",
        "pow" | "neg" | "not" => return None,
        "add" | "sub" | "mult" | "div" => return None,
        "eq" | "neq" | "lt" | "gt" | "lte" | "gte" => return None,
        "concat" | "derive" | "extract" => return None,
        "number" | "string" | "boolean" => return None,
        "cd" => "CD",
        "da" => "DA",
        "core" | "io" | "math" | "trig" | "exponential" | "rounding" | "special" | "vector"
        | "complex" | "memory" | "query" | "conversion" | "sys" | "collection" | "comparison"
        | "arithmetic" | "unary" | "types" | "operators" | "functions" => return None,
        other => return Some(other.to_uppercase()),
    };
    Some(keyword.to_string())
}

fn build_rule_keyword_map(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some((rule_name, rest)) = trimmed.split_once('=') {
            let rule_name = rule_name.trim();
            if let Some(kw) = extract_first_keyword(rest) {
                map.insert(rule_name.to_string(), kw.to_uppercase());
            }
        }
    }
    map
}

fn extract_first_keyword(body: &str) -> Option<String> {
    let body = body.trim().trim_start_matches('{').trim();
    if let Some(pos) = body.find("^\"") {
        let rest = &body[pos + 2..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn generate_keywords_file(
    out_dir: &str,
    keywords: &[String],
    module_docs: &HashMap<String, ModuleDoc>,
) {
    let dest = Path::new(out_dir).join("keywords_generated.rs");
    let mut f = fs::File::create(&dest).unwrap();
    writeln!(
        f,
        "// Auto-generated from rosy.pest + module docs by build.rs — do not edit!"
    )
    .unwrap();
    writeln!(f).unwrap();
    writeln!(f, "pub const ROSY_KEYWORD_LIST: &[(&str, &str)] = &[").unwrap();
    for kw in keywords {
        let desc = module_docs
            .get(kw.as_str())
            .map(|d| {
                if d.description.is_empty() {
                    d.title.clone()
                } else {
                    d.description.clone()
                }
            })
            .unwrap_or_else(|| "Rosy keyword".to_string());
        let desc = desc.replace('\\', "\\\\").replace('"', "\\\"");
        writeln!(f, "    (\"{kw}\", \"{desc}\"),").unwrap();
    }
    writeln!(f, "];").unwrap();
}

fn generate_hover_file(
    out_dir: &str,
    module_docs: &HashMap<String, ModuleDoc>,
    _rule_to_keyword: &HashMap<String, String>,
) {
    let dest = Path::new(out_dir).join("hover_generated.rs");
    let mut f = fs::File::create(&dest).unwrap();
    writeln!(
        f,
        "// Auto-generated from module doc comments by build.rs — do not edit!"
    )
    .unwrap();
    writeln!(f).unwrap();
    writeln!(
        f,
        "pub const ROSY_HOVER_DOCS: &[(&str, &str, &str, &str, bool)] = &["
    )
    .unwrap();
    let mut entries: Vec<_> = module_docs.iter().collect();
    entries.sort_by_key(|(k, _)| (*k).clone());
    for (keyword, doc) in &entries {
        let title = doc.title.replace('\\', "\\\\").replace('"', "\\\"");
        let desc = doc.description.replace('\\', "\\\\").replace('"', "\\\"");
        let url = &doc.doc_url;
        let is_stmt = matches!(doc.kind, ModuleKind::Statement);
        writeln!(
            f,
            "    (\"{keyword}\", \"{title}\", \"{desc}\", \"{url}\", {is_stmt}),"
        )
        .unwrap();
    }
    // Manual entry for INCLUDE — it has no statement module since it's
    // resolved during AST construction, not as a runtime Statement variant.
    writeln!(
        f,
        "    (\"INCLUDE\", \"INCLUDE — File Inclusion\", \
         \"Includes another ROSY source file at this point in the program. \
         The included file must be a complete BEGIN/END program. \
         Its statements are spliced into the including program at the INCLUDE site.\\n\\n\
         The path resolves relative to the directory of the including file. \
         If the path names a directory, INCLUDE looks for `mod.rosy` inside it \
         (Rust-style modules), letting you organize libraries as directory trees:\\n\\n\
         ```\\nINCLUDE 'helpers.rosy';            {{ single file }}\\n\
         INCLUDE 'libcosy';                  {{ directory -> libcosy/mod.rosy }}\\n```\", \
         \"\", true),"
    )
    .unwrap();
    writeln!(f, "];").unwrap();

    writeln!(f).unwrap();
    writeln!(f, "pub const ROSY_TYPE_HOVER: &[(&str, &str, &str)] = &[").unwrap();
    for (name, rust_type, desc) in [
        ("RE", "f64", "Real number"),
        ("ST", "String", "String"),
        ("LO", "bool", "Logical / boolean"),
        ("CM", "Complex64", "Complex number"),
        ("VE", "Vec<f64>", "Vector"),
        ("DA", "Taylor series", "Differential Algebra"),
        (
            "CD",
            "Complex Taylor series",
            "Complex Differential Algebra",
        ),
    ] {
        writeln!(f, "    (\"{name}\", \"**{name}** \\u{{2014}} {desc} (`{rust_type}`)\\n\\n[Documentation](https://rosy-team.github.io/rosy/rosy_lib/enum.RosyBaseType.html#variant.{name})\", \"{desc}\"),").unwrap();
    }
    writeln!(f, "];").unwrap();
}

/// Generate editor configuration files derived from the keyword list.
///
/// Derives folding/indent markers from `END*` keywords and their openers.
/// Generates VS Code language-configuration.json, Zed config.toml, and
/// Zed extension.toml + LSP settings snippet.
fn generate_editor_configs(out_dir: &str, keywords: &[String]) {
    // Find block-closer keywords (END*) and their openers
    let mut closers: Vec<String> = Vec::new();
    let mut openers: Vec<String> = Vec::new();
    let mut mid_block: Vec<String> = Vec::new();

    for kw in keywords {
        if let Some(opener) = kw.strip_prefix("END") {
            closers.push(kw.clone());
            if !opener.is_empty() && keywords.contains(&opener.to_string()) {
                openers.push(opener.to_string());
            }
        }
    }

    if keywords.contains(&"BEGIN".to_string()) {
        openers.push("BEGIN".to_string());
    }
    for kw in ["ELSEIF", "ELSE"] {
        if keywords.contains(&kw.to_string()) {
            mid_block.push(kw.to_string());
        }
    }

    openers.sort();
    openers.dedup();
    closers.sort();
    mid_block.sort();

    let openers_joined = openers.join("|");
    let closers_joined = closers.join("|");
    let mid_block_joined = mid_block.join("|");

    // ─── VS Code language-configuration.json ───────────────────────────
    let increase = format!("{openers_joined}|{mid_block_joined}");
    let decrease = format!("{closers_joined}|{mid_block_joined}");

    let vscode_config = format!(
        r#"{{
  "comments": {{
    "blockComment": ["{{", "}}"]
  }},
  "brackets": [
    ["(", ")"],
    ["[", "]"]
  ],
  "autoClosingPairs": [
    {{ "open": "(", "close": ")" }},
    {{ "open": "[", "close": "]" }},
    {{ "open": "{{", "close": "}}" }},
    {{ "open": "'", "close": "'", "notIn": ["string"] }},
    {{ "open": "\"", "close": "\"", "notIn": ["string"] }}
  ],
  "surroundingPairs": [
    ["(", ")"],
    ["[", "]"],
    ["{{", "}}"],
    ["'", "'"],
    ["\"", "\""]
  ],
  "folding": {{
    "markers": {{
      "start": "^\\s*({openers_joined})\\b",
      "end": "^\\s*({closers_joined})\\b"
    }}
  }},
  "indentationRules": {{
    "increaseIndentPattern": "^\\s*({increase})\\b",
    "decreaseIndentPattern": "^\\s*({decrease})\\b"
  }},
  "wordPattern": "[a-zA-Z_][a-zA-Z0-9_]*"
}}
"#
    );

    fs::write(
        Path::new(out_dir).join("vscode_language_configuration.json"),
        &vscode_config,
    )
    .unwrap();
}

// ─── Tree-sitter Grammar Generation ──────────────────────────────────────────

/// Names from `unary!("SIN", …)` / `binary!("POSITION", …)` in `registry.rs`, plus DA/CD.
fn intrinsic_names_from_registry(registry_path: &Path) -> Vec<String> {
    let src = fs::read_to_string(registry_path).expect("read registry.rs");
    let mut names = Vec::new();
    let mut rest = src.as_str();
    loop {
        let u = rest.find("unary!(");
        let b = rest.find("binary!(");
        let start = match (u, b) {
            (Some(u), Some(b)) => u.min(b),
            (Some(u), None) => u,
            (None, Some(b)) => b,
            (None, None) => break,
        };
        rest = &rest[start..];
        rest = match rest.find('(') {
            Some(i) => &rest[i + 1..],
            None => break,
        };
        rest = rest.trim_start();
        if !rest.starts_with('"') {
            continue;
        }
        rest = &rest[1..];
        if let Some(end) = rest.find('"') {
            let name = rest[..end].to_ascii_uppercase();
            if name.chars().all(|c| c.is_ascii_alphanumeric()) {
                names.push(name);
            }
            rest = &rest[end + 1..];
        }
    }
    names.extend(["DA".into(), "CD".into()]);
    names.sort();
    names.dedup();
    names
}

fn sync_pest_intrinsic_name(pest_path: &Path, names: &[String]) {
    let mut ordered = names.to_vec();
    ordered.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    let alts = ordered
        .iter()
        .map(|n| format!("^\"{n}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    let new_line = format!("  intrinsic_name = @{{ ({alts}) ~ !(ASCII_ALPHANUMERIC | \"_\") }}");

    let src = fs::read_to_string(pest_path).expect("read rosy.pest");
    let mut replaced = false;
    let mut out = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("intrinsic_name") {
            if line == new_line {
                return;
            }
            out.push_str(&new_line);
            replaced = true;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    assert!(replaced, "rosy.pest missing intrinsic_name rule");
    if !src.ends_with('\n') {
        out.pop();
    }
    fs::write(pest_path, out).expect("write rosy.pest");
}

/// Generate a Tree-sitter grammar.js that tokenizes ROSY source code.
///
/// Keywords, builtins, types, and booleans are string literals inside hidden
/// (`_`-prefixed) rules.  Tree-sitter produces anonymous nodes for these,
/// which highlights.scm matches by string value (e.g. `"BEGIN" @keyword`).
/// The `word` property ensures string literals only match complete words,
/// so `VE` never matches inside `VECTOR_DEMO`.
///
/// This mirrors the approach used by tree-sitter-rust and other official
/// Tree-sitter grammars.
fn generate_tree_sitter_grammar(
    out_dir: &str,
    keywords: &[String],
    intrinsics: &[String],
    types: &[&str],
) {
    let mut grammar = String::new();
    grammar.push_str("// Auto-generated from rosy.pest by build.rs — do not edit!\n");
    grammar.push_str("// Regenerate with: cargo build -p rosy-compiler\n\n");
    grammar.push_str("/// @ts-nocheck\n");
    grammar.push_str("module.exports = grammar({\n");
    grammar.push_str("  name: 'rosy',\n\n");
    grammar.push_str("  extras: $ => [/\\s/, $.comment],\n\n");
    grammar.push_str("  word: $ => $.identifier,\n\n");
    grammar.push_str("  rules: {\n");
    grammar.push_str("    source_file: $ => repeat($._item),\n\n");
    grammar.push_str("    _item: $ => choice(\n");
    grammar.push_str("      $._keyword,\n");
    grammar.push_str("      $._builtin,\n");
    grammar.push_str("      $._type_name,\n");
    grammar.push_str("      $._boolean,\n");
    grammar.push_str("      $.number,\n");
    grammar.push_str("      $.string,\n");
    grammar.push_str("      $.operator,\n");
    grammar.push_str("      $.punctuation,\n");
    grammar.push_str("      $.identifier,\n");
    grammar.push_str("    ),\n\n");

    // Comments
    grammar.push_str("    comment: $ => seq('{', repeat(choice($.comment, /[^{}]+/)), '}'),\n\n");

    // Keywords — hidden rule, produces anonymous string-literal nodes
    let stmt_keywords: Vec<&String> = keywords
        .iter()
        .filter(|kw| {
            !intrinsics.contains(kw)
                && !types.contains(&kw.as_str())
                && *kw != "TRUE"
                && *kw != "FALSE"
        })
        .collect();
    grammar.push_str("    _keyword: _ => choice(\n");
    for (i, kw) in stmt_keywords.iter().enumerate() {
        let comma = if i < stmt_keywords.len() - 1 { "," } else { "" };
        grammar.push_str(&format!(
            "      '{}', '{}'{}\n",
            kw.to_uppercase(),
            kw.to_lowercase(),
            comma
        ));
    }
    grammar.push_str("    ),\n\n");

    // Builtins — hidden rule (excluding types)
    let filtered_intrinsics: Vec<&String> = intrinsics
        .iter()
        .filter(|f| !types.contains(&f.as_str()))
        .collect();
    grammar.push_str("    _builtin: _ => choice(\n");
    for (i, func) in filtered_intrinsics.iter().enumerate() {
        let comma = if i < filtered_intrinsics.len() - 1 {
            ","
        } else {
            ""
        };
        grammar.push_str(&format!(
            "      '{}', '{}'{}\n",
            func.to_uppercase(),
            func.to_lowercase(),
            comma
        ));
    }
    grammar.push_str("    ),\n\n");

    // Type names — hidden rule
    grammar.push_str("    _type_name: _ => choice(\n");
    for (i, ty) in types.iter().enumerate() {
        let comma = if i < types.len() - 1 { "," } else { "" };
        grammar.push_str(&format!(
            "      '{}', '{}'{}\n",
            ty.to_uppercase(),
            ty.to_lowercase(),
            comma
        ));
    }
    grammar.push_str("    ),\n\n");

    // Booleans — hidden rule
    grammar.push_str("    _boolean: _ => choice('TRUE', 'true', 'FALSE', 'false'),\n\n");

    // Literals
    grammar.push_str("    number: _ => /\\d+(\\.\\d+)?/,\n");
    grammar.push_str("    string: _ => choice(/\"[^\"]*\"/, /'(?:''|[^'])*'/),\n\n");

    // Operators
    grammar.push_str("    operator: _ => choice(\n");
    grammar.push_str("      ':=', '==', '!=', '<>', '<=', '>=',\n");
    grammar.push_str("      '+', '-', '*', '/', '^', '%', '|', '&', '#',\n");
    grammar.push_str("      '=', '<', '>', '!',\n");
    grammar.push_str("    ),\n\n");

    // Punctuation & identifiers
    grammar.push_str("    punctuation: _ => choice(';', '(', ')', '[', ']', ',', '.'),\n");
    grammar.push_str("    identifier: _ => /[a-zA-Z_][a-zA-Z0-9_]*/,\n");

    grammar.push_str("  },\n");
    grammar.push_str("});\n");

    fs::write(Path::new(out_dir).join("grammar.js"), &grammar).unwrap();
}

/// Generate highlights.scm for Zed/Tree-sitter.
///
/// Keywords, builtins, types, and booleans are anonymous string-literal nodes
/// in the grammar (inside hidden `_`-prefixed rules).  We match them here by
/// their string value, e.g. `"BEGIN" @keyword` — the same pattern used by
/// tree-sitter-rust and other official grammars.
fn generate_tree_sitter_highlights(
    out_dir: &str,
    keywords: &[String],
    intrinsics: &[String],
    types: &[&str],
) {
    let mut scm = String::new();
    scm.push_str("; Auto-generated from rosy.pest by build.rs — do not edit!\n");
    scm.push_str("; Regenerate with: cargo build -p rosy-compiler\n\n");

    // ── Named nodes ───────────────────────────────────────────────────
    scm.push_str("; Comments\n");
    scm.push_str("(comment) @comment\n\n");

    scm.push_str("; Numbers\n");
    scm.push_str("(number) @number\n\n");

    scm.push_str("; Strings\n");
    scm.push_str("(string) @string\n\n");

    scm.push_str("; Operators\n");
    scm.push_str("(operator) @operator\n\n");

    scm.push_str("; Punctuation\n");
    scm.push_str("(punctuation) @punctuation\n\n");

    scm.push_str("; Identifiers (fallback for variables)\n");
    scm.push_str("(identifier) @variable\n\n");

    // ── Anonymous string-literal nodes matched by value ────────────────

    // Type names
    scm.push_str("; Type names\n");
    for ty in types {
        scm.push_str(&format!("\"{}\" @type.builtin\n", ty.to_uppercase()));
        scm.push_str(&format!("\"{}\" @type.builtin\n", ty.to_lowercase()));
    }
    scm.push('\n');

    // Builtin functions (excluding types)
    scm.push_str("; Intrinsic functions\n");
    for func in intrinsics {
        if types.contains(&func.as_str()) {
            continue;
        }
        scm.push_str(&format!("\"{}\" @function.builtin\n", func.to_uppercase()));
        scm.push_str(&format!("\"{}\" @function.builtin\n", func.to_lowercase()));
    }
    scm.push('\n');

    // Statement keywords (excluding intrinsics, types, booleans)
    scm.push_str("; Keywords\n");
    for kw in keywords {
        if intrinsics.contains(kw) || types.contains(&kw.as_str()) || kw == "TRUE" || kw == "FALSE"
        {
            continue;
        }
        scm.push_str(&format!("\"{}\" @keyword\n", kw.to_uppercase()));
        scm.push_str(&format!("\"{}\" @keyword\n", kw.to_lowercase()));
    }
    scm.push('\n');

    // Booleans
    scm.push_str("; Booleans\n");
    scm.push_str("\"TRUE\" @constant.builtin\n");
    scm.push_str("\"true\" @constant.builtin\n");
    scm.push_str("\"FALSE\" @constant.builtin\n");
    scm.push_str("\"false\" @constant.builtin\n");

    fs::write(Path::new(out_dir).join("highlights.scm"), &scm).unwrap();
}
