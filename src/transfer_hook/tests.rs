use super::*;

use spl_token_2022_interface::extension::ExtensionType;

use crate::constants::{TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};

const MINT: Address = Address::new_from_array([21; 32]);
const PROGRAM: Address = Address::new_from_array([22; 32]);
const SOURCE: Address = Address::new_from_array([23; 32]);
const DESTINATION: Address = Address::new_from_array([24; 32]);
const USER: Address = Address::new_from_array([5; 32]);
const MARKET: Address = Address::new_from_array([6; 32]);
const RENT: Address = Address::new_from_array([7; 32]);
const OTHER_MINT: Address = Address::new_from_array([8; 32]);

fn list(extras: &[ExtraAccountMeta]) -> Vec<u8> {
	let mut data =
		vec![0; ExtraAccountMetaList::size_of(extras.len()).unwrap()];
	ExtraAccountMetaList::init::<Execute>(&mut data, extras).unwrap();
	data
}

fn fixed(pubkey: Address, writable: bool) -> ExtraAccountMeta {
	ExtraAccountMeta::new_with_pubkey(&pubkey, false, writable).unwrap()
}

fn pda(seeds: &[Seed], writable: bool) -> ExtraAccountMeta {
	ExtraAccountMeta::new_with_seeds(seeds, false, writable).unwrap()
}

fn external_pda(program: u8, seeds: &[Seed]) -> ExtraAccountMeta {
	ExtraAccountMeta::new_external_pda_with_seeds(program, seeds, false, false)
		.unwrap()
}

fn pubkey_data(account: u8, offset: u8, writable: bool) -> ExtraAccountMeta {
	ExtraAccountMeta::new_with_pubkey_data(
		&PubkeyData::AccountData {
			account_index: account,
			data_index: offset,
		},
		false,
		writable,
	)
	.unwrap()
}

/// SPL's constructors refuse these extras.
fn raw(
	discriminator: u8,
	address_config: [u8; 32],
	signer: bool,
) -> ExtraAccountMeta {
	ExtraAccountMeta {
		discriminator,
		address_config,
		is_signer: signer.into(),
		is_writable: false.into(),
	}
}

fn literal(bytes: &[u8]) -> Seed {
	Seed::Literal {
		bytes: bytes.to_vec(),
	}
}

const fn account_key(index: u8) -> Seed {
	Seed::AccountKey { index }
}

const fn account_data(account_index: u8, data_index: u8, length: u8) -> Seed {
	Seed::AccountData {
		account_index,
		data_index,
		length,
	}
}

fn validation(program: &Address) -> Address {
	find_validation_address(&MINT, program)
}

const fn mint_state(transfer_hook: Option<Address>) -> MintState {
	MintState {
		token_program: TOKEN_2022_PROGRAM_ID,
		transfer_fee: None,
		transfer_hook,
		paused: false,
	}
}

/// `(address, owner, data)`.
type Accounts = [(Address, Address, Vec<u8>)];

fn refresh(
	previous: &MarketHooks,
	program: Address,
	accounts: &Accounts,
) -> MarketHooks {
	let plan = previous.plan(
		(&MINT, &mint_state(Some(program))),
		(&OTHER_MINT, &mint_state(None)),
	);
	plan.resolve(|address| {
		accounts
			.iter()
			.find(|(key, ..)| key == address)
			.map(|(_, owner, data)| AccountView { owner, data })
	})
}

fn hook(hooks: &MarketHooks) -> Hook {
	hooks.base().unwrap().clone()
}

fn loaded(data: Vec<u8>) -> Hook {
	hook(&refresh(
		&MarketHooks::default(),
		PROGRAM,
		&[(validation(&PROGRAM), PROGRAM, data)],
	))
}

fn unsupported(data: Vec<u8>) -> String {
	let HookList::Unsupported(reason) = loaded(data).list else {
		panic!("the list parsed");
	};
	reason.to_string()
}

const USER_LEG: Leg = Leg {
	source: SOURCE,
	mint: MINT,
	destination: DESTINATION,
	owner: USER,
	destination_owner: MARKET,
};

const VAULT_LEG: Leg = Leg {
	source: DESTINATION,
	mint: MINT,
	destination: SOURCE,
	owner: MARKET,
	destination_owner: USER,
};

fn hook_pda(seeds: &[&[u8]]) -> Address {
	Address::find_program_address(seeds, &PROGRAM).0
}

