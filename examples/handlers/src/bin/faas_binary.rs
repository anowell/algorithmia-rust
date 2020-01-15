use algorithmia::algo::ByteVec;
use algorithmia::prelude::*;
use std::error::Error;

// Note: `Vec<u8>` serializes/deserializes as JSON array of numbers
// So we use the `ByteVec` type to work directly with binary data
fn apply(input: ByteVec) -> Result<ByteVec, Box<dyn Error>> {
    Ok(input)
}

fn main() {
    handler::run(apply)
}
