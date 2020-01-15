use algorithmia::prelude::*;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Deserialize)]
struct Input {
    name: String,
}

#[derive(Serialize)]
struct Output {
    msg: String,
}

#[derive(Debug)]
struct CustomError {}
impl Error for CustomError {}
impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Custom Error")
    }
}

fn apply(input: Input) -> Result<Output, CustomError> {
    Ok(Output {
        msg: format!("Hello {}", input.name),
    })
}

fn main() {
    handler::run(apply)
}
