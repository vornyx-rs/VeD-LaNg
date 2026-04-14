mod ast;
mod compiler;
mod deploy;
mod dev;
mod errors;
mod interpreter;
mod lexer;
mod lsp;
mod optimize;
mod parser;
mod pkg;
mod runtime;
mod stdlib;
mod typeck;

use clap::{Parser, Subcommand, ValueEnum};
use miette::{Context, IntoDiagnostic, Result};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "vedc")]
#[command(about = "VED Language Compiler - Build web, native, and server applications")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a VED file (interpreted mode)
    Run {
        /// VED source file
        file: PathBuf,

        /// Target platform
        #[arg(short, long, value_enum, default_value = "auto")]
        target: Target,

        /// Arguments to pass to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },

    /// Build a VED file
    Build {
        /// VED source file
        file: PathBuf,

        /// Target platform
        #[arg(short, long, value_enum, default_value = "bin")]
        target: Target,

        /// Output directory
        #[arg(short, long, default_value = "dist")]
        out: PathBuf,

        /// Release mode (optimized)
        #[arg(long)]
        release: bool,

        /// Static linking
        #[arg(long)]
        static_link: bool,
    },

    /// Type check a VED file without running
    Check {
        /// VED source file
        file: PathBuf,

        /// Strict mode (all warnings as errors)
        #[arg(long)]
        strict: bool,
    },

    /// Format a VED file
    Fmt {
        /// VED source file
        file: PathBuf,

        /// Check formatting without writing
        #[arg(long)]
        check: bool,

        /// Write output to stdout
        #[arg(long)]
        stdout: bool,
    },

    /// Run tests
    Test {
        /// Test pattern to filter
        #[arg(short, long)]
        pattern: Option<String>,

        /// VED source file
        file: Option<PathBuf>,
    },

    /// Print AST for debugging
    Ast {
        /// VED source file
        file: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value = "debug")]
        format: AstFormat,
    },

    /// Print tokens for debugging
    Lex {
        /// VED source file
        file: PathBuf,

        /// Include whitespace tokens
        #[arg(long)]
        whitespace: bool,
    },

    /// Language server protocol (LSP) mode
    Lsp,

    /// Create a new VED project
    New {
        /// Project name
        name: String,

        /// Project type
        #[arg(short, long, value_enum, default_value = "app")]
        template: Template,

        /// Create in current directory
        #[arg(long)]
        here: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Target {
    /// Auto-detect from source
    Auto,
    /// Web (WASM/WebGPU)
    Web,
    /// Native binary
    Bin,
    /// Server application
    Server,
    /// All targets
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum AstFormat {
    /// Rust debug format
    Debug,
    /// JSON
    Json,
    /// Pretty printed
    Pretty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Template {
    /// Full application (client + server)
    App,
    /// Client-only application
    Client,
    /// Server-only API
    Server,
    /// Library
    Lib,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { file, target, args } => cmd_run(file, target, args, cli.verbose).await,
        Commands::Build {
            file,
            target,
            out,
            release,
            static_link,
        } => cmd_build(file, target, out, release, static_link, cli.verbose).await,
        Commands::Check { file, strict } => cmd_check(file, strict, cli.verbose),
        Commands::Fmt {
            file,
            check,
            stdout,
        } => cmd_fmt(file, check, stdout),
        Commands::Test { pattern, file } => cmd_test(pattern, file),
        Commands::Ast { file, format } => cmd_ast(file, format),
        Commands::Lex { file, whitespace } => cmd_lex(file, whitespace),
        Commands::Lsp => cmd_lsp().await,
        Commands::New {
            name,
            template,
            here,
        } => cmd_new(name, template, here),
    }
}

async fn cmd_run(file: PathBuf, target: Target, _args: Vec<String>, verbose: bool) -> Result<()> {
    let source = fs::read_to_string(&file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", file.display()))?;

    let detected_target = if target == Target::Auto {
        detect_target(&source)
    } else {
        target
    };

    if verbose {
        eprintln!("[vedc] Target: {:?}", detected_target);
        eprintln!("[vedc] Parsing...");
    }

    // Lexing
    let tokens =
        lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

    // Parsing
    let ast = parser::parse(tokens, &source)
        .map_err(|e| miette::miette!("Parse error: {}", e.message))?;

    // Type checking
    if verbose {
        eprintln!("[vedc] Type checking...");
    }
    let _typed = typeck::check(ast).map_err(|e| miette::miette!("Type error: {}", e.message))?;

    // Run based on target
    match detected_target {
        Target::Server => {
            if verbose {
                eprintln!("[vedc] Starting server runtime...");
            }
            runtime::server::start(_typed)
                .await
                .map_err(|e| miette::miette!("Runtime error: {}", e))?;
        }
        Target::Web => {
            // `run` for web: build to dist/ then tell the user how to open it
            let out = std::path::PathBuf::from("dist");
            fs::create_dir_all(&out)
                .into_diagnostic()
                .wrap_err("Failed to create dist/")?;
            compiler::web::emit(_typed, &out, false)
                .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
            println!();
            println!("Built to dist/");
            println!("  Open dist/index.html in a browser, or run a local server:");
            println!("  python3 -m http.server 8080 --directory dist");
            println!();
            println!("To build explicitly:");
            println!("  vedc build {} --target web --out dist", file.display());
        }
        _ => {
            if verbose {
                eprintln!("[vedc] Interpreting...");
            }
            let result = interpreter::run(&_typed)
                .map_err(|e| miette::miette!("Runtime error: {}", e.message))?;

            if verbose {
                eprintln!("[vedc] Result: {:?}", result);
            }

            // Print the result to stdout
            println!("{}", result);
        }
    }

    Ok(())
}

async fn cmd_build(
    file: PathBuf,
    target: Target,
    out: PathBuf,
    release: bool,
    _static_link: bool,
    verbose: bool,
) -> Result<()> {
    let source = fs::read_to_string(&file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", file.display()))?;

    if verbose {
        eprintln!("[vedc] Building {}...", file.display());
        eprintln!("[vedc] Target: {:?}", target);
        eprintln!("[vedc] Output: {}", out.display());
        eprintln!("[vedc] Mode: {}", if release { "release" } else { "debug" });
    }

    // Lexing
    let tokens =
        lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

    // Parsing
    let ast = parser::parse(tokens, &source)
        .map_err(|e| miette::miette!("Parse error: {}", e.message))?;

    // Type checking
    let typed = typeck::check(ast).map_err(|e| miette::miette!("Type error: {}", e.message))?;

    // Create output directory
    fs::create_dir_all(&out)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to create {}", out.display()))?;

    // Compile based on target
    match target {
        Target::Web => {
            if verbose {
                eprintln!("[vedc] Compiling to WASM...");
            }
            compiler::web::emit(typed, &out, release)
                .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
        }
        Target::Bin => {
            if verbose {
                eprintln!("[vedc] Compiling to native binary...");
            }
            compiler::native::emit(typed, &out, release)
                .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
        }
        Target::Server => {
            if verbose {
                eprintln!("[vedc] Compiling server...");
            }
            compiler::server::emit(typed, &out, release)
                .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
        }
        Target::All => {
            if verbose {
                eprintln!("[vedc] Compiling all targets...");
            }
            compiler::fullstack::emit(typed.clone(), &out.join("fullstack"), release)
                .map_err(|e| miette::miette!("Fullstack compile error: {:?}", e))?;
            compiler::native::emit(typed, &out.join("bin"), release)
                .map_err(|e| miette::miette!("Native compile error: {:?}", e))?;
        }
        Target::Auto => {
            let detected = detect_target(&source);
            match detected {
                Target::Server => {
                    compiler::server::emit(typed, &out, release)
                        .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
                }
                Target::Web => {
                    compiler::web::emit(typed, &out, release)
                        .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
                }
                _ => {
                    compiler::native::emit(typed, &out, release)
                        .map_err(|e| miette::miette!("Compile error: {:?}", e))?;
                }
            }
        }
    }

    println!("Built successfully to: {}", out.display());
    Ok(())
}

fn cmd_check(file: PathBuf, strict: bool, verbose: bool) -> Result<()> {
    let source = fs::read_to_string(&file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", file.display()))?;

    if verbose {
        eprintln!("[vedc] Checking {}...", file.display());
    }

    // Lexing
    let tokens =
        lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

    // Parsing
    let ast = parser::parse(tokens, &source)
        .map_err(|e| miette::miette!("Parse error: {}", e.message))?;

    // Type checking
    let _typed = typeck::check(ast).map_err(|e| {
        if strict {
            miette::miette!("Type error (strict): {}", e.message)
        } else {
            miette::miette!("Type error: {}", e.message)
        }
    })?;

    println!("No errors found.");
    Ok(())
}

fn cmd_fmt(file: PathBuf, check: bool, stdout: bool) -> Result<()> {
    let source = fs::read_to_string(&file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", file.display()))?;

    // Tokenize to check for parse errors
    let _tokens =
        lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

    // For now, just echo back the source
    // Full formatter would be implemented separately
    if check {
        println!("Would format: {}", file.display());
    } else if stdout {
        print!("{}", source);
    } else {
        println!("Formatting not yet fully implemented");
    }

    Ok(())
}

fn cmd_test(pattern: Option<String>, file: Option<PathBuf>) -> Result<()> {
    println!("Testing VED code...");

    if let Some(f) = file {
        let source = fs::read_to_string(&f)
            .into_diagnostic()
            .wrap_err_with(|| format!("Failed to read {}", f.display()))?;

        // Parse and run tests
        let tokens =
            lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

        let ast = parser::parse(tokens, &source)
            .map_err(|e| miette::miette!("Parse error: {}", e.message))?;

        let _typed =
            typeck::check(ast).map_err(|e| miette::miette!("Type error: {}", e.message))?;

        println!("Tests would run here (pattern: {:?})", pattern);
    } else {
        println!("No test file specified");
    }

    Ok(())
}

fn cmd_ast(file: PathBuf, format: AstFormat) -> Result<()> {
    let source = fs::read_to_string(&file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", file.display()))?;

    let tokens =
        lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

    let ast = parser::parse(tokens, &source)
        .map_err(|e| miette::miette!("Parse error: {}", e.message))?;

    match format {
        AstFormat::Debug => {
            println!("{:#?}", ast);
        }
        AstFormat::Json => {
            // Would need Serialize derive
            println!("JSON output not yet implemented");
        }
        AstFormat::Pretty => {
            println!("{:#?}", ast);
        }
    }

    Ok(())
}

fn cmd_lex(file: PathBuf, whitespace: bool) -> Result<()> {
    let source = fs::read_to_string(&file)
        .into_diagnostic()
        .wrap_err_with(|| format!("Failed to read {}", file.display()))?;

    let tokens =
        lexer::tokenize(&source).map_err(|e| miette::miette!("Lex error: {}", e.message))?;

    for token in tokens {
        if !whitespace && matches!(token.token, lexer::Token::Newline | lexer::Token::Comment) {
            continue;
        }
        println!(
            "{:4}:{:4}  {:20}  {:?}",
            token.line,
            token.column,
            format!("{}", token.token),
            token.text
        );
    }

    Ok(())
}

async fn cmd_lsp() -> Result<()> {
    lsp::start().await
}

fn cmd_new(name: String, template: Template, here: bool) -> Result<()> {
    let project_dir = if here {
        PathBuf::from(".")
    } else {
        PathBuf::from(&name)
    };

    if !here && project_dir.exists() {
        return Err(miette::miette!("Directory '{}' already exists", name));
    }

    fs::create_dir_all(&project_dir).into_diagnostic()?;

    // Create project structure based on template
    match template {
        Template::App => {
            fs::create_dir_all(project_dir.join("src")).into_diagnostic()?;

            // Main.ved
            let main_ved = r#"-- Main application entry point

screen Main
  remember count = 0

  box main
    fill: #0a0a0a
    center: both
    gap: 20

    words "Count: {count}"
      size: 48
      color: #7f6fe8

    tap "Increment"
      fill: #7f6fe8
      radius: 8
      =>
        count = count + 1

think main
  println "Starting VED application..."
"#;
            fs::write(project_dir.join("src/Main.ved"), main_ved).into_diagnostic()?;

            // ved.toml
            let ved_toml = format!(
                r#"[project]
name = "{}"
version = "0.1.0"
edition = "2024"

[build]
target = "web"

[web]
port = 8080
title = "VED App"

[server]
port = 3000
"#,
                name
            );
            fs::write(project_dir.join("ved.toml"), ved_toml).into_diagnostic()?;
        }
        Template::Client => {
            fs::create_dir_all(project_dir.join("src")).into_diagnostic()?;
            fs::write(
                project_dir.join("src/Main.ved"),
                "screen Main\n  words \"Hello World\"\n",
            )
            .into_diagnostic()?;
        }
        Template::Server => {
            fs::create_dir_all(project_dir.join("src")).into_diagnostic()?;

            let server_ved = r#"serve Api
  port: 8080

  GET "/" => health

think health
  give { status: "ok", version: "0.1.0" }
"#;
            fs::write(project_dir.join("src/Main.ved"), server_ved).into_diagnostic()?;
        }
        Template::Lib => {
            fs::create_dir_all(project_dir.join("src")).into_diagnostic()?;
            fs::write(project_dir.join("src/Lib.ved"), "-- Library module\n").into_diagnostic()?;
        }
    }

    // README.md
    let readme = format!(
        r#"# {}

A VED language project.

## Building

```bash
vedc build src/Main.ved
```

## Running

```bash
vedc run src/Main.ved
```

## Project Structure

- `src/` - Source files
- `dist/` - Build output
"#,
        name
    );
    fs::write(project_dir.join("README.md"), readme).into_diagnostic()?;

    if here {
        println!("Created VED project in current directory");
    } else {
        println!("Created VED project '{}'", name);
    }
    println!("  cd {}", if here { "." } else { &name });
    println!("  vedc run src/Main.ved");

    Ok(())
}

/// Detect target platform from source code
fn detect_target(source: &str) -> Target {
    // Check for server-side keywords
    if source.contains("\nserve ")
        || source.contains("\ndatabase ")
        || source.contains("\ntask ")
        || source.contains("\nGET ")
        || source.contains("\nPOST ")
    {
        return Target::Server;
    }

    // Check for client-side keywords
    if source.contains("\nscreen ")
        || source.contains("\npiece ")
        || source.contains("\nbox ")
        || source.contains("\nwords ")
        || source.contains("\ntap ")
        || source.contains("\nremember ")
    {
        return Target::Web;
    }

    // Default to native binary
    Target::Bin
}