/// A fixed account, a writable PDA from the source, and a PDA from the
/// source's owner, read from the source's data.
fn programs_test_list() -> Vec<u8> {
	list(&[
		fixed(RENT, false),
		pda(&[literal(b"ledger"), account_key(0)], true),
		pda(&[literal(b"owner"), account_data(0, 32, 32)], false),
	])
}

#[test]
fn resolves_the_programs_test_hook() {
	let hook = loaded(programs_test_list());
	assert_eq!(*hook.validation(), validation(&PROGRAM));
	assert_eq!(hook.check_routable(Unloaded::Refuse), Ok(()));

	for (leg, source, source_owner) in
		[(USER_LEG, SOURCE, USER), (VAULT_LEG, DESTINATION, MARKET)]
	{
		assert_eq!(
			hook.leg_accounts(&leg, Unloaded::Refuse).unwrap(),
			vec![
				AccountMeta::new_readonly(RENT, false),
				AccountMeta::new(
					hook_pda(&[b"ledger", source.as_ref()]),
					false
				),
				AccountMeta::new_readonly(
					hook_pda(&[b"owner", source_owner.as_ref()]),
					false
				),
				AccountMeta::new_readonly(PROGRAM, false),
				AccountMeta::new_readonly(validation(&PROGRAM), false),
			]
		);
	}
}

/// Extras have indexes after the five `Execute` accounts. The prefix of each
/// token account holds its own owner.
#[test]
fn resolves_prior_extras_data_and_external_programs() {
	let first = hook_pda(&[DESTINATION.as_ref()]);
	let hook = loaded(list(&[
		pda(&[account_key(2)], false),
		external_pda(
			5,
			&[
				account_data(2, 32, 32),
				Seed::InstructionData {
					index: 0,
					length: 8,
				},
			],
		),
		pubkey_data(2, 0, true),
		pda(&[account_data(4, 0, 8)], false),
	]));
	assert_eq!(
		hook.leg_accounts(&USER_LEG, Unloaded::Refuse).unwrap(),
		vec![
			AccountMeta::new_readonly(first, false),
			AccountMeta::new_readonly(
				Address::find_program_address(
					&[MARKET.as_ref(), &EXECUTE_DISCRIMINATOR],
					&first
				)
				.0,
				false
			),
			AccountMeta::new(MINT, false),
			// The first 8 bytes of the list: its `Execute` type.
			AccountMeta::new_readonly(
				hook_pda(&[&EXECUTE_DISCRIMINATOR]),
				false
			),
			AccountMeta::new_readonly(PROGRAM, false),
			AccountMeta::new_readonly(validation(&PROGRAM), false),
		]
	);
}

#[test]
fn refuses_what_only_the_swap_can_resolve() {
	for (extra, reason) in [
		(
			pda(
				&[Seed::InstructionData {
					index: 8,
					length: 8,
				}],
				false,
			),
			"a seed reads the transfer amount",
		),
		(
			pda(&[account_data(0, 64, 8)], false),
			"an extra reads account data only the swap can see",
		),
		(
			pda(&[account_data(3, 0, 32)], false),
			"an extra reads account data only the swap can see",
		),
		(
			pubkey_data(1, 0, false),
			"an extra reads account data only the swap can see",
		),
		(
			pda(&[account_key(5)], false),
			"an extra names an account not yet resolved",
		),
		(
			external_pda(5, &[]),
			"an extra names an account not yet resolved",
		),
		(
			raw(META_PUBKEY_DATA, [1; 32], false),
			"an extra reads a pubkey past the end of the hook's instruction data",
		),
		(raw(META_FIXED, RENT.to_bytes(), true), "an extra must sign"),
		(raw(3, [0; 32], false), "malformed extra"),
		(raw(META_PUBKEY_DATA, [0; 32], false), "malformed extra"),
		(raw(META_HOOK_PDA, [9; 32], false), "malformed seed"),
		(
			pda(&[account_data(4, 0, 33)], false),
			"seed longer than 32 bytes",
		),
	] {
		assert_eq!(unsupported(list(&[extra])), reason);
	}

	assert_eq!(unsupported(vec![9; 20]), "malformed list");
	// SPL reports a missing type only at zeroed slack. Other data is malformed.
	let mut other = [9; 8].to_vec();
	other.extend_from_slice(&0_u32.to_le_bytes());
	assert_eq!(unsupported(other.clone()), "malformed list");
	other.extend_from_slice(&[0; 12]);
	assert_eq!(unsupported(other), "no Execute entry in the list");
	// Empty data has no TLV state.
	assert_eq!(unsupported(Vec::new()), "malformed list");
	let mut short = list(&[fixed(RENT, false)]);
	short.truncate(short.len() - 1);
	assert_eq!(unsupported(short), "malformed list");
	// A count above the stored metas.
	let mut overcounted = list(&[fixed(RENT, false)]);
	overcounted[12] = 2;
	assert_eq!(unsupported(overcounted), "malformed list");
	let mut partial = list(&[fixed(RENT, false)]);
	partial[8] += 1;
	partial.push(0);
	assert_eq!(unsupported(partial), "malformed list");
	// A malformed entry after a good `Execute` one still fails the list.
	let mut trailing = list(&[fixed(RENT, false)]);
	trailing.extend_from_slice(&[9; 8]);
	trailing.extend_from_slice(&100_u32.to_le_bytes());
	assert_eq!(unsupported(trailing), "malformed list");

	let hook = loaded(list(&[pda(&[account_key(9)], false)]));
	assert_eq!(
		hook.check_routable(Unloaded::Refuse)
			.unwrap_err()
			.to_string(),
		"transfer hook cannot be resolved ahead of the swap: an extra \
		 names an account not yet resolved"
	);
	// `StandIn` accepts a list that has not loaded. It does not accept a list
	// that cannot resolve.
	assert_eq!(
		hook.check_routable(Unloaded::StandIn),
		Err(HookError::Unresolvable(
			Unresolvable::NamesUnresolvedAccount
		))
	);
}

