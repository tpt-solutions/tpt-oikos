use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};

use crate::message::NetworkMessage;

pub type ConnId = u64;

struct ManagedConnection {
    addr: SocketAddr,
    writer: Arc<Mutex<OwnedWriteHalf>>,
}

pub struct TcpTransport {
    listener: Option<TcpListener>,
    connections: HashMap<ConnId, ManagedConnection>,
    addr_to_conn: HashMap<SocketAddr, ConnId>,
    incoming_rx: mpsc::UnboundedReceiver<(ConnId, NetworkMessage)>,
    incoming_tx: mpsc::UnboundedSender<(ConnId, NetworkMessage)>,
    next_id: ConnId,
}

impl TcpTransport {
    pub async fn new(bind_addr: SocketAddr) -> anyhow::Result<Self> {
        let listener = TcpListener::bind(bind_addr)
            .await
            .context("failed to bind TCP listener")?;
        let (tx, rx) = mpsc::unbounded_channel();
        Ok(Self {
            listener: Some(listener),
            connections: HashMap::new(),
            addr_to_conn: HashMap::new(),
            incoming_rx: rx,
            incoming_tx: tx,
            next_id: 1,
        })
    }

    pub async fn connect(&mut self, addr: SocketAddr) -> anyhow::Result<ConnId> {
        let stream = TcpStream::connect(addr)
            .await
            .context("TCP connect failed")?;
        let conn_id = self.next_id;
        self.next_id += 1;
        self.setup_connection(conn_id, stream, addr);
        Ok(conn_id)
    }

    pub async fn accept_one(&mut self) -> anyhow::Result<Option<(ConnId, SocketAddr)>> {
        if let Some(listener) = &self.listener {
            match tokio::time::timeout(std::time::Duration::from_millis(1), listener.accept()).await {
                Ok(Ok((stream, addr))) => {
                    let conn_id = self.next_id;
                    self.next_id += 1;
                    self.setup_connection(conn_id, stream, addr);
                    Ok(Some((conn_id, addr)))
                }
                Ok(Err(e)) => Err(e).context("accept failed"),
                Err(_) => Ok(None), // timeout, no pending connection
            }
        } else {
            Ok(None)
        }
    }

    fn setup_connection(&mut self, conn_id: ConnId, stream: TcpStream, addr: SocketAddr) {
        let (read_half, write_half) = stream.into_split();
        let writer = Arc::new(Mutex::new(write_half));

        self.connections
            .insert(conn_id, ManagedConnection { addr, writer: writer.clone() });
        self.addr_to_conn.insert(addr, conn_id);

        let tx = self.incoming_tx.clone();
        tokio::spawn(async move {
            Self::reader_loop(conn_id, read_half, tx).await;
        });
    }

