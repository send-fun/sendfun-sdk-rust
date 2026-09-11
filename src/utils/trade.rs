use solana_address::Address;

use super::pda::find_associated_token_pda;
use crate::constants::TOKEN_2022_PROGRAM_ID;

pub struct TradeStateFields<'a> {
	pub base_mint: &'a Address,
	pub quote_mint: &'a Address,
	/// The market's (curve or pool), never the trade's; seeds `partner_config`.
	pub platform_config: &'a Address,
}

pub struct DerivedTradeAccounts {
	pub user_base_account: Address,
	pub user_quote_account: Address,
	pub partner_config: Address,
	pub event_authority: Address,
}

pub struct TradeUserContext<'a> {
	pub user: &'a Address,
	pub partner: &'a Address,
	pub quote_token_program: &'a Address,
}

pub struct DeriveTradeArgs<'a> {
	pub state: &'a TradeStateFields<'a>,
	pub ctx: TradeUserContext<'a>,
	pub event_authority: Address,
}

pub fn derive_trade_accounts(
	args: DeriveTradeArgs<'_>,
) -> DerivedTradeAccounts {
	let state = args.state;
	let user = args.ctx.user;
	let partner = args.ctx.partner;
	let quote_token_program = args.ctx.quote_token_program;
	let event_authority = args.event_authority;

	let (user_base_account, _) = find_associated_token_pda(
		user,
		state.base_mint,
		&TOKEN_2022_PROGRAM_ID,
	);
	let (user_quote_account, _) =
		find_associated_token_pda(user, state.quote_mint, quote_token_program);
	let (partner_config, _) = crate::nexus::pda::find_partner_config_pda(
		state.platform_config,
		partner,
	);

	DerivedTradeAccounts {
		user_base_account,
		user_quote_account,
		partner_config,
		event_authority,
	}
}
