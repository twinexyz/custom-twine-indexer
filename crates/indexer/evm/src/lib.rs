mod common;
mod ethereum;
mod twine;

pub use ethereum::EthereumIndexer;
pub use twine::TwineIndexer;

use common::{create_http_provider, create_ws_provider, poll_missing_logs, subscribe_stream};

pub enum EVMChain {
    Ethereum,
    Twine,
}

impl EVMChain {
    pub fn get_event_signatures(&self) -> &'static [&'static str] {
        match self {
            EVMChain::Ethereum => &ethereum::ETHEREUM_EVENT_SIGNATURES,
            EVMChain::Twine => &twine::TWINE_EVENT_SIGNATURES,
        }
    }
}

impl std::fmt::Display for EVMChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EVMChain::Ethereum => write!(f, "ethereum"),
            EVMChain::Twine => write!(f, "twine"),
        }
    }
}
