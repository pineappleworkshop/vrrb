use std::{collections::HashSet, net::SocketAddr, result::Result as StdResult};

use async_trait::async_trait;
use bytes::Bytes;
use network::{
    message::{Message, MessageBody},
    network::BroadcastEngine,
};
use primitives::{NodeType, PeerId};
use storage::vrrbdb::VrrbDbReadHandle;
use telemetry::{error, info, warn};
use theater::{ActorLabel, ActorState, Handler};
use tokio::{
    sync::{
        broadcast::{
            error::{RecvError, TryRecvError},
            Receiver,
        },
        mpsc::{channel, Receiver as MpscReceiver, Sender},
    },
    task::JoinHandle,
};
use uuid::Uuid;
use vrrb_core::event_router::{DirectedEvent, Event};

use crate::{NodeError, Result, RuntimeModule, RuntimeModuleState};

pub struct BroadcastModuleConfig {
    pub events_tx: tokio::sync::mpsc::UnboundedSender<DirectedEvent>,
    pub node_type: NodeType,
    pub vrrbdb_read_handle: VrrbDbReadHandle,
    pub udp_gossip_address_port: u16,
    pub raptorq_gossip_address_port: u16,
    pub node_id: PeerId,
}

// TODO: rename to GossipNetworkModule
#[derive(Debug)]
pub struct BroadcastModule {
    id: Uuid,
    events_tx: tokio::sync::mpsc::UnboundedSender<DirectedEvent>,
    vrrbdb_read_handle: VrrbDbReadHandle,
    // broadcast_handle: JoinHandle<Result<()>>,
    addr: SocketAddr,
    // controller_rx: MpscReceiver<Event>,
    status: ActorState,
}

impl BroadcastModule {
    pub async fn new(config: BroadcastModuleConfig) -> Result<Self> {
        let broadcast_engine = BroadcastEngine::new(config.udp_gossip_address_port, 32)
            .await
            .map_err(|err| {
                NodeError::Other(format!("unable to setup broadcast engine: {}", err))
            })?;

        let addr = broadcast_engine.local_addr();

        Ok(Self {
            events_tx: config.events_tx,
            status: ActorState::Stopped,
            vrrbdb_read_handle: config.vrrbdb_read_handle,
            addr,
            // broadcast_handle,
            // controller_rx,
            id: Uuid::new_v4(),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn name(&self) -> String {
        "BroadcastModule".to_string()
    }
}

#[async_trait]
impl Handler<Event> for BroadcastModule {
    fn id(&self) -> theater::ActorId {
        self.id.to_string()
    }

    fn label(&self) -> ActorLabel {
        self.name()
    }

    fn status(&self) -> ActorState {
        self.status.clone()
    }

    fn set_status(&mut self, actor_status: ActorState) {
        self.status = actor_status;
    }

    async fn handle(&mut self, event: Event) -> theater::Result<ActorState> {
        if event == Event::Stop {
            info!("{0} received stop signal. Stopping", self.name());
            return Ok(ActorState::Terminating);
        }

        // do something with the event

        Ok(ActorState::Running)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::channel;

    use primitives::NodeType;
    use storage::vrrbdb::{VrrbDb, VrrbDbConfig};
    use tokio::sync::mpsc::unbounded_channel;
    use vrrb_core::event_router::Event;

    use super::{BroadcastModule, BroadcastModuleConfig};

    #[tokio::test]
    #[ignore]
    async fn test_broadcast_module() {
        let (internal_events_tx, mut internal_events_rx) = unbounded_channel();

        let node_id = uuid::Uuid::new_v4().to_string().into_bytes();

        let db_config = VrrbDbConfig::default();

        let db = VrrbDb::new(db_config);

        let vrrbdb_read_handle = db.read_handle();

        let config = BroadcastModuleConfig {
            events_tx: internal_events_tx,
            vrrbdb_read_handle,
            node_type: NodeType::Full,
            udp_gossip_address_port: 0,
            raptorq_gossip_address_port: 0,
            node_id,
        };

        let broadcast_module = BroadcastModule::new(config).await.unwrap();
    }
}
