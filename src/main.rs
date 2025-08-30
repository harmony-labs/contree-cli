use anyhow::{Context, Result};
use clap::Parser;
use ignore::WalkBuilder;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
use atty::Stream;

/// A utility to provide context for projects after running commands
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to scan (defaults to current working directory)
    #[arg(short, long)]
    dir: Option<PathBuf>,

    /// Include dependency files referenced in errors (Rust projects only)
    #[arg(short = 'D', long)]
    include_deps: bool,

    /// Output file for project and dependency files
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,

    /// Grep pattern to filter files (e.g., 'transaction' or '/regex/')
    #[arg(short = 'g', long)]
    grep: Option<String>,

    /// List of files to include (comma-separated), even if they don't match grep or are outside the directory
    #[arg(short = 'i', long, value_delimiter = ',', value_parser = parse_pathbuf)]
    include: Option<Vec<PathBuf>>,

    /// Maximum depth of directories to scan (relative to dir), unlimited if not specified
    #[arg(long)]
    max_depth: Option<usize>,
}

fn parse_pathbuf(s: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(s.trim()))
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Determine the directory to work in
    let cwd = args.dir.unwrap_or_else(|| std::env::current_dir().unwrap());

    let mut full_output = String::new();
    let mut line_buffer = String::new();

    // Open the output file early if specified, default to stdout
    let mut output_writer: Box<dyn Write> = if let Some(output_path) = &args.output {
        Box::new(std::fs::File::create(output_path).context("Failed to create output file")?)
    } else {
        Box::new(std::io::stdout())
    };

    if !atty::is(Stream::Stdin) {
        // Input piped: read from stdin and passthrough to console
        let mut stdin_reader = BufReader::new(io::stdin());
        let mut stdout_handle = io::stdout();

        loop {
            line_buffer.clear();
            match stdin_reader.read_line(&mut line_buffer) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    print!("{}", line_buffer);
                    stdout_handle.flush()?;
                    full_output.push_str(&line_buffer);
                }
                Err(e) => eprintln!("Error reading stdin: {}", e),
            }
        }
    }

    // Write project context to the output file (or stdout if no file specified)
    writeln!(output_writer, "\n=== Project Context ===\n")?;

    // Print files from the scanned directory with grep filtering
    print_project_files(&cwd, &args.grep, &args.include, &args.max_depth, &mut output_writer)?;

    // Include dependencies if requested
    if args.include_deps {
        print_relevant_dependency_files(&full_output, &cwd, &mut output_writer)?;
    }

    output_writer.flush()?;
    Ok(())
}

// Check if the current directory or a parent contains a Cargo.toml
fn is_rust_project(cwd: &Path) -> bool {
    let mut current = Some(cwd);
    while let Some(dir) = current {
        if dir.join("Cargo.toml").exists() {
            return true;
        }
        current = dir.parent();
    }
    false
}

// Print a single file's contents to the writer
fn print_file(path: &Path, writer: &mut Box<dyn Write>) -> Result<()> {
    writeln!(writer, "File: {}", path.display())?;
    writeln!(writer, "```")?;
    match fs::read_to_string(path) {
        Ok(contents) => writeln!(writer, "{}", contents)?,
        Err(e) if e.to_string().contains("stream did not contain valid UTF-8") => {
            writeln!(writer, "[binary file]")?;
        }
        Err(e) => return Err(anyhow::Error::from(e).context(format!("Failed to read file: {}", path.display()))),
    }
    writeln!(writer, "```")?;
    writeln!(writer)?;
    Ok(())
}

