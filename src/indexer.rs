use alloy::{
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::types::Filter,
    sol_types::SolEvent,
};
use eyre::Result;
use futures_util::StreamExt;
use twine_evm_contracts::evm::ethereum::l1_message_queue::L1MessageQueue;
use twine_evm_contracts::evm::twine::l2_messenger::L2Messenger;
use tracing::info;

pub async fn run_indexer(rpc_url: String) -> Result<()> {
    let ws = WsConnect::new(&rpc_url);
    let provider = ProviderBuilder::new().on_ws(ws).await?;
    let chain_id = provider.get_chain_id().await?;
    info!("Connected to blockchain. Chain ID: {chain_id}");

    let filter = Filter::new(); // TODO: Filter contract addresses
    let subscription = provider.subscribe_logs(&filter).await?;
    let mut stream = subscription.into_stream();
    info!("Subscribed to log stream. Listening for events...");

    while let Some(log) = stream.next().await {
        match log.topic0() {
            Some(&L2Messenger::SentMessage::SIGNATURE_HASH) => {
                info!("L2Messenger SentMessage event received");
            }
            Some(&L1MessageQueue::QueueDepositTransaction::SIGNATURE_HASH) => {
                info!("L2Messenger SentMessage event received");
            }
            Some(&L1MessageQueue::QueueWithdrawalTransaction::SIGNATURE_HASH) => {
                info!("L2Messenger SentMessage event received");
            }
            _ => {}
        }
    }

    Ok(())
}
