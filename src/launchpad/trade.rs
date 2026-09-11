use crate::constants::{
	ATA_PROGRAM_ID, LAUNCHPAD_PROGRAM_ID, SYSTEM_PROGRAM_ID,
	TOKEN_2022_PROGRAM_ID,
};
use crate::math::amm::{self, QuoteError};
use crate::utils::{
	DeriveTradeArgs, DerivedTradeAccounts, TradeStateFields, TradeUserContext,
	derive_trade_accounts, partner_account,
};
use solana_address::Address;

#[derive(Clone, Debug)]
pub struct BuyExactInParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub virtual_base_reserves: u64,
	pub virtual_quote_reserves: u64,
	pub fee_bps: u16,
	pub slippage_bps: u16,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	/// The landing epoch's schedule; stale or wrongly `None` misprices slippage.
	pub quote_fee: Option<amm::MintFee>,
	/// Same freshness requirement as `quote_fee`.
	pub base_fee: Option<amm::MintFee>,
	/// Caps the fill at the supply left on the curve.
	pub real_base_reserves: u64,
	pub quote_amount: u64,
}

pub fn buy_exact_in(
	p: &BuyExactInParams<'_>,
) -> Result<(solana_instruction::Instruction, amm::BuyQuote), QuoteError> {
	let q = amm::buy_exact_in_with_fees(amm::BuyArgs {
		amm: amm::AmmInput {
			quote_reserves: p.virtual_quote_reserves,
			base_reserves: p.virtual_base_reserves,
			amount: p.quote_amount,
			fee_bps: p.fee_bps,
		},
		quote_fee: p.quote_fee,
		base_fee: p.base_fee,
		// The program always caps a buy at the supply left on the curve.
		base_reserve_cap: Some(p.real_base_reserves),
	})?;
	// The program floors the buyer's credit, not the vault's debit.
	let min_base_out =
		amm::calculate_slippage_down(q.base_to_user, p.slippage_bps)?;
	let ix = build_buy_exact_in_instruction(&BuildBuyExactInParams {
		user: p.user,
		payer: p.payer,
		base_mint: p.base_mint,
		quote_mint: p.quote_mint,
		partner: p.partner,
		platform_config: p.platform_config,
		quote_token_program: p.quote_token_program,
		amount_in: p.quote_amount,
		min_amount_out: min_base_out,
	});
	Ok((ix, q))
}

#[derive(Clone, Debug)]
pub struct BuyExactOutParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub virtual_base_reserves: u64,
	pub virtual_quote_reserves: u64,
	pub fee_bps: u16,
	pub slippage_bps: u16,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	/// The landing epoch's schedule; stale or wrongly `None` misprices slippage.
	pub quote_fee: Option<amm::MintFee>,
	/// Same freshness requirement as `quote_fee`.
	pub base_fee: Option<amm::MintFee>,
	/// Caps the fill at the supply left on the curve.
	pub real_base_reserves: u64,
	pub base_amount: u64,
}

pub fn buy_exact_out(
	p: &BuyExactOutParams<'_>,
) -> Result<(solana_instruction::Instruction, amm::BuyQuote), QuoteError> {
	let q = amm::buy_exact_out_with_fees(amm::BuyArgs {
		amm: amm::AmmInput {
			quote_reserves: p.virtual_quote_reserves,
			base_reserves: p.virtual_base_reserves,
			amount: p.base_amount,
			fee_bps: p.fee_bps,
		},
		quote_fee: p.quote_fee,
		base_fee: p.base_fee,
		base_reserve_cap: Some(p.real_base_reserves),
	})?;
	// The program caps the gross quote the buyer sends, transfer fee included.
	let max_quote_in =
		amm::calculate_slippage_up(q.quote_from_user, p.slippage_bps)?;
	let ix = build_buy_exact_out_instruction(&BuildBuyExactOutParams {
		user: p.user,
		payer: p.payer,
		base_mint: p.base_mint,
		quote_mint: p.quote_mint,
		partner: p.partner,
		platform_config: p.platform_config,
		quote_token_program: p.quote_token_program,
		amount_out: p.base_amount,
		max_amount_in: max_quote_in,
	});
	Ok((ix, q))
}

