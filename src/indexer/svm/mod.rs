mod db;
mod parser;
mod subscriber;
mod utils;

use super::ChainIndexer;
use async_trait::async_trait;
use eyre::{eyre, Result};
use sea_orm::DatabaseConnection;
use tokio::time::{self, Duration};
use tracing::info;

pub struct SVMIndexer {
    db: DatabaseConnection,
}

#[async_trait]
impl ChainIndexer for SVMIndexer {
    async fn new(rpc_url: String, db: &DatabaseConnection) -> eyre::Result<Self> {
        Ok(Self { db: db.clone() })
    }

    async fn run(&self) -> Result<()> {
        info!("SVM indexer running...");
        let mut interval = time::interval(Duration::from_secs(10));

        loop {
            interval.tick().await;

            // Fetch latest nonces for all tables
            let latest_native_deposit_nonce =
                db::get_latest_native_token_deposit_nonce(&self.db).await?;
            info!(
                "Latest native deposit nonce in DB: {:?}",
                latest_native_deposit_nonce
            );

            let latest_spl_deposit_nonce = db::get_latest_spl_token_deposit_nonce(&self.db).await?;
            info!(
                "Latest SPL deposit nonce in DB: {:?}",
                latest_spl_deposit_nonce
            );

            let latest_native_withdraw_nonce =
                db::get_latest_native_token_withdraw_nonce(&self.db).await?;
            info!(
                "Latest native withdraw nonce in DB: {:?}",
                latest_native_withdraw_nonce
            );

            let latest_spl_withdraw_nonce =
                db::get_latest_spl_token_withdraw_nonce(&self.db).await?;
            info!(
                "Latest SPL withdraw nonce in DB: {:?}",
                latest_spl_withdraw_nonce
            );

            let (deposits, withdrawals) = subscriber::start_polling().await?;
            info!("SVM subscriber retrieved deposits: {:?}", deposits);
            info!("SVM subscriber retrieved withdrawals: {:?}", withdrawals);

            // Process deposits
            for deposit in deposits {
                let nonce = deposit.deposit_message.nonce;
                let is_native =
                    deposit.deposit_message.l1_token == "11111111111111111111111111111111";

                if is_native {
                    if let Some(latest) = latest_native_deposit_nonce {
                        if nonce <= latest {
                            info!(
                                "Skipping native deposit with nonce {} (not newer than {})",
                                nonce, latest
                            );
                            continue;
                        }
                    }
                    db::insert_sol_deposit(&deposit, &self.db).await?;
                    info!("Inserted native deposit with nonce: {}", nonce);
                } else {
                    if let Some(latest) = latest_spl_deposit_nonce {
                        if nonce <= latest {
                            info!(
                                "Skipping SPL deposit with nonce {} (not newer than {})",
                                nonce, latest
                            );
                            continue;
                        }
                    }
                    db::insert_spl_deposit(&deposit, &self.db).await?;
                    info!("Inserted SPL deposit with nonce: {}", nonce);
                }
            }

            // Process withdrawals
            for withdraw in withdrawals {
                let nonce = withdraw.withdraw_message.nonce;
                let is_native =
                    withdraw.withdraw_message.l1_token == "11111111111111111111111111111111";

                if is_native {
                    if let Some(latest) = latest_native_withdraw_nonce {
                        let latest_u64 = latest.try_into().map_err(|e| {
                            eyre!(
                                "Failed to convert latest native withdraw nonce to u64: {:?}",
                                e
                            )
                        })?;
                        if nonce <= latest_u64 {
                            info!(
                                "Skipping native withdraw with nonce {} (not newer than {})",
                                nonce, latest
                            );
                            continue;
                        }
                    }
                    db::insert_native_withdrawal(&withdraw, &self.db).await?;
                    info!("Inserted native withdraw with nonce: {}", nonce);
                } else {
                    if let Some(latest) = latest_spl_withdraw_nonce {
                        let latest_u64 = latest.try_into().map_err(|e| {
                            eyre!(
                                "Failed to convert latest SPL withdraw nonce to u64: {:?}",
                                e
                            )
                        })?;
                        if nonce <= latest_u64 {
                            info!(
                                "Skipping SPL withdraw with nonce {} (not newer than {})",
                                nonce, latest
                            );
                            continue;
                        }
                    }
                    db::insert_spl_withdrawal(&withdraw, &self.db).await?;
                    info!("Inserted SPL withdraw with nonce: {}", nonce);
                }
            }
        }
    }

    async fn chain_id(&self) -> Result<u64> {
        Ok(900)
    }
}
