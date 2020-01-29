//! Support for running Rust-based algorithms on the Algorithmia platform [feature = "handler"]

use base64;
use serde_json;

use crate::algo::{AlgoData, ByteVec, TryFrom};
use crate::error::{err_msg, ResultExt};
use crate::prelude::AlgoIo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, BufRead, Write};
use std::process;

const ALGOOUT: &'static str = "/tmp/algoout";

#[derive(Deserialize)]
struct Request {
    data: Value,
    content_type: String,
}

#[derive(Serialize)]
struct AlgoSuccess {
    result: Value,
    metadata: RunnerMetadata,
}

#[derive(Serialize)]
struct AlgoFailure {
    error: RunnerError,
}

#[derive(Serialize)]
struct RunnerMetadata {
    content_type: String,
}

#[derive(Serialize)]
struct RunnerError {
    message: String,
    error_type: &'static str,
}

impl AlgoSuccess {
    fn new<S: Into<String>>(result: Value, content_type: S) -> AlgoSuccess {
        AlgoSuccess {
            result: result,
            metadata: RunnerMetadata {
                content_type: content_type.into(),
            },
        }
    }
}

impl AlgoFailure {
    fn new(err: &dyn Error) -> AlgoFailure {
        AlgoFailure {
            error: RunnerError {
                message: error_cause_chain(err),
                error_type: "AlgorithmError",
            },
        }
    }

    fn system(err: &dyn Error) -> AlgoFailure {
        AlgoFailure {
            error: RunnerError {
                message: error_cause_chain(err),
                error_type: "SystemError",
            },
        }
    }
}

/// Configures the Algorithmia-compatible FaaS handler
///
/// This function is only used when authoring an algorithm to run on the Algorithmia platform.
/// This function receives a handler function or closure that is used to process each individual request.
/// It will block and only return once it reaches EOF of STDIN.
///
/// The handler function makes heavy use of generic conversion traits
/// to provide flexibility in what functions it can accept.
/// Essentially the handler is any function with an input argument that
/// `AlgoIo` can be converted into, and an output that is either
/// a type that can be convert into `AlgoIo` or any error type that
/// can be coerced into a boxed `Error`.
///
/// ## Getting started
///
/// The simplest usage of this function is to just use a simple function that works entirely with `String`s:
///
/// ```rust
/// use algorithmia::prelude::*;
///
/// fn apply(name: String) -> Result<String, String> {
///     unimplemented!()
/// }
///
/// fn main() {
///     handler::run(apply)
/// }
/// ```
///
/// ## Automatic JSON serialization/deserialization
///
/// To use your own custom types as input and output, simply implement `Deserialize` and `Serialize` respectively.
///
/// ```rust
/// #[derive(Deserialize)]
/// struct Input { titles: Vec<String>, max: u32 }
///
/// #[derive(Serialize)]
/// struct Output { titles: Vec<String> }
///
/// fn apply(input: Input) -> Result<Output, Box<dyn Error>> {
///     unimplemented!();
/// }
///
/// fn main() {
///     handler::run(apply)
/// }
/// ```
///
/// ## Input/Output types:
/// **Valid input**
/// - Any type that implements `serde::Deserialize` (e.g. `#[derive(Deserialize)]`
/// - `algo::ByteVec` if working with binary input
///
/// **Valid output types (`Ok` variant of return value)**
/// - Any type that implements `serde::Serialize` (e.g. `#[derive(Serialize)]`
/// - `algo::ByteVec` if working with binary output
///
/// **Valid error types (`Err` variant of return value)**
/// Anything with an conversion to `Box<Error>`. This includes `String` and basically any type that implements the `Error` trait.
///
/// ## Preloading and Maintaining State (Advanced Usage)
///
/// If your algorithm has a preload step that doesn't vary with user input (e.g. loading a model),
/// you can perform that prior to calling `handler::run` and then passing in a reference to that stay via a capturing closure:
///
/// ```rust
/// #[derive(Deserialize)]
/// struct Input { titles: Vec<String>, max: u32 }
///
/// #[derive(Serialize)]
/// struct Output { titles: Vec<String> }
///
/// struct App { model: Vec<u8> }
///
/// fn apply(input: Input, app: &App) -> Result<Output, String> {
///     unimplemented!();
/// }
///
/// fn main() {
///     let app = App { model: load_model() };
///     handler::run(|input| apply(input, &app) )
/// }
/// ```
pub fn run<F, IN, OUT, E, E2>(mut apply: F)
where
    F: FnMut(IN) -> Result<OUT, E>,
    IN: TryFrom<AlgoIo, Error = E2>,
    OUT: Into<AlgoIo>,
    E: Into<Box<dyn Error>>,
    E2: Into<Box<dyn Error>>,
{
    println!("PIPE_INIT_COMPLETE");
    flush_std_pipes();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let output_json = match line {
            Ok(json_line) => {
                let output = build_input(json_line).and_then(|input| match IN::try_from(input) {
                    Ok(algo_io) => match apply(algo_io) {
                        Ok(out) => Ok(out.into()),
                        Err(err) => Err(err.into()),
                    },
                    Err(err) => Err(err.into()),
                });
                flush_std_pipes();
                serialize_output(output)
            }
            Err(_) => {
                let err = line.context("failed to read stdin").unwrap_err();
                serde_json::to_string(&AlgoFailure::system(&err as &dyn Error)).expect(&format!(
                    "Failed to read stdin and failed to encode the error: {}",
                    err
                ))
            }
        };
        algoout(&output_json);
    }
}

