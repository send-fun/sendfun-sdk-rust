//! Reads the transfer fee of a Token-2022 mint from raw account data, for the
//! [`math::amm`](crate::math::amm) quotes.
//! Cache a `TransferFeeConfig` only while its `transfer_fee_config_authority`
//! is `None`.

use bytemuck::Pod;
use solana_address::Address;
use solana_program::program_error::ProgramError;
use spl_token_2022_interface::error::TokenError;
use spl_token_2022_interface::extension::{
	AccountType, BaseStateWithExtensions, Extension, PodStateWithExtensions,
};
use spl_token_2022_interface::pod::PodMint;

use crate::constants::{TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
use crate::math::amm::MintFee;

pub use spl_token_2022_interface::extension::transfer_fee::{
	TransferFee, TransferFeeConfig,
};

/// A TLV entry header: the `u16` type, then the `u16` length.
const TLV_HEADER_LEN: usize = 4;

/// Unpacks a mint and its extensions. `Ok(None)` for an SPL Token mint. Fails
/// with `ProgramError::IncorrectProgramId` if no token program owns the mint.
pub fn mint_extensions<'a>(
	data: &'a [u8],
	owner: &Address,
) -> Result<Option<PodStateWithExtensions<'a, PodMint>>, ProgramError> {
	if owner == &TOKEN_PROGRAM_ID {
		return Ok(None);
	}
	if owner != &TOKEN_2022_PROGRAM_ID {
		return Err(ProgramError::IncorrectProgramId);
	}
	PodStateWithExtensions::<PodMint>::unpack(data).map(Some)
}

/// Reads the mint extension `V`.
///
/// `Ok(None)` only when the mint does not have `V`. A malformed extension
/// region is an error, not `Ok(None)`. An account extension type fails with
/// `ProgramError::InvalidAccountData`.
pub fn extension<'a, V: Extension + Pod>(
	mint: &'a PodStateWithExtensions<'_, PodMint>,
) -> Result<Option<&'a V>, ProgramError> {
	match mint.get_extension::<V>() {
		Ok(value) => Ok(Some(value)),
		// The walk reached a zero type before `V`.
		Err(error) if error == TokenError::ExtensionNotFound.into() => Ok(None),
		// The walk ran off the end, the data is cut, or `V` is an account
		// extension. Token-2022 allocates the exact size, so a clean end means
		// `V` is absent.
		Err(ProgramError::InvalidAccountData)
			if V::TYPE.get_account_type() == AccountType::Mint
				&& ends_cleanly(mint.get_tlv_data()) =>
		{
			Ok(None)
		}
		Err(error) => Err(error),
	}
}

/// `true` if every TLV entry is whole, as SPL walks the entries.
/// Does not parse types as `ExtensionType`: an unknown type must not make a
/// fee-free mint unreadable.
fn ends_cleanly(tlv: &[u8]) -> bool {
	let mut rest = tlv;
	loop {
		let Some(&[type_low, type_high]) = rest.get(..2) else {
			return true;
		};
		if u16::from_le_bytes([type_low, type_high]) == 0 {
			return true;
		}
		let Some(&[length_low, length_high]) = rest.get(2..TLV_HEADER_LEN)
		else {
			return false;
		};
		let length = usize::from(u16::from_le_bytes([length_low, length_high]));
		let Some(next) = TLV_HEADER_LEN
			.checked_add(length)
			.and_then(|end| rest.get(end..))
		else {
			return false;
		};
		rest = next;
	}
}

#[must_use]
pub fn mint_fee(fee: &TransferFee) -> MintFee {
	MintFee {
		bps: fee.transfer_fee_basis_points.into(),
		maximum_fee: fee.maximum_fee.into(),
	}
}

/// The transfer fee of a mint at `epoch`. `Ok(None)` only when the mint has no
/// `TransferFeeConfig`. A 0 bps config gives `Some`.
pub fn mint_fee_at_epoch(
	data: &[u8],
	owner: &Address,
	epoch: u64,
) -> Result<Option<MintFee>, ProgramError> {
	let Some(mint) = mint_extensions(data, owner)? else {
		return Ok(None);
	};
	Ok(extension::<TransferFeeConfig>(&mint)?
		.map(|config| mint_fee(config.get_epoch_fee(epoch))))
}

