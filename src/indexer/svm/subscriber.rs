use eyre::{self, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositResponse {
    status: String,
    data: Vec<DepositInfoResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositInfoResponse {
    deposit_count: u64,
    deposit_message: DepositMessage,
    timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositMessage {
    nonce: u64,
    chain_id: u64,
    slot_number: u64,
    from_l1_pubkey: String,
    to_twine_address: String,
    l1_token: String,
    l2_token: String,
    amount: String,
}

pub async fn poll_deposits() -> Result<Vec<DepositInfoResponse>> {
    // Adjust the path to svm_subscriber binary based on your setup
    let output = Command::new("./svm_subscriber") // Use absolute path or ensure it's in PATH
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
