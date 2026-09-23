use super::types::TradeDirection;
use crate::constants::{
	ATA_PROGRAM_ID, LAUNCHPAD_PROGRAM_ID, SYSTEM_PROGRAM_ID,
	TOKEN_2022_PROGRAM_ID,
};
use crate::math::amm::{self, QuoteError};
use crate::utils::{
	DeriveTradeArgs, TradeAccounts, TradeMode, TradeStateFields,
	TradeUserContext, derive_trade_accounts, partner_account,
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
	/// The quote mint's transfer fee at the landing epoch. `None` only for a
	/// mint without a transfer fee. A wrong value gives a wrong slippage limit.
	pub quote_fee: Option<amm::MintFee>,
	/// The base mint's transfer fee. Same rules as `quote_fee`.
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
		// The program caps every buy at the supply left on the curve.
		base_reserve_cap: Some(p.real_base_reserves),
	})?;
	// The program applies `min_amount_out` to the base that the buyer receives.
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
	/// The quote mint's transfer fee at the landing epoch. `None` only for a
	/// mint without a transfer fee. A wrong value gives a wrong slippage limit.
	pub quote_fee: Option<amm::MintFee>,
	/// The base mint's transfer fee. Same rules as `quote_fee`.
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
	// The program applies `max_amount_in` to the quote that the buyer sends,
	// with the transfer fee.
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
		// The capped amount. The program refuses an `amount_out` above the
		// supply left.
		amount_out: q.base_to_user,
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
	/// The quote mint's transfer fee at the landing epoch. `None` only for a
	/// mint without a transfer fee. A wrong value gives a wrong slippage limit.
	pub quote_fee: Option<amm::MintFee>,
	/// The base mint's transfer fee. Same rules as `quote_fee`.
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
	// The program applies `min_amount_out` to the quote that the seller
	// receives.
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
	/// The quote mint's transfer fee at the landing epoch. `None` only for a
	/// mint without a transfer fee. A wrong value gives a wrong slippage limit.
	pub quote_fee: Option<amm::MintFee>,
	/// The base mint's transfer fee. Same rules as `quote_fee`.
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
	// The program applies `max_amount_in` to the base that the seller sends,
	// with the transfer fee.
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
	launchpad_trade_accounts!(
		super::instructions::BuyExactIn,
		&derived_trade_accounts!(p)
	)
	.instruction(super::instructions::BuyExactInInstructionArgs {
		amount_in: p.amount_in,
		min_amount_out: p.min_amount_out,
	})
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
	launchpad_trade_accounts!(
		super::instructions::BuyExactOut,
		&derived_trade_accounts!(p)
	)
	.instruction(super::instructions::BuyExactOutInstructionArgs {
		amount_out: p.amount_out,
		max_amount_in: p.max_amount_in,
	})
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
	launchpad_trade_accounts!(
		super::instructions::SellExactIn,
		&derived_trade_accounts!(p)
	)
	.instruction(super::instructions::SellExactInInstructionArgs {
		amount_in: p.amount_in,
		min_amount_out: p.min_amount_out,
	})
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
	launchpad_trade_accounts!(
		super::instructions::SellExactOut,
		&derived_trade_accounts!(p)
	)
	.instruction(super::instructions::SellExactOutInstructionArgs {
		amount_out: p.amount_out,
		max_amount_in: p.max_amount_in,
	})
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TradeArgs {
	pub direction: TradeDirection,
	pub mode: TradeMode,
	/// `amount_in` for `ExactIn`. `amount_out` for `ExactOut`.
	pub amount: u64,
	/// `min_amount_out` for `ExactIn`. `max_amount_in` for `ExactOut`.
	pub limit: u64,
}

