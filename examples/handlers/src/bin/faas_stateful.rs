use algorithmia::prelude::*;
use std::collections::HashMap;
use std::error::Error;

type Dict = HashMap<char, u32>;

// This is a simple, stateful char counter
fn apply(input: String, shared_state: &mut Dict) -> Result<Dict, Box<dyn Error>> {
    for c in input.chars() {
        let counter = shared_state.entry(c).or_insert(0);
        *counter += 1;
    }
    Ok(shared_state.clone())
}

fn main() {
    let mut initial_state = Dict::new();
    handler::run(|input| apply(input, &mut initial_state))
}
