use base64::{engine::general_purpose, Engine as _};
use eyre::Result;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Debug, Serialize, Deserialize)]
pub struct DepositSuccessful {
    pub nonce: u64,
    pub from_l1_pubkey: String,
    pub to_twine_address: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub slot_number: u64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForcedWithdrawSuccessful {
    pub nonce: u64,
    pub from_twine_address: String,
    pub to_l1_pub_key: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: String,
    pub slot_number: u64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NativeWithdrawalSuccessful {
    pub nonce: u64,
    pub receiver_l1_pubkey: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplWithdrawalSuccessful {
    pub nonce: u64,
    pub receiver_l1_pubkey: String,
    pub l1_token: String,
    pub l2_token: String,
    pub chain_id: u64,
    pub amount: u64,
    pub slot_number: u64,
    pub signature: String,
}

pub struct ParsedEvent {
    pub event: serde_json::Value,
    pub slot_number: i64,
}

pub fn parse_deposit_data(
    encoded_data: &str,
    signature: Option<String>,
) -> Result<DepositSuccessful> {
    let decoded_data = general_purpose::STANDARD
        .decode(encoded_data)
        .map_err(|e| eyre::eyre!("Failed to decode base64: {}", e))?;

    let pos = 8; // Skip the 8-byte discriminator

    let parse_u64 = |data: &[u8], start: usize| -> Result<(u64, usize)> {
        if start + 8 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse u64 at position {}",
                start
            ));
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Ok((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Result<(String, usize)> {
        if start + 4 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string length at position {}",
                start
            ));
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string of length {} at position {}",
                len,
                start
            ));
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Ok((string, start + 4 + len))
    };

    let (nonce, pos) = parse_u64(&decoded_data, pos)?;
    let (from_l1_pubkey, pos) = parse_string(&decoded_data, pos)?;
    let (to_twine_address, pos) = parse_string(&decoded_data, pos)?;
    let (l1_token, pos) = parse_string(&decoded_data, pos)?;
    let (l2_token, pos) = parse_string(&decoded_data, pos)?;
    let (chain_id, pos) = parse_u64(&decoded_data, pos)?;
    let (amount, pos) = parse_string(&decoded_data, pos)?;
    let (slot_number, _) = parse_u64(&decoded_data, pos)?;

    Ok(DepositSuccessful {
        nonce,
        from_l1_pubkey,
        to_twine_address,
        l1_token,
        l2_token,
        chain_id,
        amount,
        slot_number,
        signature: signature.unwrap_or_default(),
    })
}

pub fn parse_forced_withdraw_data(
    encoded_data: &str,
    signature: Option<String>,
) -> Result<ForcedWithdrawSuccessful> {
    let decoded_data = general_purpose::STANDARD
        .decode(encoded_data)
        .map_err(|e| eyre::eyre!("Failed to decode base64: {}", e))?;

    let pos = 8;

    let parse_u64 = |data: &[u8], start: usize| -> Result<(u64, usize)> {
        if start + 8 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse u64 at position {}",
                start
            ));
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Ok((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Result<(String, usize)> {
        if start + 4 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string length at position {}",
                start
            ));
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string of length {} at position {}",
                len,
                start
            ));
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Ok((string, start + 4 + len))
    };

    let (nonce, pos) = parse_u64(&decoded_data, pos)?;
    let (from_twine_address, pos) = parse_string(&decoded_data, pos)?;
    let (to_l1_pub_key, pos) = parse_string(&decoded_data, pos)?;
    let (l1_token, pos) = parse_string(&decoded_data, pos)?;
    let (l2_token, pos) = parse_string(&decoded_data, pos)?;
    let (chain_id, pos) = parse_u64(&decoded_data, pos)?;
    let (amount, pos) = parse_string(&decoded_data, pos)?;
    let (slot_number, _) = parse_u64(&decoded_data, pos)?;

    Ok(ForcedWithdrawSuccessful {
        nonce,
        from_twine_address,
        to_l1_pub_key,
        l1_token,
        l2_token,
        chain_id,
        amount,
        slot_number,
        signature: signature.unwrap_or_default(),
    })
}

