use anyhow::{Context, Result};
use clap::Parser;
use ignore::{WalkBuilder, overrides::OverrideBuilder};
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// A utility to provide context for Rust projects after running tests
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to scan (defaults to current working directory)
    #[arg(short, long)]
    dir: Option<PathBuf>,

    /// Include dependency files referenced in errors
    #[arg(short, long)]
    include_deps: bool,

    /// Command to run (e.g., "cargo test" or "cargo test -- --nocapture")
    #[arg(long, default_value = "cargo test")]
    command: String,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Determine the directory to work in
    let cwd = args.dir.unwrap_or_else(|| std::env::current_dir().unwrap());

    // Split the command into parts for execution
    let mut command_parts = args.command.split_whitespace();
    let program = command_parts.next().context("No command provided")?;
    let command_args: Vec<&str> = command_parts.collect();

    // Run the command and capture output
    let mut child = Command::new(program)
        .args(&command_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(format!("Failed to execute command: {}", args.command))?;

    // Set up readers for stdout and stderr
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let mut stdout_reader = BufReader::new(stdout);
    let mut stderr_reader = BufReader::new(stderr);
    let mut line_buffer = String::new();
    let mut full_output = String::new(); // Collect all output here

    // Passthrough stdout and collect output
    let mut stdout_handle = io::stdout();
    loop {
        line_buffer.clear();
        match stdout_reader.read_line(&mut line_buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                stdout_handle.write_all(line_buffer.as_bytes())?;
                stdout_handle.flush()?;
                full_output.push_str(&line_buffer);
            }
            Err(e) => eprintln!("Error reading stdout: {}", e),
        }
    }

    // Passthrough stderr and collect output
    let mut stderr_handle = io::stderr();
    loop {
        line_buffer.clear();
        match stderr_reader.read_line(&mut line_buffer) {
            Ok(0) => break, // EOF
            Ok(_) => {
                stderr_handle.write_all(line_buffer.as_bytes())?;
                stderr_handle.flush()?;
                full_output.push_str(&line_buffer);
            }
            Err(e) => eprintln!("Error reading stderr: {}", e),
        }
    }

    // Wait for the command to finish
    let status = child.wait()?;
    if !status.success() {
        eprintln!("Command '{}' failed with status: {}", args.command, status);
    }

    // Print project context
    println!("\n=== Project Context ===\n");
    print_project_files(&cwd)?;

    // Optionally print dependency files
    if args.include_deps {
        print_relevant_dependency_files(&full_output)?;
    }

    Ok(())
}

// Print all files in the project, respecting .gitignore and .contreeignore
fn print_project_files(cwd: &PathBuf) -> Result<()> {
    let mut builder = WalkBuilder::new(cwd);
    builder
        .standard_filters(true) // Respect .gitignore, .git, etc.
        .add_custom_ignore_filename(".contreeignore"); // Add .contreeignore

    // Build the walker
    let walker = builder.build();

    for entry in walker {
        let entry = entry.context("Failed to read directory entry")?;
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            let path = entry.path();
            println!("File: {}", path.display());
            println!("```");
            let contents = fs::read_to_string(path)
                .context(format!("Failed to read file: {}", path.display()))?;
            println!("{}", contents);
            println!("```");
            println!();
        }
    }
    Ok(())
}

// Extract and print dependency files mentioned in errors
fn print_relevant_dependency_files(test_output: &str) -> Result<()> {
    let re = Regex::new(r"--> ([/\\].*?\.rs):(\d+):(\d+)")?;
    let mut seen_files = HashSet::new();

    for cap in re.captures_iter(test_output) {
        let file_path = cap.get(1).unwrap().as_str();
        if file_path.contains(".cargo/registry") && seen_files.insert(file_path.to_string()) {
            if let Ok(contents) = fs::read_to_string(file_path) {
                println!("Dependency File: {}", file_path);
                println!("```");
                println!("{}", contents);
                println!("```");
                println!();
            } else {
                eprintln!("Warning: Could not read dependency file: {}", file_path);
            }
        }
    }
    Ok(())
}