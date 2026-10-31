use avm_contract_sdk::{
    decode_message_input, encode_message_output, Contract, MessageOutput, StateUpdate,
};
use avm_procurement_council::{ids, payment_body, Config, Order, Pending, State, PRINCIPAL};
use avm_procurement_receiver::{Receiver, ARGS, AUTH, REJECT};
#[path = "../../council/tests/support/mod.rs"]
mod support;
use support::*;
const OWNER: [u8; 32] = [0x41; 32];
fn state(out: MessageOutput) -> Vec<u8> {
    assert_eq!(out.status_code, 0);
    let Some(StateUpdate::FullReplace(b)) = out.state_update else {
        panic!()
    };
    b
}
fn fail(out: MessageOutput, code: u32) {
    assert_eq!(out.status_code, code);
    assert!(
        out.state_update.is_none()
            && out.outbound_intents.is_empty()
            && out.callback_result_body.is_empty()
    );
}
fn init() -> Vec<u8> {
    let msg = external(&envelope(&[]), OWNER, 0, 0, address(2), true, &[]);
    let b = frame(&msg, &[], OWNER, None, address(2), 1, 0);
    state(Receiver::init(decode_message_input(&b).unwrap()))
}
fn call(s: &[u8], actor: [u8; 32], method: u64, body: &[u8]) -> MessageOutput {
    let msg = external(body, actor, method, 0, address(2), false, &[]);
    let b = frame(&msg, s, actor, None, address(2), 2, 1000);
    let out = Receiver::process_message(decode_message_input(&b).unwrap());
    encode_message_output(&out).unwrap();
    out
}
fn bound() -> Vec<u8> {
    let mut a = vec![];
    avm_procurement_council::put_address(&mut a, address(1));
    state(call(&init(), OWNER, ids::BIND, &a))
}
fn receipt(attempt: u64) -> Vec<u8> {
    let mut s = State::new(Config {
        members: vec![OWNER, [0x42; 32]],
        threshold: 2,
        delay: 5,
        fuel: 10000,
    });
    s.orders.push(Order {
        vendor: [0x43; 32],
        receiver: address(2),
        specification: [0x50; 32],
        amounts: [300, 0, 0, 0],
        deadline: 1000,
        stages: 1,
        stage: 0,
        phase: 0,
        votes: 3,
        cancel_votes: 0,
        revision: 1,
        ready_at: 10,
        delivery: [0x61; 32],
        paid: 0,
    });
    s.pending = Some(Pending {
        order: 0,
        stage: 0,
        attempt,
        amount: 300,
    });
    payment_body(&s).unwrap()
}
fn receive(
    s: &[u8],
    source: u8,
    body: &[u8],
    value: u128,
    currency: u32,
    request: bool,
) -> MessageOutput {
    let mut msg = vec![0; 176];
    msg[1] = if request { 4 } else { 0 };
    msg[2..4].copy_from_slice(&1u16.to_le_bytes());
    msg[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
    msg[8..16].copy_from_slice(&ids::RECEIVE.to_le_bytes());
    msg[32..40].copy_from_slice(&5u64.to_le_bytes());
    msg[40..44].copy_from_slice(&20u32.to_le_bytes());
    write_address_at(&mut msg, 48, address(source));
    write_address_at(&mut msg, 112, address(2));
    if request {
        msg.extend_from_slice(&ids::CALLBACK.to_le_bytes());
    }
    msg.extend_from_slice(&currency.to_le_bytes());
    msg.extend_from_slice(&value.to_le_bytes());
    msg.extend_from_slice(body);
    let b = frame(
        &msg,
        s,
        [0; 32],
        Some(address(source)),
        address(2),
        20,
        1000,
    );
    let out = Receiver::process_message(decode_message_input(&b).unwrap());
    encode_message_output(&out).unwrap();
    out
}
#[test]
fn binding_is_owner_only_and_one_time() {
    let mut a = vec![];
    avm_procurement_council::put_address(&mut a, address(1));
    fail(call(&init(), [0x42; 32], ids::BIND, &a), AUTH);
    fail(call(&bound(), OWNER, ids::BIND, &a), ARGS);
}
#[test]
fn accepts_exact_principal_and_echoes_receipt() {
    let body = receipt(1);
    let out = receive(&bound(), 1, &body, 300, PRINCIPAL, true);
    assert_eq!(out.callback_result_body, body);
    let b = state(out);
    assert_eq!(u128::from_le_bytes(b[104..120].try_into().unwrap()), 300);
    assert_eq!(u64::from_le_bytes(b[120..128].try_into().unwrap()), 1);
}
#[test]
fn wrong_source_and_missing_request_are_rejected() {
    fail(
        receive(&bound(), 3, &receipt(1), 300, PRINCIPAL, true),
        AUTH,
    );
    fail(
        receive(&bound(), 1, &receipt(1), 300, PRINCIPAL, false),
        AUTH,
    );
}
#[test]
fn unbound_destination_rejects() {
    fail(receive(&init(), 1, &receipt(1), 300, PRINCIPAL, true), AUTH);
}
#[test]
fn externally_spoofed_receive_rejects() {
    fail(call(&bound(), OWNER, ids::RECEIVE, &receipt(1)), AUTH);
}
#[test]
fn exact_currency_and_amount_are_required() {
    for value in [0, 299, 301] {
        fail(
            receive(&bound(), 1, &receipt(1), value, PRINCIPAL, true),
            ARGS,
        );
    }
    fail(receive(&bound(), 1, &receipt(1), 300, 7, true), ARGS);
}
#[test]
fn duplicate_and_stale_attempts_reject() {
    let s = state(receive(&bound(), 1, &receipt(2), 300, PRINCIPAL, true));
    for n in [1, 2] {
        fail(receive(&s, 1, &receipt(n), 300, PRINCIPAL, true), ARGS);
    }
    assert_eq!(
        receive(&s, 1, &receipt(3), 300, PRINCIPAL, true).status_code,
        0
    );
}
#[test]
fn deliberate_failure_has_no_state_or_acknowledgement() {
    let s = state(call(&bound(), OWNER, ids::REJECT, &[1]));
    fail(receive(&s, 1, &receipt(1), 300, PRINCIPAL, true), REJECT);
    let s = state(call(&s, OWNER, ids::REJECT, &[0]));
    assert_eq!(
        receive(&s, 1, &receipt(1), 300, PRINCIPAL, true).status_code,
        0
    );
}
#[test]
fn malformed_receipts_and_state_padding_reject() {
    for offset in [0, 20, 73, 79] {
        let mut body = receipt(1);
        body[offset] = 255;
        fail(receive(&bound(), 1, &body, 300, PRINCIPAL, true), ARGS);
    }
    let mut s = bound();
    s[159] = 1;
    fail(receive(&s, 1, &receipt(1), 300, PRINCIPAL, true), ARGS);
}
#[test]
fn cumulative_balance_overflow_rejects_atomically() {
    let mut s = bound();
    s[104..120].copy_from_slice(&u128::MAX.to_le_bytes());
    fail(receive(&s, 1, &receipt(1), 300, PRINCIPAL, true), ARGS);
}
