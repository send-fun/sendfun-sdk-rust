use solana_address::Address;

pub use crate::dex::generated::programs::SEND_DEX_ID as DEX_PROGRAM_ID;
pub use crate::launchpad::generated::programs::SEND_LAUNCHPAD_ID as LAUNCHPAD_PROGRAM_ID;
pub use crate::nexus::generated::programs::SEND_NEXUS_ID as NEXUS_PROGRAM_ID;

pub const TOKEN_DECIMALS: u8 = 6;

pub const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0u8; 32]);

pub const WSOL_MINT: Address =
	solana_address::address!("So11111111111111111111111111111111111111112");

pub const USDC_MINT: Address =
	solana_address::address!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

pub const TOKEN_PROGRAM_ID: Address =
	solana_address::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub const TOKEN_2022_PROGRAM_ID: Address =
	solana_address::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

pub const ATA_PROGRAM_ID: Address =
	solana_address::address!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

pub const DEFAULT_PARTNER: Address = SYSTEM_PROGRAM_ID;

pub const PYTH_SOL_USD_PRICE_ACCOUNT: Address =
	solana_address::address!("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE");

pub const STAKING_WSOL_VAULT_ADDRESS: Address =
	solana_address::address!("AyisZJxXz9ywXhMwR3sb3oePxBB6UBvixXycAQCdSgrz");

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn staking_wsol_vault_matches_derivation() {
		let (derived, _) = crate::utils::find_associated_token_pda(
			&crate::nexus::pda::STAKING_CONFIG_ADDRESS,
			&WSOL_MINT,
			&TOKEN_PROGRAM_ID,
		);
		assert_eq!(derived, STAKING_WSOL_VAULT_ADDRESS);
	}
}

/// Seeds of a platform PDA: `["platform", name]`, under the nexus program.
pub const PLATFORM_SEED: &[u8] = b"platform";

#[must_use]
pub fn find_platform_address(name: &[u8]) -> (Address, u8) {
	Address::find_program_address(&[PLATFORM_SEED, name], &NEXUS_PROGRAM_ID)
}

#[cfg(test)]
mod platform_tests {
	use super::*;

	#[test]
	fn platform_addresses_are_distinct_per_name() {
		assert_ne!(
			find_platform_address(b"sendfun").0,
			find_platform_address(b"acme").0,
		);
	}
}
