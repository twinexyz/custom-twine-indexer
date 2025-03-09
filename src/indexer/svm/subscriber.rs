use crate::indexer::svm::parser::{DepositInfoResponse, WithdrawInfoResponse}; // Import WithdrawInfoResponse
use eyre::{self, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositResponse {
    status: String,
    data: Vec<DepositInfoResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WithdrawResponse {
    status: String,
    data: Vec<WithdrawInfoResponse>,
}

pub async fn poll_deposits() -> Result<Vec<DepositInfoResponse>> {
    let output = Command::new("./svm_subscriber")
        .arg("--get-deposits")
        .output()
        .map_err(|e| eyre::eyre!("Failed to execute svm_subscriber for deposits: {:?}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let response: DepositResponse = serde_json::from_str(&stdout)?;
        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(eyre::eyre!(
                "svm_subscriber returned error for deposits: {:?}",
                response
            ))
        }
    } else {
        let stderr = String::from_utf8(output.stderr)?;
        Err(eyre::eyre!(
            "svm_subscriber failed for deposits: {:?}",
            stderr
        ))
    }
}

pub async fn poll_withdrawals() -> Result<Vec<WithdrawInfoResponse>> {
    let output = Command::new("./svm_subscriber")
        .arg("--get-withdrawals") // Assuming this flag exists
        .output()
        .map_err(|e| eyre::eyre!("Failed to execute svm_subscriber for withdrawals: {:?}", e))?;

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout)?;
        let response: WithdrawResponse = serde_json::from_str(&stdout)?;
        if response.status == "success" {
            Ok(response.data)
        } else {
            Err(eyre::eyre!(
                "svm_subscriber returned error for withdrawals: {:?}",
                response
            ))
        }
    } else {
        let stderr = String::from_utf8(output.stderr)?;
        Err(eyre::eyre!(
            "svm_subscriber failed for withdrawals: {:?}",
            stderr
        ))
    }
}

pub async fn start_polling() -> Result<(Vec<DepositInfoResponse>, Vec<WithdrawInfoResponse>)> {
    info!("Polling for new deposits and withdrawals...");

    let deposit_result = poll_deposits().await;
    let withdraw_result = poll_withdrawals().await;

    match (deposit_result, withdraw_result) {
        (Ok(deposits), Ok(withdrawals)) => {
            if deposits.is_empty() {
                info!("No new deposits found.");
            } else {
                for deposit in &deposits {
                    info!("New deposit: {:?}", deposit);
                }
            }

            if withdrawals.is_empty() {
                info!("No new withdrawals found.");
            } else {
                for withdraw in &withdrawals {
                    info!("New withdrawal: {:?}", withdraw);
                }
            }

            Ok((deposits, withdrawals))
        }
        (Err(deposit_err), Ok(_)) => {
            error!("Failed to poll deposits: {:?}", deposit_err);
            Err(deposit_err)
        }
        (Ok(_), Err(withdraw_err)) => {
            error!("Failed to poll withdrawals: {:?}", withdraw_err);
            Err(withdraw_err)
        }
        (Err(deposit_err), Err(withdraw_err)) => {
            error!("Failed to poll deposits: {:?}", deposit_err);
            error!("Failed to poll withdrawals: {:?}", withdraw_err);
            Err(deposit_err)
        }
    }
}
