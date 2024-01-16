use self::hashes::Hashes;
use hex::encode;
use sha1::{Sha1, Digest};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_bencode::to_bytes;
use std::fmt::{Display, Error as FmtError, Formatter};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Info {
    length: usize,
    name: String,
    #[serde(rename = "piece length")]
    piece_length: usize,
    /// Each entry of `pieces` is the SHA1 hash of the piece at the corresponding index.
    pieces: Hashes, // they get deserialized using the HashesVisitor
}

impl Info {
    pub fn info_hash(&self) -> String {
        let info_encoded = to_bytes(&self)
            .context("CTX: Re-encoding info back to bytes")
            .unwrap();
        let mut hasher = <Sha1 as Digest>::new();
        hasher.update(&info_encoded);
        encode(hasher.finalize())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub announce: String,
    info: Info,
}

impl Display for Torrent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        writeln!(f, "Tracker URL: {}", self.announce)?;
        writeln!(f, "Length: {}", self.info.length)?;
        write!(f, "Info Hash: {}", self.info.info_hash())?;
        Ok(())
    }
}

mod hashes {
    use serde::de::{self, Deserialize, Deserializer, Visitor};
    use serde::ser::{Serialize, Serializer};
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct Hashes(Vec<[u8; 20]>);
    struct HashesVisitor;

    impl<'de> Visitor<'de> for HashesVisitor {
        type Value = Hashes; // the output type of the visitor

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a byte string whose length is a multiple of 20")
        }
        // cannot have a visit_string because whatever we receive can possibly not be a valid UTF8 string
        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.len() % 20 != 0 {
                return Err(E::custom(format!(
                    "Invalid length of the array being deserialized: {}",
                    v.len()
                )));
            }
            Ok(Hashes(
                v.chunks_exact(20)
                    .map(|slice_20| slice_20.try_into().expect("guaranteed length 20"))
                    .collect(),
            ))
        }
    }

    impl<'de> Deserialize<'de> for Hashes {
        fn deserialize<D>(deserializer: D) -> Result<Hashes, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_bytes(HashesVisitor)
        }
    }

    impl Serialize for Hashes {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let single_slice = self.0.concat(); // iter.flatten.collect: concat flattens a vec<[u8]> to a vec<u8>
            serializer.serialize_bytes(&single_slice)
        }
    }
}