    async fn reader_loop(
        conn_id: ConnId,
        mut reader: OwnedReadHalf,
        tx: mpsc::UnboundedSender<(ConnId, NetworkMessage)>,
    ) {
        loop {
            match read_frame(&mut reader).await {
                Ok(msg) => {
                    if tx.send((conn_id, msg)).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }

    pub async fn send(&self, conn_id: ConnId, msg: &NetworkMessage) -> anyhow::Result<()> {
        let conn = self
            .connections
            .get(&conn_id)
            .ok_or_else(|| anyhow::anyhow!("connection {conn_id} not found"))?;
        let mut writer = conn.writer.lock().await;
        write_frame(&mut *writer, msg).await
    }

    pub fn poll(&mut self) -> Vec<(ConnId, NetworkMessage)> {
        let mut messages = Vec::new();
        while let Ok(msg) = self.incoming_rx.try_recv() {
            messages.push(msg);
        }
        messages
    }

    pub async fn broadcast(&self, msg: &NetworkMessage) {
        for conn in self.connections.values() {
            let mut writer = conn.writer.lock().await;
            let _ = write_frame(&mut *writer, msg).await;
        }
    }

    pub fn remove_connection(&mut self, conn_id: ConnId) {
        if let Some(conn) = self.connections.remove(&conn_id) {
            self.addr_to_conn.remove(&conn.addr);
        }
    }

    pub fn connection_id_for_addr(&self, addr: &SocketAddr) -> Option<ConnId> {
        self.addr_to_conn.get(addr).copied()
    }

    pub fn addr_for_connection(&self, conn_id: ConnId) -> Option<SocketAddr> {
        self.connections.get(&conn_id).map(|c| c.addr)
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.listener.as_ref().and_then(|l| l.local_addr().ok())
    }

    pub fn stop_listening(&mut self) {
        self.listener = None;
    }
}

async fn write_frame(
    writer: &mut (impl AsyncWriteExt + Unpin),
    msg: &NetworkMessage,
) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(msg).context("JSON serialization failed")?;
    let len = (payload.len() as u32).to_be_bytes();
    writer.write_all(&len).await.context("write length prefix")?;
    writer.write_all(&payload).await.context("write payload")?;
    Ok(())
}

async fn read_frame(reader: &mut (impl AsyncReadExt + Unpin)) -> anyhow::Result<NetworkMessage> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .await
        .context("read length prefix")?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 16 * 1024 * 1024 {
        anyhow::bail!("frame too large: {len} bytes (max 16 MiB)");
    }
    let mut payload = vec![0u8; len];
    reader
        .read_exact(&mut payload)
        .await
        .context("read payload")?;
    serde_json::from_slice(&payload).context("JSON deserialization failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{BlockHeader, NetworkMessage};

    fn dummy_header(height: u64) -> BlockHeader {
        BlockHeader {
            height,
            hash: [height as u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 1_700_000_000,
            validator_id: 1,
            tx_count: 0,
            state_root: [0u8; 32],
        }
    }

    #[tokio::test]
    async fn connect_and_send_message() {
        let mut server = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let server_addr = server.local_addr().unwrap();

        let mut client = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let client_conn = client.connect(server_addr).await.unwrap();

        let (_server_conn, _addr) = server.accept_one().await.unwrap().unwrap();

        let msg = NetworkMessage::NewBlock(dummy_header(1));
        client.send(client_conn, &msg).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let received = server.poll();
        assert_eq!(received.len(), 1);
        match &received[0].1 {
            NetworkMessage::NewBlock(h) => assert_eq!(h.height, 1),
            _ => panic!("expected NewBlock"),
        }
    }

    #[tokio::test]
    async fn length_prefixed_framing_round_trip() {
        let mut server = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let server_addr = server.local_addr().unwrap();

        let mut client = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let client_conn = client.connect(server_addr).await.unwrap();
        let (_server_conn, _) = server.accept_one().await.unwrap().unwrap();

        let messages = vec![
            NetworkMessage::Ping {
                node_id: [1u8; 32],
                height: 10,
            },
            NetworkMessage::Pong {
                node_id: [2u8; 32],
                height: 20,
            },
            NetworkMessage::BlockRequest(42),
        ];

        for msg in &messages {
            client.send(client_conn, msg).await.unwrap();
        }

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let received = server.poll();
        assert_eq!(received.len(), 3);
    }

    #[tokio::test]
    async fn broadcast_sends_to_all() {
        let mut server1 = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let addr1 = server1.local_addr().unwrap();
        let mut server2 = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let addr2 = server2.local_addr().unwrap();

        let mut broadcaster = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        broadcaster.connect(addr1).await.unwrap();
        broadcaster.connect(addr2).await.unwrap();

        server1.accept_one().await.unwrap();
        server2.accept_one().await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let msg = NetworkMessage::NewBlock(dummy_header(99));
        broadcaster.broadcast(&msg).await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let r1 = server1.poll();
        let r2 = server2.poll();
        assert_eq!(r1.len(), 1);
        assert_eq!(r2.len(), 1);
    }

    #[tokio::test]
    async fn connection_tracking() {
        let mut server = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let server_addr = server.local_addr().unwrap();

        let mut client = TcpTransport::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let _conn_id = client.connect(server_addr).await.unwrap();
        let (s_conn_id, client_addr) = server.accept_one().await.unwrap().unwrap();

        assert_eq!(server.connection_count(), 1);
        assert_eq!(server.addr_for_connection(s_conn_id), Some(client_addr));
        assert_eq!(server.connection_id_for_addr(&client_addr), Some(s_conn_id));

        server.remove_connection(s_conn_id);
        assert_eq!(server.connection_count(), 0);
    }
}
