# SDK 0.3.0 integration

This project is intended as an additional useful Rust guest example with
multiple authenticated actors and a bounded asynchronous financial workflow.
It complements the existing bounty escrow, vesting and Revenue Hub examples;
its primary additional test surface is shared-budget overcommit prevention,
M-of-N content-bound approval, quorum revocation, delay restart and emergency
pause/resume across multiple purchase orders.

## Suggested review/integration

1. Review `docs/SPEC.md`, including hard deadlines, cancellable approvals, pause
   liveness tradeoffs, finite capacity and the unresolved-callback lock.
2. Run `bash scripts/check.sh /absolute/path/to/verified/sdk/bundle` on Linux
   using the immutable SDK 0.3.0 tools/profile and Rust 1.93.0. Each stand state
   is fresh; do not reuse SDK 0.2.0 state or artifacts.
3. Review the two guest triples and `evidence/qualification.json`. Official
   builds check two independent compilation outputs; canonical IDs are read
   from official ACAB and checked against committed Rust constants.
4. To integrate into `examples/procurement-council`, switch the guest dependency
   to the SDK checkout's own crate and preserve the feature split: the receiver
   depends on council with `default-features = false`. Only the root council
   guest enables its WASM exports. This avoids duplicate entrypoint symbols.
5. Run native tests and both stand journeys again after any source, profile,
   toolchain, ABI or dependency change. Include this example in SDK CI only
   after maintainer review; no upstream approval is implied here.

This delivery includes 57 project tests (47 council, 10 receiver) and 9 inherited
vendored SDK unit tests. Native fixtures test canonical input/output with
synthetic signature bytes; signature authentication is provided by the real
stand/runtime in both integration journeys. Do not count native fixtures as
cryptographic signature verification. The capacity journey proves the upper
order and callback-record bounds work in this exact local runtime; it is not
a throughput benchmark or a general network capacity result.

For the first metadata build when intentionally changing method names, run the
official builder for both contracts, run `python3 scripts/sync_ids.py`, rebuild
both, and run `python3 scripts/sync_ids.py --check`. The `--bootstrap` option is
only for the first metadata bootstrap; it must not be used in qualification.

The original GOSH SDK and adapted fixture licenses are retained. Vendored guest
source and the official release verifier are pinned to
`3a33ece89e67e291785f07e84f5b98a00ae85752`; the immutable published binary release
binds its SDK build to `93e34e16e449`. That source is a parent of the merge and
has the matching 0.3.0 interfaces. No production migration is provided.

No PR, public message, mainnet deployment, reward submission or paid transaction
is performed by any script here. `setup_sdk.py` only downloads a public pinned
release; all other verification work is local.