#[cfg(test)]
mod tests {
	use super::*;

	use spl_token_2022_interface::extension::transfer_fee::TransferFeeAmount;

	/// SPL's `unpack` refuses a mint with this byte unset.
	const IS_INITIALIZED_OFFSET: usize = 45;

	/// Token-2022 pads the mint to the token-account length before this byte.
	const ACCOUNT_TYPE_OFFSET: usize = 165;

	const TLV_START: usize = 166;

	const FEE_TYPE: u16 = 1;

	/// Above every type that SPL 3.1.1 defines.
	const FUTURE_TYPE: u16 = 250;

	const NEWER: Option<MintFee> = Some(MintFee {
		bps: 250,
		maximum_fee: 2_000,
	});

	fn entry(epoch: u64, maximum_fee: u64, basis_points: u16) -> TransferFee {
		TransferFee {
			epoch: epoch.into(),
			maximum_fee: maximum_fee.into(),
			transfer_fee_basis_points: basis_points.into(),
		}
	}

	fn fee_payload(authority: Option<Address>) -> Vec<u8> {
		bytemuck::bytes_of(&TransferFeeConfig {
			transfer_fee_config_authority: authority.try_into().unwrap(),
			older_transfer_fee: entry(5, 1_000, 100),
			newer_transfer_fee: entry(7, 2_000, 250),
			..TransferFeeConfig::default()
		})
		.to_vec()
	}

	/// `MetadataPointer`, then `TokenMetadata`. `create_token` writes both.
	fn other_extensions() -> Vec<(u16, Vec<u8>)> {
		vec![(18, vec![0; 64]), (19, vec![0; 90])]
	}

	fn tlv_image(entries: &[(u16, Vec<u8>)]) -> Vec<u8> {
		let mut data = vec![0_u8; ACCOUNT_TYPE_OFFSET];
		data[IS_INITIALIZED_OFFSET] = 1;
		// `AccountType::Mint`.
		data.push(1);
		for (extension_type, payload) in entries {
			data.extend_from_slice(&extension_type.to_le_bytes());
			data.extend_from_slice(
				&u16::try_from(payload.len()).unwrap().to_le_bytes(),
			);
			data.extend_from_slice(payload);
		}
		data
	}

	fn fee(data: &[u8]) -> Result<Option<MintFee>, ProgramError> {
		mint_fee_at_epoch(data, &TOKEN_2022_PROGRAM_ID, 7)
	}

	#[test]
	fn the_walk_finds_the_config_wherever_it_sits() {
		for at in 0..=2 {
			let mut entries = other_extensions();
			entries.insert(at, (FEE_TYPE, fee_payload(None)));
			assert_eq!(fee(&tlv_image(&entries)), Ok(NEWER), "at {at}");
		}
	}

	#[test]
	fn the_owner_picks_the_reader() {
		let image = tlv_image(&[(FEE_TYPE, fee_payload(None))]);
		assert_eq!(fee(&image), Ok(NEWER));
		assert_eq!(mint_fee_at_epoch(&image, &TOKEN_PROGRAM_ID, 7), Ok(None));
		assert_eq!(
			mint_fee_at_epoch(&image, &crate::constants::SYSTEM_PROGRAM_ID, 7),
			Err(ProgramError::IncorrectProgramId)
		);
	}

	#[test]
	fn the_newer_entry_applies_from_its_own_epoch() {
		let image = tlv_image(&[(FEE_TYPE, fee_payload(None))]);
		assert_eq!(
			mint_fee_at_epoch(&image, &TOKEN_2022_PROGRAM_ID, 6),
			Ok(Some(MintFee {
				bps: 100,
				maximum_fee: 1_000,
			}))
		);
		assert_eq!(fee(&image), Ok(NEWER));
	}

	#[test]
	fn a_revoked_authority_reads_as_none() {
		for authority in [None, Some(Address::new_from_array([9; 32]))] {
			let image = tlv_image(&[(FEE_TYPE, fee_payload(authority))]);
			let mint = mint_extensions(&image, &TOKEN_2022_PROGRAM_ID)
				.unwrap()
				.unwrap();
			let config =
				extension::<TransferFeeConfig>(&mint).unwrap().unwrap();
			assert_eq!(
				Option::from(config.transfer_fee_config_authority),
				authority
			);
		}
	}

