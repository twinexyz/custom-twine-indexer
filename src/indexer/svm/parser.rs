use alloy::hex;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use eyre::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct DepositSuccessful {
    pub nonce: u64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String, // Excluded from Borsh serialization/deserialization
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ForcedWithdrawSuccessful {
    pub nonce: u64,
    pub from_twine_address: String,
    pub to_l1_pub_key: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizeNativeWithdrawal {
    pub nonce: u64,
    pub receiver_l1_pubkey: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizeSplWithdrawal {
    pub nonce: u64,
    pub receiver_l1_pubkey: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug)]
pub struct ParsedEvent {
    pub event: serde_json::Value,
    pub slot_number: i64,
}

pub trait HasSignature {
    fn set_signature(&mut self, signature: String);
}

impl HasSignature for DepositSuccessful {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for ForcedWithdrawSuccessful {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizeNativeWithdrawal {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizeSplWithdrawal {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

pub fn parse_borsh<T: BorshDeserialize + HasSignature>(
    encoded_data: &str,
    signature: Option<String>,
) -> Result<T> {
    debug!("Parsing Borsh data: encoded_data = {}", encoded_data);

    let decoded_data = general_purpose::STANDARD
        .decode(encoded_data)
        .map_err(|e| eyre::eyre!("Failed to decode base64: {}", e))?;

    debug!(
        "Decoded data (hex): {:?}, length: {}",
        hex::encode(&decoded_data),
        decoded_data.len()
    );

    let data_with_discriminator = if decoded_data.len() >= 8 {
        &decoded_data[8..]
    } else {
        &decoded_data[..]
    };

    debug!(
        "Attempting deserialization with discriminator skipped: data (hex) = {:?}, length = {}",
        hex::encode(data_with_discriminator),
        data_with_discriminator.len()
    );

    let mut event = match T::try_from_slice(data_with_discriminator) {
        Ok(event) => event,
        Err(e) => {
            debug!(
                "Failed with discriminator: error = {}, data (hex) = {:?}, length = {}",
                e,
                hex::encode(data_with_discriminator),
                data_with_discriminator.len()
            );
            if e.to_string().contains("Unexpected length") {
                error!(
                    "Length mismatch after skipping discriminator: data (hex) = {:?}, length = {}",
                    hex::encode(data_with_discriminator),
                    data_with_discriminator.len()
                );
                return Err(eyre::eyre!("Failed to deserialize Borsh data: {}", e));
            }
            debug!("Retrying without skipping discriminator");
            T::try_from_slice(&decoded_data).map_err(|e| {
                error!(
                    "Deserialization failed: error = {}, data (hex) = {:?}, length = {}",
                    e,
                    hex::encode(&decoded_data),
                    decoded_data.len()
                );
                eyre::eyre!("Failed to deserialize Borsh data: {}", e)
            })?
        }
    };

    if let Some(sig) = signature {
        event.set_signature(sig);
    } else {
        event.set_signature(String::new()); // Ensure signature is initialized
    }

    Ok(event)
}

pub fn parse_log(logs: &[String], signature: Option<String>) -> Result<ParsedEvent> {
    debug!("Parsing logs: {:?}", logs);

    let mut event_type = None;
    let mut encoded_data = None;

    for log in logs {
        if log == "Program log: Instruction: NativeTokenDeposit" {
            event_type = Some("native_deposit");
        } else if log == "Program log: Instruction: SplTokensDeposit" {
            event_type = Some("spl_deposit");
        } else if log == "Program log: Instruction: ForcedNativeTokenWithdrawal" {
            event_type = Some("native_withdrawal");
        } else if log == "Program log: Instruction: ForcedSplTokenWithdrawal" {
            event_type = Some("spl_withdrawal");
        } else if log == "Program log: Instruction: FinalizeNativeWithdrawal" {
            event_type = Some("finalize_native_withdrawal");
        } else if log == "Program log: Instruction: FinalizeSplWithdrawal" {
            event_type = Some("finalize_spl_withdrawal");
        }

        if log.starts_with("Program data: ") {
            encoded_data = Some(log.trim_start_matches("Program data: ").to_string());
        }
    }

    let Some(event_type) = event_type else {
        error!("No recognizable event type found in logs: {:?}", logs);
        return Err(eyre::eyre!(
            "No recognizable event type found in logs: {:?}",
            logs
        ));
    };
    let Some(encoded_data) = encoded_data else {
        error!("No encoded data found in logs: {:?}", logs);
        return Err(eyre::eyre!("No encoded data found in logs: {:?}", logs));
    };

    debug!(
        "Identified event_type: {}, encoded_data: {}",
        event_type, encoded_data
    );

    let (event, slot_number) = match event_type {
        "native_deposit" | "spl_deposit" => {
            let deposit = parse_borsh::<DepositSuccessful>(&encoded_data, signature.clone())?;
            (serde_json::to_value(&deposit)?, deposit.slot_number as i64)
        }
        "native_withdrawal" | "spl_withdrawal" => {
            let withdrawal =
                parse_borsh::<ForcedWithdrawSuccessful>(&encoded_data, signature.clone())?;
            (
                serde_json::to_value(&withdrawal)?,
                withdrawal.slot_number as i64,
            )
        }
        "finalize_native_withdrawal" => {
            let native = parse_borsh::<FinalizeNativeWithdrawal>(&encoded_data, signature.clone())?;
            (serde_json::to_value(&native)?, native.slot_number as i64)
        }
        "finalize_spl_withdrawal" => {
            let spl = parse_borsh::<FinalizeSplWithdrawal>(&encoded_data, signature.clone())?;
            (serde_json::to_value(&spl)?, spl.slot_number as i64)
        }
        _ => unreachable!("All event types should be handled above"),
    };

    Ok(ParsedEvent { event, slot_number })
}
