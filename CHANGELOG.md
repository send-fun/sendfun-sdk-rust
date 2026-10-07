# Changelog

## 3.0.0

### Features

- launchpad: Add `creator_fee_mode`, `creator_fee_bps` and `dex_creator_fee_bps` to `BondingCurve`.
- dex: Add `creator_fee_mode` and `creator_fee_bps` to `Pool`.
- nexus: Add `allowed_creator_fee_modes` to `PartnerConfig`.
- launchpad, dex: Add `pda::find_creator_fee_config_pda` and `pda::create_creator_fee_config_pda`.
- launchpad, dex: Add `creator_fee_mode` and `creator_fee_bps` to `TradeEvent`, `TokenCreateEvent` and `PoolCreateEvent`.
- launchpad: Add `dex_creator_fee_bps` to `TokenCreateEvent`.
- launchpad, dex, nexus: Add the errors `CreatorFeeTooHigh` and `CreatorFeeModeNotAllowed`.

### Fixes

- transfer_hook: Do not refuse a read-only hook account that is `user` or the market.

### Breaking

- launchpad: Remove `creator_platform` and `creator_id` from `BuildCreateTokenParams`.
- launchpad: Add `creator_fee_mode`, `creator_fee_bps` and `dex_creator_fee_bps` to `BuildCreateTokenParams`.
- launchpad: Remove `creator_platform`, `creator_id` and `creator_hash` from `CreateTokenInstructionArgs`.
- launchpad: Add `creator_fee_mode`, `creator_fee_bps` and `dex_creator_fee_bps` to `CreateTokenInstructionArgs`.
- launchpad: Remove `creator_fee_config` from `BuildMigrateParams` and `MigrateBuilder::new`.
- launchpad, dex: Remove `creator_fee_config` from `ClaimCreatorFeesBuilder::new`.
- launchpad: Add the `dex_creator_fee_config` account to `migrate`.
- launchpad, dex: Change the `creator_fee_config` account to the PDA from `pda::find_creator_fee_config_pda`.
- launchpad, dex: Make `creator_fee_config` writable in `claim_creator_fees` and `migrate`.
- launchpad, dex: Add `creator_fee_bps` to `CurveMarket` and `PoolMarket`.
- launchpad, dex: Replace `creator_fee_config` with `padding0` in `BondingCurve`, `Pool` and the events.
- launchpad, dex: Remove `creator_platform` and `creator_id` from `TokenCreateEvent`.
- launchpad: Increase `BondingCurve::LEN` from 421 to 618.
- dex: Increase `Pool::LEN` from 389 to 584.
- nexus: Increase `PartnerConfig::LEN` from 173 to 399.
- nexus: Increase `UserRewardDebt::LEN` from 182 to 214.
- nexus: Increase `UserStakePosition::LEN` from 120 to 216.
- nexus: Increase `reserved` in `FeePreset` from 32 to 64 bytes.
- nexus: Rename `creator_fee_bps` to `max_creator_fee_bps` in `LaunchpadFees` and `DexFees`.
- nexus: Add the `creator_fee_bps` argument to `total_fee_bps`.
- nexus: Change `effective_fee_bps` to take `EffectiveFeeArgs`.
- nexus: Remove `CreatorFeeConfig`, `CallerType` and the `creator_fee_config` PDA helpers.
- utils: Remove `creator_hash`, `creator_hash_from_id`, `encode_creator_id` and `decode_creator_id`.
- launchpad, dex, nexus: Rename `InvalidCreatorId` to `Unused7012`.
- dex: Rename `InvalidCreatorHash` to `InvalidCoinCreator`.
- transfer_hook: Remove `Clash::SignerNotForwarded`.

## 2.0.1

The SDK has no RPC client. Fetch accounts with your own client, then decode
them with `from_account`.

### Removed

- The `fetch` feature and its `solana-rpc-client` and `solana-account`
  dependencies.
