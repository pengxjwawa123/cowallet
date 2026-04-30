use crate::dkls23::ProtocolMessage;
use crate::errors::{MpcError, Result};
use bincode;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

/// NATS-based message relay for MPC protocol communication.
///
/// Messages are published to per-session NATS subjects:
///   cowallet.mpc.{session_id}.{to_party}
///
/// Each party subscribes to their own subject and receives messages
/// from other parties in the same session.
pub struct NatsRelay {
    client: async_nats::Client,
}

impl NatsRelay {
    /// Connect to the NATS server.
    pub async fn connect(url: &str) -> Result<Self> {
        let client = async_nats::connect(url)
            .await
            .map_err(|e| MpcError::Transport(format!("NATS connection failed: {}", e)))?;

        Ok(Self { client })
    }

    /// Publish a protocol message to the target party's subject.
    pub async fn publish(&self, msg: &ProtocolMessage) -> Result<()> {
        let subject = format!("cowallet.mpc.{}.{}", msg.session_id, msg.to);
        let payload = bincode::serialize(msg)
            .map_err(|e| MpcError::Transport(format!("serialization failed: {}", e)))?;

        self.client
            .publish(subject, payload.into())
            .await
            .map_err(|e| MpcError::Transport(format!("NATS publish failed: {}", e)))?;

        Ok(())
    }

    /// Subscribe to messages for this party in a session.
    ///
    /// Returns a receiver channel that yields incoming protocol messages.
    pub async fn subscribe(
        &self,
        session_id: &str,
        party_index: u16,
    ) -> Result<tokio::sync::mpsc::Receiver<ProtocolMessage>> {
        let subject = format!("cowallet.mpc.{}.{}", session_id, party_index);
        let mut subscriber = self
            .client
            .subscribe(subject)
            .await
            .map_err(|e| MpcError::Transport(format!("NATS subscribe failed: {}", e)))?;

        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                if let Ok(proto_msg) = bincode::deserialize::<ProtocolMessage>(&msg.payload) {
                    let _ = tx.send(proto_msg).await;
                }
            }
        });

        Ok(rx)
    }
}

/// In-memory transport for testing. Routes messages directly between
/// parties without network IO.
#[cfg(test)]
pub struct InMemoryTransport {
    queues: Arc<Mutex<HashMap<(String, u16), VecDeque<ProtocolMessage>>>>,
}

#[cfg(test)]
impl InMemoryTransport {
    /// Create a new in-memory transport.
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Publish a message to a specific party in a session.
    pub fn publish(&self, msg: ProtocolMessage) -> Result<()> {
        let mut queues = self.queues.lock().unwrap();
        let key = (msg.session_id.clone(), msg.to);
        queues.entry(key).or_insert_with(VecDeque::new).push_back(msg);
        Ok(())
    }

    /// Receive all messages for a party in a session.
    pub fn receive(&self, session_id: &str, party_index: u16) -> Result<Vec<ProtocolMessage>> {
        let mut queues = self.queues.lock().unwrap();
        let key = (session_id.to_string(), party_index);
        if let Some(queue) = queues.get_mut(&key) {
            Ok(queue.drain(..).collect())
        } else {
            Ok(Vec::new())
        }
    }

    /// Clone this transport (share the same underlying queues).
    pub fn clone_shared(&self) -> Self {
        Self {
            queues: Arc::clone(&self.queues),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dkls23::SessionConfig;

    #[test]
    fn test_in_memory_transport_send_receive() {
        let transport = InMemoryTransport::new();
        let session_id = "test-session".to_string();

        let msg = ProtocolMessage {
            session_id: session_id.clone(),
            from: 0,
            to: 1,
            round: 1,
            payload: vec![1, 2, 3, 4],
        };

        transport.publish(msg).unwrap();

        let received = transport.receive(&session_id, 1).unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].from, 0);
        assert_eq!(received[0].to, 1);
        assert_eq!(received[0].payload, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_in_memory_transport_empty_receive() {
        let transport = InMemoryTransport::new();
        let received = transport.receive("empty-session", 0).unwrap();
        assert!(received.is_empty());
    }

    #[test]
    fn test_in_memory_transport_multiparties() {
        let transport = InMemoryTransport::new();
        let session_id = "test-multi".to_string();

        // Party 0 sends to party 1
        transport
            .publish(ProtocolMessage {
                session_id: session_id.clone(),
                from: 0,
                to: 1,
                round: 1,
                payload: vec![1],
            })
            .unwrap();

        // Party 1 sends to party 0
        transport
            .publish(ProtocolMessage {
                session_id: session_id.clone(),
                from: 1,
                to: 0,
                round: 1,
                payload: vec![2],
            })
            .unwrap();

        // Party 0 receives
        let received0 = transport.receive(&session_id, 0).unwrap();
        assert_eq!(received0.len(), 1);
        assert_eq!(received0[0].payload, vec![2]);

        // Party 1 receives
        let received1 = transport.receive(&session_id, 1).unwrap();
        assert_eq!(received1.len(), 1);
        assert_eq!(received1[0].payload, vec![1]);
    }

    #[test]
    fn test_in_memory_transport_clone_shared() {
        let t1 = InMemoryTransport::new();
        let t2 = t1.clone_shared();

        let session_id = "test-shared".to_string();

        // Send via t1, receive via t2
        t1.publish(ProtocolMessage {
            session_id: session_id.clone(),
            from: 0,
            to: 1,
            round: 1,
            payload: vec![42],
        })
        .unwrap();

        let received = t2.receive(&session_id, 1).unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].payload, vec![42]);
    }
}
