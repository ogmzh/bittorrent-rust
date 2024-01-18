use self::hashes::Hashes;
use anyhow::Result;
use hex::encode;
use serde::{Deserialize, Serialize};
use serde_bencode::to_bytes;
use sha1::{Digest, Sha1};
use std::fmt::{Display, Error as FmtError, Formatter};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    pub length: usize,
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: usize,
    /// Each entry of `pieces` is the SHA1 hash of the piece at the corresponding index.
    pub pieces: Hashes, // they get deserialized using the HashesVisitor
}

impl Info {
    #[allow(clippy::unnecessary_fallible_conversions)]
    pub fn info_hash_bytes(&self) -> [u8; 20] {
        let info_encoded = to_bytes(&self).expect("Re-encoding info back to bytes");
        let mut hasher = <Sha1 as Digest>::new();
        hasher.update(&info_encoded);
        // encode(hasher.finalize()) -- this was used when the output of this fn was a String
        hasher
            .finalize()
            .try_into()
            .expect("Hasher finalize failed")
    }

    pub fn info_hash_str(&self) -> String {
        let hash_bytes = self.info_hash_bytes();
        encode(hash_bytes) // 40 bytes hex representation
    }

    // serde urlencoded does not do this properly
    pub fn info_hash_urlencoded(&self) -> String {
        let mut encoded = String::with_capacity(3 * self.info_hash_bytes().len());
        for &byte in &self.info_hash_bytes() {
            encoded.push('%');
            encoded.push_str(&encode([byte]));
        }
        encoded
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Torrent {
    pub announce: String,
    pub info: Info,
}

impl Display for Torrent {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtError> {
        writeln!(f, "Tracker URL: {}", self.announce)?;
        writeln!(f, "Length: {}", self.info.length)?;
        writeln!(f, "Info Hash: {}", self.info.info_hash_str())?;
        writeln!(f, "Piece Length: {}", self.info.piece_length)?;
        writeln!(f, "Piece Hashes:")?;
        for (index, hash) in self.info.pieces.0.iter().enumerate() {
            if index < self.info.pieces.0.len() - 1 {
                writeln!(f, "{}", encode(hash))?;
            } else {
                write!(f, "{}", encode(hash))?;
            }
        }
        Ok(())
    }
}

mod hashes {
    use serde::de::{self, Deserialize, Deserializer, Visitor};
    use serde::ser::{Serialize, Serializer};
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct Hashes(pub Vec<[u8; 20]>); // we access this vec through hashes.0
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
