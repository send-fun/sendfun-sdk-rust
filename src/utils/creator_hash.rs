use solana_address::Address;

/// SHA-256 of the byte length of `creator_platform`, `creator_platform`, the
/// byte length of `creator_id`, and `creator_id`. Each length is a
/// little-endian `u32`. Seeds the `CreatorFeeConfig` PDA.
#[must_use]
pub fn creator_hash_from_id(
	creator_platform: &str,
	creator_id: &str,
) -> Address {
	let platform_len = (creator_platform.len() as u32).to_le_bytes();
	let id_len = (creator_id.len() as u32).to_le_bytes();
	Address::new_from_array(
		solana_sha256_hasher::hashv(&[
			&platform_len,
			creator_platform.as_bytes(),
			&id_len,
			creator_id.as_bytes(),
		])
		.to_bytes(),
	)
}

/// Copies `text` into `[u8; N]` and pads it with NUL bytes. `None` if `text` is
/// longer than `N` bytes.
#[must_use]
pub fn encode_creator_id<const N: usize>(text: &str) -> Option<[u8; N]> {
	let bytes = text.as_bytes();
	let mut out = [0u8; N];
	out.get_mut(..bytes.len())?.copy_from_slice(bytes);
	Some(out)
}

/// Removes the trailing NUL bytes. `None` if the rest is not UTF-8. Pass the
/// result to [`creator_hash_from_id`]. The padded array gives a different hash.
#[must_use]
pub fn decode_creator_id(bytes: &[u8]) -> Option<&str> {
	let end = match bytes.iter().rposition(|&b| b != 0) {
		Some(last) => last.checked_add(1)?,
		None => 0,
	};
	core::str::from_utf8(bytes.get(..end)?).ok()
}

#[cfg(test)]
mod tests {
	use solana_address::{Address, address};

	use super::creator_hash_from_id;

	#[test]
	fn creator_hash_wallet_platform() {
		let hash =
			creator_hash_from_id("wallet", "11111111111111111111111111111111");
		let hash2 =
			creator_hash_from_id("wallet", "11111111111111111111111111111111");
		assert_eq!(hash, hash2);
		let hash3 =
			creator_hash_from_id("wallet", "22222222222222222222222222222222");
		assert_ne!(hash, hash3);
	}

	#[test]
	fn creator_hash_different_platforms() {
		let hash_wallet = creator_hash_from_id("wallet", "test_id");
		let hash_twitter = creator_hash_from_id("twitter", "test_id");
		assert_ne!(hash_wallet, hash_twitter);
	}

	#[test]
	fn creator_hash_matches_sha256() {
		use sha2::{Digest, Sha256};

		let platform = "wallet";
		let id = "TestWalletAddress123456789012345";
		let hash = creator_hash_from_id(platform, id);

		let mut hasher = Sha256::new();
		hasher.update((platform.len() as u32).to_le_bytes());
		hasher.update(platform.as_bytes());
		hasher.update((id.len() as u32).to_le_bytes());
		hasher.update(id.as_bytes());
		let expected = Address::new_from_array(hasher.finalize().into());
		assert_eq!(hash, expected);
	}

	#[test]
	fn creator_hash_matches_the_typescript_sdk() {
		assert_eq!(
			creator_hash_from_id("twitter", "alice123"),
			address!("oiZL4KcM2jYpYBHDtTUm9QbGUeWvEfPQN35ufpDzxTL"),
		);
	}
}
