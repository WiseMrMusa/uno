use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uno_codegen::c::Codegen;
use uno_codegen::wasm::WasmCodegen;
use uno_codegen::CodegenError;
use uno_syntax::ast::Program;
use uno_syntax::backend::Backend;
use uno_syntax::source_file::SourceFile;

#[derive(Parser)]
#[command(name = "uno", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    cmd: Subcmd,

    /// Verbose output
    #[arg(short = 'v', long, global = true, default_value_t = false)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Subcmd {
    /// Create a new Uno project
    New { name: String },
    /// Build the current project
    Build {
        /// Target backend (c, wasm)
        #[arg(long, default_value = "c")]
        target: Target,
        /// Output directory
        #[arg(long, default_value = "target")]
        out_dir: String,
    },
    /// Build and run the current project
    Run {
        /// Target backend (c, wasm)
        #[arg(long, default_value = "c")]
        target: Target,
        /// Output directory
        #[arg(long, default_value = "target")]
        out_dir: String,
    },
    /// Parse and check the current project without codegen
    Check {
        /// Output directory (reserved)
        #[arg(long, default_value = "target")]
        out_dir: String,
    },
    /// Remove the target directory
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

fn main() {
    let cli = Cli::parse();
    let res = match cli.cmd {
        Subcmd::New { name } => cmd_new(&name),
        Subcmd::Build {
            target,
            ref out_dir,
        } => cmd_build(false, &target, out_dir, cli.verbose),
        Subcmd::Run {
            target,
            ref out_dir,
        } => cmd_build(true, &target, out_dir, cli.verbose),
        Subcmd::Check { ref out_dir } => cmd_check(out_dir, cli.verbose),
        Subcmd::Clean => cmd_clean(cli.verbose),
    };
    if let Err(e) = res {
        eprintln!("error: {e}");
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

fn read_project() -> Result<(String, SourceFile, uno_parser::Parser), Box<dyn std::error::Error>> {
    let manifest_str = fs::read_to_string("uno.toml")
        .map_err(|_| "uno.toml not found (are you in an Uno project?)")?;
    let manifest: Manifest = toml::from_str(&manifest_str)?;
    let project_name = manifest.package.name;

    let source_str = fs::read_to_string("src/main.uno")
        .map_err(|_| "src/main.uno not found")?;
    let source = SourceFile::new(source_str.clone());

    let mut lexer = uno_lexer::Lexer::new(source_str);
    let tokens = lexer.tokenize();

    let parser = uno_parser::Parser::new(tokens);
    Ok((project_name, source, parser))
}

fn parse_program(
    source: &SourceFile,
    parser: &mut uno_parser::Parser,
) -> Result<Program, Box<dyn std::error::Error>> {
    parser.parse_program().map_err(|e| {
        let formatted = source.format_error(e.span, &e.message);
        format!("{formatted}").into()
    })
}

fn get_backend(target: &Target) -> Box<dyn Backend<Output = String, Err = CodegenError>> {
    match target {
        Target::C => Box::new(Codegen::new()),
        Target::Wasm => Box::new(WasmCodegen::new()),
    }
}

fn cmd_build(
    run_after: bool,
    target: &Target,
    out_dir: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (project_name, source, mut parser) = read_project()?;

    if verbose {
        eprintln!("parsing {}...", "src/main.uno");
    }
    let program = parse_program(&source, &mut parser)?;

    let target_dir = PathBuf::from(out_dir);
    fs::create_dir_all(&target_dir)?;

    if verbose {
        eprintln!("generating code for target {target:?}...");
    }

    let mut backend = get_backend(target);
    let code = backend.generate(&program)?;
    let backend_name = backend.name();

    match target {
        Target::C => {
            let c_path = target_dir.join("output.c");
            fs::write(&c_path, &code)?;

            if verbose {
                eprintln!("compiling with cc...");
            }
            let binary_name = if cfg!(target_os = "windows") {
                format!("{project_name}.exe")
            } else {
                project_name.clone()
            };
            let binary_path = target_dir.join(&binary_name);

            let status = Command::new("cc")
                .arg("-std=c23")
                .arg("-o")
                .arg(&binary_path)
                .arg(&c_path)
                .status()
                .map_err(|_| "failed to run 'cc' (is a C compiler installed?)")?;

            if !status.success() {
                return Err("C compilation failed".into());
            }

            println!("Compiled '{project_name}' ({backend_name})");

            if run_after {
                if verbose {
                    eprintln!("running {}...", binary_path.display());
                }
                let status = Command::new(&binary_path)
                    .status()
                    .map_err(|_| "failed to run binary")?;
                std::process::exit(status.code().unwrap_or(1));
            }
        }
        Target::Wasm => {
            let wat_path = target_dir.join("output.wat");
            fs::write(&wat_path, &code)?;

            if verbose {
                eprintln!("compiling with wat2wasm...");
            }
            let wasm_path = target_dir.join(format!("{project_name}.wasm"));

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
                    println!("Compiled '{project_name}' to WASM ({backend_name})");

                    if run_after {
                        run_wasm(&target_dir, &wasm_path, verbose)?;
                    }
                }
                Ok(_) => {
                    return Err(
                        "WASM compilation failed (install wabt/wat2wasm)".into(),
                    );
                }
                Err(_) => {
                    eprintln!(
                        "wrote {} — install wabt (wat2wasm) or use npx to compile",
                        wat_path.display()
                    );
                }
            }
        }
    }

    Ok(())
}

fn run_wasm(
    target_dir: &PathBuf,
    wasm_path: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = which_runtime();
    match runtime {
        Some("wasmtime") => {
            if verbose {
                eprintln!("running wasmtime {}...", wasm_path.display());
            }
            let status = Command::new("wasmtime")
                .arg(wasm_path)
                .status()
                .map_err(|_| "failed to run wasmtime")?;
            std::process::exit(status.code().unwrap_or(1));
        }
        Some("node") => {
            if verbose {
                eprintln!("running node with WebAssembly...");
            }
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
    let (_project_name, source, mut parser) = read_project()?;
    if verbose {
        eprintln!("parsing {}...", "src/main.uno");
    }
    let _program = parse_program(&source, &mut parser)?;
    println!("check: no errors found");
    let _ = out_dir;
    Ok(())
}

fn cmd_clean(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let target = PathBuf::from("target");
    if target.exists() {
        if verbose {
            eprintln!("removing {}...", target.display());
        }
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