// Print all files in the project, respecting .gitignore, .contreeignore, grep filter, include list, and max depth
fn print_project_files(
    cwd: &PathBuf,
    grep_pattern: &Option<String>,
    include_files: &Option<Vec<PathBuf>>,
    max_depth: &Option<usize>,
    writer: &mut Box<dyn Write>,
) -> Result<()> {
    // Compile the grep pattern into a regex if provided
    let grep_regex = grep_pattern.as_ref().map(|pattern| {
        let trimmed = pattern.trim();
        if trimmed.starts_with('/') && trimmed.ends_with('/') {
            Regex::new(&trimmed[1..trimmed.len() - 1])
                .context("Invalid regex pattern")
        } else {
            Regex::new(&format!("(?i){}", regex::escape(trimmed)))
                .context("Invalid grep pattern")
        }
    }).transpose()?;

    // Build the walker for the directory
    let mut builder = WalkBuilder::new(cwd);
    builder.standard_filters(true); // Respect .gitignore, etc.
    builder.hidden(false); // Skip hidden files/directories like .git by default
    builder.git_ignore(true); // Respect .gitignore
    builder.git_exclude(false);
    builder.add_custom_ignore_filename(".contreeignore");
    builder.add_ignore(".git"); // Explicitly ignore .git directories
    
    // Set max depth if specified
    if let Some(depth) = max_depth {
        builder.max_depth(Some(*depth));
    }

    // Add a custom filter to explicitly exclude .git directories at any depth
    builder.filter_entry(|entry| {
        !entry
            .path()
            .components()
            .any(|comp| comp.as_os_str() == ".git")
    });

    let walker = builder.build();
    for entry in walker {
        let entry = entry.context("Failed to read directory entry")?;
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            let path = entry.path();

            // Apply grep filter if provided
            if let Some(ref regex) = grep_regex {
                let contents = match fs::read_to_string(path) {
                    Ok(contents) => contents,
                    Err(e) if e.to_string().contains("stream did not contain valid UTF-8") => continue, // Skip binary files
                    Err(e) => return Err(anyhow::Error::from(e).context(format!("Failed to read file: {}", path.display()))),
                };
                if !regex.is_match(&contents) {
                    continue; // Skip files that don't match the grep pattern
                }
            }

            print_file(path, writer)?;
        }
    }

    // Process explicitly included files
    if let Some(ref include_files) = include_files {
        for path in include_files {
            // Skip if the file doesn't exist or isn't a file
            if !path.exists() || !path.is_file() {
                eprintln!("Warning: Included path {} does not exist or is not a file", path.display());
                continue;
            }

            // Print the file regardless of grep filter or directory
            print_file(path, writer)?;
        }
    }

    Ok(())
}