/// SPL's resolver panics on 16 seeds. The screen refuses them first.
#[test]
fn a_pda_past_the_seed_limit_is_refused_before_it_can_panic() {
	let seeds = |count: usize| vec![account_key(0); count];
	assert_eq!(
		unsupported(list(&[pda(&seeds(16), false)])),
		"a PDA has more seeds than an address allows"
	);
	assert_eq!(
		unsupported(list(&[external_pda(0, &seeds(16))])),
		"a PDA has more seeds than an address allows"
	);

	let hook = loaded(list(&[pda(&seeds(15), false)]));
	let source = SOURCE.to_bytes();
	assert_eq!(
		hook.leg_accounts(&USER_LEG, Unloaded::Refuse).unwrap()[0],
		AccountMeta::new_readonly(hook_pda(&[source.as_slice(); 15]), false)
	);
}

#[test]
fn a_list_longer_than_an_index_can_name_is_refused() {
	let extras = vec![fixed(RENT, false); 252];
	assert_eq!(unsupported(list(&extras)), "list too long");
	assert!(matches!(
		loaded(list(&extras[..251])).list,
		HookList::Ready(_)
	));
}

#[test]
fn finds_the_execute_entry_among_others() {
	let mut data = [1; 8].to_vec();
	data.extend_from_slice(&3_u32.to_le_bytes());
	data.extend_from_slice(&[0; 3]);
	data.extend_from_slice(&list(&[fixed(RENT, true)]));
	data.extend_from_slice(&[0; 40]);
	let hook = loaded(data);
	assert_eq!(
		hook.leg_accounts(&USER_LEG, Unloaded::Refuse).unwrap()[0],
		AccountMeta::new(RENT, false)
	);
}

#[test]
fn a_missing_list_is_pending_until_it_loads() {
	let pending = refresh(&MarketHooks::default(), PROGRAM, &[]);
	let hook = hook(&pending);
	assert_eq!(hook.list, HookList::Pending);
	assert_eq!(
		hook.check_routable(Unloaded::Refuse)
			.unwrap_err()
			.to_string(),
		"transfer hook accounts not loaded"
	);
	// `StandIn` gives the real hook program and validation addresses.
	assert_eq!(
		hook.leg_accounts(&USER_LEG, Unloaded::StandIn).unwrap(),
		vec![
			AccountMeta::new_readonly(PROGRAM, false),
			AccountMeta::new_readonly(validation(&PROGRAM), false),
		]
	);

	let stale = MarketHooks {
		base: Some(Hook {
			validation: RENT,
			..hook
		}),
		quote: None,
	};
	assert_eq!(
		*refresh(&stale, PROGRAM, &[]).base().unwrap().validation(),
		RENT
	);
	assert_eq!(
		*refresh(&stale, MARKET, &[]).base().unwrap().validation(),
		validation(&MARKET)
	);
}

