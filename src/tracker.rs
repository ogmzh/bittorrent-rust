use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_bencode::from_bytes;

use self::peers::Peers;
use crate::torrent::Torrent;

// info_hash: the info hash of the torrent
// 20 bytes long, will need to be URL encoded
// Note: this is NOT the hexadecimal representation, which is 40 bytes long
// peer_id: a unique identifier for your client
// A string of length 20 that you get to pick. You can use something like 00112233445566778899.
// port: the port your client is listening on
// You can set this to 6881, you will not have to support this functionality during this challenge.
// uploaded: the total amount uploaded so far
// Since your client hasn't uploaded anything yet, you can set this to 0.
// downloaded: the total amount downloaded so far
// Since your client hasn't downloaded anything yet, you can set this to 0.
// left: the number of bytes left to download
// Since you client hasn't downloaded anything yet, this'll be the total length of the file (you've extracted this value from the torrent file in previous stages)
// compact: whether the peer list should use the compact representation
// For the purposes of this challenge, set this to 1.
// The compact representation is more commonly used in the wild, the non-compact representation is mostly supported for backward-compatibility.
#[derive(Debug, Clone, Serialize)]
pub struct TrackerRequest {
    /// info_hash field is omitted because if we serialize the byte array to an urlencoded str,
    /// that str will then again get url encoded when sending the request which is wrong
    // pub info_hash: [u8; 20],
    pub peer_id: String,
    pub port: u16,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}

impl TrackerRequest {
    pub fn default(length: usize) -> Self {
        Self {
            peer_id: String::from("00112233445566778899"),
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: length,
            compact: 1,
        }
    }

    pub async fn discover_peers(&self, torrent: &Torrent) -> Result<Peers> {
        let params =
            serde_urlencoded::to_string(self).context("CTX: url encoding request params")?;
        let tracker_url = format!(
            "{}?{}&info_hash={}",
            torrent.announce,
            params,
            torrent.info.info_hash_urlencoded()
        );
        let response = reqwest::get(tracker_url)
            .await
            .context("CTX: reqwest::get tracker_url")?;
        let response_bytes = response
            .bytes()
            .await
            .context("CTX: tracker response to bytes")?;
        let response: TrackerResponse =
            from_bytes(&response_bytes).context("CTX: byte to tracker response deserialization")?;
        Ok(response.peers)
    }
}

// The tracker's response will be a bencoded dictionary with two keys:

// interval:
// An integer, indicating how often (in seconds) your client should make a request to the tracker.
// You can ignore this value for the purposes of this challenge.
// peers.
// A string, which contains list of peers that your client can connect to.
// Each peer is represented using 6 bytes. The first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TrackerResponse {
    // interval: usize,
    pub peers: Peers,
}

mod peers {
    use serde::de::{self, Deserialize, Deserializer, Visitor};
    // use serde::ser::{Serialize, Serializer};
    use std::fmt;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[derive(Debug, Clone)]
    pub struct Peers {
        pub addresses: Vec<SocketAddrV4>,
    } // v4 and not v6 because "The first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number"

    struct PeersVisitor;

    impl<'de> Visitor<'de> for PeersVisitor {
        type Value = Peers;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("6 bytes, the first 4 bytes are a peer's IP address and the last 2 are a peer's port number")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 6 != 0 {
                return Err(E::custom(format!("length is {}", v.len())));
            }

            let addresses: Vec<SocketAddrV4> = v
                .chunks_exact(6)
                .map(|chunk_6| {
                    SocketAddrV4::new(
                        Ipv4Addr::new(chunk_6[0], chunk_6[1], chunk_6[2], chunk_6[3]),
                        u16::from_be_bytes([chunk_6[4], chunk_6[5]]),
                    )
                })
                .collect();
            Ok(Peers { addresses })
        }
    }

    impl<'de> Deserialize<'de> for Peers {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(PeersVisitor)
        }
    }
}
