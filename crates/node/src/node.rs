use std::{io::Read, net::SocketAddr, path::PathBuf, sync::mpsc::channel, thread};

use crossbeam_channel::unbounded;
use mempool::{LeftRightMempool, MempoolReadHandleFactory};
use network::{message::Message, network::BroadcastEngine, packet, packet::RaptorBroadCastedData};
use primitives::{NodeIdentifier, NodeIdx, PublicKey, SecretKey};
use storage::{
    storage_utils,
    vrrbdb::{VrrbDbConfig, VrrbDbReadHandle},
};
use telemetry::info;
use theater::{Actor, ActorImpl};
use tokio::{
    sync::{
        broadcast::Receiver,
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    },
    task::JoinHandle,
};
use trecho::vm::Cpu;
use vrrb_config::NodeConfig;
use vrrb_core::{
    event_router::{DirectedEvent, Event, EventRouter, Topic},
    keypair::KeyPair,
    txn::Txn,
};
use vrrb_rpc::{
    http::HttpApiServerConfig,
    rpc::{JsonRpcServer, JsonRpcServerConfig},
};

use crate::{
    broadcast_controller::{BroadcastEngineController, BROADCAST_CONTROLLER_BUFFER_SIZE},
    broadcast_module::{BroadcastModule, BroadcastModuleConfig},
    mempool_module::{MempoolModule, MempoolModuleConfig},
    mining_module,
    result::{NodeError, Result},
    runtime::setup_runtime_components,
    validator_module,
    NodeType,
    RuntimeModule,
    RuntimeModuleState,
};

/// Node represents a member of the VRRB network and it is responsible for
/// carrying out the different operations permitted within the chain.
#[derive(Debug)]
pub struct Node {
    config: NodeConfig,

    // NOTE: core node features
    event_router_handle: JoinHandle<()>,
    running_status: RuntimeModuleState,
    control_rx: UnboundedReceiver<Event>,
    events_tx: UnboundedSender<DirectedEvent>,

    // TODO: make this private
    pub keypair: KeyPair,

    // NOTE: optional node components
    vm: Option<Cpu>,
    state_handle: Option<JoinHandle<Result<()>>>,
    mempool_handle: Option<JoinHandle<Result<()>>>,
    gossip_handle: Option<JoinHandle<Result<()>>>,
    broadcast_controller_handle: Option<JoinHandle<Result<()>>>,
    miner_handle: Option<JoinHandle<Result<()>>>,
    txn_validator_handle: Option<JoinHandle<Result<()>>>,
    jsonrpc_server_handle: Option<JoinHandle<Result<()>>>,
    http_server_handle: Option<JoinHandle<Result<()>>>,
}

impl Node {
    /// Initializes and returns a new Node instance
    pub async fn start(config: &NodeConfig, control_rx: UnboundedReceiver<Event>) -> Result<Self> {
        // Copy the original config to avoid overriding the original
        let mut config = config.clone();


        let vm = None;
        let keypair = config.keypair.clone();

        let (events_tx, mut events_rx) = unbounded_channel::<DirectedEvent>();
        let mut event_router = Self::setup_event_routing_system();

        let mempool_events_rx = event_router.subscribe(&Topic::Storage)?;
        let vrrbdb_events_rx = event_router.subscribe(&Topic::Storage)?;
        let network_events_rx = event_router.subscribe(&Topic::Network)?;
        let controller_events_rx = event_router.subscribe(&Topic::Network)?;
        let validator_events_rx = event_router.subscribe(&Topic::Consensus)?;
        let miner_events_rx = event_router.subscribe(&Topic::Consensus)?;
        let jsonrpc_events_rx = event_router.subscribe(&Topic::Control)?;
        let http_events_rx = event_router.subscribe(&Topic::Control)?;

        let (
            updated_config,
            mempool_handle,
            state_handle,
            gossip_handle,
            broadcast_controller_handle,
            jsonrpc_server_handle,
            txn_validator_handle,
            miner_handle,
            http_server_handle,
        ) = setup_runtime_components(
            &config,
            events_tx.clone(),
            mempool_events_rx,
            vrrbdb_events_rx,
            network_events_rx,
            controller_events_rx,
            validator_events_rx,
            miner_events_rx,
            jsonrpc_events_rx,
            http_events_rx,
        )
        .await?;

        config = updated_config;

        // TODO: report error from handle
        let event_router_handle =
            tokio::spawn(async move { event_router.start(&mut events_rx).await });

        Ok(Self {
            config,
            vm,
            event_router_handle,
            state_handle,
            mempool_handle,
            jsonrpc_server_handle,
            gossip_handle,
            broadcast_controller_handle,
            running_status: RuntimeModuleState::Stopped,
            control_rx,
            events_tx,
            txn_validator_handle,
            miner_handle,
            keypair,
            http_server_handle,
        })
    }

