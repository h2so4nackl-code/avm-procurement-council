#![forbid(unsafe_code)]
//! Test destination: receives real ECC and returns an exact payment acknowledgement.
use avm_contract_sdk::{
    Caller, CanonicalMessage, Contract, MessageInput, MessageOutput, StateUpdate,
};
use avm_procurement_council::{currency, ids, valid_address, Reader, PRINCIPAL};
pub const STATE_BYTES: usize = 160;
pub const AUTH: u32 = 3100;
pub const ARGS: u32 = 3101;
pub const REJECT: u32 = 3102;
pub struct Receiver;
impl Contract for Receiver {
    fn init(input: MessageInput<'_>) -> MessageOutput {
        let Caller::External(owner) = input.caller else {
            return MessageOutput::failure(AUTH);
        };
        if !input.state.is_empty()
            || !input.deploy().is_ok_and(|d| d.init_data().is_empty())
            || !input.message.extra().is_empty()
        {
            return MessageOutput::failure(ARGS);
        }
        let mut b = vec![0; STATE_BYTES];
        b[..32].copy_from_slice(&owner);
        MessageOutput::success(StateUpdate::FullReplace(b))
    }
    fn process_message(input: MessageInput<'_>) -> MessageOutput {
        process(input).unwrap_or_else(MessageOutput::failure)
    }
}
fn process(input: MessageInput<'_>) -> Result<MessageOutput, u32> {
    let b = input.state;
    if b.len() != STATE_BYTES
        || b[..32] == [0; 32]
        || b[128] > 1
        || b[129..].iter().any(|x| *x != 0)
    {
        return Err(ARGS);
    }
    let mut s = b.to_vec();
    let m = input.message.method_id();
    if m == ids::BIND || m == ids::REJECT {
        let owner: [u8; 32] = b[..32].try_into().map_err(|_| ARGS)?;
        if input.caller != Caller::External(owner) {
            return Err(AUTH);
        }
        if !input.message.extra().is_empty() {
            return Err(ARGS);
        }
        let args = input.message.body();
        if m == ids::BIND {
            if args.len() != 64 || b[32..96] != [0; 64] {
                return Err(ARGS);
            }
            let a = Reader::new(args).address()?;
            if !valid_address(a) || a == input.destination || a.dapp_id != input.destination.dapp_id
            {
                return Err(ARGS);
            }
            s[32..96].copy_from_slice(args);
        } else {
            if args.len() != 1 || args[0] > 1 {
                return Err(ARGS);
            }
            s[128] = args[0];
        }
        return Ok(MessageOutput::success(StateUpdate::FullReplace(s)));
    }
    let (Caller::Internal(source), CanonicalMessage::Internal(msg)) = (input.caller, input.message)
    else {
        return Err(AUTH);
    };
    let bound = Reader::new(&b[32..96]).address()?;
    if source != bound
        || !valid_address(bound)
        || m != ids::RECEIVE
        || msg.is_callback()
        || !msg.has_callback_request()
    {
        return Err(AUTH);
    }
    if b[128] != 0 {
        return Err(REJECT);
    }
    let body = msg.body;
    if body.len() != 80
        || &body[..4] != b"APCP"
        || body[73..] != [0; 7]
        || body[20] >= 4
        || body[41..73] == [0; 32]
    {
        return Err(ARGS);
    }
    let mut r = Reader::new(&body[4..]);
    let attempt = r.u64()?;
    let order = r.u64()?;
    let _stage = r.u8()?;
    let revision = r.u32()?;
    let amount = r.u128()?;
    let mut old = Reader::new(&b[96..128]);
    let last = old.u64()?;
    let received = old.u128()?;
    let count = old.u64()?;
    if attempt <= last
        || order >= 8
        || revision == 0
        || amount == 0
        || msg.extra.len() != 1
        || currency(msg.extra, PRINCIPAL) != amount
    {
        return Err(ARGS);
    }
    s[96..104].copy_from_slice(&attempt.to_le_bytes());
    s[104..120].copy_from_slice(&received.checked_add(amount).ok_or(ARGS)?.to_le_bytes());
    s[120..128].copy_from_slice(&count.checked_add(1).ok_or(ARGS)?.to_le_bytes());
    let mut out = MessageOutput::success(StateUpdate::FullReplace(s));
    out.callback_result_body = body.to_vec();
    Ok(out)
}
#[cfg(target_arch = "wasm32")]
avm_contract_sdk::export_message_contract!(Receiver, arena_bytes = 131_072);
