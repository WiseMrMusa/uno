use clap::{Parser, Subcommand, ValueEnum};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;
use uno_codegen::c::Codegen;
use uno_codegen::wasm::WasmCodegen;
use uno_codegen::CodegenError;
use uno_ir::lower::lower;
use uno_ir::IrBackend;
use uno_syntax::diagnostic::DiagnosticBag;
use uno_syntax::source_file::SourceFile;

#[derive(Parser)]
#[command(name = "uno", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    cmd: Subcmd,

    #[arg(short = 'v', long, global = true, default_value_t = false)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Subcmd {
    New { name: String },
    Build {
        #[arg(long, default_value = "c")]
        target: Target,
        #[arg(long, default_value = "target")]
        out_dir: String,
    },
    Run {
        #[arg(long, default_value = "c")]
        target: Target,
        #[arg(long, default_value = "target")]
        out_dir: String,
    },
    Check {
        #[arg(long, default_value = "target")]
        out_dir: String,
    },
    Clean,
}

#[derive(Clone, Debug, ValueEnum)]
enum Target {
    C,
    Wasm,
}

#[derive(serde::Deserialize)]
struct Manifest {
    package: Package,
}

#[derive(serde::Deserialize)]
struct Package {
    name: String,
}

struct ProjectContext {
    name: String,
    sources: HashMap<PathBuf, SourceFile>,
    visited: HashSet<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let res = match cli.cmd {
        Subcmd::New { name } => cmd_new(&name),
        Subcmd::Build { target, ref out_dir } => cmd_build(false, &target, out_dir, cli.verbose),
        Subcmd::Run { target, ref out_dir } => cmd_build(true, &target, out_dir, cli.verbose),
        Subcmd::Check { ref out_dir } => cmd_check(out_dir, cli.verbose),
        Subcmd::Clean => cmd_clean(cli.verbose),
    };
    if let Err(e) = res {
        let color_err = std::io::stderr().is_terminal();
        let (red, bold, reset) = if color_err {
            ("\x1b[1;31m", "\x1b[1m", "\x1b[0m")
        } else {
            ("", "", "")
        };
        eprintln!("{red}{bold}error:{reset} {e}");
        std::process::exit(1);
    }
}

fn cmd_new(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(name);
    if dir.exists() {
        return Err(format!("directory '{name}' already exists").into());
    }

    fs::create_dir_all(dir.join("src"))?;

    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
"#
    );
    fs::write(dir.join("uno.toml"), manifest)?;

    let main_source = "fn main() -> u32 {\n    return 0;\n}\n";
    fs::write(dir.join("src").join("main.uno"), main_source)?;

    println!("Created project '{name}'");
    Ok(())
}

fn read_project(verbose: bool) -> Result<ProjectContext, Box<dyn std::error::Error>> {
    let manifest_str = fs::read_to_string("uno.toml")
        .map_err(|_| "uno.toml not found (are you in an Uno project?)")?;
    let manifest: Manifest = toml::from_str(&manifest_str)?;
    let project_name = manifest.package.name;

    let src_dir = PathBuf::from("src");
    let main_path = src_dir.join("main.uno");

    let mut ctx = ProjectContext {
        name: project_name,
        sources: HashMap::new(),
        visited: HashSet::new(),
    };

    if verbose { eprintln!("reading sources..."); }
    collect_sources(&src_dir, &main_path, &mut ctx, verbose)?;

    Ok(ctx)
}

fn collect_sources(
    src_dir: &Path,
    current_path: &Path,
    ctx: &mut ProjectContext,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let canon = current_path.canonicalize().unwrap_or_else(|_| current_path.to_path_buf());
    if ctx.visited.contains(&canon) {
        return Ok(());
    }
    ctx.visited.insert(canon.clone());

    if verbose { eprintln!("  reading {}", current_path.display()); }

    let source_str = fs::read_to_string(current_path)
        .map_err(|_| format!("{} not found", current_path.display()))?;
    let sf = SourceFile::with_path(source_str.clone(), current_path.display().to_string());
    ctx.sources.insert(current_path.to_path_buf(), sf);

    let mut lexer = uno_lexer::Lexer::new(source_str);
    let tokens = lexer.tokenize();
    let mut parser = uno_parser::Parser::new(tokens);
    let program = parser.parse_program()
        .map_err(|e| {
            let sf = ctx.sources.get(current_path).unwrap();
            sf.format_parse_error(&e)
        })?;

    for import in &program.imports {
        let path = src_dir.join(format!("{import}.uno"));
        if !path.exists() {
            let sf = ctx.sources.get(current_path).unwrap();
            eprintln!(
                "{}",
                sf.format_error(uno_syntax::span::Span::empty(), &format!("import '{import}.uno' not found"))
            );
            continue;
        }
        collect_sources(src_dir, &path, ctx, verbose)?;
    }

    Ok(())
}

