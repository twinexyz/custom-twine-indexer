use crate::indexer::svm::parser::DepositInfoResponse;
use eyre::{self, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{error, info};
#[derive(Debug, Serialize, Deserialize)]
pub struct DepositResponse {
    status: String,
    data: Vec<DepositInfoResponse>,
}

pub async fn poll_deposits() -> Result<Vec<DepositInfoResponse>> {
    let output = Command::new("./svm_subscriber")
        .arg("--get-deposits")
        .output()
        .map_err(|e| eyre::eyre!("Failed to execute svm_subscriber: {:?}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let response: DepositResponse = serde_json::from_str(&stdout)?;
        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(eyre::eyre!("svm_subscriber returned error: {:?}", response))
        }
    } else {
        let stderr = String::from_utf8(output.stderr)?;
        Err(eyre::eyre!("svm_subscriber failed: {:?}", stderr))
    }
}

pub async fn start_polling() -> Result<Vec<DepositInfoResponse>> {
    info!("Polling for new deposits...");
    match poll_deposits().await {
        Ok(deposits) => {
            if deposits.is_empty() {
                info!("No new deposits found.");
            } else {
                for deposit in &deposits {
                    info!("New deposit: {:?}", deposit);
                }
            }
            Ok(deposits)
        }
        Err(e) => {
            error!("Failed to poll deposits: {:?}", e);
            Err(e)
        }
    }
}
