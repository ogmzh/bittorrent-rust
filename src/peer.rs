// The handshake is a message consisting of the following parts as described in the peer protocol:

// length of the protocol string (BitTorrent protocol) which is 19 (1 byte)
// the string BitTorrent protocol (19 bytes)
// eight reserved bytes, which are all set to zero (8 bytes)
// sha1 infohash (20 bytes) (NOT the hexadecimal representation, which is 40 bytes long)
// peer id (20 bytes) (you can use 00112233445566778899 for this challenge)
pub const HANDSHAKE_PEER_ID_BYTE_INDEX_START: u8 = 48;
pub const HANDSHAKE_BYTE_BUFFER_SIZE: u8 = 68;

pub struct Handshake {
    pub length: u8,
    pub protocol: &'static [u8; 19], // static byte slice (can also write it as &'static [u8])
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: String,
}

impl Handshake {
    pub fn new(info_hash_bytes: [u8; 20]) -> Self {
        Self {
            length: 19,
            protocol: b"BitTorrent protocol", // creates a static byte string slice
            reserved: [0; 8],
            info_hash: info_hash_bytes,
            peer_id: String::from("00112233445566778899"),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.push(self.length);
        bytes.extend_from_slice(self.protocol);
        bytes.extend_from_slice(&self.reserved);
        bytes.extend_from_slice(&self.info_hash);
        bytes.extend_from_slice(self.peer_id.as_bytes());
        bytes
    }
}

