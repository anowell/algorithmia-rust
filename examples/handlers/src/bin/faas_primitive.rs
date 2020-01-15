use algorithmia::prelude::*;
use std::error::Error;

fn apply(input: u32) -> Result<u32, Box<dyn Error>> {
    Ok(input + 42)
}

fn main() {
    handler::run(apply)
}
