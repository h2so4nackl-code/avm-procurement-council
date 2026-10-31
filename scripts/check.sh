#!/usr/bin/env bash
# Run from any directory. Only local simulated ECC is used by both journeys.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
sdk="${1:-$root/.tooling/sdk/current}"
sdk="$(cd "$sdk" && pwd -P)"
export PATH="${HOME}/.cargo/bin:${PATH}"
cd "$root"
mkdir -p evidence artifacts/{council,receiver}
cargo +1.93.0 fmt -p avm-procurement-council -p avm-procurement-receiver --check
cargo +1.93.0 test --workspace --locked --offline >evidence/native-tests.txt 2>&1
cargo +1.93.0 clippy --workspace --all-targets --locked --offline -- -D warnings >evidence/clippy.txt 2>&1
python3 scripts/generate_abi.py --check
python3 scripts/verify_sources.py
for contract in council receiver; do
  "$sdk/bin/avm-contract-build" build --manifest-path "$root/contracts/$contract/Cargo.toml" >"evidence/build-$contract.txt" 2>&1
done
python3 scripts/sync_ids.py --check
python3 scripts/e2e.py --sdk "$sdk"
python3 scripts/capacity_e2e.py --sdk "$sdk"
for contract in council receiver; do
  cp "contracts/$contract/target/avm/contract.wasm" "artifacts/$contract/contract.wasm"
  cp "contracts/$contract/target/avm/contract.acab" "artifacts/$contract/contract.acab"
  cp "contracts/$contract/target/avm/contract.descriptor" "artifacts/$contract/contract.descriptor"
done
python3 scripts/record_evidence.py --sdk "$sdk"
echo 'PASS: Rust tests, clippy, official reproducible builds, canonical IDs, both real AVM journeys and replay'