#[test]
fn a_list_still_missing_after_it_was_asked_for_stops_routing() {
	let first = refresh(&MarketHooks::default(), PROGRAM, &[]);
	assert_eq!(hook(&first).list, HookList::Pending);

	let listless = refresh(&first, PROGRAM, &[]);
	assert_eq!(hook(&listless).list, HookList::Absent);
	for unloaded in [Unloaded::Refuse, Unloaded::StandIn] {
		assert_eq!(
			listless.check_routable(unloaded).unwrap_err().to_string(),
			"transfer hook has no ExtraAccountMetaList"
		);
	}
	assert_eq!(
		hook(&listless).leg_accounts(&USER_LEG, Unloaded::Refuse),
		Err(HookError::NoList)
	);
	assert_eq!(
		hook(&refresh(&listless, PROGRAM, &[])).list,
		HookList::Absent
	);
	assert_eq!(
		hook(&refresh(&listless, MARKET, &[])).list,
		HookList::Pending
	);

	// A list that appears later loads.
	let appeared = refresh(
		&listless,
		PROGRAM,
		&[(validation(&PROGRAM), PROGRAM, list(&[fixed(RENT, false)]))],
	);
	assert_eq!(
		hook(&appeared)
			.leg_accounts(&USER_LEG, Unloaded::Refuse)
			.unwrap()
			.len(),
		3
	);
}

/// A hook that reads its list fails a swap that has only its program.
#[test]
fn a_loaded_list_that_goes_missing_waits_a_refresh() {
	let ready = MarketHooks {
		base: Some(loaded(list(&[fixed(RENT, false)]))),
		quote: None,
	};
	let missed = refresh(&ready, PROGRAM, &[]);
	assert_eq!(hook(&missed).list, HookList::Pending);
	assert_eq!(
		missed.check_routable(Unloaded::Refuse),
		Err(HookError::NotLoaded)
	);
	let gone = refresh(&missed, PROGRAM, &[]);
	assert_eq!(hook(&gone).list, HookList::Absent);
	assert_eq!(
		gone.check_routable(Unloaded::StandIn),
		Err(HookError::NoList)
	);

	let unsupported = MarketHooks {
		base: Some(loaded(Vec::new())),
		quote: None,
	};
	assert!(unsupported.check_routable(Unloaded::Refuse).is_err());
	assert_eq!(
		hook(&refresh(&unsupported, PROGRAM, &[])).list,
		HookList::Pending
	);
}

/// Token-2022 ignores a validation account that the hook program does not own.
#[test]
fn an_account_the_hook_program_does_not_own_is_no_list() {
	let squatted = [(validation(&PROGRAM), Address::default(), Vec::new())];
	let first = refresh(&MarketHooks::default(), PROGRAM, &squatted);
	assert_eq!(hook(&first).list, HookList::Pending);

	let listless = refresh(&first, PROGRAM, &squatted);
	assert_eq!(hook(&listless).list, HookList::Absent);
}

#[test]
fn the_plan_names_each_armed_hooks_list() {
	let other = Address::new_from_array([9; 32]);
	let plan = MarketHooks::default().plan(
		(&MINT, &mint_state(Some(PROGRAM))),
		(&OTHER_MINT, &mint_state(Some(other))),
	);
	assert_eq!(
		plan.accounts().collect::<Vec<_>>(),
		vec![
			validation(&PROGRAM),
			find_validation_address(&OTHER_MINT, &other)
		]
	);
	let unhooked = MarketHooks::default()
		.plan((&MINT, &mint_state(None)), (&OTHER_MINT, &mint_state(None)));
	assert_eq!(unhooked.accounts().count(), 0);
	assert_eq!(unhooked.resolve(|_| None), MarketHooks::default());
}

