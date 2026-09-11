#![doc = include_str!("../README.md")]

pub mod dex;
pub mod launchpad;
pub mod nexus;

pub mod constants;
pub mod math;
pub mod transfer_fee;

pub mod utils;

#[cfg(test)]
mod tests {

	mod pda_tests {
		use solana_address::address;

		#[test]
		fn launchpad_bonding_curve_pda() {
			let base = address!("So11111111111111111111111111111111111111112");
			let quote = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
			let (addr1, _) =
				crate::launchpad::pda::find_bonding_curve_pda(&base, &quote);
			let (addr2, _) =
				crate::launchpad::pda::find_bonding_curve_pda(&base, &quote);
			assert_eq!(addr1, addr2);
			let (addr3, _) =
				crate::launchpad::pda::find_bonding_curve_pda(&quote, &base);
			assert_ne!(addr1, addr3);
		}

		#[test]
		fn dex_pool_and_lp_mint_pdas_differ() {
			let base = address!("So11111111111111111111111111111111111111112");
			let quote = address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
			let (pool, _) = crate::dex::pda::find_pool_pda(&base, &quote);
			let (lp, _) = crate::dex::pda::find_lp_mint_pda(&base, &quote);
			assert_ne!(pool, lp);
		}

		#[test]
		fn nexus_staking_pdas() {
			let user = address!("11111111111111111111111111111112");
			let staking_mint =
				address!("So11111111111111111111111111111111111111112");
			let reward_mint =
				address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

			let (pos, _) = crate::nexus::pda::find_user_stake_position_pda(
				&user,
				&staking_mint,
			);
			let (debt, _) = crate::nexus::pda::find_user_reward_debt_pda(
				&user,
				&staking_mint,
				&reward_mint,
			);
			assert_ne!(pos, debt);
		}

		#[test]
		fn nexus_creator_fee_config_pda() {
			let quote = address!("So11111111111111111111111111111111111111112");
			let (addr1, _) =
				crate::nexus::pda::find_creator_fee_config_pda_from_id(
					"wallet",
					"SomeCreator",
					&quote,
				);
			let (addr2, _) =
				crate::nexus::pda::find_creator_fee_config_pda_from_id(
					"twitter",
					"SomeCreator",
					&quote,
				);
			assert_ne!(addr1, addr2);
		}

		#[test]
		fn event_authority_pdas_differ_per_program() {
			let (lp, _) = crate::launchpad::pda::find_event_authority_pda();
			let (dex, _) = crate::dex::pda::find_event_authority_pda();
			let (nexus, _) = crate::nexus::pda::find_event_authority_pda();
			assert_ne!(lp, dex);
			assert_ne!(dex, nexus);
			assert_ne!(lp, nexus);
		}
	}

	mod generated_constants {
		use solana_address::Address;

