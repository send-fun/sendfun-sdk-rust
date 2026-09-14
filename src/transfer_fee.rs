//! No `spl-token-2022`: it drags confidential transfers into a dep-free SDK.
//!
//! Cache a config only while [`TransferFeeConfig::authority`] is `None`.

use std::fmt;

use solana_address::Address;

use crate::constants::{TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
use crate::math::amm::MintFee;

/// A Token-2022 mint of exactly this length carries no extensions.
const MINT_BASE_LEN: usize = 82;

/// Token-2022 pads the mint to the token-account length before this byte.
const ACCOUNT_TYPE_OFFSET: usize = 165;

const ACCOUNT_TYPE_MINT: u8 = 1;

const TLV_START: usize = 166;

const TLV_TYPE_LEN: usize = 2;

const TLV_HEADER_LEN: usize = 4;

const UNINITIALIZED_TYPE: u16 = 0;

const TRANSFER_FEE_CONFIG_TYPE: u16 = 1;

/// Two 32-byte authorities, `withheld_amount: u64`, then the older and newer
/// 18-byte `TransferFee` entries.
const TRANSFER_FEE_CONFIG_LEN: usize = 108;

const CONFIG_AUTHORITY_OFFSET: usize = 0;

const OLDER_FEE_OFFSET: usize = 72;

const NEWER_FEE_OFFSET: usize = 90;

const FEE_MAXIMUM_OFFSET: usize = 8;

const FEE_BASIS_POINTS_OFFSET: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferFeeEntry {
	pub epoch: u64,
	pub maximum_fee: u64,
	pub basis_points: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferFeeConfig {
	/// `None`: the schedule is frozen forever.
	pub authority: Option<Address>,
	pub older: TransferFeeEntry,
	pub newer: TransferFeeEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferFeeDecodeError {
	UnknownOwner,
	Malformed,
}

impl fmt::Display for TransferFeeDecodeError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::UnknownOwner => {
				write!(f, "Mint is owned by neither token program")
			}
			Self::Malformed => write!(f, "Malformed mint extension data"),
		}
	}
}

impl std::error::Error for TransferFeeDecodeError {}

fn read_u16(payload: &[u8], offset: usize) -> Option<u16> {
	let end = offset.checked_add(2)?;
	let &[low, high] = payload.get(offset..end)? else {
		return None;
	};
	Some(u16::from_le_bytes([low, high]))
}

fn read_u64(payload: &[u8], offset: usize) -> Option<u64> {
	let end = offset.checked_add(8)?;
	let bytes: [u8; 8] = payload.get(offset..end)?.try_into().ok()?;
	Some(u64::from_le_bytes(bytes))
}

fn read_entry(payload: &[u8], offset: usize) -> Option<TransferFeeEntry> {
	Some(TransferFeeEntry {
		epoch: read_u64(payload, offset)?,
		maximum_fee: read_u64(
			payload,
			offset.checked_add(FEE_MAXIMUM_OFFSET)?,
		)?,
		basis_points: read_u16(
			payload,
			offset.checked_add(FEE_BASIS_POINTS_OFFSET)?,
		)?,
	})
}

/// Exact length only: an oversized payload would parse its first 108 bytes.
/// `None` means [`TransferFeeDecodeError::Malformed`], never "no fee".
fn read_config(payload: &[u8]) -> Option<TransferFeeConfig> {
	if payload.len() != TRANSFER_FEE_CONFIG_LEN {
		return None;
	}

	let end = CONFIG_AUTHORITY_OFFSET.checked_add(32)?;
	let authority: [u8; 32] =
		payload.get(CONFIG_AUTHORITY_OFFSET..end)?.try_into().ok()?;

	Some(TransferFeeConfig {
		// `OptionalNonZeroPubkey` is `None` exactly when all 32 bytes are zero.
		authority: (authority != [0; 32])
			.then(|| Address::new_from_array(authority)),
		older: read_entry(payload, OLDER_FEE_OFFSET)?,
		newer: read_entry(payload, NEWER_FEE_OFFSET)?,
	})
}

