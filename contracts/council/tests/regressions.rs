use avm_contract_sdk::{
    decode_message_input, encode_message_output, Contract, MessageOutput, StateUpdate,
};
use avm_procurement_council::*;
mod support;
use support::*;
const A: [u8; 32] = [0x41; 32];
const B: [u8; 32] = [0x42; 32];
const C: [u8; 32] = [0x44; 32];
const V: [u8; 32] = [0x43; 32];
const X: [u8; 32] = [0x99; 32];
fn config() -> Config {
    Config {
        members: vec![A, B, C],
        threshold: 2,
        delay: 5,
        fuel: 10000,
    }
}
fn proposal(amounts: [u128; 4]) -> Order {
    Order {
        vendor: V,
        receiver: address(2),
        specification: [0x51; 32],
        amounts,
        deadline: 1000,
        stages: amounts.iter().take_while(|x| **x != 0).count() as u8,
        stage: 0,
        phase: ACTIVE,
        votes: 0,
        cancel_votes: 0,
        revision: 0,
        ready_at: 0,
        delivery: [0; 32],
        paid: 0,
    }
}
fn evaluate(
    s: &[u8],
    actor: [u8; 32],
    method: u64,
    body: &[u8],
    value: u128,
    time: u32,
    balance: u128,
) -> MessageOutput {
    let msg = external(body, actor, method, value, address(1), false, &[]);
    let bytes = frame(&msg, s, actor, None, address(1), time, balance);
    let out = Council::process_message(decode_message_input(&bytes).unwrap());
    encode_message_output(&out).expect("canonical output");
    out
}
fn call(s: &[u8], actor: [u8; 32], method: u64, body: &[u8], time: u32) -> MessageOutput {
    evaluate(s, actor, method, body, 0, time, 100000)
}
fn state(out: MessageOutput) -> Vec<u8> {
    assert_eq!(out.status_code, 0);
    let Some(StateUpdate::FullReplace(b)) = out.state_update else {
        panic!("state absent")
    };
    assert_eq!(b.len(), STATE_BYTES);
    let s = State::decode(&b).unwrap();
    assert_eq!(s.encode(), b);
    b
}
fn fail(out: MessageOutput, code: u32) {
    assert_eq!(out.status_code, code);
    assert!(out.state_update.is_none());
    assert!(
        out.outbound_intents.is_empty()
            && out.callback_intents.is_empty()
            && out.event_intents.is_empty()
            && out.callback_result_body.is_empty()
    );
}
fn init() -> Vec<u8> {
    let msg = external(
        &envelope(&config().encode()),
        A,
        0,
        0,
        address(1),
        true,
        &[],
    );
    let b = frame(&msg, &[], A, None, address(1), 1, 0);
    state(Council::init(decode_message_input(&b).unwrap()))
}
fn funded() -> Vec<u8> {
    state(evaluate(&init(), A, ids::DEPOSIT, &[], 2000, 2, 2000))
}
fn created() -> Vec<u8> {
    state(call(
        &funded(),
        A,
        ids::CREATE,
        &proposal([300, 400, 300, 0]).proposal_bytes(),
        3,
    ))
}
fn submit_args(id: u64, stage: u8, h: u8) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.push(stage);
    b.extend_from_slice(&[h; 32]);
    b
}
fn vote_args(id: u64, revision: u32, h: u8, yes: bool) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.extend_from_slice(&revision.to_le_bytes());
    b.extend_from_slice(&[h; 32]);
    b.push(yes as u8);
    b
}
fn submitted() -> Vec<u8> {
    state(call(
        &created(),
        V,
        ids::SUBMIT,
        &submit_args(0, 0, 0x61),
        4,
    ))
}
fn approved() -> Vec<u8> {
    let s = state(call(
        &submitted(),
        A,
        ids::VOTE,
        &vote_args(0, 1, 0x61, true),
        5,
    ));
    state(call(&s, B, ids::VOTE, &vote_args(0, 1, 0x61, true), 6))
}
fn dispatched() -> MessageOutput {
    call(&approved(), X, ids::EXECUTE, &0u64.to_le_bytes(), 11)
}
fn cb(s: &[u8], source_id: u8, body: &[u8], status: u32, value: u128, hash: u8) -> MessageOutput {
    let source = address(source_id);
    let mut msg = vec![0; 176];
    msg[1] = 1;
    msg[2..4].copy_from_slice(&u16::from(value != 0).to_le_bytes());
    msg[4..8].copy_from_slice(&(body.len() as u32).to_le_bytes());
    msg[8..16].copy_from_slice(&ids::CALLBACK.to_le_bytes());
    msg[32..40].copy_from_slice(&5u64.to_le_bytes());
    msg[40..44].copy_from_slice(&20u32.to_le_bytes());
    write_address_at(&mut msg, 48, source);
    write_address_at(&mut msg, 112, address(1));
    msg.extend_from_slice(&[hash; 32]);
    msg.extend_from_slice(&address(1).dapp_id);
    msg.extend_from_slice(&address(1).account_id);
    msg.extend_from_slice(&status.to_le_bytes());
    if value != 0 {
        msg.extend_from_slice(&PRINCIPAL.to_le_bytes());
        msg.extend_from_slice(&value.to_le_bytes());
    }
    msg.extend_from_slice(body);
    let b = frame(&msg, s, [0; 32], Some(source), address(1), 20, 2000);
    let out = Council::process_message(decode_message_input(&b).unwrap());
    encode_message_output(&out).unwrap();
    out
}
#[test]
fn codec_roundtrip() {
    let c = config();
    assert_eq!(c.encode().len(), CONFIG_BYTES);
    assert_eq!(Config::decode(&c.encode()), Ok(c));
    assert_eq!(
        proposal([1, 2, 3, 0]).proposal_bytes().len(),
        ORDER_ARGS_BYTES
    );
    State::decode(&init()).unwrap();
}
#[test]
fn deployment_requires_council_member() {
    let msg = external(
        &envelope(&config().encode()),
        X,
        0,
        0,
        address(1),
        true,
        &[],
    );
    let b = frame(&msg, &[], X, None, address(1), 1, 0);
    fail(Council::init(decode_message_input(&b).unwrap()), AUTH);
}
#[test]
fn threshold_and_members_are_checked() {
    for threshold in [0, 1, 4, 255] {
        let mut c = config();
        c.threshold = threshold;
        assert_eq!(Config::decode(&c.encode()), Err(ARGS));
    }
    for k in [[0; 32], A] {
        let mut c = config();
        c.members[1] = k;
        assert_eq!(c.validate(), Err(ARGS));
    }
}
#[test]
fn unused_config_slots_must_be_zero() {
    let mut b = config().encode();
    b[183] = 1;
    assert_eq!(Config::decode(&b), Err(ARGS));
}
#[test]
fn deposits_require_council_and_exact_currency() {
    fail(evaluate(&init(), X, ids::DEPOSIT, &[], 100, 2, 100), AUTH);
    fail(call(&init(), A, ids::DEPOSIT, &[], 2), VALUE);
    fail(evaluate(&init(), A, ids::DEPOSIT, &[], 100, 2, 99), BALANCE);
}
#[test]
fn deposits_are_additive_and_checked() {
    let s = state(evaluate(&funded(), B, ids::DEPOSIT, &[], 500, 3, 2500));
    assert_eq!(State::decode(&s).unwrap().deposited, 2500);
    let mut s = State::new(config());
    s.deposited = u128::MAX;
    fail(
        evaluate(&s.encode(), A, ids::DEPOSIT, &[], 1, 4, u128::MAX),
        LIMIT,
    );
}
#[test]
fn unexpected_value_does_not_donate_or_mutate() {
    fail(evaluate(&created(), A, ids::PAUSE, &[], 1, 4, 2001), VALUE);
}
#[test]
fn commitments_reserve_budget_and_prevent_overbooking() {
    let s = State::decode(&created()).unwrap();
    assert_eq!(
        (s.deposited, s.reserved, s.available().unwrap()),
        (2000, 1000, 1000)
    );
    let mut o = proposal([1001, 0, 0, 0]);
    fail(
        call(&s.encode(), A, ids::CREATE, &o.proposal_bytes(), 4),
        BALANCE,
    );
    o.amounts[0] = 1000;
    let s = state(call(&s.encode(), B, ids::CREATE, &o.proposal_bytes(), 4));
    assert_eq!(State::decode(&s).unwrap().available(), Ok(0));
}
#[test]
fn orders_require_separate_vendor_and_same_dapp_receiver() {
    let mut o = proposal([1, 0, 0, 0]);
    o.vendor = A;
    fail(
        call(&funded(), A, ids::CREATE, &o.proposal_bytes(), 4),
        AUTH,
    );
    o.vendor = V;
    o.receiver = address(1);
    fail(
        call(&funded(), A, ids::CREATE, &o.proposal_bytes(), 4),
        ARGS,
    );
    o.receiver.dapp_id = [0x77; 32];
    fail(
        call(&funded(), A, ids::CREATE, &o.proposal_bytes(), 4),
        ARGS,
    );
}
#[test]
fn invalid_milestones_and_arithmetic_overflow_reject() {
    for amounts in [[0, 0, 0, 0], [1, 0, 2, 0], [u128::MAX, 1, 0, 0]] {
        assert!(Order::proposal(&proposal(amounts).proposal_bytes()).is_err());
    }
}
#[test]
fn create_requires_useful_deadline() {
    let mut o = proposal([1, 0, 0, 0]);
    o.deadline = 9;
    fail(
        call(&funded(), A, ids::CREATE, &o.proposal_bytes(), 4),
        TIME,
    );
}
#[test]
fn vendor_is_only_submitter_and_stages_are_sequential() {
    fail(
        call(&created(), A, ids::SUBMIT, &submit_args(0, 0, 0x61), 4),
        AUTH,
    );
    fail(
        call(&created(), V, ids::SUBMIT, &submit_args(0, 1, 0x61), 4),
        ARGS,
    );
}
#[test]
fn revision_clears_votes_and_rejects_stale_approval() {
    let s = state(call(
        &submitted(),
        A,
        ids::VOTE,
        &vote_args(0, 1, 0x61, true),
        5,
    ));
    let s = state(call(&s, V, ids::SUBMIT, &submit_args(0, 0, 0x62), 6));
    let o = &State::decode(&s).unwrap().orders[0];
    assert_eq!((o.revision, o.votes, o.ready_at), (2, 0, 0));
    fail(
        call(&s, B, ids::VOTE, &vote_args(0, 1, 0x61, true), 7),
        STALE,
    );
}
#[test]
fn same_delivery_cannot_be_resubmitted() {
    fail(
        call(&submitted(), V, ids::SUBMIT, &submit_args(0, 0, 0x61), 5),
        DUPLICATE,
    );
}
#[test]
fn votes_require_signer_and_exact_delivery() {
    fail(
        call(&submitted(), V, ids::VOTE, &vote_args(0, 1, 0x61, true), 5),
        AUTH,
    );
    fail(
        call(&submitted(), A, ids::VOTE, &vote_args(0, 1, 0x62, true), 5),
        STALE,
    );
}
#[test]
fn duplicate_vote_cannot_reach_threshold() {
    let s = state(call(
        &submitted(),
        A,
        ids::VOTE,
        &vote_args(0, 1, 0x61, true),
        5,
    ));
    fail(
        call(&s, A, ids::VOTE, &vote_args(0, 1, 0x61, true), 6),
        DUPLICATE,
    );
    fail(call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 100), TIME);
}
#[test]
fn threshold_starts_delay_and_boundary_is_inclusive() {
    let s = approved();
    assert_eq!(State::decode(&s).unwrap().orders[0].ready_at, 11);
    fail(call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 10), TIME);
    assert_eq!(
        call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 11).status_code,
        0
    );
}
#[test]
fn revocation_below_quorum_resets_delay() {
    let s = state(call(
        &approved(),
        A,
        ids::VOTE,
        &vote_args(0, 1, 0x61, false),
        7,
    ));
    assert_eq!(State::decode(&s).unwrap().orders[0].ready_at, 0);
    let s = state(call(&s, A, ids::VOTE, &vote_args(0, 1, 0x61, true), 8));
    assert_eq!(State::decode(&s).unwrap().orders[0].ready_at, 13);
}
#[test]
fn extra_vote_does_not_extend_existing_delay() {
    let s = state(call(
        &approved(),
        C,
        ids::VOTE,
        &vote_args(0, 1, 0x61, true),
        7,
    ));
    assert_eq!(State::decode(&s).unwrap().orders[0].ready_at, 11);
    let s = state(call(&s, A, ids::VOTE, &vote_args(0, 1, 0x61, false), 8));
    assert_eq!(State::decode(&s).unwrap().orders[0].ready_at, 11);
}
#[test]
fn pause_requires_member_and_resume_requires_quorum() {
    fail(call(&created(), X, ids::PAUSE, &[], 4), AUTH);
    let s = state(call(&approved(), A, ids::PAUSE, &[], 7));
    fail(call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 11), PAUSED);
    let s = state(call(&s, A, ids::RESUME, &[], 8));
    assert!(State::decode(&s).unwrap().paused);
    fail(call(&s, A, ids::RESUME, &[], 9), DUPLICATE);
    let s = state(call(&s, B, ids::RESUME, &[], 9));
    assert!(!State::decode(&s).unwrap().paused);
}
#[test]
fn fresh_pause_invalidates_partial_resume() {
    let s = state(call(&created(), A, ids::PAUSE, &[], 4));
    let s = state(call(&s, A, ids::RESUME, &[], 5));
    let s = state(call(&s, B, ids::PAUSE, &[], 6));
    assert_eq!(State::decode(&s).unwrap().resume_votes, 0);
}
#[test]
fn dispatch_is_pending_until_callback_not_immediately_spent() {
    let out = dispatched();
    assert_eq!(out.outbound_intents.len(), 1);
    assert_eq!(out.callback_intents.len(), 1);
    assert_eq!(out.outbound_intents[0].extra[0].amount, 300);
    let s = State::decode(&state(out)).unwrap();
    assert_eq!((s.spent, s.reserved, s.attempts), (0, 1000, 1));
    assert!(s.pending.is_some());
}
#[test]
fn pending_blocks_retry_and_other_money_mutations() {
    let s = state(dispatched());
    for m in [
        ids::EXECUTE,
        ids::CREATE,
        ids::EXPIRE,
        ids::CANCEL,
        ids::SUBMIT,
        ids::VOTE,
    ] {
        fail(call(&s, A, m, &0u64.to_le_bytes(), 20), PENDING);
    }
    fail(evaluate(&s, A, ids::DEPOSIT, &[], 10, 20, 2010), PENDING);
}
#[test]
fn pending_permits_pause_status_and_authenticated_settlement() {
    let out = dispatched();
    let receipt = out.outbound_intents[0].body.clone();
    let s = state(out);
    let s = state(call(&s, A, ids::PAUSE, &[], 12));
    let s = state(call(&s, X, ids::STATUS, &[], 13));
    let s = state(cb(&s, 2, &receipt, 0, 0, 0x71));
    let s = State::decode(&s).unwrap();
    assert!(s.paused);
    assert_eq!((s.spent, s.reserved), (300, 700));
}
#[test]
fn successful_callback_advances_only_current_stage() {
    let out = dispatched();
    let body = out.outbound_intents[0].body.clone();
    let s = state(cb(&state(out), 2, &body, 0, 0, 0x71));
    let s = State::decode(&s).unwrap();
    let o = &s.orders[0];
    assert_eq!(
        (o.stage, o.votes, o.ready_at, o.delivery, o.paid),
        (1, 0, 0, [0; 32], 300)
    );
    assert_eq!(
        (s.spent, s.reserved, s.available().unwrap()),
        (300, 700, 1000)
    );
}
#[test]
fn failed_callback_requires_exact_refund_and_preserves_approval() {
    let s = state(dispatched());
    fail(cb(&s, 2, &[], 3102, 299, 0x71), CALLBACK);
    fail(cb(&s, 2, &[], 3102, 301, 0x71), CALLBACK);
    let s = state(cb(&s, 2, &[], 3102, 300, 0x71));
    let d = State::decode(&s).unwrap();
    assert_eq!((d.spent, d.reserved, d.orders[0].stage), (0, 1000, 0));
    assert_eq!(d.orders[0].ready_at, 11);
    assert_eq!(
        call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 21).status_code,
        0
    );
}
#[test]
fn wrong_source_or_altered_receipt_cannot_settle() {
    let out = dispatched();
    let mut body = out.outbound_intents[0].body.clone();
    let s = state(out);
    fail(cb(&s, 3, &body, 0, 0, 0x71), CALLBACK);
    body[21] ^= 1;
    fail(cb(&s, 2, &body, 0, 0, 0x71), CALLBACK);
}
#[test]
fn success_must_not_refund_principal() {
    let out = dispatched();
    let body = out.outbound_intents[0].body.clone();
    let s = state(out);
    fail(cb(&s, 2, &body, 0, 300, 0x71), CALLBACK);
}
#[test]
fn settled_causal_hash_cannot_settle_retry() {
    let s = state(dispatched());
    let s = state(cb(&s, 2, &[], 3102, 300, 0x71));
    let out = call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 21);
    let body = out.outbound_intents[0].body.clone();
    let s = state(out);
    fail(cb(&s, 2, &body, 0, 0, 0x71), CALLBACK);
    assert_eq!(cb(&s, 2, &body, 0, 0, 0x72).status_code, 0);
}
#[test]
fn callback_without_pending_is_rejected() {
    fail(cb(&approved(), 2, &[], 3102, 300, 0x71), CALLBACK);
}
#[test]
fn execute_checks_actual_account_balance() {
    fail(
        evaluate(
            &approved(),
            X,
            ids::EXECUTE,
            &0u64.to_le_bytes(),
            0,
            11,
            1999,
        ),
        BALANCE,
    );
}
#[test]
fn cancellation_requires_quorum_and_releases_only_unpaid_budget() {
    let out = dispatched();
    let body = out.outbound_intents[0].body.clone();
    let s = state(cb(&state(out), 2, &body, 0, 0, 0x71));
    let mut args = 0u64.to_le_bytes().to_vec();
    args.push(1);
    let s = state(call(&s, A, ids::CANCEL, &args, 30));
    assert_eq!(State::decode(&s).unwrap().reserved, 700);
    let s = state(call(&s, B, ids::CANCEL, &args, 31));
    let s = State::decode(&s).unwrap();
    assert_eq!(
        (
            s.reserved,
            s.spent,
            s.available().unwrap(),
            s.orders[0].phase
        ),
        (0, 300, 1700, CANCELLED)
    );
}
#[test]
fn expiry_boundary_releases_reservation_and_cannot_repeat() {
    fail(
        call(&created(), X, ids::EXPIRE, &0u64.to_le_bytes(), 999),
        TIME,
    );
    let s = state(call(&created(), X, ids::EXPIRE, &0u64.to_le_bytes(), 1000));
    assert_eq!(State::decode(&s).unwrap().reserved, 0);
    fail(call(&s, X, ids::EXPIRE, &0u64.to_le_bytes(), 1001), PHASE);
}
#[test]
fn deadline_prevents_payment_even_if_approved() {
    fail(
        call(&approved(), X, ids::EXECUTE, &0u64.to_le_bytes(), 1000),
        TIME,
    );
}
#[test]
fn order_capacity_and_ids_are_not_reused() {
    let mut s = funded();
    for i in 0..MAX_ORDERS {
        s = state(call(
            &s,
            A,
            ids::CREATE,
            &proposal([1, 0, 0, 0]).proposal_bytes(),
            4,
        ));
        s = state(call(&s, X, ids::EXPIRE, &(i as u64).to_le_bytes(), 1000));
    }
    fail(
        call(
            &s,
            A,
            ids::CREATE,
            &proposal([1, 0, 0, 0]).proposal_bytes(),
            4,
        ),
        LIMIT,
    );
}
#[test]
fn invalid_payload_lengths_and_unknown_methods_have_no_effects() {
    let s = created();
    for (m, len) in [
        (ids::CREATE, 197),
        (ids::SUBMIT, 41),
        (ids::VOTE, 45),
        (ids::EXECUTE, 8),
        (ids::CANCEL, 9),
    ] {
        for n in [0, 1, len - 1, len + 1] {
            let out = call(&s, A, m, &vec![0; n], 4);
            assert_ne!(out.status_code, 0);
            assert!(out.state_update.is_none() && out.outbound_intents.is_empty());
        }
    }
    fail(call(&s, A, u64::MAX, &[], 4), ARGS);
}
#[test]
fn corrupt_state_and_noncanonical_padding_are_rejected() {
    let s = created();
    for n in [0, 1, 183, 248, STATE_BYTES - 1, STATE_BYTES + 1] {
        assert!(State::decode(&vec![0; n]).is_err());
    }
    for offset in [0, STATE_BYTES - 1, 204, 220] {
        let mut b = s.clone();
        b[offset] ^= 0x80;
        assert!(State::decode(&b).is_err(), "offset {offset}");
    }
}
#[test]
fn maximum_state_remains_bounded_and_roundtrips() {
    let mut s = State::new(config());
    s.deposited = 8;
    for _ in 0..8 {
        s.orders.push(proposal([1, 0, 0, 0]));
    }
    s.reserved = 8;
    s.attempts = 64;
    s.receipts = (1..=64).map(|x| [x; 32]).collect();
    s.validate().unwrap();
    assert_eq!(State::decode(&s.encode()), Ok(s));
}
#[test]
fn full_three_stage_journey_and_closure() {
    let mut s = created();
    for (i, amount) in [300, 400, 300].into_iter().enumerate() {
        let h = 0x60 + i as u8;
        let t = 20 + i as u32 * 20;
        s = state(call(&s, V, ids::SUBMIT, &submit_args(0, i as u8, h), t));
        let revision = State::decode(&s).unwrap().orders[0].revision;
        for actor in [A, B] {
            s = state(call(
                &s,
                actor,
                ids::VOTE,
                &vote_args(0, revision, h, true),
                t + 1,
            ));
        }
        let out = call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), t + 6);
        assert_eq!(out.outbound_intents[0].extra[0].amount, amount);
        let receipt = out.outbound_intents[0].body.clone();
        s = state(cb(&state(out), 2, &receipt, 0, 0, 0x70 + i as u8));
    }
    let d = State::decode(&s).unwrap();
    assert_eq!(
        (d.spent, d.reserved, d.orders[0].phase, d.attempts),
        (1000, 0, CLOSED, 3)
    );
    fail(call(&s, X, ids::EXECUTE, &0u64.to_le_bytes(), 99), PHASE);
}
#[test]
fn deterministic_mixed_operations_preserve_budget_or_are_effect_free() {
    let mut bytes = funded();
    let mut seed = 0x123456789abcdefu64;
    for t in 3..503 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let actor = [A, B, C, V, X][seed as usize % 5];
        let id = (seed >> 8) % 10;
        let op = (seed >> 16) % 8;
        let (method, args) = match op {
            0 => (ids::CREATE, proposal([1, 2, 0, 0]).proposal_bytes()),
            1 => (ids::SUBMIT, submit_args(id, 0, (seed % 254 + 1) as u8)),
            2 => (ids::VOTE, vote_args(id, 1, 0x61, true)),
            3 => (ids::EXECUTE, id.to_le_bytes().to_vec()),
            4 => (ids::EXPIRE, id.to_le_bytes().to_vec()),
            5 => (ids::PAUSE, vec![]),
            6 => (ids::RESUME, vec![]),
            _ => (ids::STATUS, vec![]),
        };
        let out = call(&bytes, actor, method, &args, t);
        if out.status_code == 0 {
            bytes = state(out);
        } else {
            assert!(
                out.state_update.is_none()
                    && out.outbound_intents.is_empty()
                    && out.callback_intents.is_empty()
            );
        }
        let s = State::decode(&bytes).unwrap();
        assert_eq!(s.available().unwrap() + s.spent + s.reserved, s.deposited);
    }
}

