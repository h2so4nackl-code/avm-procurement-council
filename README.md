# AVM Procurement Council

[Review guide](docs/REVIEW.md) · [CI workflow](.github/workflows/qualification.yml)

A Rust AVM contract for a small organization's shared purchasing budget: several
signers approve exact supplier deliveries, a timelock separates approval from
payment, and ECC accounting settles only after the runtime returns an authenticated
callback. Multiple orders share one budget without overbooking it.

Built for the **AVM SDK 0.3.0 test suite**, not a production custody service.
The companion receiver is a deterministic integration fixture with an intentional
failure switch. All demonstrations use synthetic local ECC; no real funds or
mainnet transactions are required.

## What it does

- Immutable council: 2–5 distinct signing keys and an M-of-N threshold (minimum 2).
- Up to 8 distinct orders, each with 1–4 positive milestone amounts, an immutable
  supplier key, recipient address, specification digest and hard deadline.
- Separates supplier and approver keys; suppliers cannot be council members.
- Reserves the whole unpaid order budget at creation, preventing double allocation.
- Supplier revisions erase prior approvals. Votes include both revision and digest.
- Threshold crossing starts a configured delay; revocation below threshold resets it.
- Anyone may execute an approved, mature payment; authorization comes from council votes.
- One in-flight transfer across the treasury. Principal is booked as spent only
  after an exact successful callback; failure requires the full principal refund.
- Records causal hashes for up to 64 settled attempts to reject replayed callbacks.
- A council member can pause new orders/deliveries/votes/payments. Resuming requires
  a fresh quorum; cancellation, expiry, funding and callback settlement remain possible.
- Quorum cancellation or deadline expiry releases unpaid reservations back to the pool.
- Canonical external change events carry available, reserved and spent amounts.

## Run all checks

Linux x86_64-v3, Python 3.11+, Bash, and Rust **1.93.0** with
`wasm32-unknown-unknown`, `rustfmt`, and `clippy` are required. The project's
`rust-toolchain.toml` pins these components. Follow the official SDK's GLIBC/CPU
requirements; this archive does not contain its binary tools.

```bash
# From the extracted project directory:
python3 scripts/setup_sdk.py
bash scripts/check.sh
```

The optional setup helper downloads exactly release
`avm-dev-2662a32e7c91-sdk-93e34e16e449`, checks the pinned archive SHA-256, and uses
the included unmodified official verifier for its inventory/provenance checks.
It installs under this project only. A fresh private stand is created per journey.
To use an already verified SDK bundle instead:

```bash
bash scripts/check.sh /absolute/path/to/avm-sdk-linux-x86_64
```

For native Rust tests without the SDK binary bundle:

```bash
cargo +1.93.0 test --workspace --locked --offline
```

The local guest SDK is vendored with its license; there are no crates.io
dependencies. The official builder compiles each contract twice and checks
reproducibility. No WASM postprocessor or optimizer is necessary.

## Verified result

The receipts below are local qualification. Hosted CI has not run yet; the
included workflow is ready to run after repository publication.

The included evidence records **57 project tests + 9 inherited SDK unit tests**,
clean strict Clippy, official contract builds, canonical ABI identifier checks,
and two real AVM stand journeys with matching replay roots and fees:

| Journey | Result |
|---|---|
| Purchasing lifecycle | 3 milestones paid, one rejected transfer refunded, successful retry, revision/stale vote, quorum resume, cancellation and expiry |
| Capacity | 8 orders with 4 milestones each, 63 rejected/refunded attempts followed by a successful 64th attempt, and rejection before a 65th outbound |

The primary journey deposits 3,000 synthetic units, pays 1,000 to the receiver,
and ends with 0 reserved and **2,000 unallocated**. Its 4 settled attempts include
the one failure. The receiver confirms exactly 3 successful payments totaling 1,000.

`evidence/qualification.json` binds source hashes, artifacts, tool hashes and
journey receipts. The first journey's detailed journal is `evidence/avm-e2e.json`;
the capacity journey is `evidence/avm-capacity.json`. Journals contain public
identities and simulated state, not stand signing keys. Temporary private stand
files are removed after each run.

## Files

| Path | Purpose |
|---|---|
| `contracts/council` | Treasury, governance, order lifecycle and settlement |
| `contracts/receiver` | Bound test recipient; acknowledgements and controlled failure |
| `contracts/*/avm-contract.toml` | Official builder metadata and explicit errors |
| `scripts/encode.py` | Public argument encoding and state-receipt inspection |
| `scripts/check.sh` | Full reproducible verification command |
| `scripts/sync_ids.py` | Extracts method/event IDs from official ACAB; never guesses hashes |
| `artifacts/{council,receiver}` | Verified WASM, ACAB and descriptor triples |
| `docs/SPEC.md` | Authorization, binary layouts, invariants and failure behavior |
| `docs/SDK-INTEGRATION.md` | Integration recipe and remaining limitations |

Example payload generation (replace the values with public identities):

```bash
python3 scripts/encode.py id 0
python3 scripts/encode.py vote --id 0 --revision 1 --digest "$DELIVERY_DIGEST"
# Output can be passed directly as the value of --args-json.
```

## Boundaries

This is a finite-budget, finite-capacity reference contract. It has **no signer
rotation, code migration, general withdrawal method, automatic callback recovery,
or arbitrary token support**. Deposits belong to the shared budget, not to
per-depositor refundable balances. Cancellation releases a reservation; it does
not send funds back to the depositor. Never fund more than the bounded test needs.

An unresolved callback keeps the lock; there is deliberately no timeout that
could cause a duplicate payout. A pause cannot recall an already emitted transfer.
The hard order deadline also stops approved payments; cancellation can revoke an
unpaid milestone even after it was approved. These semantics must be accepted
by all parties before any adaptation beyond local testing.

The principal uses local synthetic ECC ID 0; SHELL fees/rent use ID 2. Neither is
a claim about network asset mapping or current network fees. Contract validation
and test evidence do not constitute a security audit or production qualification.

License: MIT. See `THIRD_PARTY_NOTICES.md` for original SDK and fixture credits.
