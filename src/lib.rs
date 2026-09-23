#![doc = include_str!("../README.md")]
//!
//! ## Reading accounts
//!
//! Read an account that can be missing:
//!
//! ```
//! # use solana_address::Address;
//! # struct Account { owner: Address, data: Vec<u8> }
//! # struct Response<T> { value: T }
//! # struct RpcClient;
//! # impl RpcClient {
//! #     fn commitment(&self) {}
//! #     fn get_account_with_commitment(
//! #         &self,
//! #         _: &Address,
//! #         _: (),
//! #     ) -> Result<Response<Option<Account>>, std::io::Error> {
//! #         let owner = sendfun_sdk::constants::SYSTEM_PROGRAM_ID;
//! #         Ok(Response { value: Some(Account { owner, data: Vec::new() }) })
//! #     }
//! # }
//! # let rpc = RpcClient;
//! # let pda = Address::new_from_array([9; 32]);
//! use sendfun_sdk::nexus::accounts::PartnerConfig;
//!
//! let partner = match rpc
//!     .get_account_with_commitment(&pda, rpc.commitment())?
//!     .value
//! {
//!     Some(account) => {
//!         PartnerConfig::from_account_maybe(&account.owner, &account.data)?
//!     }
//!     None => None,
//! };
//! # assert!(partner.is_none());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Read several accounts in one call. The RPC accepts at most 100 addresses
//! per call:
//!
//! ```
//! # use solana_address::Address;
//! # struct Account { owner: Address, data: Vec<u8> }
//! # struct RpcClient;
//! # impl RpcClient {
//! #     fn get_multiple_accounts(
//! #         &self,
//! #         addresses: &[Address],
//! #     ) -> Result<Vec<Option<Account>>, std::io::Error> {
//! #         Ok(addresses.iter().map(|_| None).collect())
//! #     }
//! # }
//! # let rpc = RpcClient;
//! # let keys = [Address::new_from_array([1; 32]), Address::new_from_array([2; 32])];
//! use sendfun_sdk::launchpad::accounts::BondingCurve;
//!
//! let curves = rpc
//!     .get_multiple_accounts(&keys)?
//!     .iter()
//!     .map(|account| match account {
//!         Some(account) => {
//!             BondingCurve::from_account_maybe(&account.owner, &account.data)
//!         }
//!         None => Ok(None),
//!     })
//!     .collect::<Result<Vec<_>, _>>()?;
//! # assert_eq!(curves.len(), 2);
//! # assert!(curves.iter().all(Option::is_none));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod dex;
pub mod launchpad;
pub mod nexus;