#[test]
fn timestamp_and_revision_overflow_are_rejected_without_effects() {
    let mut s = State::decode(&submitted()).unwrap();
    s.orders[0].revision = u32::MAX;
    fail(
        call(&s.encode(), V, ids::SUBMIT, &submit_args(0, 0, 0x62), 5),
        LIMIT,
    );
    let mut s = State::decode(&submitted()).unwrap();
    s.config.delay = u32::MAX;
    let bytes = state(call(
        &s.encode(),
        A,
        ids::VOTE,
        &vote_args(0, 1, 0x61, true),
        5,
    ));
    fail(
        call(&bytes, B, ids::VOTE, &vote_args(0, 1, 0x61, true), 6),
        TIME,
    );
}
#[test]
fn topup_accepts_only_positive_shell_and_is_allowed_during_pending() {
    let s = state(dispatched());
    for (extra, expected) in [
        (vec![(SHELL, 100)], 0),
        (vec![(SHELL, 0)], VALUE),
        (vec![(7, 100)], VALUE),
    ] {
        let msg = external(&[], X, ids::TOPUP, 0, address(1), false, &extra);
        let bytes = frame(&msg, &s, X, None, address(1), 20, 2000);
        let out = Council::process_message(decode_message_input(&bytes).unwrap());
        if expected == 0 {
            assert_eq!(
                State::decode(&state(out)).unwrap(),
                State::decode(&s).unwrap()
            );
        } else {
            fail(out, expected);
        }
    }
}
#[test]
fn outbound_fee_reserve_cannot_spend_principal_or_exceed_shell_balance() {
    let mut s = State::decode(&approved()).unwrap();
    s.config.fuel = 1000001;
    fail(
        call(&s.encode(), X, ids::EXECUTE, &0u64.to_le_bytes(), 11),
        BALANCE,
    );
}
#[test]
fn five_member_council_can_require_all_five_distinct_votes() {
    let mut s = State::decode(&submitted()).unwrap();
    s.config.members = vec![A, B, C, [0x45; 32], [0x46; 32]];
    s.config.threshold = 5;
    let mut bytes = s.encode();
    for (i, key) in s.config.members.iter().enumerate() {
        bytes = state(call(
            &bytes,
            *key,
            ids::VOTE,
            &vote_args(0, 1, 0x61, true),
            5,
        ));
        assert_eq!(
            State::decode(&bytes).unwrap().orders[0].ready_at,
            if i == 4 { 10 } else { 0 }
        );
    }
    assert_eq!(
        call(&bytes, X, ids::EXECUTE, &0u64.to_le_bytes(), 10).status_code,
        0
    );
}
#[test]
fn cancellation_and_expiry_remain_available_during_pause() {
    let paused = state(call(&created(), A, ids::PAUSE, &[], 4));
    assert_eq!(
        call(&paused, X, ids::EXPIRE, &0u64.to_le_bytes(), 1000).status_code,
        0
    );
    let mut body = 0u64.to_le_bytes().to_vec();
    body.push(1);
    let s = state(call(&paused, A, ids::CANCEL, &body, 5));
    let s = state(call(&s, B, ids::CANCEL, &body, 6));
    assert_eq!(State::decode(&s).unwrap().reserved, 0);
}
#[test]
fn failed_attempt_receipt_capacity_stops_before_new_outbound() {
    let mut s = State::decode(&approved()).unwrap();
    s.attempts = 64;
    s.receipts = (1..=64).map(|x| [x; 32]).collect();
    fail(
        call(&s.encode(), X, ids::EXECUTE, &0u64.to_le_bytes(), 11),
        LIMIT,
    );
}
#[test]
fn corrupted_full_state_never_panics_on_single_byte_mutations() {
    let s = approved();
    for i in 0..STATE_BYTES {
        let mut b = s.clone();
        b[i] ^= 0xff;
        if let Ok(decoded) = State::decode(&b) {
            assert_eq!(decoded.encode(), b);
            decoded.validate().unwrap();
        }
    }
}