- The generated `fetch_x`, `fetch_all_x`, `fetch_maybe_x` and
  `fetch_all_maybe_x` functions.
- `DecodedAccount`, `MaybeAccount`, and the `launchpad::shared`, `dex::shared`
  and `nexus::shared` modules.
- `transfer_fee::TransferFeeEntry`, `TransferFeeDecodeError`,
  `decode_transfer_fee_config` and `transfer_fee_at_epoch`.
- `launchpad::migrate::build_migrate_instruction_from_parts` and
  `BuildMigrateFromPartsParams`. Use `build_migrate_instruction`.

### Changed

- `transfer_fee::TransferFeeConfig` and `TransferFee` are SPL's types, from
  `spl_token_2022_interface::extension::transfer_fee`. Get an epoch's fee with
  `config.get_epoch_fee(epoch)`.
- `transfer_fee::mint_extensions` returns
  `Result<Option<PodStateWithExtensions<'_, PodMint>>, ProgramError>`. A classic
  SPL mint is `Ok(None)`. A mint that no token program owns is
  `ProgramError::IncorrectProgramId`.
- `transfer_fee::mint_fee_at_epoch` fails with `ProgramError`.
- `launchpad::migrate::BuildMigrateParams` takes `base_mint`, `quote_mint` and
  `creator_fee_config`, not `curve` and `curve_key`.

### Added

- `from_account(&owner, &data)` and `from_account_maybe(&owner, &data)` on each
  account type. Both fail with `std::io::Error`.
    - `from_account` checks the owner, then the discriminator.
    - `from_account_maybe` returns `Ok(None)` for an uncreated PDA that holds
      lamports.
- `launchpad::quote(&CurveMarket, QuoteRequest)` and
  `dex::quote(&PoolMarket, QuoteRequest)` calculate a trade off-chain.
- `TradeArgs`, `trade_instruction`, `trade_account_metas` and `trade_data` in
  `launchpad::trade` and `dex::trade`.
- `utils::trade` is public. `utils` re-exports its `TradeAccounts`,
  `TradeDirection`, `TradeMode`, `QuoteRequest`, `MarketQuote`,
  `MarketQuoteError`, `Landing` and `StandInTrade`.
- `utils::TradeDirection` converts to `dex::types::TradeDirection` with `From`.
- `transfer_hook` resolves Token-2022 transfer-hook accounts offline.
- `transfer_fee::extension::<V>` reads one mint extension.
  `transfer_fee::mint_fee` converts a `TransferFee` to a `MintFee`.

### Migrating from 1.x

Remove `features = ["fetch"]`. Fetch each account with your client. Decode it
with `from_account`, or `from_account_maybe` if it can be missing:

```rust,ignore
// 1.x
use sendfun_sdk::launchpad::accounts::{
    fetch_bonding_curve, fetch_maybe_bonding_curve,
};
use sendfun_sdk::launchpad::shared::MaybeAccount;

let curve = fetch_bonding_curve(&rpc, &address)?.data;
let maybe = match fetch_maybe_bonding_curve(&rpc, &address)? {
    MaybeAccount::Exists(account) => Some(account.data),
    MaybeAccount::NotFound(_) => None,
};

// 2.0
use sendfun_sdk::launchpad::accounts::BondingCurve;

let account = rpc.get_account(&address)?;
let curve = BondingCurve::from_account(&account.owner, &account.data)?;

let maybe = match rpc.get_account_with_commitment(&address, rpc.commitment())?.value {
    Some(account) => BondingCurve::from_account_maybe(&account.owner, &account.data)?,
    None => None,
};
```

For `fetch_all_x` and `fetch_all_maybe_x`, decode each entry of
`get_multiple_accounts`. A nonblocking client is the same with `.await`.

An RPC failure is your client's error. A decode failure is `std::io::Error`.

### Clients

Compatible: `solana-rpc-client` 3.x and 4.1+, blocking and nonblocking.

Not compatible: `solana-rpc-client` 2.x and 4.0.x.
