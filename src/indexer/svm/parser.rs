// parser.rs
use alloy::hex;
use base64::{engine::general_purpose, Engine as _};
use borsh::{BorshDeserialize, BorshSerialize};
use eyre::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

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
    pub signature: String,
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

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct CommitBatch {
    pub start_block: u64,
    pub end_block: u64,
    pub chain_id: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedBatch {
    pub start_block: u64,
    pub end_block: u64,
    pub batch_hash: String,
    pub chain_id: u64,
    #[borsh(skip)]
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct FinalizedTransaction {
    pub start_block: u64,
    pub end_block: u64,
    pub deposit_count: u64,
    pub withdraw_count: u64,
    pub chain_id: u64,
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

impl HasSignature for CommitBatch {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizedBatch {
    fn set_signature(&mut self, signature: String) {
        self.signature = signature;
    }
}

impl HasSignature for FinalizedTransaction {
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
            debug!("Retrying without skipping discriminator");
            T::try_from_slice(&decoded_data)
                .map_err(|e| eyre::eyre!("Failed to deserialize Borsh data: {}", e))?
        }
    };

    if let Some(sig) = signature {
        event.set_signature(sig);
    } else {
        event.set_signature(String::new());
    }

    Ok(event)
}

pub fn parse_log(logs: &[String], signature: Option<String>) -> Option<ParsedEvent> {
    debug!("Parsing logs: {:?}", logs);

    let mut event_type = None;
    let mut encoded_data = None;

    // Identify event type and encoded data from logs
    for log in logs {
        match log.as_str() {
            "Program log: Instruction: NativeTokenDeposit" => event_type = Some("native_deposit"),
            "Program log: Instruction: SplTokensDeposit" => event_type = Some("spl_deposit"),
            "Program log: Instruction: ForcedNativeTokenWithdrawal" => {
                event_type = Some("native_withdrawal")
            }
            "Program log: Instruction: ForcedSplTokenWithdrawal" => {
                event_type = Some("spl_withdrawal")
            }
            "Program log: Instruction: FinalizeNativeWithdrawal" => {
                event_type = Some("finalize_native_withdrawal")
            }
            "Program log: Instruction: FinalizeSplWithdrawal" => {
                event_type = Some("finalize_spl_withdrawal")
            }
            "Program log: Instruction: CommitBatch" => event_type = Some("commit_batch"),
            log if log.starts_with("Program data: ") => {
                encoded_data = Some(log.trim_start_matches("Program data: ").to_string());
            }
            "Program log: Instruction: FinalizedBatch" => event_type = Some("finalize_batch"),
            log if log.starts_with("Program data: ") => {
                encoded_data = Some(log.trim_start_matches("Program data: ").to_string());
            }
            "Program log: Instruction: CommitAndFinalizeTransaction" => {
                event_type = Some("commit_and_finalize_transaction")
            }
            log if log.starts_with("Program data: ") => {
                encoded_data = Some(log.trim_start_matches("Program data: ").to_string());
            }
            _ => continue,
        }
    }

    // If no recognized event type is found, skip processing
    let Some(event_type) = event_type else {
        return None;
    };

    // If no encoded data is found, skip processing
    let Some(encoded_data) = encoded_data else {
        info!(
            "No encoded data found for event {} in logs: {:?}",
            event_type, logs
        );
        return None;
    };

    debug!(
        "Identified event_type: {}, encoded_data: {}",
        event_type, encoded_data
    );

    // Parse the specified event types
    let parsed_event = match event_type {
        "native_deposit" | "spl_deposit" => {
            let deposit =
                parse_borsh::<DepositSuccessful>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&deposit).ok()?;
            Some(ParsedEvent {
                event,
                slot_number: deposit.slot_number as i64,
            })
        }
        "native_withdrawal" | "spl_withdrawal" => {
            let withdrawal =
                parse_borsh::<ForcedWithdrawSuccessful>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&withdrawal).ok()?;
            Some(ParsedEvent {
                event,
                slot_number: withdrawal.slot_number as i64,
            })
        }
        "finalize_native_withdrawal" => {
            let native =
                parse_borsh::<FinalizeNativeWithdrawal>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&native).ok()?;
            Some(ParsedEvent {
                event,
                slot_number: native.slot_number as i64,
            })
        }
        "finalize_spl_withdrawal" => {
            let spl =
                parse_borsh::<FinalizeSplWithdrawal>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&spl).ok()?;
            Some(ParsedEvent {
                event,
                slot_number: spl.slot_number as i64,
            })
        }
        "commit_batch" => {
            let commit = parse_borsh::<CommitBatch>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&commit).ok()?;
            let slot_number = 1;
            Some(ParsedEvent { event, slot_number })
        }
        "finalize_batch" => {
            let finalize = parse_borsh::<FinalizedBatch>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&finalize).ok()?;
            let slot_number = 1;
            Some(ParsedEvent { event, slot_number })
        }
        "commit_and_finalize_transaction" => {
            let commit_finalize_transaction =
                parse_borsh::<FinalizedTransaction>(&encoded_data, signature.clone()).ok()?;
            let event = serde_json::to_value(&commit_finalize_transaction).ok()?;
            let slot_number = 1;
            Some(ParsedEvent { event, slot_number })
        }
        _ => None,
    };

    // Print the parsed result if it exists
    if let Some(ref event) = parsed_event {
        println!("Parsed Event: {:#?}", event);
    }

    parsed_event
}