#[derive(Clone, Debug)]
pub struct SellExactInParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub virtual_base_reserves: u64,
	pub virtual_quote_reserves: u64,
	pub fee_bps: u16,
	pub slippage_bps: u16,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	/// The landing epoch's schedule; stale or wrongly `None` misprices slippage.
	pub quote_fee: Option<amm::MintFee>,
	/// Same freshness requirement as `quote_fee`.
	pub base_fee: Option<amm::MintFee>,
	pub base_amount: u64,
}

pub fn sell_exact_in(
	p: &SellExactInParams<'_>,
) -> Result<(solana_instruction::Instruction, amm::SellQuote), QuoteError> {
	let q = amm::sell_exact_in_with_fees(amm::SellArgs {
		amm: amm::AmmInput {
			quote_reserves: p.virtual_quote_reserves,
			base_reserves: p.virtual_base_reserves,
			amount: p.base_amount,
			fee_bps: p.fee_bps,
		},
		quote_fee: p.quote_fee,
		base_fee: p.base_fee,
	})?;
	// The program floors the seller's credit, not what leaves the vault.
	let min_quote_out =
		amm::calculate_slippage_down(q.quote_to_user, p.slippage_bps)?;
	let ix = build_sell_exact_in_instruction(&BuildSellExactInParams {
		user: p.user,
		payer: p.payer,
		base_mint: p.base_mint,
		quote_mint: p.quote_mint,
		partner: p.partner,
		platform_config: p.platform_config,
		quote_token_program: p.quote_token_program,
		amount_in: p.base_amount,
		min_amount_out: min_quote_out,
	});
	Ok((ix, q))
}

#[derive(Clone, Debug)]
pub struct SellExactOutParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub virtual_base_reserves: u64,
	pub virtual_quote_reserves: u64,
	pub fee_bps: u16,
	pub slippage_bps: u16,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	/// The landing epoch's schedule; stale or wrongly `None` misprices slippage.
	pub quote_fee: Option<amm::MintFee>,
	/// Same freshness requirement as `quote_fee`.
	pub base_fee: Option<amm::MintFee>,
	pub quote_amount: u64,
}

pub fn sell_exact_out(
	p: &SellExactOutParams<'_>,
) -> Result<(solana_instruction::Instruction, amm::SellQuote), QuoteError> {
	let q = amm::sell_exact_out_with_fees(amm::SellArgs {
		amm: amm::AmmInput {
			quote_reserves: p.virtual_quote_reserves,
			base_reserves: p.virtual_base_reserves,
			amount: p.quote_amount,
			fee_bps: p.fee_bps,
		},
		quote_fee: p.quote_fee,
		base_fee: p.base_fee,
	})?;
	// The program caps the gross base the seller sends, transfer fee included.
	let max_base_in =
		amm::calculate_slippage_up(q.base_from_user, p.slippage_bps)?;
	let ix = build_sell_exact_out_instruction(&BuildSellExactOutParams {
		user: p.user,
		payer: p.payer,
		base_mint: p.base_mint,
		quote_mint: p.quote_mint,
		partner: p.partner,
		platform_config: p.platform_config,
		quote_token_program: p.quote_token_program,
		amount_out: p.quote_amount,
		max_amount_in: max_base_in,
	});
	Ok((ix, q))
}

#[derive(Clone, Debug)]
pub struct BuildBuyExactInParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	pub amount_in: u64,
	pub min_amount_out: u64,
}

#[must_use]
pub fn build_buy_exact_in_instruction(
	p: &BuildBuyExactInParams<'_>,
) -> solana_instruction::Instruction {
	launchpad_trade_accounts!(super::instructions::BuyExactIn, p).instruction(
		super::instructions::BuyExactInInstructionArgs {
			amount_in: p.amount_in,
			min_amount_out: p.min_amount_out,
		},
	)
}

#[derive(Clone, Debug)]
pub struct BuildBuyExactOutParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	pub amount_out: u64,
	pub max_amount_in: u64,
}

#[must_use]
pub fn build_buy_exact_out_instruction(
	p: &BuildBuyExactOutParams<'_>,
) -> solana_instruction::Instruction {
	launchpad_trade_accounts!(super::instructions::BuyExactOut, p).instruction(
		super::instructions::BuyExactOutInstructionArgs {
			amount_out: p.amount_out,
			max_amount_in: p.max_amount_in,
		},
	)
}

