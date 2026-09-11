#[must_use]
pub fn parse_token_amount(data: &[u8]) -> Option<u64> {
	const AMOUNT_OFFSET: usize = 64; // mint (32) + owner (32)
	let bytes: [u8; 8] = data
		.get(AMOUNT_OFFSET..AMOUNT_OFFSET + 8)?
		.try_into()
		.ok()?;
	Some(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
	use super::parse_token_amount;

	#[test]
	fn parse_token_amount_correct_offset() {
		let mut data = vec![0u8; 165];
		let expected: u64 = 1_000_000_000;
		data[64..72].copy_from_slice(&expected.to_le_bytes());
		assert_eq!(parse_token_amount(&data), Some(expected));
	}

	#[test]
	fn parse_token_amount_too_short() {
		let data = vec![0u8; 60];
		assert_eq!(parse_token_amount(&data), None);
	}
}
