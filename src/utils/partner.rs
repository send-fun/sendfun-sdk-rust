use solana_address::Address;

use crate::constants::DEFAULT_PARTNER;

/// Makes the `partner` value for the generated builders. Only a partner that
/// is not `DEFAULT_PARTNER` signs.
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