#[must_use]
pub fn trade_instruction(
	accounts: &TradeAccounts,
	args: TradeArgs,
) -> solana_instruction::Instruction {
	let TradeArgs {
		direction,
		mode,
		amount,
		limit,
	} = args;
	match (direction, mode) {
		(TradeDirection::Buy, TradeMode::ExactIn) => {
			launchpad_trade_accounts!(super::instructions::BuyExactIn, accounts)
				.instruction(super::instructions::BuyExactInInstructionArgs {
					amount_in: amount,
					min_amount_out: limit,
				})
		}
		(TradeDirection::Buy, TradeMode::ExactOut) => {
			launchpad_trade_accounts!(
				super::instructions::BuyExactOut,
				accounts
			)
			.instruction(super::instructions::BuyExactOutInstructionArgs {
				amount_out: amount,
				max_amount_in: limit,
			})
		}
		(TradeDirection::Sell, TradeMode::ExactIn) => {
			launchpad_trade_accounts!(
				super::instructions::SellExactIn,
				accounts
			)
			.instruction(super::instructions::SellExactInInstructionArgs {
				amount_in: amount,
				min_amount_out: limit,
			})
		}
		(TradeDirection::Sell, TradeMode::ExactOut) => {
			launchpad_trade_accounts!(
				super::instructions::SellExactOut,
				accounts
			)
			.instruction(super::instructions::SellExactOutInstructionArgs {
				amount_out: amount,
				max_amount_in: limit,
			})
		}
	}
}

#[must_use]
pub fn trade_account_metas(
	accounts: &TradeAccounts,
) -> Vec<solana_instruction::AccountMeta> {
	launchpad_trade_accounts!(super::instructions::BuyExactIn, accounts)
		.instruction(super::instructions::BuyExactInInstructionArgs {
			amount_in: 0,
			min_amount_out: 0,
		})
		.accounts
}

#[must_use]
pub fn trade_data(args: TradeArgs) -> Vec<u8> {
	// Default accounts: `trade_instruction` must not check its accounts.
	trade_instruction(&TradeAccounts::default(), args).data
}

macro_rules! launchpad_trade_accounts {
	($ty:path, $accounts:expr) => {{
		let a: &TradeAccounts = $accounts;
		$ty {
			user: a.user,
			payer: a.payer,
			bonding_curve: a.market,
			base_mint: a.base_mint,
			quote_mint: a.quote_mint,
			base_vault: a.base_vault,
			quote_vault: a.quote_vault,
			user_base_account: a.user_base_account,
			user_quote_account: a.user_quote_account,
			partner: partner_account(&a.partner),
			partner_config: a.partner_config,
			base_token_program: TOKEN_2022_PROGRAM_ID,
			quote_token_program: a.quote_token_program,
			associated_token_program: ATA_PROGRAM_ID,
			system_program: SYSTEM_PROGRAM_ID,
			event_authority: super::pda::EVENT_AUTHORITY_ADDRESS,
			program: LAUNCHPAD_PROGRAM_ID,
		}
	}};
}
use launchpad_trade_accounts;

macro_rules! derived_trade_accounts {
	($p:expr) => {
		derive(
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
			$p.payer,
		)
	};
}
use derived_trade_accounts;

fn derive(
	state: &TradeStateFields<'_>,
	ctx: TradeUserContext<'_>,
	payer: Option<&Address>,
) -> TradeAccounts {
	let (bonding_curve, _) =
		super::pda::find_bonding_curve_pda(state.base_mint, state.quote_mint);
	// Every base mint is a Token-2022 mint.
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
	let user = *ctx.user;
	let payer = *payer.unwrap_or(ctx.user);
	let partner = *ctx.partner;
	let quote_token_program = *ctx.quote_token_program;
	let trade = derive_trade_accounts(DeriveTradeArgs { state, ctx });
	TradeAccounts {
		user,
		payer,
		market: bonding_curve,
		base_mint: *state.base_mint,
		quote_mint: *state.quote_mint,
		base_vault,
		quote_vault,
		user_base_account: trade.user_base_account,
		user_quote_account: trade.user_quote_account,
		partner,
		partner_config: trade.partner_config,
		quote_token_program,
	}
}

#[cfg(test)]
mod tests {
	use solana_address::Address;

	use super::{
		BuildBuyExactInParams, BuildBuyExactOutParams, BuildSellExactInParams,
		BuildSellExactOutParams, BuyExactInParams, BuyExactOutParams,
		TradeArgs, build_buy_exact_in_instruction,
		build_buy_exact_out_instruction, build_sell_exact_in_instruction,
		build_sell_exact_out_instruction, buy_exact_in, buy_exact_out,
		trade_account_metas, trade_data, trade_instruction,
	};
	use crate::constants::{
		ATA_PROGRAM_ID, DEFAULT_PARTNER, LAUNCHPAD_PROGRAM_ID,
		SYSTEM_PROGRAM_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID, WSOL_MINT,
	};
	use crate::launchpad::events::TradeEvent;
	use crate::launchpad::types::TradeDirection;
	use crate::math::amm::MintFee;
	use crate::utils::{TradeAccounts, TradeMode};
	use solana_instruction::AccountMeta;

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