#[test]
fn trailing_accounts_merge_by_key_and_refuse_the_trades_own() {
	let hook = loaded(list(&[fixed(RENT, false), fixed(MARKET, false)]));
	let fixed_metas = [AccountMeta::new_readonly(MINT, false)];

	assert_eq!(
		trailing_accounts(
			&[(Some(&hook), USER_LEG), (None, VAULT_LEG)],
			&fixed_metas,
			Unloaded::Refuse,
		)
		.unwrap(),
		hook.leg_accounts(&USER_LEG, Unloaded::Refuse).unwrap()
	);

	let writable_rent = loaded(list(&[fixed(RENT, true)]));
	assert_eq!(
		trailing_accounts(
			&[(Some(&hook), USER_LEG), (Some(&writable_rent), VAULT_LEG)],
			&fixed_metas,
			Unloaded::Refuse,
		)
		.unwrap(),
		vec![
			AccountMeta::new(RENT, false),
			AccountMeta::new_readonly(MARKET, false),
			AccountMeta::new_readonly(PROGRAM, false),
			AccountMeta::new_readonly(validation(&PROGRAM), false),
		]
	);

	assert!(
		trailing_accounts(
			&[(Some(&hook), USER_LEG)],
			&[AccountMeta::new_readonly(RENT, false)],
			Unloaded::Refuse,
		)
		.is_ok()
	);
	// A repeat that is writable on either side fails.
	for (extras, fixed) in [
		(&hook, AccountMeta::new(MARKET, false)),
		(&writable_rent, AccountMeta::new_readonly(RENT, false)),
	] {
		assert_eq!(
			trailing_accounts(
				&[(Some(extras), USER_LEG)],
				&[fixed],
				Unloaded::Refuse,
			)
			.unwrap_err()
			.to_string(),
			"transfer hook repeats an account of the trade, writable"
		);
	}
	assert_eq!(
		trailing_accounts(
			&[(Some(&hook), USER_LEG)],
			&[AccountMeta::new_readonly(RENT, true)],
			Unloaded::Refuse,
		)
		.unwrap_err()
		.to_string(),
		"transfer hook needs a signer the programs do not forward"
	);
}

fn trade() -> TradeAccounts {
	TradeAccounts {
		user: USER,
		payer: Address::new_from_array([10; 32]),
		market: MARKET,
		base_mint: OTHER_MINT,
		quote_mint: MINT,
		base_vault: Address::new_from_array([11; 32]),
		quote_vault: DESTINATION,
		user_base_account: Address::new_from_array([12; 32]),
		user_quote_account: SOURCE,
		partner: Address::new_from_array([13; 32]),
		partner_config: Address::new_from_array([14; 32]),
		quote_token_program: TOKEN_2022_PROGRAM_ID,
	}
}

#[test]
fn each_direction_hooks_its_own_leg() {
	let quote_hooked = MarketHooks {
		base: None,
		quote: Some(loaded(programs_test_list())),
	};
	let trailing = |direction| {
		quote_hooked
			.swap_accounts(
				HookSwap {
					trade: &trade(),
					direction,
					fixed: &[],
				},
				Unloaded::Refuse,
			)
			.unwrap()
	};
	assert_eq!(
		trailing(TradeDirection::Buy),
		loaded(programs_test_list())
			.leg_accounts(&USER_LEG, Unloaded::Refuse)
			.unwrap()
	);
	assert_eq!(
		trailing(TradeDirection::Sell),
		loaded(programs_test_list())
			.leg_accounts(&VAULT_LEG, Unloaded::Refuse)
			.unwrap()
	);
	assert_eq!(quote_hooked.programs(), vec![PROGRAM]);
}

#[test]
fn screening_unroutes_only_a_clash_every_trader_hits() {
	let stand_in = trade().with_stand_in_trader();
	let fixed_metas = [
		AccountMeta::new_readonly(stand_in.user, true),
		AccountMeta::new(stand_in.quote_vault, false),
	];
	let screened = |extra| {
		MarketHooks {
			base: None,
			quote: Some(loaded(list(&[extra]))),
		}
		.screen(&stand_in, &fixed_metas)
	};

	assert_eq!(
		screened(fixed(stand_in.quote_vault, false))
			.check_routable(Unloaded::Refuse)
			.unwrap_err()
			.to_string(),
		"transfer hook cannot be resolved ahead of the swap: transfer hook \
		 repeats an account of the trade, writable"
	);
	// On a buy, the source owner is the trader, who signs.
	assert_eq!(
		screened(pubkey_data(0, 32, false)).check_routable(Unloaded::Refuse),
		Err(HookError::Unresolvable(Unresolvable::Clash(
			Clash::SignerNotForwarded
		)))
	);

	let one_wallet = screened(fixed(USER, false));
	assert_eq!(one_wallet.check_routable(Unloaded::Refuse), Ok(()));
	assert_eq!(
		one_wallet.swap_accounts(
			HookSwap {
				trade: &trade(),
				direction: TradeDirection::Buy,
				fixed: &[AccountMeta::new_readonly(USER, true)],
			},
			Unloaded::Refuse,
		),
		Err(HookError::Clash(Clash::SignerNotForwarded))
	);
}

