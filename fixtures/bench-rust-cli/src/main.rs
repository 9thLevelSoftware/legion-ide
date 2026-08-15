//! wordtally: count lines, words, and characters in text files.
//!
//! Usage: wordtally [--lines] [--words] [--chars] [--top N] FILE...
//! With no display flags, all three counters are printed.

mod cli;
mod report;
mod stats;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let opts = match cli::parse(&args) {
        Ok(opts) => opts,
        Err(msg) => {
            eprintln!("wordtally: {msg}");
            eprintln!("{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };

    if opts.files.is_empty() {
        eprintln!("wordtally: no input files");
        eprintln!("{}", cli::USAGE);
        return ExitCode::from(2);
    }

    let mut input = String::new();
    for path in &opts.files {
        match std::fs::read_to_string(path) {
            Ok(text) => input.push_str(&text),
            Err(err) => {
                eprintln!("wordtally: {path}: {err}");
                return ExitCode::from(1);
            }
        }
    }

    let summary = stats::summarize(&input);
    print!("{}", report::render(&summary, &opts));
    ExitCode::SUCCESS
}
