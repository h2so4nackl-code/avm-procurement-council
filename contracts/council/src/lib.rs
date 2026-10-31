#![forbid(unsafe_code)]
//! Bounded, multi-party procurement treasury for AVM SDK 0.3.0.
use avm_contract_sdk::{
    AccountAddress, CallbackIntent, Caller, CanonicalMessage, Contract, CurrencyAmount,
    CurrencyAmounts, EventField, EventIntent, EventKind, EventValue, MessageInput, MessageOutput,
    OutboundIntent, StateUpdate,
};
pub mod ids;
pub const MAX_MEMBERS: usize = 5;
pub const MAX_ORDERS: usize = 8;
pub const MAX_STAGES: usize = 4;
pub const MAX_ATTEMPTS: usize = 64;
pub const STATE_BYTES: usize = 6144;
pub const CONFIG_BYTES: usize = 184;
pub const ORDER_ARGS_BYTES: usize = 197;
pub const PRINCIPAL: u32 = 0; // Synthetic ECC used by the local stand; not a mainnet asset claim.
pub const SHELL: u32 = 2;
pub const ACTIVE: u8 = 0;
pub const CLOSED: u8 = 1;
pub const CANCELLED: u8 = 2;
pub const EXPIRED: u8 = 3;
pub const ARGS: u32 = 3000;
pub const AUTH: u32 = 3001;
pub const STATE: u32 = 3002;
pub const PHASE: u32 = 3003;
pub const VALUE: u32 = 3004;
pub const TIME: u32 = 3005;
pub const PENDING: u32 = 3006;
pub const CALLBACK: u32 = 3007;
pub const LIMIT: u32 = 3008;
pub const BALANCE: u32 = 3009;
pub const PAUSED: u32 = 3010;
pub const STALE: u32 = 3011;
pub const DUPLICATE: u32 = 3012;
pub type Result<T> = std::result::Result<T, u32>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub members: Vec<[u8; 32]>,
    pub threshold: u8,
    pub delay: u32,
    pub fuel: u128,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if !(2..=MAX_MEMBERS).contains(&self.members.len())
            || self.threshold < 2
            || usize::from(self.threshold) > self.members.len()
            || self.delay == 0
            || self.fuel == 0
            || self
                .members
                .iter()
                .enumerate()
                .any(|(i, k)| *k == [0; 32] || self.members[..i].contains(k))
        {
            return Err(ARGS);
        }
        Ok(())
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Vec::new();
        w.push(self.members.len() as u8);
        w.push(self.threshold);
        w.extend_from_slice(&[0; 2]);
        w.extend_from_slice(&self.delay.to_le_bytes());
        w.extend_from_slice(&self.fuel.to_le_bytes());
        for i in 0..MAX_MEMBERS {
            w.extend_from_slice(self.members.get(i).unwrap_or(&[0; 32]));
        }
        w
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != CONFIG_BYTES || b[2..4] != [0; 2] {
            return Err(ARGS);
        }
        let n = b[0] as usize;
        if !(2..=MAX_MEMBERS).contains(&n) {
            return Err(ARGS);
        }
        let mut r = Reader::new(&b[4..]);
        let delay = r.u32()?;
        let fuel = r.u128()?;
        let mut members = Vec::new();
        for i in 0..MAX_MEMBERS {
            let k = r.array()?;
            if i < n {
                members.push(k);
            } else if k != [0; 32] {
                return Err(ARGS);
            }
        }
        let c = Self {
            members,
            threshold: b[1],
            delay,
            fuel,
        };
        c.validate()?;
        Ok(c)
    }
    fn bit(&self, k: [u8; 32]) -> Result<u8> {
        self.members
            .iter()
            .position(|x| *x == k)
            .map(|i| 1u8 << i)
            .ok_or(AUTH)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Order {
    pub vendor: [u8; 32],
    pub receiver: AccountAddress,
    pub specification: [u8; 32],
    pub amounts: [u128; MAX_STAGES],
    pub deadline: u32,
    pub stages: u8,
    pub stage: u8,
    pub phase: u8,
    pub votes: u8,
    pub cancel_votes: u8,
    pub revision: u32,
    pub ready_at: u32,
    pub delivery: [u8; 32],
    pub paid: u128,
}
impl Order {
    pub fn proposal(b: &[u8]) -> Result<Self> {
        if b.len() != ORDER_ARGS_BYTES {
            return Err(ARGS);
        }
        let mut r = Reader::new(b);
        let vendor = r.array()?;
        let receiver = r.address()?;
        let specification = r.array()?;
        let mut amounts = [0; MAX_STAGES];
        for a in &mut amounts {
            *a = r.u128()?;
        }
        let deadline = r.u32()?;
        let stages = r.u8()?;
        let o = Self {
            vendor,
            receiver,
            specification,
            amounts,
            deadline,
            stages,
            stage: 0,
            phase: ACTIVE,
            votes: 0,
            cancel_votes: 0,
            revision: 0,
            ready_at: 0,
            delivery: [0; 32],
            paid: 0,
        };
        o.basic()?;
        Ok(o)
    }
    pub fn proposal_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(&self.vendor);
        put_address(&mut b, self.receiver);
        b.extend_from_slice(&self.specification);
        for a in self.amounts {
            b.extend_from_slice(&a.to_le_bytes());
        }
        b.extend_from_slice(&self.deadline.to_le_bytes());
        b.push(self.stages);
        b
    }
    pub fn total(&self) -> Result<u128> {
        self.amounts
            .iter()
            .try_fold(0u128, |a, x| a.checked_add(*x).ok_or(STATE))
    }
    fn basic(&self) -> Result<()> {
        let n = self.stages as usize;
        if n == 0
            || n > MAX_STAGES
            || self.vendor == [0; 32]
            || !valid_address(self.receiver)
            || self.specification == [0; 32]
            || self.deadline == 0
            || self.amounts[..n].contains(&0)
            || self.amounts[n..].iter().any(|x| *x != 0)
        {
            return Err(ARGS);
        }
        self.total()?;
        Ok(())
    }
    pub fn remaining(&self) -> Result<u128> {
        self.total()?.checked_sub(self.paid).ok_or(STATE)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub order: u8,
    pub stage: u8,
    pub attempt: u64,
    pub amount: u128,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub config: Config,
    pub deposited: u128,
    pub spent: u128,
    pub reserved: u128,
    pub attempts: u64,
    pub paused: bool,
    pub resume_votes: u8,
    pub orders: Vec<Order>,
    pub pending: Option<Pending>,
    pub receipts: Vec<[u8; 32]>,
}
impl State {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            deposited: 0,
            spent: 0,
            reserved: 0,
            attempts: 0,
            paused: false,
            resume_votes: 0,
            orders: Vec::new(),
            pending: None,
            receipts: Vec::new(),
        }
    }
    pub fn available(&self) -> Result<u128> {
        self.deposited
            .checked_sub(self.spent)
            .and_then(|x| x.checked_sub(self.reserved))
            .ok_or(STATE)
    }
    pub fn validate(&self) -> Result<()> {
        self.config.validate().map_err(|_| STATE)?;
        self.available()?;
        let mask = (1u8 << self.config.members.len()) - 1;
        if self.orders.len() > MAX_ORDERS
            || self.attempts > MAX_ATTEMPTS as u64
            || self.receipts.len() + usize::from(self.pending.is_some()) != self.attempts as usize
            || self.resume_votes & !mask != 0
            || (!self.paused && self.resume_votes != 0)
            || self.resume_votes.count_ones() >= u32::from(self.config.threshold)
            || self
                .receipts
                .iter()
                .enumerate()
                .any(|(i, h)| *h == [0; 32] || self.receipts[..i].contains(h))
        {
            return Err(STATE);
        }
        let mut spent = 0u128;
        let mut reserved = 0u128;
        for o in &self.orders {
            o.basic().map_err(|_| STATE)?;
            let n = o.stages as usize;
            if self.config.members.contains(&o.vendor)
                || o.phase > EXPIRED
                || o.stage > o.stages
                || o.votes & !mask != 0
                || o.cancel_votes & !mask != 0
                || (o.phase == CLOSED) != (o.stage == o.stages)
                || o.paid != o.amounts[..o.stage as usize].iter().sum::<u128>()
                || (o.delivery == [0; 32] && (o.votes != 0 || o.ready_at != 0))
                || (o.delivery != [0; 32] && o.revision == 0)
                || (o.votes.count_ones() >= u32::from(self.config.threshold)) != (o.ready_at != 0)
                || (o.phase != ACTIVE
                    && (o.votes != 0
                        || o.cancel_votes != 0
                        || o.ready_at != 0
                        || o.delivery != [0; 32]))
                || (o.phase == ACTIVE
                    && (o.stage as usize >= n
                        || o.cancel_votes.count_ones() >= u32::from(self.config.threshold)))
            {
                return Err(STATE);
            }
            spent = spent.checked_add(o.paid).ok_or(STATE)?;
            if o.phase == ACTIVE {
                reserved = reserved.checked_add(o.remaining()?).ok_or(STATE)?;
            }
        }
        if spent != self.spent || reserved != self.reserved {
            return Err(STATE);
        }
        if let Some(p) = self.pending {
            let o = self.orders.get(p.order as usize).ok_or(STATE)?;
            if o.phase != ACTIVE
                || p.stage != o.stage
                || p.attempt != self.attempts
                || p.amount != o.amounts[o.stage as usize]
                || o.ready_at == 0
            {
                return Err(STATE);
            }
        }
        Ok(())
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::from(*b"APC1");
        b.extend_from_slice(&self.config.encode());
        for x in [self.deposited, self.spent, self.reserved] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b.extend_from_slice(&self.attempts.to_le_bytes());
        b.push(self.paused as u8);
        b.push(self.resume_votes);
        b.push(self.orders.len() as u8);
        b.push(self.receipts.len() as u8);
        match self.pending {
            Some(p) => {
                b.push(1);
                b.push(p.order);
                b.push(p.stage);
                b.extend_from_slice(&p.attempt.to_le_bytes());
                b.extend_from_slice(&p.amount.to_le_bytes());
            }
            None => b.extend_from_slice(&[0; 27]),
        }
        for o in &self.orders {
            b.extend_from_slice(&o.proposal_bytes());
            b.extend_from_slice(&[o.stage, o.phase, o.votes, o.cancel_votes]);
            b.extend_from_slice(&o.revision.to_le_bytes());
            b.extend_from_slice(&o.ready_at.to_le_bytes());
            b.extend_from_slice(&o.delivery);
            b.extend_from_slice(&o.paid.to_le_bytes());
        }
        for h in &self.receipts {
            b.extend_from_slice(h);
        }
        assert!(b.len() <= STATE_BYTES);
        b.resize(STATE_BYTES, 0);
        b
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        if b.len() != STATE_BYTES || &b[..4] != b"APC1" {
            return Err(STATE);
        }
        let mut r = Reader::new(&b[4..]);
        let config = Config::decode(r.take(CONFIG_BYTES)?).map_err(|_| STATE)?;
        let deposited = r.u128()?;
        let spent = r.u128()?;
        let reserved = r.u128()?;
        let attempts = r.u64()?;
        let pause = r.u8()?;
        if pause > 1 {
            return Err(STATE);
        }
        let resume_votes = r.u8()?;
        let n = r.u8()? as usize;
        let count = r.u8()? as usize;
        if n > MAX_ORDERS || count > MAX_ATTEMPTS {
            return Err(STATE);
        }
        let flag = r.u8()?;
        let order = r.u8()?;
        let stage = r.u8()?;
        let attempt = r.u64()?;
        let amount = r.u128()?;
        let pending = match flag {
            0 if order == 0 && stage == 0 && attempt == 0 && amount == 0 => None,
            1 => Some(Pending {
                order,
                stage,
                attempt,
                amount,
            }),
            _ => return Err(STATE),
        };
        let mut orders = Vec::new();
        for _ in 0..n {
            let mut o = Order::proposal(r.take(ORDER_ARGS_BYTES)?).map_err(|_| STATE)?;
            o.stage = r.u8()?;
            o.phase = r.u8()?;
            o.votes = r.u8()?;
            o.cancel_votes = r.u8()?;
            o.revision = r.u32()?;
            o.ready_at = r.u32()?;
            o.delivery = r.array()?;
            o.paid = r.u128()?;
            orders.push(o);
        }
        let mut receipts = Vec::new();
        for _ in 0..count {
            receipts.push(r.array()?);
        }
        if r.take(r.remaining())?.iter().any(|x| *x != 0) {
            return Err(STATE);
        }
        let s = Self {
            config,
            deposited,
            spent,
            reserved,
            attempts,
            paused: pause != 0,
            resume_votes,
            orders,
            pending,
            receipts,
        };
        s.validate()?;
        Ok(s)
    }
}
pub struct Council;
impl Contract for Council {
    fn init(input: MessageInput<'_>) -> MessageOutput {
        initialize(input).unwrap_or_else(MessageOutput::failure)
    }
    fn process_message(input: MessageInput<'_>) -> MessageOutput {
        process(input).unwrap_or_else(MessageOutput::failure)
    }
}
fn initialize(input: MessageInput<'_>) -> Result<MessageOutput> {
    let c = Config::decode(input.deploy().map_err(|_| ARGS)?.init_data())?;
    let Caller::External(actor) = input.caller else {
        return Err(AUTH);
    };
    c.bit(actor)?;
    zero_value(&input)?;
    if !input.state.is_empty() {
        return Err(STATE);
    }
    finish(State::new(c), 0, 255)
}
fn process(input: MessageInput<'_>) -> Result<MessageOutput> {
    let mut s = State::decode(input.state)?;
    if input.message.is_callback() {
        return settle(input, s);
    }
    let Caller::External(actor) = input.caller else {
        return Err(AUTH);
    };
    let m = input.message.method_id();
    let body = input.message.body();
    let now = input.context.block_time;
    if m == ids::STATUS {
        empty(body)?;
        zero_value(&input)?;
        return finish(s, 1, 255);
    }
    if m == ids::TOPUP {
        empty(body)?;
        if input.message.extra().is_empty()
            || input
                .message
                .extra()
                .iter()
                .any(|c| c.currency_id != SHELL || c.amount == 0)
        {
            return Err(VALUE);
        }
        return finish(s, 2, 255);
    }
    zero_value_except_deposit(&input, m)?;
    // Emergency pause is admitted even while an outbound transfer is pending.
    if m == ids::PAUSE {
        empty(body)?;
        s.config.bit(actor)?;
        s.paused = true;
        s.resume_votes = 0;
        return finish(s, 3, 255);
    }
    if s.pending.is_some() {
        return Err(PENDING);
    }
    if m == ids::RESUME {
        empty(body)?;
        let bit = s.config.bit(actor)?;
        if !s.paused {
            return Err(PHASE);
        }
        if s.resume_votes & bit != 0 {
            return Err(DUPLICATE);
        }
        s.resume_votes |= bit;
        if s.resume_votes.count_ones() >= u32::from(s.config.threshold) {
            s.paused = false;
            s.resume_votes = 0;
        }
        return finish(s, 4, 255);
    }
    if m == ids::DEPOSIT {
        empty(body)?;
        s.config.bit(actor)?;
        let amount = currency(input.message.extra(), PRINCIPAL);
        if amount == 0 || input.message.extra().len() != 1 {
            return Err(VALUE);
        }
        s.deposited = s.deposited.checked_add(amount).ok_or(LIMIT)?;
        if currency(input.extra, PRINCIPAL) < s.deposited - s.spent {
            return Err(BALANCE);
        }
        return finish(s, 5, 255);
    }
    if m == ids::CREATE {
        s.config.bit(actor)?;
        if s.paused {
            return Err(PAUSED);
        }
        if s.orders.len() == MAX_ORDERS {
            return Err(LIMIT);
        }
        let o = Order::proposal(body)?;
        if s.config.members.contains(&o.vendor) {
            return Err(AUTH);
        }
        if o.receiver == input.destination || o.receiver.dapp_id != input.destination.dapp_id {
            return Err(ARGS);
        }
        if o.deadline <= now.checked_add(s.config.delay).ok_or(TIME)? {
            return Err(TIME);
        }
        let total = o.total()?;
        if total > s.available()? {
            return Err(BALANCE);
        }
        s.reserved = s.reserved.checked_add(total).ok_or(LIMIT)?;
        let i = s.orders.len() as u8;
        s.orders.push(o);
        return finish(s, 6, i);
    }
    let mut r = Reader::new(body);
    let id = r.u64()?;
    let i = usize::try_from(id).map_err(|_| ARGS)?;
    let o = s.orders.get_mut(i).ok_or(ARGS)?;
    if o.phase != ACTIVE {
        return Err(PHASE);
    }
    if m == ids::EXPIRE {
        empty(r.take(r.remaining())?)?;
        if now < o.deadline {
            return Err(TIME);
        }
        s.reserved -= o.remaining()?;
        close(o, EXPIRED);
        return finish(s, 7, i as u8);
    }
    if m == ids::CANCEL {
        let bit = s.config.bit(actor)?;
        let yes = r.u8()?;
        empty(r.take(r.remaining())?)?;
        if yes > 1 {
            return Err(ARGS);
        }
        set_vote(&mut o.cancel_votes, bit, yes != 0)?;
        if o.cancel_votes.count_ones() >= u32::from(s.config.threshold) {
            s.reserved -= o.remaining()?;
            close(o, CANCELLED);
        }
        return finish(s, 8, i as u8);
    }
    if now >= o.deadline {
        return Err(TIME);
    }
    if s.paused {
        return Err(PAUSED);
    }
    if m == ids::SUBMIT {
        if actor != o.vendor {
            return Err(AUTH);
        }
        let stage = r.u8()?;
        let digest = r.array()?;
        empty(r.take(r.remaining())?)?;
        if stage != o.stage || digest == [0; 32] {
            return Err(ARGS);
        }
        if digest == o.delivery {
            return Err(DUPLICATE);
        }
        o.revision = o.revision.checked_add(1).ok_or(LIMIT)?;
        o.delivery = digest;
        o.votes = 0;
        o.ready_at = 0;
        return finish(s, 9, i as u8);
    }
    if m == ids::VOTE {
        let bit = s.config.bit(actor)?;
        let revision = r.u32()?;
        let digest = r.array::<32>()?;
        let yes = r.u8()?;
        empty(r.take(r.remaining())?)?;
        if o.delivery == [0; 32] || revision != o.revision || digest != o.delivery {
            return Err(STALE);
        }
        if yes > 1 {
            return Err(ARGS);
        }
        set_vote(&mut o.votes, bit, yes != 0)?;
        if o.votes.count_ones() < u32::from(s.config.threshold) {
            o.ready_at = 0;
        } else if o.ready_at == 0 {
            o.ready_at = now.checked_add(s.config.delay).ok_or(TIME)?;
        }
        return finish(s, 10, i as u8);
    }
    if m == ids::EXECUTE {
        empty(r.take(r.remaining())?)?;
        if o.ready_at == 0 || now < o.ready_at {
            return Err(TIME);
        }
        if s.attempts == MAX_ATTEMPTS as u64 {
            return Err(LIMIT);
        }
        if currency(input.extra, PRINCIPAL) < s.deposited - s.spent
            || currency(input.extra, SHELL) < s.config.fuel
        {
            return Err(BALANCE);
        }
        let destination = o.receiver;
        let stage = o.stage;
        let amount = o.amounts[stage as usize];
        s.attempts += 1;
        s.pending = Some(Pending {
            order: i as u8,
            stage,
            attempt: s.attempts,
            amount,
        });
        let receipt = payment_body(&s)?;
        let fuel = s.config.fuel;
        let mut out = finish(s, 11, i as u8)?;
        out.outbound_intents.push(OutboundIntent {
            input_index: 0,
            destination,
            fuel_fee_shells: fuel,
            extra: vec![CurrencyAmount {
                currency_id: PRINCIPAL,
                amount,
            }],
            method_id: ids::RECEIVE,
            body: receipt,
        });
        out.callback_intents.push(CallbackIntent {
            outbound_index: 0,
            handler_method_id: ids::CALLBACK,
        });
        return Ok(out);
    }
    Err(ARGS)
}
fn settle(input: MessageInput<'_>, mut s: State) -> Result<MessageOutput> {
    let (Caller::Internal(source), CanonicalMessage::Internal(msg)) = (input.caller, input.message)
    else {
        return Err(CALLBACK);
    };
    let p = s.pending.ok_or(CALLBACK)?;
    let o = &s.orders[p.order as usize];
    let cb = msg.callback.ok_or(CALLBACK)?;
    if source != o.receiver
        || msg.method_id != ids::CALLBACK
        || cb.cause_address != input.destination
        || cb.cause_msg_hash == [0; 32]
        || s.receipts.contains(&cb.cause_msg_hash)
        || msg.has_callback_request()
        || msg
            .extra
            .iter()
            .any(|c| c.currency_id != SHELL && c.currency_id != PRINCIPAL)
    {
        return Err(CALLBACK);
    }
    let success = cb.status_code == 0;
    if success {
        if msg.body != payment_body(&s)? || msg.extra.iter().any(|c| c.currency_id != SHELL) {
            return Err(CALLBACK);
        }
        s.spent = s.spent.checked_add(p.amount).ok_or(STATE)?;
        s.reserved = s.reserved.checked_sub(p.amount).ok_or(STATE)?;
        let o = &mut s.orders[p.order as usize];
        o.paid = o.paid.checked_add(p.amount).ok_or(STATE)?;
        o.stage += 1;
        o.votes = 0;
        o.cancel_votes = 0;
        o.ready_at = 0;
        o.delivery = [0; 32];
        if o.stage == o.stages {
            o.phase = CLOSED;
        }
    } else if !msg.body.is_empty() || currency(msg.extra, PRINCIPAL) != p.amount {
        return Err(CALLBACK);
    }
    s.receipts.push(cb.cause_msg_hash);
    s.pending = None;
    finish(s, if success { 12 } else { 13 }, p.order)
}
pub fn payment_body(s: &State) -> Result<Vec<u8>> {
    let p = s.pending.ok_or(PENDING)?;
    let o = s.orders.get(p.order as usize).ok_or(STATE)?;
    let mut b = Vec::from(*b"APCP");
    b.extend_from_slice(&p.attempt.to_le_bytes());
    b.extend_from_slice(&(p.order as u64).to_le_bytes());
    b.push(p.stage);
    b.extend_from_slice(&o.revision.to_le_bytes());
    b.extend_from_slice(&p.amount.to_le_bytes());
    b.extend_from_slice(&o.delivery);
    b.resize(80, 0);
    Ok(b)
}
fn close(o: &mut Order, phase: u8) {
    o.phase = phase;
    o.votes = 0;
    o.cancel_votes = 0;
    o.ready_at = 0;
    o.delivery = [0; 32];
}
fn set_vote(mask: &mut u8, bit: u8, yes: bool) -> Result<()> {
    if (*mask & bit != 0) == yes {
        return Err(DUPLICATE);
    }
    if yes {
        *mask |= bit;
    } else {
        *mask &= !bit;
    }
    Ok(())
}
fn finish(s: State, kind: u64, order: u8) -> Result<MessageOutput> {
    s.validate()?;
    let fields = avm_contract_sdk::encode_event_fields(&[
        EventField {
            name: "available".into(),
            value: EventValue::U128(s.available()?),
        },
        EventField {
            name: "kind".into(),
            value: EventValue::U64(kind),
        },
        EventField {
            name: "order".into(),
            value: EventValue::U64(order as u64),
        },
        EventField {
            name: "reserved".into(),
            value: EventValue::U128(s.reserved),
        },
        EventField {
            name: "spent".into(),
            value: EventValue::U128(s.spent),
        },
    ])
    .map_err(|_| STATE)?;
    let mut out = MessageOutput::success(StateUpdate::FullReplace(s.encode()));
    out.event_intents.push(EventIntent {
        input_index: 0,
        kind: EventKind::External,
        event_class: ids::EVENT,
        event_name: "procurement.changed".into(),
        topics: vec![],
        fields_binary: fields,
    });
    Ok(out)
}
fn zero_value(input: &MessageInput<'_>) -> Result<()> {
    if input.message.extra().is_empty() {
        Ok(())
    } else {
        Err(VALUE)
    }
}
fn zero_value_except_deposit(input: &MessageInput<'_>, m: u64) -> Result<()> {
    if m == ids::DEPOSIT {
        Ok(())
    } else {
        zero_value(input)
    }
}
fn empty(b: &[u8]) -> Result<()> {
    if b.is_empty() {
        Ok(())
    } else {
        Err(ARGS)
    }
}
pub fn currency(c: CurrencyAmounts<'_>, id: u32) -> u128 {
    c.iter()
        .find(|c| c.currency_id == id)
        .map_or(0, |c| c.amount)
}
pub fn valid_address(a: AccountAddress) -> bool {
    a.dapp_id != [0; 32] && a.account_id != [0; 32]
}
pub fn put_address(b: &mut Vec<u8>, a: AccountAddress) {
    b.extend_from_slice(&a.dapp_id);
    b.extend_from_slice(&a.account_id);
}
pub struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Reader<'a> {
    pub fn new(b: &'a [u8]) -> Self {
        Self { b, p: 0 }
    }
    pub fn remaining(&self) -> usize {
        self.b.len() - self.p
    }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.p.checked_add(n).ok_or(ARGS)?;
        let b = self.b.get(self.p..end).ok_or(ARGS)?;
        self.p = end;
        Ok(b)
    }
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| ARGS)
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    pub fn u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.array()?))
    }
    pub fn address(&mut self) -> Result<AccountAddress> {
        Ok(AccountAddress {
            dapp_id: self.array()?,
            account_id: self.array()?,
        })
    }
}
#[cfg(all(target_arch = "wasm32", feature = "guest"))]
avm_contract_sdk::export_message_contract!(Council, arena_bytes = 131_072);
