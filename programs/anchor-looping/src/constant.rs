use anchor_lang::{prelude::*, solana_program::pubkey};

pub const PROTOCOL_AUTHORITY_BUMP: u8 =
    const_crypto::ed25519::derive_program_address(&[b"auth"], &crate::id_const().to_bytes()).1;


pub const KAMINO_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
pub const JUPITER_PROGRAM_ID: Pubkey = pubkey!("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");

/// Since deserializing the data from the smart contract side is extremely complex, we are going to pass in 
/// flags from the frontend to indicate if the obligation has collateral or borrows. 
/// 
/// Note: this is safe since it's only used for telling the program if it needs to refresh the collateral or borrows.
pub const FLAG_HAS_COLLATERAL: u8 = 1 << 0;
pub const FLAG_HAS_BORROWS: u8 = 1 << 1;
