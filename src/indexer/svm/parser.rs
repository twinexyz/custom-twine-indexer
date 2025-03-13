use base64::{engine::general_purpose, Engine as _};
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

pub fn parse_deposit_data(
    encoded_data: &str,
    signature: Option<String>,
) -> Option<DepositSuccessful> {
    let decoded_data = match general_purpose::STANDARD.decode(encoded_data) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decode base64: {}", e);
            return None;
        }
    };

    let pos = 8; // Skip the 8-byte discriminator

    let parse_u64 = |data: &[u8], start: usize| -> Option<(u64, usize)> {
        if start + 8 > data.len() {
            error!("Not enough data to parse u64 at position {}", start);
            return None;
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Some((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Option<(String, usize)> {
        if start + 4 > data.len() {
            error!(
                "Not enough data to parse string length at position {}",
                start
            );
            return None;
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            error!(
                "Not enough data to parse string of length {} at position {}",
                len, start
            );
            return None;
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Some((string, start + 4 + len))
    };

    let (nonce, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (from_l1_pubkey, pos) = match parse_string(&decoded_data, pos) {
        // Added parsing for from_l1_pubkey
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (to_twine_address, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l1_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l2_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (chain_id, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (amount, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (slot_number, _) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    Some(DepositSuccessful {
        nonce,
        from_l1_pubkey, // Added to the struct
        to_twine_address,
        l1_token,
        l2_token,
        chain_id,
        amount,
        slot_number,
        signature: signature.unwrap_or_default(),
    })
}

// The rest of the file (other parsing functions) remains unchanged
pub fn parse_forced_withdraw_data(
    encoded_data: &str,
    signature: Option<String>,
) -> Option<ForcedWithdrawSuccessful> {
    let decoded_data = match general_purpose::STANDARD.decode(encoded_data) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decode base64: {}", e);
            return None;
        }
    };

    let pos = 8;

    let parse_u64 = |data: &[u8], start: usize| -> Option<(u64, usize)> {
        if start + 8 > data.len() {
            error!("Not enough data to parse u64 at position {}", start);
            return None;
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Some((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Option<(String, usize)> {
        if start + 4 > data.len() {
            error!(
                "Not enough data to parse string length at position {}",
                start
            );
            return None;
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            error!(
                "Not enough data to parse string of length {} at position {}",
                len, start
            );
            return None;
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Some((string, start + 4 + len))
    };

    let (nonce, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (from_twine_address, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (to_l1_pub_key, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l1_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l2_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (chain_id, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (amount, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (slot_number, _) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    Some(ForcedWithdrawSuccessful {
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
) -> Option<NativeWithdrawalSuccessful> {
    let decoded_data = match general_purpose::STANDARD.decode(encoded_data) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decode base64: {}", e);
            return None;
        }
    };

    let pos = 8;

    let parse_u64 = |data: &[u8], start: usize| -> Option<(u64, usize)> {
        if start + 8 > data.len() {
            error!("Not enough data to parse u64 at position {}", start);
            return None;
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Some((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Option<(String, usize)> {
        if start + 4 > data.len() {
            error!(
                "Not enough data to parse string length at position {}",
                start
            );
            return None;
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            error!(
                "Not enough data to parse string of length {} at position {}",
                len, start
            );
            return None;
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Some((string, start + 4 + len))
    };

    let (nonce, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (receiver_l1_pubkey, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l1_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l2_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (chain_id, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (amount, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (slot_number, _) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    Some(NativeWithdrawalSuccessful {
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
) -> Option<SplWithdrawalSuccessful> {
    let decoded_data = match general_purpose::STANDARD.decode(encoded_data) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to decode base64: {}", e);
            return None;
        }
    };

    let pos = 8;

    let parse_u64 = |data: &[u8], start: usize| -> Option<(u64, usize)> {
        if start + 8 > data.len() {
            error!("Not enough data to parse u64 at position {}", start);
            return None;
        }
        let mut value_bytes = [0u8; 8];
        value_bytes.copy_from_slice(&data[start..start + 8]);
        Some((u64::from_le_bytes(value_bytes), start + 8))
    };

    let parse_string = |data: &[u8], start: usize| -> Option<(String, usize)> {
        if start + 4 > data.len() {
            error!(
                "Not enough data to parse string length at position {}",
                start
            );
            return None;
        }
        let len_bytes = [
            data[start],
            data[start + 1],
            data[start + 2],
            data[start + 3],
        ];
        let len = u32::from_le_bytes(len_bytes) as usize;
        if start + 4 + len > data.len() {
            error!(
                "Not enough data to parse string of length {} at position {}",
                len, start
            );
            return None;
        }
        let string_bytes = &data[start + 4..start + 4 + len];
        let string = String::from_utf8_lossy(string_bytes).to_string();
        Some((string, start + 4 + len))
    };

    let (nonce, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (receiver_l1_pubkey, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l1_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (l2_token, pos) = match parse_string(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (chain_id, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (amount, pos) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    let (slot_number, _) = match parse_u64(&decoded_data, pos) {
        Some((val, new_pos)) => (val, new_pos),
        None => return None,
    };

    Some(SplWithdrawalSuccessful {
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

pub fn parse_data(
    encoded_data: &str,
    signature: Option<String>,
    event_type: &str,
) -> Option<serde_json::Value> {
    match event_type {
        "native_deposit" | "spl_deposit" => {
            parse_deposit_data(encoded_data, signature).map(|d| serde_json::to_value(d).unwrap())
        }
        "native_withdrawal" | "spl_withdrawal" => {
            parse_forced_withdraw_data(encoded_data, signature)
                .map(|fw| serde_json::to_value(fw).unwrap())
        }
        "native_withdrawal_successful" => {
            parse_native_withdrawal_successful(encoded_data, signature)
                .map(|nws| serde_json::to_value(nws).unwrap())
        }
        "spl_withdrawal_successful" => parse_spl_withdrawal_successful(encoded_data, signature)
            .map(|sws| serde_json::to_value(sws).unwrap()),
        _ => {
            error!("Unknown event type: {}", event_type);
            None
        }
    }
}