/// `(type, payload)` in account order; empty for an SPL mint. Every entry up to
/// the first zero type, or a tail too short for one, must be whole.
pub fn mint_extensions<'a>(
	data: &'a [u8],
	owner: &Address,
) -> Result<Vec<(u16, &'a [u8])>, TransferFeeDecodeError> {
	if owner == &TOKEN_PROGRAM_ID {
		return Ok(Vec::new());
	}
	if owner != &TOKEN_2022_PROGRAM_ID {
		return Err(TransferFeeDecodeError::UnknownOwner);
	}
	if data.len() == MINT_BASE_LEN {
		return Ok(Vec::new());
	}
	if data.len() < TLV_START {
		return Err(TransferFeeDecodeError::Malformed);
	}
	// A token account shares this TLV layout with different extension types.
	if data.get(ACCOUNT_TYPE_OFFSET) != Some(&ACCOUNT_TYPE_MINT) {
		return Err(TransferFeeDecodeError::Malformed);
	}

	let mut extensions = Vec::new();
	let mut offset = TLV_START;
	while offset < data.len() {
		let type_end = offset
			.checked_add(TLV_TYPE_LEN)
			.ok_or(TransferFeeDecodeError::Malformed)?;
		// Too short for a type, or a zero type: trailing slack, as SPL reads it.
		let Some(&[type_low, type_high]) = data.get(offset..type_end) else {
			break;
		};
		let extension_type = u16::from_le_bytes([type_low, type_high]);
		if extension_type == UNINITIALIZED_TYPE {
			break;
		}

		let header_end = offset
			.checked_add(TLV_HEADER_LEN)
			.ok_or(TransferFeeDecodeError::Malformed)?;
		// A set type with a cut length is malformed, not the end of the list.
		let Some(&[length_low, length_high]) = data.get(type_end..header_end)
		else {
			return Err(TransferFeeDecodeError::Malformed);
		};

		let length = usize::from(u16::from_le_bytes([length_low, length_high]));
		let payload_end = header_end
			.checked_add(length)
			.ok_or(TransferFeeDecodeError::Malformed)?;
		let payload = data
			.get(header_end..payload_end)
			.ok_or(TransferFeeDecodeError::Malformed)?;
		extensions.push((extension_type, payload));
		offset = payload_end;
	}

	Ok(extensions)
}

/// `Ok(None)` only without the extension. A 0 bps config stays `Some`: its
/// rate can rise, and a leg grossed up as fee-free fails the program's bound.
pub fn decode_transfer_fee_config(
	data: &[u8],
	owner: &Address,
) -> Result<Option<TransferFeeConfig>, TransferFeeDecodeError> {
	mint_extensions(data, owner)?
		.into_iter()
		.find(|(extension_type, _)| *extension_type == TRANSFER_FEE_CONFIG_TYPE)
		.map(|(_, payload)| {
			read_config(payload).ok_or(TransferFeeDecodeError::Malformed)
		})
		.transpose()
}

/// Mirrors SPL's `get_epoch_fee`.
#[must_use]
pub const fn transfer_fee_at_epoch(
	config: &TransferFeeConfig,
	epoch: u64,
) -> MintFee {
	let entry = if epoch >= config.newer.epoch {
		&config.newer
	} else {
		&config.older
	};

	MintFee {
		bps: entry.basis_points,
		maximum_fee: entry.maximum_fee,
	}
}

pub fn mint_fee_at_epoch(
	data: &[u8],
	owner: &Address,
	epoch: u64,
) -> Result<Option<MintFee>, TransferFeeDecodeError> {
	Ok(decode_transfer_fee_config(data, owner)?
		.map(|config| transfer_fee_at_epoch(&config, epoch)))
}

#[cfg(test)]
mod tests {
	use super::*;

	const OLDER: TransferFeeEntry = TransferFeeEntry {
		epoch: 5,
		maximum_fee: 1_000,
		basis_points: 100,
	};

	const NEWER: TransferFeeEntry = TransferFeeEntry {
		epoch: 7,
		maximum_fee: 2_000,
		basis_points: 250,
	};

	const AUTHORITY: [u8; 32] = [9; 32];

	/// `MetadataPointer` then `TokenMetadata`: what `create_token` writes.
	fn other_extensions() -> Vec<(u16, Vec<u8>)> {
		vec![(18, vec![0; 64]), (19, vec![0; 90])]
	}