	#[test]
	fn a_bare_mint_has_no_tlv_region_to_walk() {
		let mut bare = tlv_image(&[]);
		bare.truncate(82);
		assert_eq!(fee(&bare), Ok(None));
		bare[IS_INITIALIZED_OFFSET] = 0;
		assert_eq!(fee(&bare), Err(ProgramError::UninitializedAccount));
	}

	/// Extensions that total `Multisig::LEN` get two bytes of padding. SPL
	/// reads one trailing byte as slack.
	#[test]
	fn a_region_that_ends_cleanly_carries_no_config() {
		for tail in [
			vec![],
			vec![0],
			vec![0; 2],
			vec![0; 3],
			vec![0; 512],
			vec![7],
		] {
			let mut image = tlv_image(&other_extensions());
			image.extend_from_slice(&tail);
			assert_eq!(fee(&image), Ok(None), "tail {tail:?}");
		}
	}

	/// Token-2022 also stops at the first match. It does not read the entries
	/// after it.
	#[test]
	fn a_cut_entry_past_the_config_is_not_read() {
		let mut image = tlv_image(&[(FEE_TYPE, fee_payload(None))]);
		image.extend_from_slice(&[18, 0, 64, 0]);
		assert_eq!(fee(&image), Ok(NEWER));
	}

	/// An account extension on a mint is a caller error, not an absent
	/// extension.
	#[test]
	fn an_account_extension_is_not_read_off_a_mint() {
		let image = tlv_image(&other_extensions());
		let mint = mint_extensions(&image, &TOKEN_2022_PROGRAM_ID)
			.unwrap()
			.unwrap();
		assert_eq!(
			extension::<TransferFeeAmount>(&mint),
			Err(ProgramError::InvalidAccountData)
		);
	}

	/// SPL's `get_extension_types` refuses unknown types. The region check must
	/// not use it.
	#[test]
	fn a_future_extension_type_leaves_the_mint_readable() {
		let future = (FUTURE_TYPE, vec![0; 16]);
		let fee_free = tlv_image(&[(18, vec![0; 64]), future.clone()]);
		let mint = mint_extensions(&fee_free, &TOKEN_2022_PROGRAM_ID)
			.unwrap()
			.unwrap();
		assert_eq!(
			mint.get_extension_types(),
			Err(ProgramError::InvalidAccountData)
		);
		assert_eq!(extension::<TransferFeeConfig>(&mint), Ok(None));
		let fee_after = tlv_image(&[future, (FEE_TYPE, fee_payload(None))]);
		assert_eq!(fee(&fee_after), Ok(NEWER));
	}

	#[test]
	fn a_cut_region_is_malformed() {
		let mut short = tlv_image(&[]);
		short.truncate(ACCOUNT_TYPE_OFFSET);
		let mut header = tlv_image(&other_extensions());
		header.truncate(TLV_START + 4 + 64 + 2);
		let mut overrun = tlv_image(&[(18, vec![0; 64])]);
		overrun.truncate(TLV_START + 4 + 32);
		let mut payload = tlv_image(&other_extensions());
		payload.extend_from_slice(&[18, 0, 64, 0]);
		let mut config = tlv_image(&[(FEE_TYPE, fee_payload(None))]);
		config.pop();
		for (label, image) in [
			("no account type", short),
			("cut header", header),
			("length past the end", overrun),
			("payload never arrives", payload),
			("config payload cut", config),
		] {
			assert_eq!(
				fee(&image),
				Err(ProgramError::InvalidAccountData),
				"{label}"
			);
		}
	}

	#[test]
	fn a_config_payload_of_the_wrong_length_is_malformed() {
		for length in [100_usize, 116] {
			let image = tlv_image(&[(FEE_TYPE, vec![0; length])]);
			assert_eq!(
				fee(&image),
				Err(ProgramError::InvalidArgument),
				"payload length {length}"
			);
		}
	}

	#[test]
	fn a_non_mint_account_type_is_malformed() {
		let mut image = tlv_image(&[(FEE_TYPE, fee_payload(None))]);
		// `AccountType::Account`.
		image[ACCOUNT_TYPE_OFFSET] = 2;
		assert_eq!(fee(&image), Err(ProgramError::InvalidAccountData));
	}
}
