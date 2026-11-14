use std::collections::HashMap;

use anyhow::{Context, Result};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;
use uuid::Uuid;

use fenrir_core::engine::broker::{BrokerHandle, ClientSender};
use fenrir_core::engine::protocol::{ClientRequest, ServerEvent};

#[derive(Clone)]
pub struct ConnectionsHandle {
  requests: Sender<ConnectionsRequest>,
}

enum ConnectionsRequest {
  Create {
    cwd: String,
    reply: oneshot::Sender<Result<Uuid>>,
  },
  Sender {
    client_id: Uuid,
    reply: oneshot::Sender<Option<ClientSender>>,
  },
  TakeEvents {
    client_id: Uuid,
    reply: oneshot::Sender<EventsReply>,
  },
  Disconnect {
    client_id: Uuid,
  },
}

pub enum EventsReply {
  Missing,
  Taken(Receiver<ServerEvent>),
  AlreadyOpen,
}

struct Connection {
  sender: ClientSender,
  events: Option<Receiver<ServerEvent>>,
}

struct Connections {
  broker: BrokerHandle,
  connections: HashMap<Uuid, Connection>,
  inbox: Receiver<ConnectionsRequest>,
}

impl ConnectionsHandle {
  pub fn spawn(broker: BrokerHandle) -> Self {
    let (requests, inbox) = mpsc::channel(256);
    let actor = Connections {
      broker,
      connections: HashMap::new(),
      inbox,
    };
    tokio::spawn(actor.run());
    return Self { requests };
  }

  pub async fn create(&self, cwd: String) -> Result<Uuid> {
    let (reply, result) = oneshot::channel();
    self
      .requests
      .send(ConnectionsRequest::Create { cwd, reply })
      .await
      .context("Connections actor is not running")?;
    return result.await.context("Connections actor did not reply")?;
  }

  pub async fn sender(&self, client_id: Uuid) -> Result<Option<ClientSender>> {
    let (reply, result) = oneshot::channel();
    self
      .requests
      .send(ConnectionsRequest::Sender { client_id, reply })
      .await
      .context("Connections actor is not running")?;
    return result.await.context("Connections actor did not reply");
  }

  pub async fn take_events(&self, client_id: Uuid) -> Result<EventsReply> {
    let (reply, result) = oneshot::channel();
    self
      .requests
      .send(ConnectionsRequest::TakeEvents { client_id, reply })
      .await
      .context("Connections actor is not running")?;
    return result.await.context("Connections actor did not reply");
  }

  pub async fn disconnect(&self, client_id: Uuid) -> Result<()> {
    return self
      .requests
      .send(ConnectionsRequest::Disconnect { client_id })
      .await
      .context("Connections actor is not running");
  }
}

impl Connections {
  async fn run(mut self) {
    while let Some(request) = self.inbox.recv().await {
      self.handle(request).await;
    }
    let ids: Vec<Uuid> = self.connections.keys().copied().collect();
    for client_id in ids {
      self.disconnect(client_id).await;
    }
  }

  async fn handle(&mut self, request: ConnectionsRequest) {
    match request {
      ConnectionsRequest::Create { cwd, reply } => {
        let result = self.create(cwd).await;
        let _ = reply.send(result);
      }
      ConnectionsRequest::Sender { client_id, reply } => {
        let sender = self
          .connections
          .get(&client_id)
          .map(|connection| connection.sender.clone());
        let _ = reply.send(sender);
      }
      ConnectionsRequest::TakeEvents { client_id, reply } => {
        let events = match self.connections.get_mut(&client_id) {
          Some(connection) => match connection.events.take() {
            Some(events) => EventsReply::Taken(events),
            None => EventsReply::AlreadyOpen,
          },
          None => EventsReply::Missing,
        };
        let _ = reply.send(events);
      }
      ConnectionsRequest::Disconnect { client_id } => {
        self.disconnect(client_id).await;
      }
    }
  }

  async fn create(&mut self, cwd: String) -> Result<Uuid> {
    let handle = self.broker.connect(&cwd).await?;
    let (sender, events) = handle.into_parts();
    let client_id = sender.id;
    self.connections.insert(
      client_id,
      Connection {
        sender,
        events: Some(events),
      },
    );
    return Ok(client_id);
  }

  async fn disconnect(&mut self, client_id: Uuid) {
    let Some(connection) = self.connections.remove(&client_id) else {
      return;
    };
    let _ = connection.sender.send(ClientRequest::Disconnect).await;
  }
}