fn parse_all(
    ctx: &mut ProjectContext,
    verbose: bool,
) -> Result<uno_ir::IrProgram, Box<dyn std::error::Error>> {
    let mut all_bag = DiagnosticBag::new();
    let mut all_functions = Vec::new();

    if verbose { eprintln!("parsing..."); }
    let entry_order: Vec<PathBuf> = ctx.sources.keys().cloned().collect();
    for path in &entry_order {
        if verbose { eprintln!("  parsing {}", path.display()); }
        let source_str = fs::read_to_string(path)
            .map_err(|_| format!("cannot read {}", path.display()))?;

        let mut lexer = uno_lexer::Lexer::new(source_str);
        let tokens = lexer.tokenize();
        let mut parser = uno_parser::Parser::new(tokens);
        let (program, mut diags) = parser.parse_program_check();
        all_bag.merge(&mut diags);
        all_functions.extend(program.functions);
    }

    if all_bag.has_errors() {
        let sf = SourceFile::new(String::new());
        eprint!("{}", sf.format_diagnostics(&all_bag));
        return Err("compilation failed due to errors".into());
    }

    let merged = uno_syntax::ast::Program {
        imports: Vec::new(),
        functions: all_functions,
    };

    if verbose { eprintln!("including standard library..."); }
    let merged = inject_stdlib(merged);

    if verbose { eprintln!("lowering to IR..."); }
    lower(&merged).map_err(|e| format!("lowering error: {e}").into())
}

fn inject_stdlib(program: uno_syntax::ast::Program) -> uno_syntax::ast::Program {
    let mut prog = program;

    let print_source = r#"
fn __print_u32(val: u32) -> u32 { return val; }
fn __print_bool(val: bool) -> u32 { return 0; }
"#;

    let mut lexer = uno_lexer::Lexer::new(print_source.to_string());
    let tokens = lexer.tokenize();
    let mut parser = uno_parser::Parser::new(tokens);
    if let Ok(mut std_prog) = parser.parse_program() {
        prog.functions.append(&mut std_prog.functions);
    }

    prog
}

fn get_backend(target: &Target) -> Box<dyn IrBackend<Output = String, Error = CodegenError>> {
    match target {
        Target::C => Box::new(Codegen::new()),
        Target::Wasm => Box::new(WasmCodegen::new()),
    }
}

struct CacheEntry {
    hash: u64,
    output: String,
}

fn file_hash(path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let meta = fs::metadata(path)?;
    let modified = meta.modified()
        .unwrap_or(SystemTime::UNIX_EPOCH)
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let len = meta.len();
    Ok(modified.wrapping_mul(31).wrapping_add(len))
}

