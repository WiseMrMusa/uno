use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uno_codegen::c::Codegen;
use uno_codegen::wasm::WasmCodegen;
use uno_syntax::ast::Program;

#[derive(Parser)]
#[command(name = "uno", version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    cmd: Subcmd,
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
    },
    /// Build and run the current project
    Run {
        /// Target backend (c, wasm)
        #[arg(long, default_value = "c")]
        target: Target,
    },
}

#[derive(Clone, ValueEnum)]
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
    match cli.cmd {
        Subcmd::New { name } => cmd_new(&name),
        Subcmd::Build { target } => cmd_build(false, &target),
        Subcmd::Run { target } => cmd_build(true, &target),
    }
}

fn cmd_new(name: &str) {
    let dir = PathBuf::from(name);
    if dir.exists() {
        eprintln!("error: directory '{name}' already exists");
        std::process::exit(1);
    }

    fs::create_dir_all(dir.join("src")).expect("failed to create src directory");

    let manifest = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"
"#
    );
    fs::write(dir.join("uno.toml"), manifest).expect("failed to write uno.toml");

    let main_source = "fn main() -> u32 {\n    return 0;\n}\n";
    fs::write(dir.join("src").join("main.uno"), main_source)
        .expect("failed to write src/main.uno");

    println!("Created project '{name}'");
}

fn read_project() -> (String, uno_parser::Parser) {
    let manifest_str = fs::read_to_string("uno.toml").unwrap_or_else(|_| {
        eprintln!("error: uno.toml not found (are you in an Uno project?)");
        std::process::exit(1);
    });
    let manifest: Manifest = toml::from_str(&manifest_str).unwrap_or_else(|e| {
        eprintln!("error: failed to parse uno.toml: {e}");
        std::process::exit(1);
    });

    let project_name = manifest.package.name;

    let source = fs::read_to_string("src/main.uno").unwrap_or_else(|_| {
        eprintln!("error: src/main.uno not found");
        std::process::exit(1);
    });

    let mut lexer = uno_lexer::Lexer::new(source);
    let tokens = lexer.tokenize();

    let parser = uno_parser::Parser::new(tokens);
    (project_name, parser)
}

fn cmd_build(run_after: bool, target: &Target) {
    let (project_name, mut parser) = read_project();

    let program = parser.parse_program().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    let target_dir = PathBuf::from("target");
    fs::create_dir_all(&target_dir).expect("failed to create target/ directory");

    match target {
        Target::C => build_c(&project_name, &target_dir, &program, run_after),
        Target::Wasm => build_wasm(&project_name, &target_dir, &program, run_after),
    }
}

fn build_c(project_name: &str, target_dir: &PathBuf, program: &Program, run_after: bool) {
    let c_code = Codegen::generate(program);

    let c_path = target_dir.join("output.c");
    fs::write(&c_path, c_code).expect("failed to write output.c");

    let binary_name = if cfg!(target_os = "windows") {
        format!("{project_name}.exe")
    } else {
        project_name.to_string()
    };
    let binary_path = target_dir.join(&binary_name);

    let status = Command::new("cc")
        .arg("-std=c23")
        .arg("-o")
        .arg(&binary_path)
        .arg(&c_path)
        .status()
        .unwrap_or_else(|_| {
            eprintln!("error: failed to run 'cc' (is a C compiler installed?)");
            std::process::exit(1);
        });

    if !status.success() {
        eprintln!("error: C compilation failed");
        std::process::exit(1);
    }

    println!("Compiled '{project_name}'");

    if run_after {
        let status = Command::new(&binary_path).status().unwrap_or_else(|_| {
            eprintln!("error: failed to run binary");
            std::process::exit(1);
        });
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn build_wasm(project_name: &str, target_dir: &PathBuf, program: &Program, run_after: bool) {
    let wat = WasmCodegen::generate(program);

    let wat_path = target_dir.join("output.wat");
    fs::write(&wat_path, wat).expect("failed to write output.wat");

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
            println!("Compiled '{project_name}' to WASM");

            if run_after {
                let runtime = which_runtime();
                match runtime {
                    Some("wasmtime") => {
                        let status = Command::new("wasmtime")
                            .arg(&wasm_path)
                            .status()
                            .unwrap_or_else(|_| {
                                eprintln!("error: failed to run wasmtime");
                                std::process::exit(1);
                            });
                        std::process::exit(status.code().unwrap_or(1));
                    }
                    Some("node") => {
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
                        fs::write(&runner, js).expect("failed to write runner script");
                        let status = Command::new("node")
                            .arg(&runner)
                            .status()
                            .unwrap_or_else(|_| {
                                eprintln!("error: failed to run node");
                                std::process::exit(1);
                            });
                        std::process::exit(status.code().unwrap_or(1));
                    }
                    _ => {
                        eprintln!("note: install wasmtime or node to run WASM");
                    }
                }
            }
        }
        Ok(_) => {
            eprintln!("error: WASM compilation failed");
            eprintln!("note: install wabt (wat2wasm) to compile .wat files");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("wrote {} — install wabt (wat2wasm) or use npx to compile", wat_path.display());
        }
    }
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
