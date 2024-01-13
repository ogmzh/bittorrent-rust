use std::{env, collections::HashMap};

// instead of parsing these strings manually and recursively (for lists), we could just call
// but I'm doing it manually for some rust practice.
// we return a tuple so we can always return the remainder of the string after recursive parsing
fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
    match encoded_value.chars().next() {
        Some('i') => {
            if let Some((n, rest)) = encoded_value
                .split_at(1)
                .1
                .split_once('e') // integer encoded strings look like i25e
                .and_then(|(digits, rest)| {
                    let n = digits.parse::<i64>().ok()?;
                    Some((n, rest))
                })
            {
                return (n.into(), rest);
            }
        }
        Some('l') => {
            let mut values = Vec::new();
            let mut remainder = encoded_value.split_at(1).1; // lists look like l5:helloi52ee
            while !remainder.starts_with('e') {  // e character is the terminator
                let (value, rest) = decode_bencoded_value(remainder);
                values.push(value);
                remainder = rest;
            }
            // return the list with whatever is left after in the encoded string, as the list has been terminated in the while with 'e'
            return (values.into(), &remainder[1..]); // skip the e terminating the list
        }
        Some('d') => {
            let mut map = serde_json::Map::new();
            let mut remainder = encoded_value.split_at(1).1; // dictionaries look like d3:foo3:bar5:helloi52ee
            let mut count = 0;
            let mut key: String = String::new();
            let mut map_value: serde_json::Value;
            while !remainder.starts_with('e') {
                let (value, rest) = decode_bencoded_value(remainder);
                if count == 0 {
                    match value {
                        serde_json::Value::String(k) => key = k,
                        k => {
                            panic!("Dict keys must be strings, not {k:?}");
                        }
                    };
                    count += 1;
                } else {
                    map_value = value;
                    map.insert(key.clone(), map_value);
                    count = 0;
                }
                remainder = rest;
            }
            return (map.into(), &remainder[1..]); // skip the e terminating the dict
        }
        Some('0'..='9') => {
            if let Some((length, rest)) = encoded_value.split_once(':') {
                // string encoded values look like 5:hello
                if let Ok(length) = length.parse::<usize>() {
                    return (rest[..length].into(), &rest[length..]);
                }
            }
        }
        _ => {}
    }

    panic!("Unhandled encoded value: {}", encoded_value)
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        // You can use print statements as follows for debugging, they'll be visible when running tests.
        eprintln!("Logs from your program will appear here!");

        // Uncomment this block to pass the first stage
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.0);
    } else {
        println!("unknown command: {}", args[1])
    }
}
