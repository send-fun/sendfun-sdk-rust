//! Resolves the Token-2022 transfer-hook accounts of a trade from accounts that
//! you fetch. A list that reads the transfer amount or needs a signer is
//! [`Unresolvable`].

use std::fmt;

use solana_address::{Address, MAX_SEEDS};
use solana_instruction::AccountMeta;
use spl_discriminator::{ArrayDiscriminator, SplDiscriminate};
use spl_tlv_account_resolution::account::ExtraAccountMeta;
use spl_tlv_account_resolution::pubkey_data::PubkeyData;
use spl_tlv_account_resolution::seeds::Seed;
use spl_tlv_account_resolution::solana_program_error::ProgramError;
use spl_tlv_account_resolution::state::ExtraAccountMetaList;
use spl_token_2022_interface::extension::pausable::PausableConfig;
use spl_token_2022_interface::extension::transfer_hook::TransferHook;
use spl_type_length_value::error::TlvError;
use spl_type_length_value::state::TlvStateBorrowed;

use crate::math::amm::MintFee;
use crate::transfer_fee::{
	TransferFeeConfig, extension, mint_extensions, mint_fee,
};
use crate::utils::{StandInTrade, TradeAccounts, TradeDirection};

/// Discriminator of the hook's `Execute` instruction. Also the TLV type of the
/// entry that holds the account list.
pub const EXECUTE_DISCRIMINATOR: [u8; 8] =
	[105, 37, 101, 197, 75, 251, 102, 26];

/// Seeds of the validation account: `["extra-account-metas", mint]`, under
/// the hook program.
pub const VALIDATION_SEED: &[u8] = b"extra-account-metas";

/// `Execute` accounts: source, mint, destination, owner, validation account.
const SOURCE_INDEX: usize = 0;
const DESTINATION_INDEX: usize = 2;
const VALIDATION_INDEX: usize = 4;
const EXECUTE_ACCOUNTS: usize = 5;

/// Mint and owner: the token-account bytes that are known before the swap.
const TOKEN_ACCOUNT_KNOWN_LEN: usize = 64;

const PUBKEY_LEN: usize = 32;

/// 128 and above: a PDA of the program at index `discriminator - 128`.
const META_FIXED: u8 = 0;
const META_HOOK_PDA: u8 = 1;
const META_PUBKEY_DATA: u8 = 2;
const META_EXTERNAL_PDA: u8 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AccountView<'a> {
	pub owner: &'a Address,
	pub data: &'a [u8],
}

