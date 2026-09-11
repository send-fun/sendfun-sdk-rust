use solana_address::Address;

use crate::constants::ATA_PROGRAM_ID;

#[must_use]
pub fn find_associated_token_pda(
	wallet: &Address,
	mint: &Address,
	token_program: &Address,
) -> (Address, u8) {
	Address::find_program_address(
		&[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
		&ATA_PROGRAM_ID,
	)
}
