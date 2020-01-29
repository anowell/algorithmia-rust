use algorithmia::data::ReadAcl;
use algorithmia::Algorithmia;
use std::env;
use std::error::Error;

fn print_cause_chain(err: &dyn Error) {
    eprintln!("{}", err);
    let mut e = err;
    while let Some(cause) = e.source() {
        eprintln!("caused by {}", cause);
        e = cause;
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next(); // discard args[0]
    let path = match args.next() {
        Some(arg) => arg,
        None => panic!("USAGE: datatest <COLLECTION>"),
    };

    let api_key = match env::var("ALGORITHMIA_API_KEY") {
        Ok(key) => key,
        Err(e) => {
            panic!("Error getting ALGORITHMIA_API_KEY: {}", e);
        }
    };

    let client = Algorithmia::client(&*api_key)?;
    match &client.dir(&*path).create(ReadAcl::Private) {
        Ok(_) => println!("Successfully created collection {}", path),
        Err(e) => print_cause_chain(e),
    }

    match &client.dir(&*path).delete(true) {
        Ok(_) => println!("Successfully deleted collection {}", path),
        Err(e) => print_cause_chain(e),
    }
    Ok(())
}
