//! Small non-GUI runner for git-knife's rewrite engine.
//!
//! The desktop application remains the supported user interface. This binary
//! is only used for a controlled, repeatable batch operation: it accepts one
//! JSON request on stdin and delegates the actual history rebuild to the same
//! `rewrite::apply_edits` implementation used by the app.

use std::io::{self, Read};

use git_knife::rewrite::{ApplyResult, CommitEdit};
use serde::Deserialize;

#[derive(Deserialize)]
struct Request {
    path: String,
    branch: String,
    edits: Vec<CommitEdit>,
    #[serde(default)]
    sign: bool,
    #[serde(default)]
    resign: bool,
}

fn main() {
    let mut input = String::new();
    if let Err(err) = io::stdin().read_to_string(&mut input) {
        eprintln!("failed to read request: {err}");
        std::process::exit(2);
    }

    let request: Request = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("invalid request JSON: {err}");
            std::process::exit(2);
        }
    };

    match git_knife::rewrite::apply_edits(
        &request.path,
        &request.branch,
        &request.edits,
        request.sign,
        request.resign,
    ) {
        Ok(result) => print_json(&result),
        Err(err) => {
            eprintln!("rewrite failed: {err}");
            std::process::exit(1);
        }
    }
}

fn print_json(result: &ApplyResult) {
    match serde_json::to_string(result) {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("failed to encode result: {err}");
            std::process::exit(2);
        }
    }
}