/// A Token-2022 mint with `extensions` as `(type, payload)`.
fn token_2022_mint(extensions: &[(u16, Vec<u8>)]) -> Vec<u8> {
	let mut data = vec![0; 165];
	// `is_initialized`, then `AccountType::Mint`.
	data[45] = 1;
	data.push(1);
	for (extension_type, payload) in extensions {
		data.extend_from_slice(&extension_type.to_le_bytes());
		data.extend_from_slice(
			&u16::try_from(payload.len()).unwrap().to_le_bytes(),
		);
		data.extend_from_slice(payload);
	}
	data
}

fn load(owner: &Address, data: &[u8]) -> Result<MintState, ProgramError> {
	MintState::load(AccountView { owner, data })
}

fn hook_extension(program_id: [u8; 32]) -> (u16, Vec<u8>) {
	(
		ExtensionType::TransferHook.into(),
		[[7; 32], program_id].concat(),
	)
}

fn pausable(paused: bool) -> (u16, Vec<u8>) {
	let mut payload = vec![7; 32];
	payload.push(u8::from(paused));
	(ExtensionType::Pausable.into(), payload)
}

/// `MetadataPointer`, then `TokenMetadata`. `create_token` writes both.
fn metadata() -> [(u16, Vec<u8>); 2] {
	[(18, vec![0; 64]), (19, vec![0; 90])]
}

#[test]
fn a_launchpad_base_mint_passes() {
	assert_eq!(
		load(&TOKEN_2022_PROGRAM_ID, &token_2022_mint(&metadata())),
		Ok(mint_state(None))
	);
}

#[test]
fn an_spl_mint_passes() {
	assert_eq!(
		load(&TOKEN_PROGRAM_ID, &[0; 82]),
		Ok(MintState {
			token_program: TOKEN_PROGRAM_ID,
			..mint_state(None)
		})
	);
}

/// Every mainnet xStock has an unset hook.
#[test]
fn an_unset_hook_is_no_hook() {
	let [pointer, metadata] = metadata();
	let data = token_2022_mint(&[pointer, hook_extension([0; 32]), metadata]);
	assert_eq!(
		load(&TOKEN_2022_PROGRAM_ID, &data).unwrap().transfer_hook,
		None
	);
}

#[test]
fn a_set_hook_reads_its_program() {
	let [pointer, metadata] = metadata();
	let data = token_2022_mint(&[pointer, hook_extension([9; 32]), metadata]);
	assert_eq!(
		load(&TOKEN_2022_PROGRAM_ID, &data),
		Ok(mint_state(Some(Address::new_from_array([9; 32]))))
	);
}

#[test]
fn a_pause_holds_and_a_resume_clears() {
	let paused = token_2022_mint(&[hook_extension([9; 32]), pausable(true)]);
	let state = load(&TOKEN_2022_PROGRAM_ID, &paused).unwrap();
	assert!(state.paused);
	assert_eq!(state.transfer_hook, Some(Address::new_from_array([9; 32])));
	let resumed = token_2022_mint(&[pausable(false)]);
	assert!(!load(&TOKEN_2022_PROGRAM_ID, &resumed).unwrap().paused);
}

#[test]
fn a_cut_extension_is_malformed() {
	let mut data = token_2022_mint(&[hook_extension([9; 32])]);
	data.truncate(data.len() - 1);
	assert_eq!(
		load(&TOKEN_2022_PROGRAM_ID, &data),
		Err(ProgramError::InvalidAccountData)
	);
	// Whole entries, each one byte shorter than the payload of its type.
	for (extension_type, mut payload) in
		[hook_extension([9; 32]), pausable(true)]
	{
		payload.pop();
		assert_eq!(
			load(
				&TOKEN_2022_PROGRAM_ID,
				&token_2022_mint(&[(extension_type, payload)])
			),
			Err(ProgramError::InvalidArgument),
			"type {extension_type}"
		);
	}
	assert_eq!(
		load(&MINT, &token_2022_mint(&[])),
		Err(ProgramError::IncorrectProgramId)
	);
}

/// A type that SPL 3.1.1 does not define must not read as a malformed region.
#[test]
fn a_future_extension_type_loads_as_no_extension() {
	let [pointer, _] = metadata();
	let data = token_2022_mint(&[pointer, (250, vec![0; 16])]);
	assert_eq!(load(&TOKEN_2022_PROGRAM_ID, &data), Ok(mint_state(None)));
}