	fn fee_payload(
		authority: Option<[u8; 32]>,
		older: TransferFeeEntry,
		newer: TransferFeeEntry,
	) -> Vec<u8> {
		let mut payload = Vec::with_capacity(TRANSFER_FEE_CONFIG_LEN);
		payload.extend_from_slice(&authority.unwrap_or([0; 32]));
		// `withdraw_withheld_authority`, then `withheld_amount`.
		payload.extend_from_slice(&[0; 32]);
		payload.extend_from_slice(&0_u64.to_le_bytes());
		for entry in [older, newer] {
			payload.extend_from_slice(&entry.epoch.to_le_bytes());
			payload.extend_from_slice(&entry.maximum_fee.to_le_bytes());
			payload.extend_from_slice(&entry.basis_points.to_le_bytes());
		}
		payload
	}

	fn default_fee_payload() -> Vec<u8> {
		fee_payload(Some(AUTHORITY), OLDER, NEWER)
	}

	fn tlv_image(entries: &[(u16, Vec<u8>)]) -> Vec<u8> {
		let mut data = vec![0_u8; ACCOUNT_TYPE_OFFSET];
		data.push(ACCOUNT_TYPE_MINT);
		for (extension_type, payload) in entries {
			data.extend_from_slice(&extension_type.to_le_bytes());
			data.extend_from_slice(
				&u16::try_from(payload.len()).unwrap().to_le_bytes(),
			);
			data.extend_from_slice(payload);
		}
		data
	}

	fn decode(
		data: &[u8],
	) -> Result<Option<TransferFeeConfig>, TransferFeeDecodeError> {
		decode_transfer_fee_config(data, &TOKEN_2022_PROGRAM_ID)
	}

	fn expected() -> Option<TransferFeeConfig> {
		Some(TransferFeeConfig {
			authority: Some(Address::new_from_array(AUTHORITY)),
			older: OLDER,
			newer: NEWER,
		})
	}

	#[test]
	fn the_walk_finds_the_config_wherever_it_sits() {
		let entry = (TRANSFER_FEE_CONFIG_TYPE, default_fee_payload());

		let mut first = vec![entry.clone()];
		first.extend(other_extensions());
		assert_eq!(decode(&tlv_image(&first)), Ok(expected()));

		let mut middle = other_extensions();
		middle.insert(1, entry.clone());
		assert_eq!(decode(&tlv_image(&middle)), Ok(expected()));

		let mut last = other_extensions();
		last.push(entry);
		assert_eq!(decode(&tlv_image(&last)), Ok(expected()));
	}

	#[test]
	fn a_revoked_authority_reads_as_none() {
		let image = tlv_image(&[(
			TRANSFER_FEE_CONFIG_TYPE,
			fee_payload(None, OLDER, NEWER),
		)]);

		assert_eq!(
			decode(&image),
			Ok(Some(TransferFeeConfig {
				authority: None,
				older: OLDER,
				newer: NEWER,
			}))
		);
	}

	#[test]
	fn the_terminator_ends_the_walk() {
		let mut image = tlv_image(&other_extensions());
		image.extend(core::iter::repeat_n(0_u8, 512));

		assert_eq!(decode(&image), Ok(None));
	}

	#[test]
	fn a_region_ending_on_a_boundary_carries_no_config() {
		assert_eq!(decode(&tlv_image(&other_extensions())), Ok(None));
	}

	#[test]
	fn a_bare_mint_has_no_tlv_region_to_walk() {
		assert_eq!(decode(&[0_u8; MINT_BASE_LEN]), Ok(None));
	}

	#[test]
	fn a_truncated_header_is_malformed() {
		let mut image = tlv_image(&other_extensions());
		image.truncate(TLV_START + TLV_HEADER_LEN + 64 + 2);

		assert_eq!(decode(&image), Err(TransferFeeDecodeError::Malformed));
	}

	#[test]
	fn a_length_running_past_the_buffer_is_malformed() {
		let mut image = tlv_image(&[(18, vec![0; 64])]);
		image.truncate(TLV_START + TLV_HEADER_LEN + 32);

		assert_eq!(decode(&image), Err(TransferFeeDecodeError::Malformed));
	}