// Extract and print dependency files mentioned in errors (for Rust projects only)
fn print_relevant_dependency_files(
    test_output: &str,
    cwd: &PathBuf,
    writer: &mut Box<dyn Write>,
) -> Result<()> {
    let mut relevant_files: HashMap<String, HashSet<String>> = HashMap::new();

    // Handle direct file references in test output
    let re_direct = Regex::new(r"--> ([/\\].*?\.rs):(\d+):(\d+)")?;
    for cap in re_direct.captures_iter(test_output) {
        let file_path = cap.get(1).unwrap().as_str().to_string();
        if file_path.contains(".cargo/registry") {
            relevant_files
                .entry(file_path.clone())
                .or_insert_with(HashSet::new)
                .insert("directly referenced".to_string());
        }
    }

    // Extract type and macro names from test output
    let re_type_e0599 = Regex::new(r"method not found in `([^`]+)`")?;
    let re_macro_e0308 = Regex::new(r"this error originates in the macro `([^`]+)`")?;
    let re_types_e0308 = Regex::new(r"expected `([^`]+)`, found `([^`]+)`")?;

    let mut types = HashSet::new();
    let mut macros = HashSet::new();

    for cap in re_type_e0599.captures_iter(test_output) {
        let type_name = cap.get(1).unwrap().as_str().split('<').next().unwrap();
        types.insert(type_name.to_string());
    }
    for cap in re_macro_e0308.captures_iter(test_output) {
        macros.insert(cap.get(1).unwrap().as_str().to_string());
    }
    for cap in re_types_e0308.captures_iter(test_output) {
        let expected = cap.get(1).unwrap().as_str().split('<').next().unwrap();
        let found = cap.get(2).unwrap().as_str().split('<').next().unwrap();
        types.insert(expected.to_string());
        types.insert(found.to_string());
    }

    // Only process Rust dependencies if in a Rust project
    if !is_rust_project(cwd) {
        return Ok(()); // Skip dependency processing in non-Rust projects
    }

    // Get the used crate versions dynamically (only if in a Rust project)
    let used_crate_versions = get_used_crate_versions(cwd)?;

    // Determine the registry path dynamically
    let cargo_home = env::var("CARGO_HOME").unwrap_or_else(|_| {
        let home = env::var("HOME").expect("HOME environment variable not set");
        format!("{}/.cargo", home)
    });
    let registry_path = PathBuf::from(cargo_home).join("registry").join("src");

    // Search only within directories matching used crate versions
    for entry in WalkDir::new(registry_path)
        .max_depth(2)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_dir() {
            let dir_name = entry.file_name().to_str().unwrap();
            if used_crate_versions.iter().any(|s| s == dir_name) {
                for sub_entry in WalkDir::new(entry.path())
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = sub_entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                        let file_name = path.file_name().unwrap().to_str().unwrap().to_lowercase();
                        let content = fs::read_to_string(path).unwrap_or_default().to_lowercase();

                        // Check for types
                        for type_name in &types {
                            let type_name_lower = type_name.to_lowercase();
                            if file_name.contains(&type_name_lower) || content.contains(&type_name_lower) {
                                let path_str = path.to_str().unwrap().to_string();
                                relevant_files
                                    .entry(path_str.clone())
                                    .or_insert_with(HashSet::new)
                                    .insert(format!("type {}", type_name));
                            }
                        }

                        // Check for macros
                        for macro_name in macros.iter() {
                            if content.contains(&format!("macro_rules! {}", macro_name)) {
                                let path_str = path.to_str().unwrap().to_string();
                                relevant_files
                                    .entry(path_str.clone())
                                    .or_insert_with(HashSet::new)
                                    .insert(format!("macro {}", macro_name));
                            }
                        }
                    }
                }
            }
        }
    }

    // Print the relevant files with their contents
    if !relevant_files.is_empty() {
        writeln!(writer, "\n=== Relevant Dependency Files ===\n")?;
        for (file_path, reasons) in relevant_files {
            writeln!(writer, "File: {}", file_path)?;
            writeln!(writer, "  - {}", reasons.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n  - "))?;
            writeln!(writer, "```")?;
            match fs::read_to_string(&file_path) {
                Ok(contents) => writeln!(writer, "{}", contents)?,
                Err(e) => writeln!(writer, "(Failed to read file: {})", e)?,
            }
            writeln!(writer, "```")?;
            writeln!(writer)?;
        }
    }

    Ok(())
}

// Function to get used crate versions dynamically (for Rust projects)
fn get_used_crate_versions(cwd: &PathBuf) -> Result<Vec<String>> {
    // Run `cargo tree` in the project directory
    let output = Command::new("cargo")
        .arg("tree")
        .current_dir(cwd)
        .output()
        .context("Failed to run cargo tree")?;

    // Convert output to a UTF-8 string
    let output_str = String::from_utf8(output.stdout)
        .context("cargo tree output is not UTF-8")?;

    // Regex to match lines like "├── crate_name vX.Y.Z" or "└── crate_name vX.Y.Z"
    let re = Regex::new(r"^\s*[├└]── (.+) v(\d+\.\d+\.\d+)")
        .context("Failed to compile regex")?;

    // Collect unique crate-version pairs
    let mut crate_versions = HashSet::new();
    for line in output_str.lines() {
        if let Some(cap) = re.captures(line) {
            let crate_name = cap.get(1).unwrap().as_str();
            let version = cap.get(2).unwrap().as_str();
            crate_versions.insert(format!("{}-{}", crate_name, version));
        }
    }

    Ok(crate_versions.into_iter().collect())
}