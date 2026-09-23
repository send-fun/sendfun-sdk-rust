use solana_address::Address;

use crate::constants::DEFAULT_PARTNER;

/// `(partner, is_signer)` for the `partner` field of the generated
/// instructions. `is_signer` is `false` only for `DEFAULT_PARTNER`.
#[must_use]
pub fn partner_account(partner: &Address) -> (Address, bool) {
	(*partner, *partner != DEFAULT_PARTNER)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_non_default_partner_signs() {
		let partner = Address::new_unique();
		assert_eq!(partner_account(&partner), (partner, true));
	}

	#[test]
	fn the_default_partner_does_not_sign() {
		assert_eq!(partner_account(&DEFAULT_PARTNER), (DEFAULT_PARTNER, false));
	}
}