	/// The quote token program changes the quote vault address, not the base
	/// vault address.
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

	/// Builds a trade from a `TradeEvent` alone.
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

	fn signed_buy_exact_out(
		real_base_reserves: u64,
		base_fee: Option<MintFee>,
		base_amount: u64,
	) -> (u64, crate::math::amm::BuyQuote) {
		let (ix, quote) = buy_exact_out(&BuyExactOutParams {
			user: &USER,
			payer: None,
			base_mint: &BASE_MINT,
			quote_mint: &WSOL_MINT,
			virtual_base_reserves: 1_000_000_000_000_000,
			virtual_quote_reserves: 30_000_000_000,
			fee_bps: 100,
			slippage_bps: 100,
			partner: &PARTNER,
			platform_config: &PLATFORM_CONFIG,
			quote_token_program: &TOKEN_PROGRAM_ID,
			quote_fee: None,
			base_fee,
			real_base_reserves,
			base_amount,
		})
		.unwrap();
		assert_eq!(
			ix.data[..8],
			crate::launchpad::instructions::BUY_EXACT_OUT_DISCRIMINATOR
		);
		let args = <crate::launchpad::instructions::BuyExactOutInstructionArgs as borsh::BorshDeserialize>::try_from_slice(&ix.data[8..]).unwrap();
		(args.amount_out, quote)
	}

	#[test]
	fn buy_exact_out_signs_an_uncapped_request_unchanged() {
		let (amount_out, quote) =
			signed_buy_exact_out(500_000_000_000_000, None, 10_000_000_000);
		assert_eq!(quote.base_to_user, 10_000_000_000);
		assert_eq!(amount_out, 10_000_000_000);
	}

	#[test]
	fn buy_exact_out_clamps_a_request_over_the_supply_left() {
		let (amount_out, quote) =
			signed_buy_exact_out(1_000_000_000, None, 5_000_000_000);
		assert_eq!(quote.base_to_user, 1_000_000_000);
		assert_eq!(amount_out, 1_000_000_000);
	}

	#[test]
	fn buy_exact_out_clamps_to_the_net_under_a_base_transfer_fee() {
		let (amount_out, quote) = signed_buy_exact_out(
			1_000_000_000,
			Some(MintFee {
				bps: 20,
				maximum_fee: u64::MAX,
			}),
			5_000_000_000,
		);
		assert_eq!(quote.base_amount, 1_000_000_000);
		assert_eq!(quote.base_to_user, 998_000_000);
		assert_eq!(amount_out, 998_000_000);
	}

	const AMOUNT: u64 = 1_234_567;
	const LIMIT: u64 = 7_654_321;

	/// Each key is different, so a wrong field mapping fails the test.
	const ACCOUNTS: TradeAccounts = TradeAccounts {
		user: Address::new_from_array([11u8; 32]),
		payer: Address::new_from_array([12u8; 32]),
		market: Address::new_from_array([13u8; 32]),
		base_mint: Address::new_from_array([14u8; 32]),
		quote_mint: Address::new_from_array([15u8; 32]),
		base_vault: Address::new_from_array([16u8; 32]),
		quote_vault: Address::new_from_array([17u8; 32]),
		user_base_account: Address::new_from_array([18u8; 32]),
		user_quote_account: Address::new_from_array([19u8; 32]),
		partner: Address::new_from_array([20u8; 32]),
		partner_config: Address::new_from_array([21u8; 32]),
		quote_token_program: Address::new_from_array([22u8; 32]),
	};

	const TRADES: [(TradeDirection, TradeMode); 4] = [
		(TradeDirection::Buy, TradeMode::ExactIn),
		(TradeDirection::Buy, TradeMode::ExactOut),
		(TradeDirection::Sell, TradeMode::ExactIn),
		(TradeDirection::Sell, TradeMode::ExactOut),
	];

	const fn trade_args(
		direction: TradeDirection,
		mode: TradeMode,
	) -> TradeArgs {
		TradeArgs {
			direction,
			mode,
			amount: AMOUNT,
			limit: LIMIT,
		}
	}