pub fn parse_native_withdrawal_successful(
    encoded_data: &str,
    signature: Option<String>,
) -> Result<NativeWithdrawalSuccessful> {
    let decoded_data = general_purpose::STANDARD
        .decode(encoded_data)
        .map_err(|e| eyre::eyre!("Failed to decode base64: {}", e))?;

    let pos = 8;

    let parse_u64 = |data: &[u8], start: usize| -> Result<(u64, usize)> {
        if start + 8 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse u64 at position {}",
                start
            ));
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Ok((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Result<(String, usize)> {
        if start + 4 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string length at position {}",
                start
            ));
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string of length {} at position {}",
                len,
                start
            ));
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Ok((string, start + 4 + len))
    };

    let (nonce, pos) = parse_u64(&decoded_data, pos)?;
    let (receiver_l1_pubkey, pos) = parse_string(&decoded_data, pos)?;
    let (l1_token, pos) = parse_string(&decoded_data, pos)?;
    let (l2_token, pos) = parse_string(&decoded_data, pos)?;
    let (chain_id, pos) = parse_u64(&decoded_data, pos)?;
    let (amount, pos) = parse_u64(&decoded_data, pos)?;
    let (slot_number, _) = parse_u64(&decoded_data, pos)?;

    Ok(NativeWithdrawalSuccessful {
        nonce,
        receiver_l1_pubkey,
        l1_token,
        l2_token,
        chain_id,
        amount,
        slot_number,
        signature: signature.unwrap_or_default(),
    })
}

pub fn parse_spl_withdrawal_successful(
    encoded_data: &str,
    signature: Option<String>,
) -> Result<SplWithdrawalSuccessful> {
    let decoded_data = general_purpose::STANDARD
        .decode(encoded_data)
        .map_err(|e| eyre::eyre!("Failed to decode base64: {}", e))?;

    let pos = 8;

    let parse_u64 = |data: &[u8], start: usize| -> Result<(u64, usize)> {
        if start + 8 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse u64 at position {}",
                start
            ));
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Ok((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Result<(String, usize)> {
        if start + 4 > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string length at position {}",
                start
            ));
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            return Err(eyre::eyre!(
                "Not enough data to parse string of length {} at position {}",
                len,
                start
            ));
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Ok((string, start + 4 + len))
    };

    let (nonce, pos) = parse_u64(&decoded_data, pos)?;
    let (receiver_l1_pubkey, pos) = parse_string(&decoded_data, pos)?;
    let (l1_token, pos) = parse_string(&decoded_data, pos)?;
    let (l2_token, pos) = parse_string(&decoded_data, pos)?;
    let (chain_id, pos) = parse_u64(&decoded_data, pos)?;
    let (amount, pos) = parse_u64(&decoded_data, pos)?;
    let (slot_number, _) = parse_u64(&decoded_data, pos)?;

    Ok(SplWithdrawalSuccessful {
        nonce,
        receiver_l1_pubkey,
        l1_token,
        l2_token,
        chain_id,
        amount,
        slot_number,
        signature: signature.unwrap_or_default(),
    })
}

pub fn parse_log(
    encoded_data: &str,
    signature: Option<String>,
    event_type: &str,
) -> Result<ParsedEvent> {
    let event = match event_type {
        "native_deposit" | "spl_deposit" => {
            let deposit = parse_deposit_data(encoded_data, signature.clone())?;
            serde_json::to_value(deposit)?
        }
        "native_withdrawal" | "spl_withdrawal" => {
            let withdrawal = parse_forced_withdraw_data(encoded_data, signature.clone())?;
            serde_json::to_value(withdrawal)?
        }
        "native_withdrawal_successful" => {
            let native_success =
                parse_native_withdrawal_successful(encoded_data, signature.clone())?;
            serde_json::to_value(native_success)?
        }
        "spl_withdrawal_successful" => {
            let spl_success = parse_spl_withdrawal_successful(encoded_data, signature.clone())?;
            serde_json::to_value(spl_success)?
        }
        _ => {
            error!("Unknown event type: {}", event_type);
            return Err(eyre::eyre!("Unknown event type: {}", event_type));
        }
    };

    let slot_number = match event_type {
        "native_deposit" | "spl_deposit" => {
            parse_deposit_data(encoded_data, signature.clone())?.slot_number
        }
        "native_withdrawal" | "spl_withdrawal" => {
            parse_forced_withdraw_data(encoded_data, signature.clone())?.slot_number
        }
        "native_withdrawal_successful" => {
            parse_native_withdrawal_successful(encoded_data, signature.clone())?.slot_number
        }
        "spl_withdrawal_successful" => {
            parse_spl_withdrawal_successful(encoded_data, signature)?.slot_number
        }
        _ => unreachable!(), // Covered by error above
    };

    Ok(ParsedEvent {
        event,
        slot_number: slot_number as i64,
    })
}
