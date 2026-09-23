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
	/// `BondingCurve::creator_fee_config`. `migrate` refuses any other account.
	pub creator_fee_config: &'a Address,
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
		creator_fee_config: *p.creator_fee_config,
		base_token_program: TOKEN_2022_PROGRAM_ID,
		quote_token_program: *p.quote_token_program,
		associated_token_program: ATA_PROGRAM_ID,
		system_program: SYSTEM_PROGRAM_ID,
		event_authority: super::pda::EVENT_AUTHORITY_ADDRESS,
		program: LAUNCHPAD_PROGRAM_ID,
	};
	migrate.instruction()
}
