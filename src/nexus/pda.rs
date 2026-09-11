pub use crate::nexus::generated::pdas::*;

/// Seeds: `["creator_fee_balance", creator_hash, quote_mint]`; frozen string.
#[must_use]
pub fn find_creator_fee_config_pda_from_id(
	creator_platform: &str,
	creator_id: &str,
	quote_mint: &solana_address::Address,
) -> (solana_address::Address, u8) {
	let creator_hash =
		crate::utils::creator_hash_from_id(creator_platform, creator_id);
	find_creator_fee_config_pda(&creator_hash, quote_mint)
}