#[derive(Clone, Debug)]
pub struct BuildSellExactInParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	pub amount_in: u64,
	pub min_amount_out: u64,
}

#[must_use]
pub fn build_sell_exact_in_instruction(
	p: &BuildSellExactInParams<'_>,
) -> solana_instruction::Instruction {
	launchpad_trade_accounts!(super::instructions::SellExactIn, p).instruction(
		super::instructions::SellExactInInstructionArgs {
			amount_in: p.amount_in,
			min_amount_out: p.min_amount_out,
		},
	)
}

#[derive(Clone, Debug)]
pub struct BuildSellExactOutParams<'a> {
	pub user: &'a Address,
	pub payer: Option<&'a Address>,
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	pub partner: &'a Address,
	pub platform_config: &'a Address,
	pub quote_token_program: &'a Address,
	pub amount_out: u64,
	pub max_amount_in: u64,
}

#[must_use]
pub fn build_sell_exact_out_instruction(
	p: &BuildSellExactOutParams<'_>,
) -> solana_instruction::Instruction {
	launchpad_trade_accounts!(super::instructions::SellExactOut, p).instruction(
		super::instructions::SellExactOutInstructionArgs {
			amount_out: p.amount_out,
			max_amount_in: p.max_amount_in,
		},
	)
}

macro_rules! launchpad_trade_accounts {
	($ty:path, $p:expr) => {{
		let d = derive(
			&TradeStateFields {
				base_mint: $p.base_mint,
				quote_mint: $p.quote_mint,
				platform_config: $p.platform_config,
			},
			TradeUserContext {
				user: $p.user,
				partner: $p.partner,
				quote_token_program: $p.quote_token_program,
			},
		);
		$ty {
			user: *$p.user,
			payer: *$p.payer.unwrap_or($p.user),
			bonding_curve: d.bonding_curve,
			base_mint: *$p.base_mint,
			quote_mint: *$p.quote_mint,
			base_vault: d.base_vault,
			quote_vault: d.quote_vault,
			user_base_account: d.trade.user_base_account,
			user_quote_account: d.trade.user_quote_account,
			partner: partner_account($p.partner),
			partner_config: d.trade.partner_config,
			base_token_program: TOKEN_2022_PROGRAM_ID,
			quote_token_program: *$p.quote_token_program,
			associated_token_program: ATA_PROGRAM_ID,
			system_program: SYSTEM_PROGRAM_ID,
			event_authority: d.trade.event_authority,
			program: LAUNCHPAD_PROGRAM_ID,
		}
	}};
}
use launchpad_trade_accounts;

struct CurveTradeAccounts {
	bonding_curve: Address,
	base_vault: Address,
	quote_vault: Address,
	trade: DerivedTradeAccounts,
}