pub mod constants;
pub mod math;
pub mod transfer_fee;
pub mod transfer_hook;

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
			// Platform-seeded PDAs have no constants. Compare the finders.
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
			// Two platforms must not share a fee preset.
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
			// Two platforms must not share partner metadata.
			assert_ne!(
				crate::nexus::pda::find_default_partner_metadata_pda(&platform)
					.0,
				crate::nexus::pda::find_default_partner_metadata_pda(
					&crate::constants::find_platform_address(b"acme").0
				)
				.0,
			);
		}

		/// Must match the published addresses. Only these checks find a wrong
		/// seed order.
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

	mod from_account {
		use borsh::BorshSerialize;
		use solana_address::Address;

		use crate::constants::{
			DEX_PROGRAM_ID, LAUNCHPAD_PROGRAM_ID, NEXUS_PROGRAM_ID,
			SYSTEM_PROGRAM_ID,
		};
		use crate::launchpad::accounts::{
			BONDING_CURVE_DISCRIMINATOR, BondingCurve,
			GLOBAL_CONFIG_DISCRIMINATOR, GlobalConfig,
		};
		use crate::launchpad::types::BondingCurveStatus;
		use crate::nexus::accounts::StakingConfig;

		const STRANGER: Address = Address::new_from_array([99; 32]);

		fn curve() -> BondingCurve {
			BondingCurve {
				discriminator: BONDING_CURVE_DISCRIMINATOR,
				version: 1,
				bump: 254,
				status: BondingCurveStatus::Funding,
				base_mint: Address::new_from_array([1; 32]),
				base_decimals: 6,
				base_supply: 1_000_000_000_000_000,
				initial_virtual_base: 1_073_000_000_000_000,
				initial_virtual_quote: 30_000_000_000,
				initial_real_base: 793_100_000_000_000,
				quote_mint: Address::new_from_array([2; 32]),
				quote_decimals: 9,
				coin_creator: Address::new_from_array([3; 32]),
				creator_fee_config: Address::new_from_array([4; 32]),
				partner: Address::new_from_array([5; 32]),
				base_vault: Address::new_from_array([6; 32]),
				quote_vault: Address::new_from_array([7; 32]),
				virtual_base_reserves: 1_073_000_000_000_000,
				virtual_quote_reserves: 30_000_000_000,
				real_base_reserves: 793_100_000_000_000,
				real_quote_reserves: 1,
				created_at: 1_700_000_000,
				protocol_owed: 0,
				creator_owed: 0,
				platform_config: Address::new_from_array([8; 32]),
				reserved: [0; 64],
			}
		}

		fn curve_data() -> Vec<u8> {
			let mut data = Vec::new();
			curve().serialize(&mut data).unwrap();
			data
		}

		#[test]
		fn decodes_an_account_its_program_owns() {
			assert_eq!(
				BondingCurve::from_account(
					&LAUNCHPAD_PROGRAM_ID,
					&curve_data()
				)
				.unwrap(),
				curve()
			);
		}

		#[test]
		fn rejects_another_owner() {
			let error = BondingCurve::from_account(&STRANGER, &curve_data())
				.unwrap_err();
			assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
			assert_eq!(
				error.to_string(),
				format!(
					"invalid account owner: {STRANGER}, expected {LAUNCHPAD_PROGRAM_ID}"
				)
			);
		}

		#[test]
		fn rejects_another_accounts_discriminator() {
			let error =
				StakingConfig::from_account(&NEXUS_PROGRAM_ID, &curve_data())
					.unwrap_err();
			assert_eq!(error.to_string(), "invalid account discriminator");
		}

		/// Same-named accounts of two programs have the same discriminator.
		/// Only the owner check tells them apart.
		#[test]
		fn rejects_a_same_named_account_of_another_program() {
			let config = GlobalConfig {
				discriminator: GLOBAL_CONFIG_DISCRIMINATOR,
				version: 1,
				bump: 255,
				authority: Address::new_from_array([1; 32]),
				operator: Address::new_from_array([2; 32]),
				create_token_disabled: false,
				normalized_sol_price_cents: 15_000,
				initial_base_supply: 1_000_000_000_000_000,
				initial_virtual_base_reserves: 1_073_000_000_000_000,
				initial_real_base_reserves: 793_100_000_000_000,
				initial_virtual_quote_reserves: 30_000_000_000,
				reserved: [0; 64],
			};
			let mut data = Vec::new();
			config.serialize(&mut data).unwrap();

			assert_eq!(
				crate::dex::accounts::GLOBAL_CONFIG_DISCRIMINATOR,
				GLOBAL_CONFIG_DISCRIMINATOR
			);
			assert!(
				crate::dex::accounts::GlobalConfig::from_bytes(&data).is_ok()
			);
			let error = crate::dex::accounts::GlobalConfig::from_account(
				&LAUNCHPAD_PROGRAM_ID,
				&data,
			)
			.unwrap_err();
			assert_eq!(
				error.to_string(),
				format!(
					"invalid account owner: {LAUNCHPAD_PROGRAM_ID}, expected {DEX_PROGRAM_ID}"
				)
			);
		}

		#[test]
		fn maybe_is_none_for_a_lamports_only_address() {
			assert_eq!(
				BondingCurve::from_account_maybe(&SYSTEM_PROGRAM_ID, &[])
					.unwrap(),
				None
			);
		}

		#[test]
		fn maybe_rejects_system_owned_data() {
			let error = BondingCurve::from_account_maybe(
				&SYSTEM_PROGRAM_ID,
				&curve_data(),
			)
			.unwrap_err();
			assert_eq!(
				error.to_string(),
				format!(
					"invalid account owner: {SYSTEM_PROGRAM_ID}, expected {LAUNCHPAD_PROGRAM_ID}"
				)
			);
		}

		#[test]
		fn maybe_rejects_empty_data_its_program_owns() {
			let error =
				BondingCurve::from_account_maybe(&LAUNCHPAD_PROGRAM_ID, &[])
					.unwrap_err();
			assert_eq!(error.to_string(), "invalid account discriminator");
		}

		#[test]
		fn maybe_decodes_an_account_its_program_owns() {
			assert_eq!(
				BondingCurve::from_account_maybe(
					&LAUNCHPAD_PROGRAM_ID,
					&curve_data()
				)
				.unwrap(),
				Some(curve())
			);
		}
	}
}
