//! Cardano node data source implementation

use crate::Result;
use std::path::PathBuf;

/// Cardano node client
pub struct NodeDataSource {
    _socket_path: PathBuf,
    _network_magic: Option<u32>,
}

impl NodeDataSource {
    pub fn new(socket_path: PathBuf, network_magic: Option<u32>) -> Result<Self> {
        if !socket_path.exists() {
            return Err(crate::Error::Config(format!(
                "Node socket path does not exist: {:?}",
                socket_path
            )));
        }
        Ok(Self {
            _socket_path: socket_path,
            _network_magic: network_magic,
        })
    }
}

// TODO: implement
