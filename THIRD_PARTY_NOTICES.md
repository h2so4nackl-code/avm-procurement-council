# Third-party sources

- `vendor/avm-contract-sdk/src/*.rs`: unmodified guest SDK source from
  https://github.com/gosh-sh/avm-sdk at commit
  `3a33ece89e67e291785f07e84f5b98a00ae85752`, MIT, GOSH Technology Ltd.
  The standalone Cargo manifest supplies the original workspace metadata locally;
  no third-party Rust crate or network resolution is required.
  Original license: `vendor/avm-contract-sdk/LICENSE.md`.
- `tools/release.py`: unmodified official installer/verifier from the same commit,
  under that same GOSH MIT license. Used only for optional SDK setup.
- Canonical native frame builders in `contracts/council/tests/support/mod.rs` and
  the `Stand` subprocess helper in `scripts/e2e.py` adapt the SDK's
  `examples/bounty-escrow` fixtures. Their MIT license is retained in
  `licenses/bounty-escrow-LICENSE`.
- Council logic, receiver logic, governance/accounting tests, capacity journey,
  schema generator, payload CLI and project documentation are new project work.
- The official AVM binary tools and Rust toolchain are not redistributed in this
  project archive. The archive includes the two newly built contract triples.
