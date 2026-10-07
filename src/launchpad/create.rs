use crate::constants::{
	ATA_PROGRAM_ID, LAUNCHPAD_PROGRAM_ID, NEXUS_PROGRAM_ID,
	PYTH_SOL_USD_PRICE_ACCOUNT, STAKING_WSOL_VAULT_ADDRESS, SYSTEM_PROGRAM_ID,
	TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use crate::nexus::pda::STAKING_CONFIG_ADDRESS;
use solana_address::Address;

#[derive(Clone, Debug)]
pub struct BuildCreateTokenParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	/// The wallet that claims the creator fees.
	pub coin_creator: &'a Address,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	pub name: &'a str,
	pub symbol: &'a str,
	pub uri: &'a str,
	pub creator_fee_mode: u8,
	pub creator_fee_bps: u16,
	/// The creator fee of the pool after migration, in bps.
	pub dex_creator_fee_bps: u16,
}

/// `base_mint` must be the address of a new keypair. Sign the transaction with
/// that keypair.
#[must_use]
pub fn build_create_token_instruction(
	p: &BuildCreateTokenParams<'_>,
) -> solana_instruction::Instruction {
	let (bonding_curve, _) =
		super::pda::find_bonding_curve_pda(p.base_mint, p.quote_mint);
	let (base_vault, _) = crate::utils::find_associated_token_pda(
		&bonding_curve,
		p.base_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (quote_vault, _) = crate::utils::find_associated_token_pda(
		&bonding_curve,
		p.quote_mint,
		p.quote_token_program,
	);
	let (partner_config, _) = crate::nexus::pda::find_partner_config_pda(
		p.platform_config,
		p.partner,
	);
	let (staking_reward_state, _) = crate::nexus::pda::find_reward_state_pda(
		&STAKING_CONFIG_ADDRESS,
		p.quote_mint,
	);
	let (staking_vault, _) = crate::utils::find_associated_token_pda(
		&STAKING_CONFIG_ADDRESS,
		p.quote_mint,
		p.quote_token_program,
	);
	let (reward_accrual, _) =
		super::generated::pdas::find_reward_accrual_pda(p.quote_mint);
	let (creator_fee_config, _) =
		super::pda::find_creator_fee_config_pda(p.base_mint, p.quote_mint);

	let create = super::instructions::CreateToken {
		user: *p.user,
		payer: *p.payer.unwrap_or(p.user),
		coin_creator: *p.coin_creator,
		global_config: super::pda::GLOBAL_CONFIG_ADDRESS,
		base_mint: *p.base_mint,
		quote_mint: *p.quote_mint,
		bonding_curve,
		base_vault,
		quote_vault,
		creator_fee_config,
		nexus_program: NEXUS_PROGRAM_ID,
		nexus_global_config: crate::nexus::pda::GLOBAL_CONFIG_ADDRESS,
		partner: crate::utils::partner_account(p.partner),
		partner_config,
		nexus_staking_config: STAKING_CONFIG_ADDRESS,
		staking_reward_state,
		reward_accrual,
		staking_vault,
		staking_wsol_vault: STAKING_WSOL_VAULT_ADDRESS,
		wsol_reward_accrual:
			super::generated::pdas::WSOL_REWARD_ACCRUAL_ADDRESS,
		pyth_price_feed: PYTH_SOL_USD_PRICE_ACCOUNT,
		base_token_program: TOKEN_2022_PROGRAM_ID,
		quote_token_program: *p.quote_token_program,
		wsol_token_program: TOKEN_PROGRAM_ID,
		associated_token_program: ATA_PROGRAM_ID,
		system_program: SYSTEM_PROGRAM_ID,
		event_authority: super::pda::EVENT_AUTHORITY_ADDRESS,
		program: LAUNCHPAD_PROGRAM_ID,
	};
	create.instruction(super::instructions::CreateTokenInstructionArgs {
		platform_config: *p.platform_config,
		name: p.name.to_owned(),
		symbol: p.symbol.to_owned(),
		uri: p.uri.to_owned(),
		creator_fee_mode: p.creator_fee_mode,
		creator_fee_bps: p.creator_fee_bps,
		dex_creator_fee_bps: p.dex_creator_fee_bps,
	})
}

#[cfg(test)]
mod tests {
	use solana_address::Address;

	use super::{BuildCreateTokenParams, build_create_token_instruction};
	use crate::constants::{
		DEFAULT_PARTNER, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
	};

	const COIN_CREATOR_INDEX: usize = 2;
	const CREATOR_FEE_CONFIG_INDEX: usize = 9;
	const PARTNER_INDEX: usize = 12;

	/// Differs from `user`.
	const COIN_CREATOR: Address = Address::new_from_array([5u8; 32]);

	/// A Token-2022 quote mint that the creator does not hold.
	const TSLAX_MINT: Address =
		solana_address::address!("XsDoVfqeBukxuZHWhdvWHBhgEHjGNst4MLodqsJHzoB");

	fn sample_instruction(
		partner: &Address,
		quote_mint: &Address,
		quote_token_program: &Address,
	) -> solana_instruction::Instruction {
		let user = Address::new_from_array([1u8; 32]);
		let base_mint = Address::new_from_array([2u8; 32]);
		let platform_config = Address::new_from_array([4u8; 32]);
		build_create_token_instruction(&BuildCreateTokenParams {
			user: &user,
			payer: None,
			coin_creator: &COIN_CREATOR,
			base_mint: &base_mint,
			quote_mint,
			partner,
			platform_config: &platform_config,
			quote_token_program,
			name: "Sample Token",
			symbol: "SMPL",
			uri: "https://example.com/meta.json",
			creator_fee_mode: 0,
			creator_fee_bps: 0,
			dex_creator_fee_bps: 0,
		})
	}

	fn wsol_instruction(partner: &Address) -> solana_instruction::Instruction {
		sample_instruction(partner, &WSOL_MINT, &TOKEN_PROGRAM_ID)
	}

	#[test]
	fn non_default_partner_is_marked_as_signer() {
		let partner = Address::new_from_array([3u8; 32]);
		let ix = wsol_instruction(&partner);

		assert_eq!(ix.accounts[PARTNER_INDEX].pubkey, partner);
		assert!(ix.accounts[PARTNER_INDEX].is_signer);

		let matches =
			ix.accounts.iter().filter(|m| m.pubkey == partner).count();
		assert_eq!(matches, 1);
	}

	#[test]
	fn default_partner_is_not_marked_as_signer() {
		let ix = wsol_instruction(&DEFAULT_PARTNER);

		assert_eq!(ix.accounts[PARTNER_INDEX].pubkey, DEFAULT_PARTNER);
		assert!(!ix.accounts[PARTNER_INDEX].is_signer);
	}

	/// The creation fee is in SOL. The creator needs no quote tokens.
	#[test]
	fn no_account_is_derived_from_the_creator_and_the_quote_mint() {
		let user = Address::new_from_array([1u8; 32]);
		let ix = sample_instruction(
			&DEFAULT_PARTNER,
			&TSLAX_MINT,
			&TOKEN_2022_PROGRAM_ID,
		);

		let (user_quote_ata, _) = crate::utils::find_associated_token_pda(
			&user,
			&TSLAX_MINT,
			&TOKEN_2022_PROGRAM_ID,
		);
		assert!(!ix.accounts.iter().any(|m| m.pubkey == user_quote_ata));

		let (staking_wsol_vault, _) = crate::utils::find_associated_token_pda(
			&crate::nexus::pda::STAKING_CONFIG_ADDRESS,
			&WSOL_MINT,
			&TOKEN_PROGRAM_ID,
		);
		assert!(ix.accounts.iter().any(|m| m.pubkey == staking_wsol_vault));
	}

	/// `user` pays the creation fee. `payer` pays the rent.
	#[test]
	fn the_fee_paying_user_slot_is_writable() {
		let ix = wsol_instruction(&DEFAULT_PARTNER);

		assert!(ix.accounts[0].is_writable);
		assert!(ix.accounts[0].is_signer);
	}

	#[test]
	fn coin_creator_is_the_named_wallet_and_does_not_sign() {
		let ix = wsol_instruction(&DEFAULT_PARTNER);

		assert_eq!(ix.accounts[COIN_CREATOR_INDEX].pubkey, COIN_CREATOR);
		assert!(!ix.accounts[COIN_CREATOR_INDEX].is_signer);
		assert!(!ix.accounts[COIN_CREATOR_INDEX].is_writable);
	}

	#[test]
	fn the_creator_fee_config_slot_is_the_curves_pda() {
		let ix = sample_instruction(
			&DEFAULT_PARTNER,
			&TSLAX_MINT,
			&TOKEN_2022_PROGRAM_ID,
		);
		let base_mint = Address::new_from_array([2u8; 32]);
		let (derived, _) = Address::find_program_address(
			&[
				b"creator_fee_config",
				base_mint.as_ref(),
				TSLAX_MINT.as_ref(),
			],
			&crate::constants::LAUNCHPAD_PROGRAM_ID,
		);

		let slot = &ix.accounts[CREATOR_FEE_CONFIG_INDEX];
		assert_eq!(slot.pubkey, derived);
		assert_eq!(
			slot.pubkey,
			solana_address::address!(
				"7wnaFX6M9fKrZmkuQitz2eKYyYDAbfAEZWEc4VagHYz1"
			),
		);
		assert!(!slot.is_signer);
		assert!(slot.is_writable);
	}

	#[test]
	fn name_follows_platform_config_at_offset_40() {
		let ix = wsol_instruction(&DEFAULT_PARTNER);

		assert_eq!(ix.data[8..40], [4u8; 32]);
		assert_eq!(ix.data[40..44], 12u32.to_le_bytes());
		assert_eq!(&ix.data[44..56], b"Sample Token");
		assert_eq!(ix.data[56..60], 4u32.to_le_bytes());
		assert_eq!(&ix.data[60..64], b"SMPL");
		assert_eq!(ix.data[64..68], 29u32.to_le_bytes());
		assert_eq!(&ix.data[68..97], b"https://example.com/meta.json");
		// `creator_fee_mode`, `creator_fee_bps`, `dex_creator_fee_bps`.
		assert_eq!(ix.data[97..102], [0u8; 5]);
		assert_eq!(ix.data.len(), 102);
	}
}
