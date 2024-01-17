// #[repr(C)]
// #[repr(packed)]
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

