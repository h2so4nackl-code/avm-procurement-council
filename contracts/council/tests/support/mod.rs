#![allow(clippy::too_many_arguments)]
// Canonical test frame helpers adapted from GOSH SDK bounty-escrow MIT fixture.
use avm_contract_sdk::AccountAddress;
use avm_procurement_council::*;
pub fn address(id: u8) -> AccountAddress {
    AccountAddress {
        dapp_id: [0x11; 32],
        account_id: [id; 32],
    }
}
pub fn write_address_at(b: &mut [u8], p: usize, a: AccountAddress) {
    b[p..p + 32].copy_from_slice(&a.dapp_id);
    b[p + 32..p + 64].copy_from_slice(&a.account_id);
}
pub fn envelope(init: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 288];
    b[..4].copy_from_slice(b"ACDP");
    b[268..272].copy_from_slice(&(init.len() as u32).to_le_bytes());
    b.extend_from_slice(init);
    b
}
fn extras(b: &mut Vec<u8>, entries: &[(u32, u128)]) {
    b.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for (id, amount) in entries {
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(&amount.to_le_bytes());
    }
}
pub fn frame(
    message: &[u8],
    state: &[u8],
    actor: [u8; 32],
    source: Option<AccountAddress>,
    destination: AccountAddress,
    time: u32,
    balance: u128,
) -> Vec<u8> {
    let mut b = Vec::from(*b"ACMI");
    b.extend_from_slice(&1u64.to_le_bytes());
    b.extend_from_slice(&time.to_le_bytes());
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&[0x12; 32]);
    b.push(u8::from(source.is_none()));
    b.extend_from_slice(&[0; 7]);
    if let Some(a) = source {
        b.extend_from_slice(&a.dapp_id);
        b.extend_from_slice(&a.account_id);
        b.extend_from_slice(&[0; 32]);
    } else {
        b.extend_from_slice(&[0; 64]);
        b.extend_from_slice(&actor);
    }
    b.extend_from_slice(&destination.dapp_id);
    b.extend_from_slice(&destination.account_id);
    b.extend_from_slice(&[0x44; 32]);
    b.extend_from_slice(&[0; 8]);
    extras(&mut b, &[(PRINCIPAL, balance), (SHELL, 1000000)]);
    b.extend_from_slice(&(state.len() as u32).to_le_bytes());
    b.extend_from_slice(state);
    b.extend_from_slice(&(message.len() as u32).to_le_bytes());
    b.extend_from_slice(message);
    b.extend_from_slice(&4u32.to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b
}
pub fn external(
    body: &[u8],
    actor: [u8; 32],
    method: u64,
    value: u128,
    destination: AccountAddress,
    deploy: bool,
    extra: &[(u32, u128)],
) -> Vec<u8> {
    let mut entries = Vec::new();
    if value != 0 {
        entries.push((PRINCIPAL, value));
    }
    entries.extend_from_slice(extra);
    let mut b = vec![0; 192];
    b[0] = 1;
    b[1] = if deploy { 3 } else { 1 };
    b[2..4].copy_from_slice(&(entries.len() as u16).to_le_bytes());
    b[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
    b[8..16].copy_from_slice(&method.to_le_bytes());
    b[16..32].copy_from_slice(&10000u128.to_le_bytes());
    b[32..40].copy_from_slice(&1u64.to_le_bytes());
    b[40..44].copy_from_slice(&1u32.to_le_bytes());
    b[44..48].copy_from_slice(&1000u32.to_le_bytes());
    write_address_at(&mut b, 48, destination);
    b[112..144].copy_from_slice(&actor);
    b[144..146].copy_from_slice(&64u16.to_le_bytes());
    for (id, n) in entries {
        b.extend_from_slice(&id.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
    }
    b.extend_from_slice(body);
    b.extend_from_slice(&[0; 64]);
    b
}