	macro_rules! by_hand {
		($ty:ident) => {
			crate::launchpad::instructions::$ty {
				user: ACCOUNTS.user,
				payer: ACCOUNTS.payer,
				bonding_curve: ACCOUNTS.market,
				base_mint: ACCOUNTS.base_mint,
				quote_mint: ACCOUNTS.quote_mint,
				base_vault: ACCOUNTS.base_vault,
				quote_vault: ACCOUNTS.quote_vault,
				user_base_account: ACCOUNTS.user_base_account,
				user_quote_account: ACCOUNTS.user_quote_account,
				partner: (ACCOUNTS.partner, true),
				partner_config: ACCOUNTS.partner_config,
				base_token_program: TOKEN_2022_PROGRAM_ID,
				quote_token_program: ACCOUNTS.quote_token_program,
				associated_token_program: ATA_PROGRAM_ID,
				system_program: SYSTEM_PROGRAM_ID,
				event_authority: crate::launchpad::pda::EVENT_AUTHORITY_ADDRESS,
				program: LAUNCHPAD_PROGRAM_ID,
			}
		};
	}

	#[test]
	fn trade_instruction_maps_every_account_onto_each_builder() {
		use crate::launchpad::instructions::{
			BuyExactInInstructionArgs, BuyExactOutInstructionArgs,
			SellExactInInstructionArgs, SellExactOutInstructionArgs,
		};
		let expected = [
			by_hand!(BuyExactIn).instruction(BuyExactInInstructionArgs {
				amount_in: AMOUNT,
				min_amount_out: LIMIT,
			}),
			by_hand!(BuyExactOut).instruction(BuyExactOutInstructionArgs {
				amount_out: AMOUNT,
				max_amount_in: LIMIT,
			}),
			by_hand!(SellExactIn).instruction(SellExactInInstructionArgs {
				amount_in: AMOUNT,
				min_amount_out: LIMIT,
			}),
			by_hand!(SellExactOut).instruction(SellExactOutInstructionArgs {
				amount_out: AMOUNT,
				max_amount_in: LIMIT,
			}),
		];
		for ((direction, mode), expected) in TRADES.into_iter().zip(expected) {
			assert_eq!(
				trade_instruction(&ACCOUNTS, trade_args(direction, mode)),
				expected,
				"{direction:?} {mode:?}"
			);
		}
	}

	#[test]
	fn all_four_trades_share_the_account_metas() {
		for (direction, mode) in TRADES {
			assert_eq!(
				trade_account_metas(&ACCOUNTS),
				trade_instruction(&ACCOUNTS, trade_args(direction, mode))
					.accounts,
				"{direction:?} {mode:?}"
			);
		}
	}

	#[test]
	fn trade_data_selects_each_trades_discriminator() {
		use crate::launchpad::instructions::{
			BUY_EXACT_IN_DISCRIMINATOR, BUY_EXACT_OUT_DISCRIMINATOR,
			SELL_EXACT_IN_DISCRIMINATOR, SELL_EXACT_OUT_DISCRIMINATOR,
		};
		let discriminators = [
			BUY_EXACT_IN_DISCRIMINATOR,
			BUY_EXACT_OUT_DISCRIMINATOR,
			SELL_EXACT_IN_DISCRIMINATOR,
			SELL_EXACT_OUT_DISCRIMINATOR,
		];
		for ((direction, mode), discriminator) in
			TRADES.into_iter().zip(discriminators)
		{
			assert_eq!(
				trade_data(trade_args(direction, mode)),
				[discriminator, AMOUNT.to_le_bytes(), LIMIT.to_le_bytes()]
					.concat(),
				"{direction:?} {mode:?}"
			);
		}
	}

	#[test]
	fn trade_data_ignores_the_accounts() {
		for (direction, mode) in TRADES {
			let args = trade_args(direction, mode);
			assert_eq!(
				trade_data(args),
				trade_instruction(&ACCOUNTS, args).data
			);
		}
	}

	/// `DEFAULT_PARTNER` is the system program. It cannot sign.
	#[test]
	fn only_a_partner_other_than_the_default_signs() {
		let unpartnered = TradeAccounts {
			partner: DEFAULT_PARTNER,
			..ACCOUNTS
		};
		assert_eq!(
			trade_account_metas(&unpartnered)[PARTNER_INDEX],
			AccountMeta::new_readonly(DEFAULT_PARTNER, false)
		);
		assert_eq!(
			trade_account_metas(&ACCOUNTS)[PARTNER_INDEX],
			AccountMeta::new_readonly(ACCOUNTS.partner, true)
		);
	}
}
