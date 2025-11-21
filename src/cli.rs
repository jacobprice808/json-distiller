// src/cli.rs

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Distills large JSON files by summarizing repetitive list structures.", long_about = None)]
pub struct CliArgs {
    #[arg(index = 1)]
    pub input_file_pos: Option<PathBuf>,

    #[arg(short, long = "input", value_name = "FILE", conflicts_with = "input_file_pos")]
    pub input_file_flag: Option<PathBuf>,

    #[arg(short, long, value_name = "FILE")]
    pub output_file: Option<PathBuf>,

    /// Enable strict type checking (int vs float are different structures).
    /// When true: treats integers and floats as distinct structure types.
    /// When false: treats all numbers as the same type.
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub strict_typing: bool,

    /// Position-dependent mode: controls how examples are shown across nesting levels.
    /// When true: shows examples independently at each depth (predictable, depth-aware).
    /// When false: shows examples only at shallowest occurrence (more concise, globally unique).
    #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
    pub position_dependent: bool,

    /// Minimum repeat count for pattern summarization (internal, affects formatting).
    /// Controls how patterns are displayed in summaries. Value >=2 recommended.
    #[arg(short, long, value_name = "N", default_value_t = 1)]
    pub repeat_threshold: usize,

    #[arg(long = "mcp-server",
          conflicts_with_all = ["input_file_pos", "input_file_flag", "output_file"]
    )]
    pub mcp_mode: bool,

    #[arg(last = true, hide = true)]
    pub mcp_args: Vec<String>,
}

impl CliArgs {
    pub fn get_input_path(&self) -> Result<PathBuf, &'static str> {
        match (&self.input_file_pos, &self.input_file_flag) {
            (Some(pos), None) => Ok(pos.clone()),
            (None, Some(flag)) => Ok(flag.clone()),
            (None, None) => Err("Input file path is required for CLI mode."),
            (Some(_), Some(_)) => Err("Specify input file either positionally or with -i, not both."),
        }
    }
}