fn cmd_build(
    run_after: bool,
    target: &Target,
    out_dir: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_dir = PathBuf::from(out_dir);
    fs::create_dir_all(&target_dir)?;

    let cache_path = target_dir.join(".uno_cache.toml");
    let mut cache: HashMap<String, CacheEntry> = if cache_path.exists() {
        let _data = fs::read_to_string(&cache_path)?;
        HashMap::new()
    } else {
        HashMap::new()
    };

    let mut ctx = read_project(verbose)?;

    let mut hash_combined: u64 = 0;
    for path in ctx.sources.keys() {
        hash_combined = hash_combined.wrapping_add(file_hash(path)?);
    }

    let cache_key = format!("{:?}_{}", target, hash_combined);
    if let Some(_entry) = cache.get(&cache_key) {
        let output_path = match target {
            Target::C => {
                let bin = if cfg!(target_os = "windows") {
                    format!("{}.exe", ctx.name)
                } else {
                    ctx.name.clone()
                };
                target_dir.join(&bin)
            }
            Target::Wasm => target_dir.join(format!("{}.wasm", ctx.name)),
        };
        if output_path.exists() {
            if verbose { eprintln!("cache hit — skipping compilation"); }
            if run_after {
                if let Target::C = target {
                    let status = Command::new(&output_path).status()
                        .map_err(|_| "failed to run binary")?;
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            println!("{} (cached)", ctx.name);
            return Ok(());
        }
    }

    let ir = parse_all(&mut ctx, verbose)?;

    if verbose { eprintln!("generating code for target {target:?}..."); }

    let mut backend = get_backend(target);
    let code = backend.generate(&ir)?;
    let backend_name = backend.name();

    match target {
        Target::C => {
            let c_path = target_dir.join("output.c");

            let code_with_lines = add_line_directives(&code, "src/main.uno");
            fs::write(&c_path, &code_with_lines)?;

            if verbose { eprintln!("compiling with cc..."); }
            let binary_name = if cfg!(target_os = "windows") {
                format!("{}.exe", ctx.name)
            } else {
                ctx.name.clone()
            };
            let binary_path = target_dir.join(&binary_name);

            let status = Command::new("cc")
                .arg("-std=c23")
                .arg("-g")
                .arg("-o")
                .arg(&binary_path)
                .arg(&c_path)
                .status()
                .map_err(|_| "failed to run 'cc' (is a C compiler installed?)")?;

            if !status.success() {
                return Err("C compilation failed".into());
            }

            cache.insert(cache_key.clone(), CacheEntry {
                hash: hash_combined,
                output: c_path.display().to_string(),
            });

            println!("Compiled '{}' ({backend_name})", ctx.name);

            if run_after {
                if verbose { eprintln!("running {}...", binary_path.display()); }
                let status = Command::new(&binary_path)
                    .status()
                    .map_err(|_| "failed to run binary")?;
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Target::Wasm => {
            let wat_path = target_dir.join("output.wat");
            fs::write(&wat_path, &code)?;

            if verbose { eprintln!("compiling with wat2wasm..."); }
            let wasm_path = target_dir.join(format!("{}.wasm", ctx.name));

            let wat2wasm = Command::new("npx")
                .args(["--yes", "wat2wasm"])
                .arg(&wat_path)
                .arg("-o")
                .arg(&wasm_path)
                .status()
                .or_else(|_| {
                    Command::new("wat2wasm")
                        .arg(&wat_path)
                        .arg("-o")
                        .arg(&wasm_path)
                        .status()
                });

            match wat2wasm {
                Ok(status) if status.success() => {
                    cache.insert(cache_key.clone(), CacheEntry {
                        hash: hash_combined,
                        output: wasm_path.display().to_string(),
                    });
                    println!("Compiled '{}' to WASM ({backend_name})", ctx.name);
                    if run_after {
                        run_wasm(&target_dir, &wasm_path, verbose)?;
                    }
                }
                Ok(_) => return Err("WASM compilation failed (install wabt/wat2wasm)".into()),
                Err(_) => {
                    eprintln!("wrote {} — install wabt (wat2wasm) or use npx to compile", wat_path.display());
                }
            }
        }
    }

    let cache_data = format!("# incremental cache\n");
    fs::write(&cache_path, cache_data)?;

    Ok(())
}

fn add_line_directives(code: &str, source_file: &str) -> String {
    let mut out = String::new();
    out.push_str("# 1 \"");
    out.push_str(source_file);
    out.push_str("\" 1\n");
    out.push_str(code);
    out
}

fn run_wasm(
    target_dir: &PathBuf,
    wasm_path: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = which_runtime();
    match runtime {
        Some("wasmtime") => {
            if verbose { eprintln!("running wasmtime {}...", wasm_path.display()); }
            let status = Command::new("wasmtime")
                .arg(wasm_path)
                .status()
                .map_err(|_| "failed to run wasmtime")?;
            std::process::exit(status.code().unwrap_or(1));
        }
        Some("node") => {
            if verbose { eprintln!("running node with WebAssembly..."); }
            let runner = target_dir.join("run.mjs");
            let js = format!(
                r#"import {{ readFile }} from "node:fs/promises";
const wasm = await readFile("{}");
const mod = await WebAssembly.compile(wasm);
const instance = await WebAssembly.instantiate(mod);
const result = instance.exports.main();
if (typeof result === "bigint") {{
  console.log("exit:", Number(result));
}} else {{
  console.log("exit:", result);
}}
"#,
                wasm_path.display()
            );
            fs::write(&runner, js)?;
            let status = Command::new("node")
                .arg(&runner)
                .status()
                .map_err(|_| "failed to run node")?;
            std::process::exit(status.code().unwrap_or(1));
        }
        _ => {
            eprintln!("note: install wasmtime or node to run WASM");
            Ok(())
        }
    }
}

fn cmd_check(out_dir: &str, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx = read_project(verbose)?;
    let ir = parse_all(&mut ctx, verbose)?;
    if verbose {
        eprintln!("IR: {} functions", ir.functions.len());
    }
    println!("check: no errors found");
    let _ = out_dir;
    Ok(())
}

fn cmd_clean(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let target = PathBuf::from("target");
    if target.exists() {
        if verbose { eprintln!("removing {}...", target.display()); }
        fs::remove_dir_all(&target)?;
        println!("removed {}", target.display());
    } else {
        println!("nothing to clean");
    }
    Ok(())
}

fn which_runtime() -> Option<&'static str> {
    for cmd in &["wasmtime", "node"] {
        if Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cmd);
        }
    }
    None
}
