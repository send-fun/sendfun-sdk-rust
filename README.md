# sendfun-sdk

Rust SDK for the send.fun programs on Solana.

- `launchpad`: token creation, bonding curve trades, migration.
- `dex`: AMM trades after migration.
- `nexus`: staking and partner fees.
- `transfer_hook`: the Token-2022 transfer-hook accounts of a trade. It
  resolves them from accounts that you fetch, without RPC.

The SDK builds instructions, decodes accounts and events, and calculates quotes
(`launchpad::quote`, `dex::quote`). It has no RPC client and does not send
transactions.

Fetch accounts with your own client. Compatible: `solana-rpc-client` 3.x and
4.1+, blocking and nonblocking. Not compatible: 4.0.x.

Decode an account with the `from_account` function of its type. `from_account`
checks the owner, then the discriminator:

```rust,ignore
use sendfun_sdk::launchpad::accounts::BondingCurve;

let account = rpc.get_account(&address)?;
let curve = BondingCurve::from_account(&account.owner, &account.data)?;
```

`get_account` fails when the account does not exist. For an account that can
be missing, use `get_account_with_commitment`. Its `value` is `None` for a
missing account. Decode with `from_account_maybe`. It returns `Ok(None)` for an
uncreated PDA that holds lamports.

## Install

```sh
cargo add sendfun-sdk
```

Optional features:

- `serde`: `Serialize` and `Deserialize` on accounts, events and types.
- `anchor`: `anchor-lang` interop for on-chain callers.

## Rules

- For a trade, read `platform_config` from the bonding curve or the pool.
- A partner other than `DEFAULT_PARTNER` must sign.
- `constants::find_platform_address(slug)` returns the key of a platform.
- Do not edit the `generated` modules. They come from the program IDLs.

## Documentation

<https://docs.send.fun>

## License

MIT, see the `LICENSE` file.