pub fn load_and_run<F, LOAD, IN, OUT, STATE, E, E2, E3>(
    load: LOAD,
    mut apply: F,
) -> Result<(), Box<dyn Error>>
where
    F: FnMut(IN, &mut STATE) -> Result<OUT, E>,
    LOAD: FnOnce() -> Result<STATE, E3>,
    IN: TryFrom<AlgoIo, Error = E2>,
    OUT: Into<AlgoIo>,
    E: Into<Box<dyn Error>>,
    E2: Into<Box<dyn Error>>,
    E3: Into<Box<dyn Error>>,
{
    let mut state = load().map_err(|err| err.into())?;
    run(|input| apply(input, &mut state));
    Ok(())
}

impl From<AlgoIo> for AlgoSuccess {
    fn from(output: AlgoIo) -> AlgoSuccess {
        match output.data {
            AlgoData::Text(text) => AlgoSuccess::new(Value::String(text), "text"),
            AlgoData::Json(json_obj) => AlgoSuccess::new(json_obj, "json"),
            AlgoData::Binary(bytes) => {
                let result = base64::encode(&bytes);
                AlgoSuccess::new(Value::String(result), "binary")
            }
        }
    }
}

fn error_cause_chain(err: &dyn Error) -> String {
    let mut causes = vec![err.to_string()];
    let mut e = err;
    while let Some(cause) = e.source() {
        causes.push(cause.to_string());
        e = cause;
    }
    causes.join("\ncaused by: ")
}

fn serialize_output(output: Result<AlgoIo, Box<dyn Error>>) -> String {
    let json_result = match output {
        Ok(output) => serde_json::to_string(&AlgoSuccess::from(output)),
        Err(err) => serde_json::to_string(&AlgoFailure::new(&*err as &dyn Error)),
    };

    json_result.expect("Failed to encode JSON")
}

fn flush_std_pipes() {
    let _ = io::stdout().flush();
    let _ = io::stderr().flush();
}

fn algoout(output_json: &str) {
    match OpenOptions::new().write(true).open(ALGOOUT) {
        Ok(mut f) => {
            let _ = f.write(output_json.as_bytes());
            let _ = f.write(b"\n");
        }
        Err(e) => {
            println!("Cannot write to algoout pipe: {}\n", e);
            process::exit(-1);
        }
    };
}

fn build_input(stdin: String) -> Result<AlgoIo, Box<dyn Error>> {
    let req = serde_json::from_str(&stdin).context("Error decoding JSON request")?;
    let Request { data, content_type } = req;
    let input = match (&*content_type, data) {
        ("text", Value::String(text)) => AlgoIo::from(text),
        ("binary", Value::String(ref encoded)) => {
            let bytes =
                base64::decode(encoded).context("Error decoding request input as binary")?;
            AlgoIo::from(ByteVec::from(bytes))
        }
        ("json", json_obj) => AlgoIo::from(json_obj),
        (_, _) => {
            return Err(err_msg(format!("Content type '{}' is invalid", content_type)).into())
        }
    };
    Ok(input)
}