#[must_use]
pub fn find_validation_address(mint: &Address, program: &Address) -> Address {
	Address::find_program_address(&[VALIDATION_SEED, mint.as_ref()], program).0
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MintState {
	pub token_program: Address,
	pub transfer_fee: Option<TransferFeeConfig>,
	/// The hook program. Token-2022 calls it on every transfer. The swap must
	/// include its accounts.
	pub transfer_hook: Option<Address>,
	/// Token-2022 fails every transfer while this is `true`.
	pub paused: bool,
}

impl MintState {
	pub fn load(mint: AccountView<'_>) -> Result<Self, ProgramError> {
		let token_program = *mint.owner;
		let Some(state) = mint_extensions(mint.data, &token_program)? else {
			return Ok(Self {
				token_program,
				transfer_fee: None,
				transfer_hook: None,
				paused: false,
			});
		};
		Ok(Self {
			token_program,
			transfer_fee: extension::<TransferFeeConfig>(&state)?.copied(),
			// Token-2022 calls a hook only when `program_id` is set.
			transfer_hook: extension::<TransferHook>(&state)?
				.and_then(|hook| Option::from(hook.program_id)),
			paused: extension::<PausableConfig>(&state)?
				.is_some_and(|config| config.paused.into()),
		})
	}

	#[must_use]
	pub fn fee_at(&self, epoch: u64) -> Option<MintFee> {
		self.transfer_fee
			.as_ref()
			.map(|config| mint_fee(config.get_epoch_fee(epoch)))
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unloaded {
	Refuse,
	/// Use only to measure a swap. For an unloaded list, the hook program and
	/// the validation account replace the extra accounts.
	StandIn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unresolvable {
	MalformedList,
	NoExecuteEntry,
	/// An extra is at an account index above 255.
	ListTooLong,
	MalformedExtra,
	MalformedSeed,
	/// A PDA has more than 15 seeds. The bump is the 16th.
	TooManySeeds,
	SeedTooLong,
	/// An extra must sign. The programs do not pass a signer to the hook.
	ExtraSigns,
	/// A seed reads the transfer amount. The program sets the amount of one leg
	/// during the swap.
	ReadsTransferAmount,
	/// An extra reads a pubkey from the `Execute` data. The data is 16 bytes,
	/// shorter than a pubkey.
	ReadsPastInstructionData,
	NamesUnresolvedAccount,
	ReadsSwapTimeData,
	/// A clash that [`MarketHooks::screen`] finds. It applies to every trader.
	Clash(Clash),
}

impl fmt::Display for Unresolvable {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match self {
			Self::MalformedList => "malformed list",
			Self::NoExecuteEntry => "no Execute entry in the list",
			Self::ListTooLong => "list too long",
			Self::MalformedExtra => "malformed extra",
			Self::MalformedSeed => "malformed seed",
			Self::TooManySeeds => "a PDA has more seeds than an address allows",
			Self::SeedTooLong => "seed longer than 32 bytes",
			Self::ExtraSigns => "an extra must sign",
			Self::ReadsTransferAmount => "a seed reads the transfer amount",
			Self::ReadsPastInstructionData => {
				"an extra reads a pubkey past the end of the hook's instruction data"
			}
			Self::NamesUnresolvedAccount => {
				"an extra names an account not yet resolved"
			}
			Self::ReadsSwapTimeData => {
				"an extra reads account data only the swap can see"
			}
			Self::Clash(clash) => return clash.fmt(f),
		})
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Clash {
	SignerNotForwarded,
	/// The hook repeats an account of the trade, and one copy is writable. A
	/// swap with it fails.
	WritableDuplicate,
	ExtraUnresolved,
}

impl fmt::Display for Clash {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.write_str(match self {
			Self::SignerNotForwarded => {
				"transfer hook needs a signer the programs do not forward"
			}
			Self::WritableDuplicate => {
				"transfer hook repeats an account of the trade, writable"
			}
			Self::ExtraUnresolved => "transfer hook extra did not resolve",
		})
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum HookError {
	NotLoaded,
	/// The validation account is missing, or the hook program does not own it.
	/// Token-2022 still calls the hook, and the transfer fails.
	NoList,
	Unresolvable(Unresolvable),
	/// A clash of this swap only, for example with this trader's accounts.
	Clash(Clash),
}

impl fmt::Display for HookError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::NotLoaded => write!(f, "transfer hook accounts not loaded"),
			Self::NoList => {
				write!(f, "transfer hook has no ExtraAccountMetaList")
			}
			Self::Unresolvable(reason) => write!(
				f,
				"transfer hook cannot be resolved ahead of the swap: {reason}"
			),
			Self::Clash(clash) => clash.fmt(f),
		}
	}
}

impl std::error::Error for HookError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hook {
	program: Address,
	validation: Address,
	list: HookList,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HookList {
	/// Not loaded: the hook is new, its program changed, or its list is missing
	/// for the first time.
	Pending,
	/// Missing on two refreshes in a row.
	Absent,
	/// Screened. Its bytes resolve the extras of each swap.
	Ready(Vec<u8>),
	Unsupported(Unresolvable),
}

impl Hook {
	#[must_use]
	pub const fn program(&self) -> &Address {
		&self.program
	}

	#[must_use]
	pub const fn validation(&self) -> &Address {
		&self.validation
	}

	/// `Ok` if a swap can include this hook. `Unloaded::StandIn` accepts an
	/// unloaded list. It does not accept an unresolvable list.
	pub const fn check_routable(
		&self,
		unloaded: Unloaded,
	) -> Result<(), HookError> {
		match &self.list {
			HookList::Ready(_) => Ok(()),
			HookList::Pending => match unloaded {
				Unloaded::StandIn => Ok(()),
				Unloaded::Refuse => Err(HookError::NotLoaded),
			},
			HookList::Absent => Err(HookError::NoList),
			HookList::Unsupported(reason) => {
				Err(HookError::Unresolvable(*reason))
			}
		}
	}

	/// A missing or foreign-owned list is `Pending` once, then `Absent`. One
	/// missed read stops quotes but does not remove a list.
	fn load(planned: Planned, account: Option<AccountView<'_>>) -> Self {
		// Anyone can fund the address. Only the hook program writes a list.
		let list = account
			.filter(|account| *account.owner == planned.program)
			.map_or(planned.missing, |account| {
				match screen_list(account.data) {
					Ok(()) => HookList::Ready(account.data.to_vec()),
					Err(reason) => HookList::Unsupported(reason),
				}
			});
		Self {
			program: planned.program,
			validation: planned.validation,
			list,
		}
	}

	/// SPL resolver order: extras, hook program, validation account.
	fn leg_accounts(
		&self,
		leg: &Leg,
		unloaded: Unloaded,
	) -> Result<Vec<AccountMeta>, HookError> {
		let data = match &self.list {
			HookList::Ready(data) => data,
			HookList::Pending if unloaded == Unloaded::StandIn => {
				return Ok(vec![
					AccountMeta::new_readonly(self.program, false),
					AccountMeta::new_readonly(self.validation, false),
				]);
			}
			HookList::Pending | HookList::Absent | HookList::Unsupported(_) => {
				return Err(self
					.check_routable(Unloaded::Refuse)
					.err()
					.unwrap_or(HookError::NotLoaded));
			}
		};
		let unresolved = HookError::Clash(Clash::ExtraUnresolved);
		let extras = unpack_list(data).map_err(|_| unresolved)?;
		let source = leg.token_account_prefix(&leg.owner);
		let destination = leg.token_account_prefix(&leg.destination_owner);
		let mut keys = vec![
			leg.source,
			leg.mint,
			leg.destination,
			leg.owner,
			self.validation,
		];
		let mut metas = Vec::with_capacity(extras.len());
		for extra in &extras {
			let resolved = extra
				.resolve(&EXECUTE_DISCRIMINATOR, &self.program, |index| {
					let data = match index {
						SOURCE_INDEX => Some(source.as_slice()),
						DESTINATION_INDEX => Some(destination.as_slice()),
						VALIDATION_INDEX => Some(data.as_slice()),
						_ => None,
					};
					keys.get(index).map(|key| (key, data))
				})
				.map_err(|_| unresolved)?;
			keys.push(resolved.pubkey);
			metas.push(AccountMeta {
				pubkey: resolved.pubkey,
				is_signer: false,
				is_writable: resolved.is_writable,
			});
		}
		metas.push(AccountMeta::new_readonly(self.program, false));
		metas.push(AccountMeta::new_readonly(self.validation, false));
		Ok(metas)
	}
}

/// One transfer of a trade. The source owner signs it.
struct Leg {
	source: Address,
	mint: Address,
	destination: Address,
	owner: Address,
	destination_owner: Address,
}

impl Leg {
	fn token_account_prefix(&self, owner: &Address) -> [u8; 64] {
		let mut prefix = [0; TOKEN_ACCOUNT_KNOWN_LEN];
		let (mint, rest) = prefix.split_at_mut(PUBKEY_LEN);
		mint.copy_from_slice(self.mint.as_ref());
		rest.copy_from_slice(owner.as_ref());
		prefix
	}
}

struct Execute;

impl SplDiscriminate for Execute {
	const SPL_DISCRIMINATOR: ArrayDiscriminator =
		ArrayDiscriminator::new(EXECUTE_DISCRIMINATOR);
}

fn unpack_list(data: &[u8]) -> Result<Vec<ExtraAccountMeta>, Unresolvable> {
	let list_error = |error: ProgramError| {
		if error == ProgramError::from(TlvError::TypeNotFound) {
			Unresolvable::NoExecuteEntry
		} else {
			Unresolvable::MalformedList
		}
	};
	let state = TlvStateBorrowed::unpack(data).map_err(list_error)?;
	let extras = ExtraAccountMetaList::unpack_with_tlv_state::<Execute>(&state)
		.map_err(list_error)?;
	Ok(extras.iter().copied().collect())
}

/// Refuses, before the swap, a list that SPL's resolver fails or panics on.
fn screen_list(data: &[u8]) -> Result<(), Unresolvable> {
	for (index, extra) in unpack_list(data)?.iter().enumerate() {
		let position = index
			.checked_add(EXECUTE_ACCOUNTS)
			.filter(|position| u8::try_from(*position).is_ok())
			.ok_or(Unresolvable::ListTooLong)?;
		screen_extra(extra, position)?;
	}
	Ok(())
}

fn screen_extra(
	extra: &ExtraAccountMeta,
	position: usize,
) -> Result<(), Unresolvable> {
	if bool::from(extra.is_signer) {
		return Err(Unresolvable::ExtraSigns);
	}
	match extra.discriminator {
		META_FIXED => Ok(()),
		META_HOOK_PDA => screen_seeds(&extra.address_config, position),
		META_PUBKEY_DATA => match PubkeyData::unpack(&extra.address_config)
			.map_err(|_| Unresolvable::MalformedExtra)?
		{
			PubkeyData::AccountData {
				account_index,
				data_index,
			} => readable(
				DataRead {
					account: account_index,
					offset: data_index,
					length: PUBKEY_LEN,
				},
				position,
			),
			PubkeyData::InstructionData { .. } => {
				Err(Unresolvable::ReadsPastInstructionData)
			}
			PubkeyData::Uninitialized => Err(Unresolvable::MalformedExtra),
		},
		external => match external.checked_sub(META_EXTERNAL_PDA) {
			Some(program) => {
				prior(program, position)?;
				screen_seeds(&extra.address_config, position)
			}
			None => Err(Unresolvable::MalformedExtra),
		},
	}
}

fn screen_seeds(
	config: &[u8; 32],
	position: usize,
) -> Result<(), Unresolvable> {
	let seeds = Seed::unpack_address_config(config)
		.map_err(|_| Unresolvable::MalformedSeed)?;
	// With the bump, this is above `MAX_SEEDS`: `find_program_address` panics.
	if seeds.len() >= MAX_SEEDS {
		return Err(Unresolvable::TooManySeeds);
	}
	for seed in &seeds {
		match *seed {
			// A literal fits in the 32-byte config. It is never too long.
			Seed::Uninitialized | Seed::Literal { .. } => {}
			Seed::InstructionData { index, length } => {
				let end = usize::from(index)
					.checked_add(usize::from(length))
					.ok_or(Unresolvable::MalformedSeed)?;
				if end > EXECUTE_DISCRIMINATOR.len() {
					return Err(Unresolvable::ReadsTransferAmount);
				}
			}
			Seed::AccountKey { index } => prior(index, position)?,
			Seed::AccountData {
				account_index,
				data_index,
				length,
			} => {
				let length = usize::from(length);
				if length > PUBKEY_LEN {
					return Err(Unresolvable::SeedTooLong);
				}
				readable(
					DataRead {
						account: account_index,
						offset: data_index,
						length,
					},
					position,
				)?;
			}
		}
	}
	Ok(())
}

/// `index` must name an account resolved before `position`, as on-chain.
fn prior(index: u8, position: usize) -> Result<(), Unresolvable> {
	if usize::from(index) < position {
		Ok(())
	} else {
		Err(Unresolvable::NamesUnresolvedAccount)
	}
}

/// `length` bytes at `offset` of `Execute` account `account`.
struct DataRead {
	account: u8,
	offset: u8,
	length: usize,
}

/// `Ok` if the read is known before the swap: the list itself, or the mint and
/// owner of a token account.
fn readable(read: DataRead, position: usize) -> Result<(), Unresolvable> {
	prior(read.account, position)?;
	let end = usize::from(read.offset)
		.checked_add(read.length)
		.ok_or(Unresolvable::MalformedSeed)?;
	match usize::from(read.account) {
		SOURCE_INDEX | DESTINATION_INDEX if end <= TOKEN_ACCOUNT_KNOWN_LEN => {
			Ok(())
		}
		// Reads the stored list. A read past its end fails at resolve time, as
		// on-chain.
		VALIDATION_INDEX => Ok(()),
		_ => Err(Unresolvable::ReadsSwapTimeData),
	}
}

#[derive(Clone, Copy, Debug)]
pub struct HookSwap<'a> {
	pub trade: &'a TradeAccounts,
	pub direction: TradeDirection,
	/// The trade instruction's own accounts.
	pub fixed: &'a [AccountMeta],
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarketHooks {
	base: Option<Hook>,
	quote: Option<Hook>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Planned {
	program: Address,
	validation: Address,
	/// What the list becomes if its account is missing.
	missing: HookList,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookPlan {
	base: Option<Planned>,
	quote: Option<Planned>,
}

impl HookPlan {
	pub fn accounts(&self) -> impl Iterator<Item = Address> + '_ {
		[&self.base, &self.quote]
			.into_iter()
			.flatten()
			.map(|planned| planned.validation)
	}

	/// Loads each list with `accounts`. `None` from `accounts` is a missing
	/// account.
	pub fn resolve<'a>(
		self,
		accounts: impl Fn(&Address) -> Option<AccountView<'a>>,
	) -> MarketHooks {
		let load = |planned: Planned| {
			let account = accounts(&planned.validation);
			Hook::load(planned, account)
		};
		MarketHooks {
			base: self.base.map(load),
			quote: self.quote.map(load),
		}
	}
}

impl MarketHooks {
	#[must_use]
	pub const fn base(&self) -> Option<&Hook> {
		self.base.as_ref()
	}

	#[must_use]
	pub const fn quote(&self) -> Option<&Hook> {
		self.quote.as_ref()
	}

	/// Plans a refresh from the current mint states. A validation address is
	/// derived again only when its hook program changes.
	#[must_use]
	pub fn plan(
		&self,
		base: (&Address, &MintState),
		quote: (&Address, &MintState),
	) -> HookPlan {
		let plan = |previous: Option<&Hook>,
		            (mint, state): (&Address, &MintState)| {
			state.transfer_hook.map(|program| {
				let asked = previous.filter(|hook| hook.program == program);
				Planned {
					program,
					validation: asked.map_or_else(
						|| find_validation_address(mint, &program),
						|hook| hook.validation,
					),
					missing: match asked.map(|hook| &hook.list) {
						Some(HookList::Pending | HookList::Absent) => {
							HookList::Absent
						}
						Some(HookList::Ready(_) | HookList::Unsupported(_))
						| None => HookList::Pending,
					},
				}
			})
		};
		HookPlan {
			base: plan(self.base(), base),
			quote: plan(self.quote(), quote),
		}
	}

	/// Marks each hook unresolvable if a buy or a sell cannot include it. The
	/// result applies to every trader. `fixed` must be the instruction accounts
	/// of `trade`.
	#[must_use]
	pub fn screen(self, trade: &StandInTrade, fixed: &[AccountMeta]) -> Self {
		let screen = |hook: Hook, alone: &Self| {
			if hook.check_routable(Unloaded::Refuse).is_err() {
				return hook;
			}
			let clash = [TradeDirection::Buy, TradeDirection::Sell]
				.into_iter()
				.find_map(|direction| {
					let swap = HookSwap {
						trade,
						direction,
						fixed,
					};
					alone.swap_accounts(swap, Unloaded::Refuse).err()
				});
			let reason = match clash {
				Some(HookError::Unresolvable(reason)) => reason,
				Some(HookError::Clash(clash)) => Unresolvable::Clash(clash),
				// Unreachable for a routable hook.
				None | Some(HookError::NotLoaded | HookError::NoList) => {
					return hook;
				}
			};
			Hook {
				list: HookList::Unsupported(reason),
				..hook
			}
		};
		Self {
			base: self.base.map(|hook| {
				let alone = Self {
					base: Some(hook.clone()),
					quote: None,
				};
				screen(hook, &alone)
			}),
			quote: self.quote.map(|hook| {
				let alone = Self {
					base: None,
					quote: Some(hook.clone()),
				};
				screen(hook, &alone)
			}),
		}
	}

	fn iter(&self) -> impl Iterator<Item = &Hook> {
		[&self.base, &self.quote].into_iter().flatten()
	}

	/// The validation accounts of the current hooks. Fetch them for the next
	/// refresh.
	pub fn validation_accounts(&self) -> impl Iterator<Item = Address> + '_ {
		self.iter().map(|hook| hook.validation)
	}

	pub fn check_routable(&self, unloaded: Unloaded) -> Result<(), HookError> {
		self.iter()
			.try_for_each(|hook| hook.check_routable(unloaded))
	}

	#[must_use]
	pub fn programs(&self) -> Vec<Address> {
		let mut programs: Vec<Address> = Vec::new();
		for hook in self.iter() {
			if !programs.contains(&hook.program) {
				programs.push(hook.program);
			}
		}
		programs
	}

	/// The hook accounts of both legs, merged. Append them after `swap.fixed`.
	pub fn swap_accounts(
		&self,
		swap: HookSwap<'_>,
		unloaded: Unloaded,
	) -> Result<Vec<AccountMeta>, HookError> {
		let HookSwap {
			trade,
			direction,
			fixed,
		} = swap;
		let into_vault = |source: Address, mint: Address, vault: Address| Leg {
			source,
			mint,
			destination: vault,
			owner: trade.user,
			destination_owner: trade.market,
		};
		let out_of_vault =
			|vault: Address, mint: Address, destination: Address| Leg {
				source: vault,
				mint,
				destination,
				owner: trade.market,
				destination_owner: trade.user,
			};
		let legs = match direction {
			TradeDirection::Buy => [
				(
					self.quote(),
					into_vault(
						trade.user_quote_account,
						trade.quote_mint,
						trade.quote_vault,
					),
				),
				(
					self.base(),
					out_of_vault(
						trade.base_vault,
						trade.base_mint,
						trade.user_base_account,
					),
				),
			],
			TradeDirection::Sell => [
				(
					self.base(),
					into_vault(
						trade.user_base_account,
						trade.base_mint,
						trade.base_vault,
					),
				),
				(
					self.quote(),
					out_of_vault(
						trade.quote_vault,
						trade.quote_mint,
						trade.user_quote_account,
					),
				),
			],
		};
		trailing_accounts(&legs, fixed, unloaded)
	}
}

/// Refuses a hook account that repeats a trade signer. The programs do not
/// pass the signer to the hook. Refuses a hook account that repeats a trade
/// account when either copy is writable. The swap fails in both cases.
fn trailing_accounts(
	legs: &[(Option<&Hook>, Leg)],
	fixed: &[AccountMeta],
	unloaded: Unloaded,
) -> Result<Vec<AccountMeta>, HookError> {
	let mut merged: Vec<AccountMeta> = Vec::new();
	for (hook, leg) in legs {
		let Some(hook) = hook else {
			continue;
		};
		for meta in hook.leg_accounts(leg, unloaded)? {
			if let Some(clash) =
				fixed.iter().find(|account| account.pubkey == meta.pubkey)
			{
				if clash.is_signer {
					return Err(HookError::Clash(Clash::SignerNotForwarded));
				}
				if clash.is_writable || meta.is_writable {
					return Err(HookError::Clash(Clash::WritableDuplicate));
				}
			}
			match merged
				.iter_mut()
				.find(|account| account.pubkey == meta.pubkey)
			{
				Some(existing) => existing.is_writable |= meta.is_writable,
				None => merged.push(meta),
			}
		}
	}
	Ok(merged)
}

#[cfg(test)]
mod tests;
