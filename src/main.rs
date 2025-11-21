// src/main.rs

mod cli;
mod core;
mod error;
mod mcp_server;

use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::CliArgs;
use error::DistillError;
use path_absolutize::Absolutize;
use std::fs;

fn main() -> Result<()> {
    let args = CliArgs::parse();

    if args.mcp_mode {
        // Only initialize tracing and tokio for MCP mode
        run_mcp_mode()
    } else {
        // CLI mode: pure synchronous execution, no overhead
        run_cli(&args)
    }
}

#[tokio::main]
async fn run_mcp_mode() -> Result<()> {
    // Initialize tracing only for MCP server mode where we need it
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("Running in MCP Server mode...");
    mcp_server::start_mcp().await.context("MCP Server failed")
}

fn run_cli(args: &CliArgs) -> Result<()> {
    println!("Starting JSON Distiller CLI...");

    let initial_input_path = args.get_input_path()
        .map_err(|e| DistillError::InvalidInput(e.to_string()))
        .context("Input file is required when not running in --mcp-server mode")?;

    let input_path = initial_input_path
        .absolutize()
        .context("Failed to make input path absolute")?;
    let input_path_ref = input_path.as_ref();

     if !input_path_ref.exists() {
         bail!(DistillError::Io(std::io::Error::new(
             std::io::ErrorKind::NotFound,
             format!("Input file not found at '{}'", input_path_ref.display()),
         )));
     }
     if !input_path_ref.is_file() {
         bail!(DistillError::InvalidInput(format!(
            "Input path is not a file: '{}'",
            input_path_ref.display()
        )));
     }

    let output_path = match &args.output_file {
        Some(path) => path.clone(),
        None => {
            let input_filename = input_path_ref
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            let default_filename = format!("{}_distilled.json", input_filename);
            std::env::current_dir()
                 .context("Failed to get current directory")?
                 .join(default_filename)
        }
    };

     let output_path_abs = output_path
         .absolutize()
         .context("Failed to make output path absolute")?;
     let output_path_ref = output_path_abs.as_ref();


    println!("Input File: {}", input_path_ref.display());
    println!("Output File: {}", output_path_ref.display());
    println!("Strict Typing: {}", args.strict_typing);
    println!("Repeat Threshold: {}", args.repeat_threshold);

    // Read and parse JSON
    let input_content = fs::read_to_string(input_path_ref)
        .with_context(|| format!("Failed to read input file: {}", input_path_ref.display()))?;

    let input_json: serde_json::Value = serde_json::from_str(&input_content)
        .with_context(|| format!("Failed to parse JSON from file: {}", input_path_ref.display()))?;

    println!("Distilling JSON...");
    let distilled_json = core::distill_json(input_json, args.strict_typing, args.repeat_threshold, args.position_dependent)
        .context("Distillation process failed")?;
    println!("Distillation complete.");

    if let Some(parent_dir) = output_path_ref.parent() {
        fs::create_dir_all(parent_dir)
            .with_context(|| format!("Failed to create output directory: {}", parent_dir.display()))?;
    }

    let output_content = serde_json::to_string_pretty(&distilled_json)
        .context("Failed to serialize distilled JSON")?;

    fs::write(output_path_ref, output_content)
         .with_context(|| format!("Failed to write output file: {}", output_path_ref.display()))?;

    println!(
        "Successfully processed and saved distilled JSON to: {}",
        output_path_ref.display()
    );

    Ok(())
}