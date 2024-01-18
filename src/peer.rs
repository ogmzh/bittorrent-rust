use anyhow::{anyhow, Context, Result};
use std::{net::SocketAddrV4, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::torrent::Torrent;

use self::{
    handshake::{Handshake, HANDSHAKE_BYTE_BUFFER_SIZE},
    message::MessageType,
};

pub struct Stream {
    pub connection: TcpStream,
}

impl Stream {
    pub async fn connect(peer_addr: &SocketAddrV4) -> Result<Self> {
        let connection = TcpStream::connect(peer_addr).await.context(format!(
            "CTX: Stream connection failed to peer address: {peer_addr}"
        ))?;
        Ok(Self { connection })
    }

    pub async fn handshake(
        &mut self,
        handshake: Handshake,
    ) -> Result<[u8; HANDSHAKE_BYTE_BUFFER_SIZE]> {
        self.connection
            .write_all(&handshake.as_bytes())
            .await
            .context("CTX: Write handshake bytes failed")?;
        let mut buf = [0u8; HANDSHAKE_BYTE_BUFFER_SIZE];
        self.connection
            .read_exact(&mut buf)
            .await
            .context("CTX: Read handshake bytes failed")?;
        Ok(buf)
    }

    pub async fn bitfield(&mut self) -> Result<()> {
        let length = self.get_message_length().await?;
        let mut buf = vec![0u8; length as usize];
        self.connection
            .read_exact(&mut buf)
            .await
            .context("CTX: Read bitfield buffer failed")?;

        match MessageType::from_id(buf[0]) {
            Some(MessageType::Bitfield) => Ok(()),
            _ => Err(anyhow!("Expected bitfield")),
        }
    }

    pub async fn interested(&mut self) -> Result<()> {
        let mut interested = [0u8; 5];
        interested[3] = 1;
        interested[4] = MessageType::Interested.id();
        self.connection
            .write_all(&interested)
            .await
            .context("CTX: Write interested buffer failed")?;
        Ok(())
    }

    pub async fn get_piece_data(&mut self, piece: u32, torrent: &Torrent) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        let mut block_index: u32 = 0;
        let mut block_size: u32 = 16 * 1024; // 16Kb // 2^14
        let mut remaining_bytes: u32 = if piece == torrent.info.pieces.0.len() as u32 - 1 {
            (torrent.info.length as u32) % (torrent.info.piece_length as u32)
        } else {
            torrent.info.piece_length as u32
        };

        while remaining_bytes > 0 {

            if remaining_bytes < block_size {
                block_size = remaining_bytes;
            }
            self.send_request_piece(piece, block_index, block_size)
                .await?;
            let request_buf = self
                .read_request_piece()
                .await
                .context("CTX: Reading request piece")?;

            let mut piece_data_index = [0u8; 4];
            piece_data_index.copy_from_slice(&request_buf[1..5]);
            let mut piece_offset_begin = [0u8; 4];
            piece_offset_begin.copy_from_slice(&request_buf[5..9]);
            let data_block = request_buf[9..].to_vec();
            data.extend(data_block);
            remaining_bytes -= block_size;
            block_index += block_size;
        }
        Ok(data)
    }

    async fn send_request_piece(
        &mut self,
        piece: u32,
        block_index: u32,
        block_size: u32,
    ) -> Result<()> {
        let mut request_piece_buf = [0u8; 17];
        request_piece_buf[0..4].copy_from_slice(&13u32.to_be_bytes()); // Message length: 13
        request_piece_buf[4] = MessageType::Request.id();
        request_piece_buf[5..9].copy_from_slice(&piece.to_be_bytes());
        request_piece_buf[9..13].copy_from_slice(&block_index.to_be_bytes());
        request_piece_buf[13..17].copy_from_slice(&block_size.to_be_bytes());
        self.connection
            .write_all(&request_piece_buf)
            .await
            .context("CTX: send request piece")?;
        Ok(())
    }

    async fn read_request_piece(&mut self) -> Result<Vec<u8>> {
        let length = self.get_message_length().await?;
        let mut request_buf = vec![0; length as usize];
        self.connection
            .read_exact(&mut request_buf)
            .await
            .context("CTX: request piece buf")?;
        if request_buf[0] != MessageType::Piece.id() {
            panic!("expected request piece");
        }
        Ok(request_buf)
    }

    async fn get_message_length(&mut self) -> Result<u32> {
        let mut length_buf = [0u8; 4];
        self.connection
            .read_exact(&mut length_buf)
            .await
            .context("CTX: read length buffer")?;
        let length = u32::from_be_bytes(length_buf);
        Ok(length)
    }

    async fn read_with_timeout(
        &mut self,
        buffer: &mut [u8],
        timeout_duration: Duration,
    ) -> Result<()> {
        timeout(timeout_duration, self.connection.read_exact(buffer))
            .await
            .context("CTX: read operation timed out")??;
        Ok(())
    }

    pub async fn wait_unchoke(&mut self) -> Result<()> {
        let length = self.get_message_length().await?;
        let mut unchoke_message_buffer = vec![0; length as usize];
        // i think this is fundamentally wrong because we will get the first byte anyway
        // whether or not it is unchoke, and i'm unsure if we should reinitialize the entire connection from the handshake
        // or when is the peer going to send us another byte? ¯\_(ツ)_/¯ but leave this here for future reference on async + timeout
        loop {
            self.read_with_timeout(&mut unchoke_message_buffer, Duration::from_secs(10))
                .await?;
            if unchoke_message_buffer[0] == MessageType::Unchoke.id() {
                break;
            }
        }
        Ok(())
    }
}

