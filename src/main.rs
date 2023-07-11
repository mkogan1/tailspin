mod color;
mod config_parser;
mod config_util;
mod highlight_processor;
mod highlight_utils;
mod highlighters;
mod less;
mod line_info;
mod tail;

use clap::Parser;
use rand::random;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::process::exit;
use tokio::sync::oneshot;

#[derive(Parser)]
struct Args {
    /// Filepath
    #[clap(name = "FILE")]
    file_path: Option<String>,

    /// Follow (tail) the contents of the file
    #[clap(short = 'f', long = "follow")]
    follow: bool,

    /// Provide a custom path configuration file
    #[clap(long = "config")]
    config_path: Option<String>,

    /// Subcommand
    #[clap(subcommand)]
    command: Option<SubCommand>,
}

#[derive(Parser)]
enum SubCommand {
    /// Generate a new configuration file
    GenerateConfig,
}

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();

    // if a subcommand is specified, run it
    if let Some(command) = args.command {
        match command {
            SubCommand::GenerateConfig => {
                config_parser::generate_default_config();

                exit(0);
            }
        }
    }

    let file_path = match args.file_path {
        Some(path) => path,
        None => {
            println!("Missing filename (`spin --help` for help) ");

            exit(0);
        }
    };

    let config_path = args.config_path.clone();
    let config = config_parser::load_config(config_path);

    let highlighter = highlighters::Highlighters::new(config);
    let highlight_processor = highlight_processor::HighlightProcessor::new(highlighter);

    let (_temp_dir, output_path, output_writer) = create_temp_file();
    let (reached_eof_tx, reached_eof_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        tail::tail_file(
            &file_path,
            args.follow,
            output_writer,
            highlight_processor,
            Some(reached_eof_tx),
        )
        .await
        .expect("Failed to tail file");
    });

    reached_eof_rx
        .await
        .expect("Could not receive EOF signal from oneshot channel");

    less::open_file_with_less(output_path.to_str().unwrap(), args.follow);

    cleanup(output_path);
}

fn create_temp_file() -> (tempfile::TempDir, PathBuf, BufWriter<File>) {
    let unique_id: u32 = random();
    let filename = format!("tailspin.temp.{}", unique_id);

    let temp_dir = tempfile::tempdir().unwrap();

    let output_path = temp_dir.path().join(filename);
    let output_file = File::create(&output_path).unwrap();
    let output_writer = BufWriter::new(output_file);

    (temp_dir, output_path, output_writer)
}

fn cleanup(output_path: PathBuf) {
    if let Err(err) = std::fs::remove_file(output_path) {
        eprintln!("Failed to remove the temporary file: {}", err);
    }
}