    pub async fn wait(mut self) -> anyhow::Result<()> {
        // TODO: notify bootstrap nodes that this node is joining the network so they
        // can add it to their peer list

        self.running_status = RuntimeModuleState::Running;

        // NOTE: wait for stop signal
        self.control_rx
            .recv()
            .await
            .ok_or_else(|| NodeError::Other(String::from("failed to receive control signal")))?;

        info!("node received stop signal");

        self.events_tx.send((Topic::Control, Event::Stop))?;

        if let Some(handle) = self.state_handle {
            handle.await??;
            info!("shutdown complete for state management module ");
        }

        if let Some(handle) = self.miner_handle {
            handle.await??;
            info!("shutdown complete for mining module ");
        }

        if let Some(handle) = self.gossip_handle {
            handle.await??;
            info!("shutdown complete for gossip module");
        }

        if let Some(handle) = self.txn_validator_handle {
            handle.await??;
            info!("shutdown complete for mining module ");
        }

        if let Some(handle) = self.jsonrpc_server_handle {
            handle.await??;
            info!("rpc server shut down");
        }

        if let Some(handle) = self.http_server_handle {
            handle.await??;
            info!("http server shut down");
        }

        self.event_router_handle.await?;

        info!("node shutdown complete");

        self.running_status = RuntimeModuleState::Stopped;

        Ok(())
    }

    pub async fn config(&self) -> NodeConfig {
        self.config.clone()
    }

    /// Returns a string representation of the Node id
    pub fn id(&self) -> String {
        self.config.id.clone()
    }

    /// Returns the idx of the Node
    pub fn node_idx(&self) -> u16 {
        self.config.idx
    }

    #[deprecated(note = "use node_idx instead")]
    pub fn get_node_idx(&self) -> u16 {
        self.node_idx()
    }

    /// Returns the node's type
    pub fn node_type(&self) -> NodeType {
        self.config.node_type
    }

    #[deprecated(note = "use node_type instead")]
    pub fn get_node_type(&self) -> NodeType {
        self.node_type()
    }

    pub fn is_bootsrap(&self) -> bool {
        matches!(self.node_type(), NodeType::Bootstrap)
    }

    pub fn status(&self) -> RuntimeModuleState {
        self.running_status.clone()
    }

    pub fn keypair(&self) -> KeyPair {
        self.keypair.clone()
    }

    pub fn udp_gossip_address(&self) -> SocketAddr {
        self.config.udp_gossip_address
    }

    pub fn raprtorq_gossip_address(&self) -> SocketAddr {
        self.config.raptorq_gossip_address
    }

    pub fn bootstrap_node_addresses(&self) -> Vec<SocketAddr> {
        self.config.bootstrap_node_addresses.clone()
    }

    pub fn jsonrpc_server_address(&self) -> SocketAddr {
        self.config.jsonrpc_server_address
    }

    fn setup_event_routing_system() -> EventRouter {
        let mut event_router = EventRouter::new();
        event_router.add_topic(Topic::Control, Some(1));
        event_router.add_topic(Topic::State, Some(1));
        event_router.add_topic(Topic::Network, Some(100));
        event_router.add_topic(Topic::Consensus, Some(100));
        event_router.add_topic(Topic::Storage, Some(100));

        event_router
    }
}