	#[test]
	fn a_config_payload_of_the_wrong_length_is_malformed() {
		for length in [TRANSFER_FEE_CONFIG_LEN - 8, TRANSFER_FEE_CONFIG_LEN + 8]
		{
			let image =
				tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, vec![0; length])]);
			assert_eq!(
				decode(&image),
				Err(TransferFeeDecodeError::Malformed),
				"payload length {length}"
			);
		}
	}

	#[test]
	fn a_non_mint_account_type_is_malformed() {
		let mut image =
			tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, default_fee_payload())]);
		image.splice(ACCOUNT_TYPE_OFFSET..TLV_START, core::iter::once(2_u8));

		assert_eq!(decode(&image), Err(TransferFeeDecodeError::Malformed));
	}

	#[test]
	fn a_length_short_of_the_tlv_region_is_malformed() {
		for length in [MINT_BASE_LEN + 1, ACCOUNT_TYPE_OFFSET] {
			assert_eq!(
				decode(&vec![0_u8; length]),
				Err(TransferFeeDecodeError::Malformed),
				"account length {length}"
			);
		}
	}

	#[test]
	fn an_unknown_owner_is_rejected() {
		let image =
			tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, default_fee_payload())]);

		assert_eq!(
			decode_transfer_fee_config(
				&image,
				&crate::constants::SYSTEM_PROGRAM_ID
			),
			Err(TransferFeeDecodeError::UnknownOwner)
		);
	}

	#[test]
	fn a_classic_owner_short_circuits_the_walk() {
		let image =
			tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, default_fee_payload())]);

		assert_eq!(
			decode_transfer_fee_config(&image, &TOKEN_PROGRAM_ID),
			Ok(None)
		);
	}

	/// Mainnet fixtures never change rate across epochs; this test does.
	#[test]
	fn mint_fee_at_epoch_reads_the_entry_for_the_epoch() {
		let image =
			tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, default_fee_payload())]);

		assert_eq!(
			mint_fee_at_epoch(&image, &TOKEN_2022_PROGRAM_ID, NEWER.epoch - 1),
			Ok(Some(MintFee {
				bps: OLDER.basis_points,
				maximum_fee: OLDER.maximum_fee,
			}))
		);
		assert_eq!(
			mint_fee_at_epoch(&image, &TOKEN_2022_PROGRAM_ID, NEWER.epoch),
			Ok(Some(MintFee {
				bps: NEWER.basis_points,
				maximum_fee: NEWER.maximum_fee,
			}))
		);
	}

	#[test]
	fn mint_extensions_lists_every_entry_in_order() {
		let mut entries = other_extensions();
		entries.insert(1, (TRANSFER_FEE_CONFIG_TYPE, default_fee_payload()));
		let mut image = tlv_image(&entries);
		image.extend(core::iter::repeat_n(0_u8, 64));

		let extensions =
			mint_extensions(&image, &TOKEN_2022_PROGRAM_ID).unwrap();
		let expected: Vec<(u16, &[u8])> = entries
			.iter()
			.map(|(extension_type, payload)| {
				(*extension_type, payload.as_slice())
			})
			.collect();
		assert_eq!(extensions, expected);
		assert_eq!(mint_extensions(&image, &TOKEN_PROGRAM_ID), Ok(Vec::new()));
	}

	/// SPL's walk ends on a tail too short for a type, or on a zero type too
	/// short for a length: a mint whose extensions total `Multisig::LEN` is
	/// allocated two bytes past them.
	#[test]
	fn a_short_zero_tail_ends_the_walk() {
		for tail in 1..TLV_HEADER_LEN {
			let mut image =
				tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, default_fee_payload())]);
			image.extend(core::iter::repeat_n(0_u8, tail));

			assert_eq!(decode(&image), Ok(expected()), "tail of {tail}");
		}
	}

	/// Every entry must be whole, including those after the fee config.
	#[test]
	fn a_cut_entry_after_the_config_is_malformed() {
		let mut image =
			tlv_image(&[(TRANSFER_FEE_CONFIG_TYPE, default_fee_payload())]);
		image.extend_from_slice(&[18, 0, 64, 0]);

		assert_eq!(decode(&image), Err(TransferFeeDecodeError::Malformed));
	}
}
