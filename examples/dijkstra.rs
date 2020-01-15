use algorithmia::algo::AlgoResponse;
use algorithmia::Algorithmia;
use std::collections::HashMap;
use std::env;
use std::error::Error;

macro_rules! hashmap {
    ($( $key: expr => $val: expr ),*) => {{
         let mut map = ::std::collections::HashMap::new();
         $( map.insert($key, $val); )*
         map
    }}
}

// Map of all possible points to their possible destinations (with distances)
type SrcDestMap<'a> = HashMap<&'a str, HashMap<&'a str, u32>>;

// The input format for the kenny/Dijkstra algorithm
type DijkstraInput<'a> = (SrcDestMap<'a>, &'a str, &'a str);

// The output format for the kenny/Dijkstra algorithm
type Route = Vec<String>;

struct RouteMap<'a> {
    map: SrcDestMap<'a>,
}

impl<'a> RouteMap<'a> {
    pub fn get_dijkstra_route(
        self,
        start: &'a str,
        end: &'a str,
    ) -> Result<AlgoResponse, Box<dyn Error>> {
        let api_key = match env::var("ALGORITHMIA_API_KEY") {
            Ok(key) => key,
            Err(e) => {
                panic!("Error getting ALGORITHMIA_API_KEY: {}", e);
            }
        };
        let client = Algorithmia::client(&*api_key)?;
        let dijkstra = client.algo("anowell/Dijkstra");

        println!("Making request to: {}", dijkstra.to_url().unwrap());

        // Declaring type explicitly to enforce valid input types during build
        let input_data: DijkstraInput<'_> = (self.map, start, end);
        // println!("Input: {:?}", input_data);
        println!(
            "Input:\n{}",
            serde_json::to_string_pretty(&input_data).unwrap()
        );

        dijkstra.pipe(&input_data).map_err(Into::into)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    args.next(); // discard args[0]
    let start = args.next().unwrap_or_else(|| "a".to_string());
    let end = args.next().unwrap_or_else(|| "c".to_string());

    let input_map = RouteMap {
        map: hashmap!(
            "a" => hashmap!("b" => 1),
            "b" => hashmap!("a" => 2, "c" => 2),
            "c" => hashmap!("b" => 2, "d" => 1),
            "d" => hashmap!("a" => 1, "c" => 3)
        ),
    };

    let output = input_map.get_dijkstra_route(&start, &end)?;
    let duration = output.metadata.duration;
    let result: Route = output.decode()?;
    println!("Shortest route: {}", result.join("->"));
    println!("Completed in {} seconds.", duration);
    Ok(())
}
