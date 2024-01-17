use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_bencode::from_bytes;
use std::{fs, path::PathBuf};

use bittorrent_starter_rust::torrent::Torrent;
use bittorrent_starter_rust::tracker::{TrackerRequest, TrackerResponse};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Decode { value: String },
    Info { torrent: PathBuf },
    Peers { torrent: PathBuf },
}

// for the actual invocation we will use the serde_bencode::from_str as it is safer and will work with non-utf8 strings
fn decode_bencoded_value(encoded_value: &str) -> (serde_json::Value, &str) {
    // we return a tuple so we can always return the remainder of the string after recursive parsing
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
            while !remainder.starts_with('e') {
                // e character is the terminator
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Decode { value } => {
            let decoded_value = decode_bencoded_value(&value).0;
            println!("{decoded_value}");
        }
        Command::Info { torrent } => {
            let file = fs::read(torrent).context("CTX: Open torent file")?;
            let torrent: Torrent = from_bytes(&file).context("CTX: torrent file to bytes")?;
            println!("{torrent}")
        }
        Command::Peers { torrent } => {
            let file = fs::read(torrent).context("CTX: Open torent file")?;
            let torrent: Torrent = from_bytes(&file).context("CTX: torrent file to bytes")?;
            let info_hash = torrent.info.info_hash_urlencoded();
            let request = TrackerRequest {
                peer_id: String::from("00112233445566778899"),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: torrent.info.length,
                compact: 1,
            };

            let params = serde_urlencoded::to_string(&request)
                .context("CTX: url encoding request params")?;
            let tracker_url = format!("{}?{}&info_hash={}", torrent.announce, params, info_hash);
            let response = reqwest::get(tracker_url)
                .await
                .context("CTX: reqwest::get tracker_url")?;
            let response_bytes = response
                .bytes()
                .await
                .context("CTX: tracker response to bytes")?;
            let response: TrackerResponse = from_bytes(&response_bytes)
                .context("CTX: byte to tracker response deserialization")?;
            eprintln!("{:?}", response);
            response.peers.0.iter().for_each(|peer| println!("{peer}"));

            // let tracker_url = Url::from(&torrent.announce);
            // tracker_url.quer
        }
    }

    Ok(())
}
