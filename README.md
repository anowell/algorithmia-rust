Algorithmia Rust Client Library
-------------------------------

A rust client library for the Algorithmia API.

[Documentation](http://algorithmiaio.github.io/algorithmia-rust/algorithmia/)

[![Build Status](https://travis-ci.org/algorithmiaio/algorithmia-rust.svg)](https://travis-ci.org/algorithmiaio/algorithmia-rust)
[![Crates.io](https://img.shields.io/crates/v/algorithmia.svg?maxAge=2592000)](https://crates.io/crates/algorithmia)

## Getting started

The Algorithmia Rust client is published to [crates.io](https://crates.io/crates/algorithmia).
Add `algorithmia = "3.0"` to the dependencies in your `Cargo.toml` and run `cargo install`.

Instantiate an Algorithmia client using your API key:

```rust
use algorithmia::Algorithmia;

let client = Algorithmia::client("YOUR_API_KEY")?;
```

Now you're ready to call algorithms.

## Calling algorithms

This client provides an `Algorithm` type (generally created by `client.algo(..)`) which provides
methods for calling an algorithm hosted on the Algorithmia platform.
The following examples of calling algorithms are organized by type of input/output which vary between algorithms.

Note: a single algorithm may have different input and output types, or accept multiple types of input,
so consult the algorithm's description for usage examples specific to that algorithm.

### Text input/output

Call an algorithm with text input by simply passing `&str` into `Algorithm::pipe`.
If the algorithm output is text, call the `as_string` method on the response.

```rust
let algo = client.algo("algo://demo/Hello/0.1.1");
let response = algo.pipe("HAL 9000")?;
println!("{}", response.into_string()?);
```

### JSON input/output

Call an algorithm with JSON input calling `pipe` with a reference to a type that implements `serde::Serialize`.
If the algorithm output is JSON, you can call `decode` to deserialize the resonse into a type that implements `serde::Deserialize`.

This includes many primitive types, tuples, `Vec`, and other collection types from the standard library:

```rust
let algo = client.algo("algo://WebPredict/ListAnagrams/0.1.0");
let response = algo.pipe(vec!["transformer", "terraforms", "retransform"])?;
let output: Vec<String> = response.decode()?;
// -> ["transformer", "retransform"] as Vec<String>
```

Implementing `Serialize` or `Deserialize` for your custom types is generally as easy as adding a `derive` annotation.

```rust
#[derive(Deserialize, Serialize)]
struct MyStruct {
    some_field: String,
    other_field: u32,
}
// now you can call `pipe` with `&MyStruct` or `decode` into `MyStruct`
```

With `serde_json`, you can also sue the `json!` macro or implement custom serialization/deserialization.
See [serde.rs](https://serde.rs/) for more details on using serde.

Alternatively, you can work with raw JSON strings:

```rust
let response = algo.pipe_json(r#"["transformer", "terraforms", "retransform"]"#)?;
let output = response.to_json()?;
// -> "[\"transformer\", \"retransform\"]"
```

### Binary input/output

Call an algorithm with binary input by calling the `pipe` method with a slice of bytes (`&[u8]`).
If the algorithm response is binary data, then call the `as_bytes` method on the response
to obtain a byte vector (`Vec<u8>`).

```rust
let mut input = Vec::new();
File::open("/path/to/bender.jpg").read_to_end(&mut input);
let response = client.algo("opencv/SmartThumbnail/0.1").pipe(&input)?;
let output = response.as_bytes()?;
// -> Vec<u8>
```

### Error handling

True to the nature of explicit error handling in rust,
the `pipe` and response parsing methods all return `Result`-wrapped types:

```rust
let algo = client.algo("algo://demo/Hello/0.1.1");
match algo.pipe(&[]) {
    Ok(response) => { /* success */ },
    Err(err) => println!("error calling demo/Hello: {}", err),
}
// -> error calling demo/Hello: apply() functions do not match input data
```

Note: this crate makes use of the `error-chain` crate, so for many error variants,
there may be additional errors chained to `err.source()`.

### Request options

The client exposes options that can configure algorithm requests via a builder pattern.
This includes support for changing the timeout or indicating that the API should include stdout in the response.

```rust
let mut algo = client.algo("algo://demo/Hello/0.1.1");
let algo = algo.timeout(10).stdout(true);
let response = algo.pipe(input)?;
if let Some(ref stdout) = response.metadata.stdout {
    println!("{}", stdout);
}
```

Note: `stdout(true)` is ignored if you do not have access to the algorithm source.

## Managing data

The Algorithmia Rust client also provides a way to manage both Algorithmia hosted data
and data from Dropbox or S3 accounts that you've connected to you Algorithmia account.

This client provides a `DataFile` type (generally created by `client.file(uri)`)
and a `DataDir` type (generally created by `client.dir(uri)`) that provide methods for managing your data.

### Create directories

Create directories by instantiating a `DataDir` object and calling `create()` with a `DataAcl`:

```rust
let robots = client.dir("data://.my/robots");
robots.create(DataAcl::default())

let robots = client.dir("dropbox://robots");
robots.create(DataAcl::default())
```

### Upload files to a directory

Upload files by calling `put` on a `DataFile` object, or by calling `put_file` on a `DataDir` object.

```rust
let robots = client.dir("data://.my/robots");

// Upload local file
robots.put_file("/path/to/Optimus_Prime.png");
// Write a text file
robots.child::<DataFile>("Optimus_Prime.txt").put("Leader of the Autobots");
// Write a binary file
robots.child::<DataFile>("Optimus_Prime.key").put(b"transform");
```

### Download contents of file

Download files by calling `get` on a `DataFile` object
which returns a `Result`-wrapped `DataResponse` that implements `Read`:

```rust
// Download and locally save file
let mut t800_png_reader = client.file("data://.my/robots/T-800.png").get()?;
let mut t800_png = File::create("/path/to/save/t800.png")?;
std::io::copy(&mut t800_png_reader, &mut t800_png);

// Get the file's contents as a string
let mut t800_text_reader = robots.file("data://.my/robots/T-800.txt").get()?;
let mut t800_text = String::new();
t800_text_reader.read_to_string(&mut t800_text);

// Get the file's contents as a byte array
let mut t800_png_reader = robots.file("data://.my/robots/T-800.png").get()?;
let mut t800_bytes = Vec::new();
t800_png_reader.read_to_end(&mut t800_bytes);
```

### Delete files and directories

Delete files and directories by calling delete on their respective `DataFile` or `DataDir` object.
DataDirectories take a `force` parameter that indicates whether the directory should be deleted if it contains files or other directories.

```rust
client.file("data://.my/robots/C-3PO.txt").delete();
client.dir("data://.my/robots").delete(false);
```

### List directory contents

Iterate over the contents of a directory using the iterator returned by calling `list` on a `DataDir` object:

```rust
let my_robots = client.dir("data://.my/robots");
for entry in my_robots.list() {
    match entry {
        Ok(DirEntry::Dir(dir)) => println!("Directory {}", dir.to_data_uri()),
        Ok(DirEntry::File(file)) => println!("File {}", file.to_data_uri()),
        Err(err) => println!("Error listing my robots: {}", err),
    }
}
```

## Examples

For examples that use this client, see:

- Basic test [examples](https://github.com/algorithmiaio/algorithmia-rust/tree/master/examples)
- [Algorithmia CLI](https://github.com/algorithmiaio/algorithmia-cli) - CLI build with this client
- [Algorithmia FUSE](https://github.com/anowell/algorithmia-fuse) - experimental filesystem build with this client


## Build & Test

This project is built and tested with cargo:

```bash
cargo build
cargo test
cargo doc --no-deps
```