// The handshake is a message consisting of the following parts as described in the peer protocol:
pub mod handshake {
    // length of the protocol string (BitTorrent protocol) which is 19 (1 byte)
    // the string BitTorrent protocol (19 bytes)
    // eight reserved bytes, which are all set to zero (8 bytes)
    // sha1 infohash (20 bytes) (NOT the hexadecimal representation, which is 40 bytes long)
    // peer id (20 bytes) (you can use 00112233445566778899 for this challenge)
    pub const HANDSHAKE_PEER_ID_BYTE_INDEX_START: usize = 48;
    pub const HANDSHAKE_BYTE_BUFFER_SIZE: usize = 68;

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
}

pub mod message {
    #[derive(Debug)]
    pub enum MessageType {
        Choke,
        Unchoke,
        Interested,
        NotInterested,
        Have,
        Bitfield,
        Request,
        Piece,
        Cancel,
    }

    impl MessageType {
        pub fn id(&self) -> u8 {
            match self {
                MessageType::Choke => 0,
                MessageType::Unchoke => 1,
                MessageType::Interested => 2,
                MessageType::NotInterested => 3,
                MessageType::Have => 4,
                MessageType::Bitfield => 5,
                MessageType::Request => 6,
                MessageType::Piece => 7,
                MessageType::Cancel => 8,
            }
        }

        pub fn from_id(id: u8) -> Option<MessageType> {
            match id {
                0 => Some(MessageType::Choke),
                1 => Some(MessageType::Unchoke),
                2 => Some(MessageType::Interested),
                3 => Some(MessageType::NotInterested),
                4 => Some(MessageType::Have),
                5 => Some(MessageType::Bitfield),
                6 => Some(MessageType::Request),
                7 => Some(MessageType::Piece),
                8 => Some(MessageType::Cancel),
                _ => None,
            }
        }

        pub fn get_write_buffer<F>(&self, get_values: F) -> Vec<u8>
        where
            F: Fn() -> (u32, u32, u32), // pass a closure to MessageType::Request
        {
            match self {
                MessageType::Interested => {
                    let mut buf = [0u8; 5];
                    buf[3] = 1;
                    buf[4] = MessageType::Interested.id();
                    buf.to_vec()
                }
                MessageType::Request => {
                    let (piece, block_index, block_size) = get_values();
                    let mut buf = [0u8; 17];
                    buf[0..4].copy_from_slice(&13u32.to_be_bytes()); // Message length: 13
                    buf[4] = MessageType::Request.id(); // Message ID: 6 (request)
                    buf[5..9].copy_from_slice(&piece.to_be_bytes()); // Piece index
                    buf[9..13].copy_from_slice(&block_index.to_be_bytes()); // Offset
                    buf[13..17].copy_from_slice(&block_size.to_be_bytes()); // Length
                    buf.to_vec()
                }
                _ => Vec::new(),
            }
        }
    }
}
