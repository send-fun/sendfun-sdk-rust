use crate::constants::{
	ATA_PROGRAM_ID, DEX_PROGRAM_ID, LAUNCHPAD_PROGRAM_ID, NEXUS_PROGRAM_ID,
	SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID,
};
use solana_address::Address;

#[derive(Clone, Debug)]
pub struct BuildMigrateParams<'a> {
	pub caller: &'a Address,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub quote_token_program: &'a Address,
}

#[must_use]
pub fn build_migrate_instruction(
	p: &BuildMigrateParams<'_>,
) -> solana_instruction::Instruction {
	let base_mint = p.base_mint;
	let quote_mint = p.quote_mint;

	let (bonding_curve, _) =
		super::pda::find_bonding_curve_pda(base_mint, quote_mint);
	let (base_vault, _) = crate::utils::find_associated_token_pda(
		&bonding_curve,
		base_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (quote_vault, _) = crate::utils::find_associated_token_pda(
		&bonding_curve,
		quote_mint,
		p.quote_token_program,
	);
	let (creator_fee_config, _) =
		super::pda::find_creator_fee_config_pda(base_mint, quote_mint);

	let (pool, _) = crate::dex::pda::find_pool_pda(base_mint, quote_mint);
	let (lp_mint, _) = crate::dex::pda::find_lp_mint_pda(base_mint, quote_mint);
	let (pool_base_vault, _) = crate::utils::find_associated_token_pda(
		&pool,
		base_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (pool_quote_vault, _) = crate::utils::find_associated_token_pda(
		&pool,
		quote_mint,
		p.quote_token_program,
	);
	let (pool_lp_account, _) = crate::utils::find_associated_token_pda(
		&pool,
		&lp_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (dex_creator_fee_config, _) =
		crate::dex::pda::find_creator_fee_config_pda(base_mint, quote_mint);

	let migrate = super::instructions::Migrate {
		caller: *p.caller,
		bonding_curve,
		base_mint: *base_mint,
		quote_mint: *quote_mint,
		base_vault,
		quote_vault,
		dex_program: DEX_PROGRAM_ID,
		migration_authority: super::pda::MIGRATION_AUTHORITY_ADDRESS,
		pool,
		lp_mint,
		pool_base_vault,
		pool_quote_vault,
		pool_lp_account,
		dex_event_authority: crate::dex::pda::EVENT_AUTHORITY_ADDRESS,
		nexus_program: NEXUS_PROGRAM_ID,
		creator_fee_config,
		base_token_program: TOKEN_2022_PROGRAM_ID,
		quote_token_program: *p.quote_token_program,
		associated_token_program: ATA_PROGRAM_ID,
		system_program: SYSTEM_PROGRAM_ID,
		event_authority: super::pda::EVENT_AUTHORITY_ADDRESS,
		program: LAUNCHPAD_PROGRAM_ID,
		dex_creator_fee_config,
	};
	migrate.instruction()
}

#[cfg(test)]
mod tests {
	use solana_address::Address;

	use super::{BuildMigrateParams, build_migrate_instruction};
	use crate::constants::{
		DEX_PROGRAM_ID, LAUNCHPAD_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
	};

	const CREATOR_FEE_CONFIG_INDEX: usize = 15;
	const DEX_CREATOR_FEE_CONFIG_INDEX: usize = 22;

	const BASE_MINT: Address = Address::new_from_array([2u8; 32]);

	fn migrate() -> solana_instruction::Instruction {
		let caller = Address::new_from_array([1u8; 32]);
		build_migrate_instruction(&BuildMigrateParams {
			caller: &caller,
			base_mint: &BASE_MINT,
			quote_mint: &WSOL_MINT,
			quote_token_program: &TOKEN_PROGRAM_ID,
		})
	}

	fn creator_fee_config_pda(program: &Address) -> Address {
		Address::find_program_address(
			&[
				b"creator_fee_config",
				BASE_MINT.as_ref(),
				WSOL_MINT.as_ref(),
			],
			program,
		)
		.0
	}

	#[test]
	fn the_creator_fee_config_slot_is_the_curves_pda() {
		let ix = migrate();

		assert_eq!(ix.accounts.len(), 23);
		let slot = &ix.accounts[CREATOR_FEE_CONFIG_INDEX];
		assert_eq!(slot.pubkey, creator_fee_config_pda(&LAUNCHPAD_PROGRAM_ID));
		assert_eq!(
			slot.pubkey,
			solana_address::address!(
				"BMrF9UWCQwk4bYq9nYsfLWFCVx6UhgLXrxJCABENvd19"
			),
		);
		assert!(!slot.is_signer);
		assert!(slot.is_writable);
	}

	#[test]
	fn the_last_slot_is_the_pools_creator_fee_config_pda() {
		let ix = migrate();

		let slot = &ix.accounts[DEX_CREATOR_FEE_CONFIG_INDEX];
		assert_eq!(slot.pubkey, creator_fee_config_pda(&DEX_PROGRAM_ID));
		assert_eq!(
			slot.pubkey,
			solana_address::address!(
				"DBtuuqp2KVdSJG6ffEd3NiLwhuBwybSXANx7YtjUsG6w"
			),
		);
		assert!(!slot.is_signer);
		assert!(slot.is_writable);
	}

	#[test]
	fn the_curves_and_the_pools_slots_differ() {
		let ix = migrate();

		assert_ne!(
			ix.accounts[CREATOR_FEE_CONFIG_INDEX].pubkey,
			ix.accounts[DEX_CREATOR_FEE_CONFIG_INDEX].pubkey,
		);
	}
}
