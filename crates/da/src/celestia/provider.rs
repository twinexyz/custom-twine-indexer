use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64},
};

use celestia_rpc::{BlobClient, Client, HeaderClient};
use celestia_types::{Blob, nmt::Namespace};
use eyre::Result;
// use hex::encode as hex_encode;
use sea_orm::ActiveValue::Set;
use serde::Deserialize;
use serde_json::Value;

use crate::db::{DbClient, DbModel};

use super::parser::{CelestiaBlob, parse_blob};

#[derive(Debug, Deserialize)]
struct BlockInfo {
    number: u64,
    parent_hash: String,
}

pub struct CelestiaProvider {
    client: Client,
    namespace: Namespace,
    start_height: u64,

    // last_known_height: AtomicU64,
    // catch_up_completed: AtomicBool,
    db: Arc<DbClient>,
}

impl CelestiaProvider {
    pub async fn new(config: common::config::CelestiaConfig, db: Arc<DbClient>) -> Self {
        let client = Client::new(&config.wss_url, Some(&config.rpc_auth_token))
            .await
            .expect("Failed creating rpc client");

        let mut start_from = config.start_height;

        if start_from == 0 {
            start_from = 1;
        }

        let namespace =
            Namespace::new_v0(config.namespace.as_bytes()).expect("failed to build namespace");

        Self {
            client,
            namespace,
            db,
            start_height: start_from,
            // last_known_height: AtomicU64::new(start_from),
            // catch_up_completed: AtomicBool::new(false),
        }
    }

    pub async fn get_latest_height(&self) -> Result<u64> {
        let latest_height = self.client.header_local_head().await?.header.height.value();
        Ok(latest_height)
    }

    pub async fn get_blobs_by_height(&self, height: u64) -> Result<Option<Vec<Blob>>> {
        let namespace = self.namespace.clone();
        let namespaces = vec![namespace];
        let blobs = self.client.blob_get_all(height, &namespaces).await?;
        Ok(blobs)
    }

    pub async fn catch_missing_blocks(&self) {
        let head = match self.client.header_local_head().await {
            Ok(h) => h.header.height.value(),
            Err(e) => {
                tracing::error!("Failed get head: {:?}", e);
                return;
            }
        };

        let mut height = self.start_height;
        while height <= head {
            if let Ok(Some(blobs)) = self.get_blobs_by_height(height).await {
                tracing::info!("Found some blobs: {}", blobs.len());

                let parsed_blobs: Vec<CelestiaBlob> =
                    match blobs.into_iter().try_fold(Vec::new(), |mut acc, blob| {
                        let mut parsed: Vec<CelestiaBlob> =
                            parse_blob(height, &blob).map_err(|e| eyre::Report::from(e))?; // explicitly map error to eyre::Report
                        acc.append(&mut parsed); // move parsed items into acc
                        Ok::<_, eyre::Report>(acc) // explicitly specify the Result's error type
                    }) {
                        Ok(parsed) => parsed,
                        Err(e) => {
                            tracing::error!("Error parsing blobs: {:?}", e);
                            continue;
                        }
                    };

                for blb in parsed_blobs {
                    println!("Found blob: {}", blb.height.to_string());
                    println!("Commitment {}", blb.commitment_hash);
                }
            }
            height += 1;
        }
    }

    pub async fn subscribe_headers(&self) -> Result<()> {
        let mut header_sub = self
            .client
            .header_subscribe()
            .await
            .expect("Failed subscribing to incoming headers");

        while let Some(extended_header) = header_sub.next().await {
            match extended_header {
                Ok(header) => {
                    let height = header.header.height.value();
                    // fetch all blobs at the height of the new header

                    let blobs = match self.client.blob_get_all(height, &[self.namespace]).await {
                        Ok(Some(blobs)) => blobs,
                        Ok(None) => {
                            continue;
                        }
                        Err(e) => {
                            eprintln!("Error fetching blobs: {}", e);
                            continue;
                        }
                    };

                    let parsed_blobs: Vec<CelestiaBlob> =
                        blobs.into_iter().try_fold(Vec::new(), |mut acc, blob| {
                            // parse_blob(blob) -> Result<Vec<ParsedBlob>, E>
                            let mut parsed: Vec<CelestiaBlob> =
                                parse_blob(height, &blob).map_err(|e| eyre::Report::from(e))?; // explicitly map error to eyre::Report
                            acc.append(&mut parsed); // move parsed items into acc
                            Ok::<_, eyre::Report>(acc) // explicitly specify the Result's error type
                        })?;
                }
                Err(e) => {
                    eprintln!("Error receiving header: {}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn run_indexer(&self) -> Result<()> {
        tracing::info!("Started DA Indexer");

        let latest_height = self.get_latest_height().await?;
        tracing::info!("Celestia Latest height: {}", latest_height);

        // Additional indexing logic can be added here

        let catch_up = self.catch_missing_blocks();

        tokio::join!(catch_up);

        Ok(())
    }
}