		struct PdaCheck<'a> {
			label: &'a str,
			address: Address,
			bump: u8,
			seeds: &'a [&'a [u8]],
			program_id: Address,
		}

		#[track_caller]
		fn assert_pda(check: PdaCheck<'_>) {
			let (derived, derived_bump) =
				Address::find_program_address(check.seeds, &check.program_id);
			assert_eq!(check.address, derived, "{}: address", check.label);
			assert_eq!(check.bump, derived_bump, "{}: bump", check.label);
		}

		#[test]
		fn launchpad_pda_constants_match_derivation() {
			use crate::launchpad::generated::SEND_LAUNCHPAD_ID as ID;
			use crate::launchpad::pda::{
				EVENT_AUTHORITY_ADDRESS, EVENT_AUTHORITY_BUMP,
				EVENT_AUTHORITY_SEED, GLOBAL_CONFIG_ADDRESS,
				GLOBAL_CONFIG_BUMP, GLOBAL_CONFIG_SEED,
				MIGRATION_AUTHORITY_ADDRESS, MIGRATION_AUTHORITY_BUMP,
				MIGRATION_AUTHORITY_SEED,
			};

			assert_pda(PdaCheck {
				label: "launchpad global_config",
				address: GLOBAL_CONFIG_ADDRESS,
				bump: GLOBAL_CONFIG_BUMP,
				seeds: &[GLOBAL_CONFIG_SEED],
				program_id: ID,
			});
			assert_pda(PdaCheck {
				label: "launchpad event_authority",
				address: EVENT_AUTHORITY_ADDRESS,
				bump: EVENT_AUTHORITY_BUMP,
				seeds: &[EVENT_AUTHORITY_SEED],
				program_id: ID,
			});
			assert_pda(PdaCheck {
				label: "launchpad migration_authority",
				address: MIGRATION_AUTHORITY_ADDRESS,
				bump: MIGRATION_AUTHORITY_BUMP,
				seeds: &[MIGRATION_AUTHORITY_SEED],
				program_id: ID,
			});
		}

		#[test]
		fn dex_pda_constants_match_derivation() {
			use crate::dex::generated::SEND_DEX_ID as ID;
			use crate::dex::pda::{
				EVENT_AUTHORITY_ADDRESS, EVENT_AUTHORITY_BUMP,
				EVENT_AUTHORITY_SEED, GLOBAL_CONFIG_ADDRESS,
				GLOBAL_CONFIG_BUMP, GLOBAL_CONFIG_SEED,
			};

			assert_pda(PdaCheck {
				label: "dex global_config",
				address: GLOBAL_CONFIG_ADDRESS,
				bump: GLOBAL_CONFIG_BUMP,
				seeds: &[GLOBAL_CONFIG_SEED],
				program_id: ID,
			});
			assert_pda(PdaCheck {
				label: "dex event_authority",
				address: EVENT_AUTHORITY_ADDRESS,
				bump: EVENT_AUTHORITY_BUMP,
				seeds: &[EVENT_AUTHORITY_SEED],
				program_id: ID,
			});
		}

		#[test]
		fn nexus_pda_constants_match_derivation() {
			use crate::nexus::generated::SEND_NEXUS_ID as ID;
			use crate::nexus::pda::{
				ALT_REGISTRY_ADDRESS, ALT_REGISTRY_BUMP, ALT_REGISTRY_SEED,
				EVENT_AUTHORITY_ADDRESS, EVENT_AUTHORITY_BUMP,
				EVENT_AUTHORITY_SEED, GLOBAL_CONFIG_ADDRESS,
				GLOBAL_CONFIG_BUMP, GLOBAL_CONFIG_SEED, STAKING_CONFIG_ADDRESS,
				STAKING_CONFIG_BUMP, STAKING_CONFIG_SEED,
			};

			assert_pda(PdaCheck {
				label: "nexus global_config",
				address: GLOBAL_CONFIG_ADDRESS,
				bump: GLOBAL_CONFIG_BUMP,
				seeds: &[GLOBAL_CONFIG_SEED],
				program_id: ID,
			});
			assert_pda(PdaCheck {
				label: "nexus staking_config",
				address: STAKING_CONFIG_ADDRESS,
				bump: STAKING_CONFIG_BUMP,
				seeds: &[STAKING_CONFIG_SEED],
				program_id: ID,
			});
			assert_pda(PdaCheck {
				label: "nexus event_authority",
				address: EVENT_AUTHORITY_ADDRESS,
				bump: EVENT_AUTHORITY_BUMP,
				seeds: &[EVENT_AUTHORITY_SEED],
				program_id: ID,
			});
			assert_pda(PdaCheck {
				label: "nexus alt_registry",
				address: ALT_REGISTRY_ADDRESS,
				bump: ALT_REGISTRY_BUMP,
				seeds: &[ALT_REGISTRY_SEED],
				program_id: ID,
			});
			// Platform-seeded finders can't be folded constants; pin them
			// against the general finders.
			let platform =
				crate::constants::find_platform_address(b"sendfun").0;
			assert_eq!(
				crate::nexus::pda::find_default_partner_pda(&platform),
				crate::nexus::pda::find_partner_config_pda(
					&platform,
					&crate::constants::DEFAULT_PARTNER,
				),
			);
			assert_ne!(
				crate::nexus::pda::find_default_partner_pda(&platform).0,
				crate::nexus::pda::find_default_partner_pda(
					&crate::constants::find_platform_address(b"acme").0
				)
				.0,
			);
			assert_eq!(
				crate::nexus::pda::find_default_fee_preset_pda(&platform),
				crate::nexus::pda::find_fee_preset_pda(&platform, 0),
			);
			// Two platforms must not share a tier table, or index 0 on one
			// would price launches on the other.
			assert_ne!(
				crate::nexus::pda::find_default_fee_preset_pda(&platform).0,
				crate::nexus::pda::find_default_fee_preset_pda(
					&crate::constants::find_platform_address(b"acme").0
				)
				.0,
			);
			assert_eq!(
				crate::nexus::pda::find_default_partner_metadata_pda(&platform),
				crate::nexus::pda::find_partner_metadata_pda(
					&platform,
					&crate::constants::DEFAULT_PARTNER,
				),
			);
			// If these collided a rename on one platform would rename both.
			assert_ne!(
				crate::nexus::pda::find_default_partner_metadata_pda(&platform)
					.0,
				crate::nexus::pda::find_default_partner_metadata_pda(
					&crate::constants::find_platform_address(b"acme").0
				)
				.0,
			);
		}

		/// Pinned to `app/docs/developers/addresses.mdx`. The checks above compare
		/// finders to finders, so only these catch a seed-order mistake.
		#[test]
		fn sendfun_default_partner_addresses_are_pinned() {
			use crate::constants::{DEFAULT_PARTNER, find_platform_address};
			use solana_address::address;

			let platform = find_platform_address(b"sendfun").0;
			assert_eq!(
				platform,
				address!("2PJedAsa7pCnScks2XM3U4o2nPaTvvBmi2553VcCnGWB"),
				"sendfun platform"
			);

			assert_eq!(
				crate::nexus::pda::find_partner_config_pda(
					&platform,
					&DEFAULT_PARTNER
				)
				.0,
				address!("B77P3Hvj9xLi4tcFtyjNcf6NjhkQrfJvLkz3nVZnBzec"),
				"default partner config"
			);
			assert_eq!(
				crate::nexus::pda::find_fee_preset_pda(&platform, 0).0,
				address!("DTWXzPHfg7vjnhwhE7rxhdPZPGGFjuqD1excog3xmKTx"),
				"default fee preset 0"
			);
			assert_eq!(
				crate::nexus::pda::find_partner_metadata_pda(
					&platform,
					&DEFAULT_PARTNER
				)
				.0,
				address!("8WqZvqH4B1LtcfxZHeAdYWCYvwfMaUkQvJqZsw4fmii5"),
				"default partner metadata"
			);
		}
	}

	#[test]
	fn nexus_find_creator_fee_config_pda_from_id_matches_hash_derivation() {
		use solana_address::address;
		let quote = address!("So11111111111111111111111111111111111111112");
		let creator_hash =
			crate::utils::creator_hash_from_id("wallet", "SomeCreator");
		let (addr1, bump1) =
			crate::nexus::pda::find_creator_fee_config_pda_from_id(
				"wallet",
				"SomeCreator",
				&quote,
			);
		let (addr2, bump2) = crate::nexus::pda::find_creator_fee_config_pda(
			&creator_hash,
			&quote,
		);
		assert_eq!(addr1, addr2);
		assert_eq!(bump1, bump2);
	}

	#[test]
	fn bonding_curve_status_values() {
		assert_eq!(
			crate::launchpad::types::BondingCurveStatus::Funding as u8,
			0
		);
		assert_eq!(
			crate::launchpad::types::BondingCurveStatus::Completed as u8,
			1
		);
		assert_eq!(
			crate::launchpad::types::BondingCurveStatus::Migrated as u8,
			2
		);
		assert_eq!(
			crate::launchpad::types::BondingCurveStatus::Paused as u8,
			3
		);
	}

	#[test]
	fn pool_status_values() {
		assert_eq!(crate::dex::types::PoolStatus::Active as u8, 0);
		assert_eq!(crate::dex::types::PoolStatus::Paused as u8, 1);
	}

	#[test]
	fn launchpad_fees_total() {
		let fees = crate::nexus::types::LaunchpadFees {
			creation_fee_cents: 0,
			protocol_fee_bps: 100,
			creator_fee_bps: 50,
			fee_decay_seconds: 0,
			fee_decay_start_bps: 0,
		};
		assert_eq!(fees.total_fee_bps(), Some(150));
	}

	#[test]
	fn dex_fees_total() {
		let fees = crate::nexus::types::DexFees {
			creation_fee_cents: 0,
			protocol_fee_bps: 80,
			lp_fee_bps: 30,
			creator_fee_bps: 40,
			fee_decay_seconds: 0,
			fee_decay_start_bps: 0,
		};
		assert_eq!(fees.total_fee_bps(), Some(150));
	}
}
