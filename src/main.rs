use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
    Build,
    /// Build and run the current project
    Run,
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
        Subcmd::Build => cmd_build(false),
        Subcmd::Run => cmd_build(true),
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

fn cmd_build(run_after: bool) {
    let manifest_str = fs::read_to_string("uno.toml")
        .unwrap_or_else(|_| {
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

    let mut parser = uno_parser::Parser::new(tokens);
    let program = parser.parse_program().unwrap_or_else(|err| {
        eprintln!("error: {err}");
        std::process::exit(1);
    });

    let c_code = uno_codegen::Codegen::generate(&program);

    let target_dir = PathBuf::from("target");
    fs::create_dir_all(&target_dir).expect("failed to create target/ directory");

    let c_path = target_dir.join("output.c");
    fs::write(&c_path, c_code).expect("failed to write output.c");

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