fn derive(
	state: &TradeStateFields<'_>,
	ctx: TradeUserContext<'_>,
) -> CurveTradeAccounts {
	let (bonding_curve, _) =
		super::pda::find_bonding_curve_pda(state.base_mint, state.quote_mint);
	// Base mints are always Token-2022, so the base vault's ATA seeds are too.
	let (base_vault, _) = crate::utils::find_associated_token_pda(
		&bonding_curve,
		state.base_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (quote_vault, _) = crate::utils::find_associated_token_pda(
		&bonding_curve,
		state.quote_mint,
		ctx.quote_token_program,
	);
	let trade = derive_trade_accounts(DeriveTradeArgs {
		state,
		ctx,
		event_authority: super::pda::EVENT_AUTHORITY_ADDRESS,
	});
	CurveTradeAccounts {
		bonding_curve,
		base_vault,
		quote_vault,
		trade,
	}
}

#[cfg(test)]
mod tests {
	use solana_address::Address;

	use super::{
		BuildBuyExactInParams, BuildBuyExactOutParams, BuildSellExactInParams,
		BuildSellExactOutParams, BuyExactInParams,
		build_buy_exact_in_instruction, build_buy_exact_out_instruction,
		build_sell_exact_in_instruction, build_sell_exact_out_instruction,
		buy_exact_in,
	};
	use crate::constants::{
		TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
	};
	use crate::launchpad::events::TradeEvent;
	use crate::launchpad::types::TradeDirection;

	const BONDING_CURVE_INDEX: usize = 2;
	const BASE_VAULT_INDEX: usize = 5;
	const QUOTE_VAULT_INDEX: usize = 6;
	const PARTNER_INDEX: usize = 9;
	const PARTNER_CONFIG_INDEX: usize = 10;

	const USER: Address = Address::new_from_array([1u8; 32]);
	const BASE_MINT: Address = Address::new_from_array([2u8; 32]);
	const PARTNER: Address = Address::new_from_array([3u8; 32]);
	const PLATFORM_CONFIG: Address = Address::new_from_array([4u8; 32]);

	fn assert_derived_market_accounts(
		ix: &solana_instruction::Instruction,
		quote_mint: &Address,
		quote_token_program: &Address,
	) {
		let (bonding_curve, _) = crate::launchpad::pda::find_bonding_curve_pda(
			&BASE_MINT, quote_mint,
		);
		let (base_vault, _) = crate::utils::find_associated_token_pda(
			&bonding_curve,
			&BASE_MINT,
			&TOKEN_2022_PROGRAM_ID,
		);
		let (quote_vault, _) = crate::utils::find_associated_token_pda(
			&bonding_curve,
			quote_mint,
			quote_token_program,
		);
		assert_eq!(ix.accounts.len(), 17);
		assert_eq!(ix.accounts[BONDING_CURVE_INDEX].pubkey, bonding_curve);
		assert_eq!(ix.accounts[BASE_VAULT_INDEX].pubkey, base_vault);
		assert_eq!(ix.accounts[QUOTE_VAULT_INDEX].pubkey, quote_vault);
		assert_eq!(ix.accounts[PARTNER_INDEX].pubkey, PARTNER);
		assert!(ix.accounts[PARTNER_INDEX].is_signer);
	}

	#[test]
	fn build_buy_exact_in_derives_the_market_accounts() {
		let ix = build_buy_exact_in_instruction(&BuildBuyExactInParams {
			user: &USER,
			payer: None,
			base_mint: &BASE_MINT,
			quote_mint: &WSOL_MINT,
			partner: &PARTNER,
			platform_config: &PLATFORM_CONFIG,
			quote_token_program: &TOKEN_PROGRAM_ID,
			amount_in: 1_000,
			min_amount_out: 900,
		});
		assert_derived_market_accounts(&ix, &WSOL_MINT, &TOKEN_PROGRAM_ID);
	}

	#[test]
	fn build_buy_exact_out_derives_the_market_accounts() {
		let ix = build_buy_exact_out_instruction(&BuildBuyExactOutParams {
			user: &USER,
			payer: None,
			base_mint: &BASE_MINT,
			quote_mint: &WSOL_MINT,
			partner: &PARTNER,
			platform_config: &PLATFORM_CONFIG,
			quote_token_program: &TOKEN_PROGRAM_ID,
			amount_out: 1_000,
			max_amount_in: 1_100,
		});
		assert_derived_market_accounts(&ix, &WSOL_MINT, &TOKEN_PROGRAM_ID);
	}

	#[test]
	fn build_sell_exact_in_derives_the_market_accounts() {
		let ix = build_sell_exact_in_instruction(&BuildSellExactInParams {
			user: &USER,
			payer: None,
			base_mint: &BASE_MINT,
			quote_mint: &WSOL_MINT,
			partner: &PARTNER,
			platform_config: &PLATFORM_CONFIG,
			quote_token_program: &TOKEN_PROGRAM_ID,
			amount_in: 1_000,
			min_amount_out: 900,
		});
		assert_derived_market_accounts(&ix, &WSOL_MINT, &TOKEN_PROGRAM_ID);
	}

	#[test]
	fn build_sell_exact_out_derives_the_market_accounts() {
		let ix = build_sell_exact_out_instruction(&BuildSellExactOutParams {
			user: &USER,
			payer: None,
			base_mint: &BASE_MINT,
			quote_mint: &WSOL_MINT,
			partner: &PARTNER,
			platform_config: &PLATFORM_CONFIG,
			quote_token_program: &TOKEN_PROGRAM_ID,
			amount_out: 1_000,
			max_amount_in: 1_100,
		});
		assert_derived_market_accounts(&ix, &WSOL_MINT, &TOKEN_PROGRAM_ID);
	}

	/// A Token-2022 quote mint moves the quote vault, not the base vault.
	#[test]
	fn the_quote_vault_follows_the_quote_token_program() {
		let quote_mint = Address::new_from_array([5u8; 32]);
		let ix = build_buy_exact_in_instruction(&BuildBuyExactInParams {
			user: &USER,
			payer: None,
			base_mint: &BASE_MINT,
			quote_mint: &quote_mint,
			partner: &PARTNER,
			platform_config: &PLATFORM_CONFIG,
			quote_token_program: &TOKEN_2022_PROGRAM_ID,
			amount_in: 1_000,
			min_amount_out: 900,
		});
		assert_derived_market_accounts(
			&ix,
			&quote_mint,
			&TOKEN_2022_PROGRAM_ID,
		);
	}

	/// An indexer holding only the event builds the next trade without a fetch.
	#[test]
	fn buy_exact_in_builds_from_a_trade_event() {
		let (bonding_curve, _) = crate::launchpad::pda::find_bonding_curve_pda(
			&BASE_MINT, &WSOL_MINT,
		);
		let event = TradeEvent {
			bonding_curve,
			base_mint: BASE_MINT,
			quote_mint: WSOL_MINT,
			user: Address::new_from_array([6u8; 32]),
			coin_creator: Address::new_from_array([7u8; 32]),
			creator_fee_config: Address::new_from_array([8u8; 32]),
			platform_config: PLATFORM_CONFIG,
			base_amount: 0,
			quote_amount_gross: 0,
			quote_amount_net: 0,
			trade_direction: TradeDirection::Buy,
			partner: Address::new_from_array([9u8; 32]),
			is_partner: false,
			protocol_fee: 0,
			creator_fee: 0,
			base_transfer_fee: 0,
			quote_transfer_fee: 0,
			virtual_base_reserves: 1_000_000_000_000_000,
			virtual_quote_reserves: 30_000_000_000,
			real_base_reserves: 500_000_000_000_000,
			real_quote_reserves: 0,
			timestamp: 0,
		};

		let (ix, quote) = buy_exact_in(&BuyExactInParams {
			user: &USER,
			payer: None,
			base_mint: &event.base_mint,
			quote_mint: &event.quote_mint,
			virtual_base_reserves: event.virtual_base_reserves,
			virtual_quote_reserves: event.virtual_quote_reserves,
			fee_bps: 100,
			slippage_bps: 100,
			partner: &PARTNER,
			platform_config: &event.platform_config,
			quote_token_program: &TOKEN_PROGRAM_ID,
			quote_fee: None,
			base_fee: None,
			real_base_reserves: event.real_base_reserves,
			quote_amount: 1_000_000_000,
		})
		.unwrap();

		assert_eq!(quote.quote_from_user, 1_000_000_000);
		assert_eq!(quote.fee, 10_000_000);
		assert_eq!(quote.base_to_user, 31_945_788_964_181);

		assert_derived_market_accounts(&ix, &WSOL_MINT, &TOKEN_PROGRAM_ID);
		assert_eq!(
			ix.accounts[BONDING_CURVE_INDEX].pubkey,
			event.bonding_curve
		);
		let (partner_config, _) = crate::nexus::pda::find_partner_config_pda(
			&event.platform_config,
			&PARTNER,
		);
		assert_eq!(ix.accounts[PARTNER_CONFIG_INDEX].pubkey, partner_config);

		assert_eq!(
			ix.data[..8],
			crate::launchpad::instructions::BUY_EXACT_IN_DISCRIMINATOR
		);
		let args = <crate::launchpad::instructions::BuyExactInInstructionArgs as borsh::BorshDeserialize>::try_from_slice(&ix.data[8..]).unwrap();
		assert_eq!(args.amount_in, 1_000_000_000);
		// 31_945_788_964_181 * 9_900 / 10_000, floored.
		assert_eq!(args.min_amount_out, 31_626_331_074_539);
	}
}
