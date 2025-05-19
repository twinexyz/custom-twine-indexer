pub mod evm;

pub trait ChainProvider {
    type Height;
    type Transaction;

    async fn get_latest_height() -> eyre::Result<Self::Height>;
}
