# Review guide

## Concrete use case

A team reserves one purchasing pool across several supplier orders. Different
members approve the exact delivery revision; payment is allowed only after
quorum and a delay. Failed recipient execution must return the full principal
before a new attempt is allowed. This is a bounded SDK example, with no claim
of production custody readiness.

## Review order

1. `docs/SPEC.md`: settle the intended authority, deadline and cancellation rules.
2. `contracts/council/src/lib.rs`: inspect budget conservation, revision-bound
   vote masks, delay restart, pause/resume and the one-pending-transfer lock.
3. `contracts/receiver/src/lib.rs`: inspect source binding, exact receipt echo
   and duplicate-attempt rejection; its failure switch is a test fixture.
4. `contracts/council/tests/regressions.rs`: adversarial state, authorization,
   arithmetic, callback source/body/refund and governance cases.
5. `scripts/e2e.py` and `scripts/capacity_e2e.py`: actual SDK WASM execution,
   authenticated external callers, failure refunds and final replay agreement.

## Reproduce

```bash
python3 scripts/setup_sdk.py
bash scripts/check.sh
```

The GitHub Actions workflow runs this exact path on Ubuntu 24.04, with Rust
1.93.0, the immutable SDK release and source-pinned GitHub actions. It requires
no repository secrets, signing keys or real funds. Its initial presence is not
a claim that hosted CI has passed; inspect the run linked to the revision.

The included native result contains 57 project cases plus 9 inherited SDK unit
cases. Native synthetic signature bytes are not a cryptographic signature test.
Both stand journeys use the runtime's authenticated message paths.

## Deliberate scope limits

- Lifetime capacity: 8 orders, 4 milestones/order, 5 signers, 64 payment attempts.
- No migration, signer rotation, general withdrawal, principal remapping or
  unresolved-callback recovery. Unallocated funds stay in the purchasing pool.
- One member may repeatedly pause; approvals may be cancelled by a quorum.
- A deadline prevents a new payment even if its delivery was approved earlier.
- An in-flight callback can still settle after pause/deadline.
- Synthetic ECC ID 0 and SHELL ID 2 refer to the selected local SDK fixture.

For SDK inclusion, preserve licenses and change the vendored guest dependency to
the checkout's crate. Requalify after changing any source/tool/profile. No
upstream approval, reward eligibility or mainnet compatibility is asserted.
