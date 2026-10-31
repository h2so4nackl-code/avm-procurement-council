# Procurement Council v1 specification

## Authority and lifecycle

The deployer must be a configured council member. Deployment carries a 184-byte
configuration inside the SDK deploy envelope, starts with empty state, and carries
no ECC transfer. Configuration is immutable; keys must be distinct and nonzero,
threshold is at least 2 and at most member count, delay and outbound fee reserve
are positive. Member order determines vote-bit positions.

A member deposits positive synthetic principal. A member can reserve an order
whose complete budget fits the unallocated pool. Order IDs are zero-based indices
and never reused. Only the order's supplier signs submissions; its identity must
be separate from all members. Recipient addresses must be nonzero, in the same
DApp as the treasury, and different from the treasury itself.

Submission addresses the current sequential milestone and carries a nonzero
32-byte delivery digest. A new digest increments a checked u32 revision, clears
all delivery votes and removes the timelock. Repeating an identical digest is an
error. Milestones cannot be skipped, reduced or reordered. Digests commit to
external artifacts; the contract does not verify the goods or their quality.

A council vote signs `(order_id, revision, delivery_digest, yes/no)` through the
runtime's external-message signature. Each member contributes one bit. Duplicate
approval and duplicate revocation fail. Quorum starts `ready_at = block_time +
delay` with checked arithmetic. Additional votes do not extend the delay.
Dropping below quorum clears it; crossing quorum again restarts the whole delay.

Anyone with a signed external message can execute at `now >= ready_at` and
`now < deadline`. The supplier does not control the receiver address. Execution
also checks all accounted unspent principal is present and the separate SHELL
outbound reserve is available. One outbound with an exact receipt and a callback
request is emitted atomically with pending state. All ordinary mutations are
locked until settlement except pause, fee topup and status.

A successful authenticated runtime callback from the exact configured recipient
must return the exact 80-byte receipt and no principal. Only then are `spent`,
`reserved`, `order.paid`, and the stage cursor changed. A failed callback must
have an empty body and return exactly the pending principal. Failure records its
causal hash and releases the lock but preserves the current stage and approvals,
allowing explicit retry with a new attempt sequence. Every accepted callback's
nonzero causal hash is retained and cannot settle another attempt. The SDK/runtime
reserves callback headers; ordinary guest outbounds cannot forge one.

No action guesses the status of an unresolved transfer. If the callback is lost,
there is no retry/unlock API. Public status and fee topup still work, and an
emergency pause can be set; a pending callback may settle during pause or after
the order deadline. Settlement of an already emitted payment is not a new payment
request. Native callback tests assume runtime-authenticated headers; the stand
journeys test actual runtime construction and routing.

## Cancellation, expiry and pause

Each order has a separate whole-order cancellation vote mask. A quorum cancels
the unpaid remainder and clears its stage votes, digest and delay. Cancellation
votes refer to the immutable order terms and survive a delivery revision; they
are cleared after a successful stage payment. Revocation of an existing
cancellation vote is supported before quorum. A single member cannot cancel.

Any external signer may expire an active order at `now >= deadline`. Expiry and
cancellation release only the unpaid reservation; they transfer no funds and
never claw back settled stages. Both work while paused, but neither works during
an in-flight transfer. Closed/cancelled/expired orders cannot be reopened.

Any member can pause, including during pending state. Every pause resets the
resume mask. Distinct council members must vote to resume until quorum; duplicate
resume votes fail. Resume is blocked by pending state. Pausing inhibits create,
submit, stage voting and execute, but permits deposits, fee topups, status,
cancellation, expiry and callbacks as specified above. A malicious member can
repeatedly pause and impair liveness; that is an explicit emergency-governance
tradeoff, not Byzantine liveness protection.

## Accounting invariants

All amounts are u128 with checked arithmetic. For the current state:

- `available = deposited - spent - reserved >= 0`.
- `spent = sum(order.paid)` over every order.
- `reserved = sum(order.total - order.paid)` over active orders only.
- `order.paid = sum(amounts of stages strictly before order.stage)`.
- A pending transfer still belongs to reserved; it is never treated as settled.
- `attempts = settled causal hash count + (pending ? 1 : 0)`.
- Closed orders have all stages settled; cancelled/expired ones have no active votes.

Fees and rent are not principal and do not participate in these equalities.
Incoming unsolicited transfers are not credited to the accounting pool. Accepted
deposits require exactly one positive principal currency entry; every other
ordinary method rejects ECC value except fee topup, which accepts only positive
SHELL entries. Status is not a deposit mechanism. Native failures return no
state update, outbounds, callback intents or events. Protocol-level fees and
failure settlement are the runtime's responsibility, not a guest refund promise.

## Binary interfaces

All integers are little-endian. Keys/digests are raw bytes. Addresses are
`dapp_id[32] || account_id[32]`. Bodies must have exactly the declared length.
Empty-body methods use no argument type. `scripts/encode.py` emits a JSON hex
string suitable for `avm-local-stand --args-json` or `--init-json`.

| Method | Body |
|---|---|
| `procurement.deploy` | configuration[184] |
| `procurement.deposit` | empty; principal in message extra currencies |
| `procurement.create` | order proposal[197] |
| `procurement.submit` | id:u64, stage:u8, digest[32] = 41 bytes |
| `procurement.vote` | id:u64, revision:u32, digest[32], yes:u8 = 45 bytes |
| `procurement.execute` | id:u64 |
| `procurement.cancel` | id:u64, yes:u8 |
| `procurement.expire` | id:u64 |
| `procurement.pause`, `.resume`, `.status`, `.topup` | empty |
| `procurement.callback` | runtime callback; success receipt[80], failure empty |

Configuration: count:u8, threshold:u8, zero[2], delay:u32, fuel:u128,
5 key slots[32]. Unused key slots must be zero. Length = 4 + 4 + 16 + 160 = 184.

Proposal: vendor[32], recipient[64], specification[32], amounts[4 × u128],
deadline:u32, stage_count:u8. Length = 32 + 64 + 32 + 64 + 4 + 1 = 197.
Amounts in active stages must be positive; unused amounts must be zero.

Treasury data is always 6,144 bytes, well below the stand's FullReplace threshold
including its account overhead. It begins with `APC1`, then configuration[184],
deposited:u128, spent:u128, reserved:u128, attempts:u64, paused:u8,
resume_mask:u8, order_count:u8, receipt_count:u8. These occupy offsets 0–247.
The 27-byte pending record is at offset 248: active:u8, order:u8, stage:u8,
attempt:u64, amount:u128. An inactive record must be entirely zero.

At offset 275 come `order_count` records of 257 bytes each: the 197-byte proposal,
then stage:u8, phase:u8, approval_mask:u8, cancellation_mask:u8, revision:u32,
ready_at:u32, delivery[32], paid:u128. Phase is 0 active, 1 closed, 2 cancelled,
3 expired. Receipt hashes[32] follow. All remaining bytes must be zero.
Maximum used bytes = 275 + 8 × 257 + 64 × 32 = **4,379**; the rest is zero padding.

The 80-byte payment receipt is `APCP`, attempt:u64, order_id:u64, stage:u8,
revision:u32, amount:u128, delivery[32], zero[7]. Its sequence is global to this
treasury. Receiver state records last accepted sequence and cumulative principal.
The fixture owner binds its treasury address once after deployment, avoiding
circular deployment-address dependencies. Only this bound source may pay it.
The owner's rejection switch exists for tests and is not a production guarantee
about a recipient's behavior.

## Events and errors

`procurement.changed` has canonical fields in name order: available:u128,
kind:u64, order:u64, reserved:u128, spent:u128. Order 255 denotes a global action.
Kinds: 0 created, 1 status, 2 fee topup, 3 paused, 4 resume vote, 5 deposit,
6 order created, 7 expired, 8 cancellation vote, 9 delivery submitted,
10 delivery vote, 11 pending transfer, 12 successful settlement, 13 refunded failure.

Guest errors 3000–3012 correspond to invalid args, unauthorized caller, invalid
state, invalid phase, unexpected value, timelock/deadline, pending transfer,
invalid callback, capacity/overflow, insufficient balance, pause, stale delivery
and duplicate action. Fixture errors 3100–3102 are unauthorized, invalid args
and deliberate rejection. The official metadata enumerates these statuses.

## Explicit limits

8 lifetime orders, 4 stages/order, 5 council members, 64 attempted payments and
one unresolved outbound. Neither stale state migration, signer rotation,
withdrawal, principal remapping nor callback recovery is implemented. A capacity
limit is a terminal guard, not an automatically expandable quota. A future
production design would require reviewed rollover/withdrawal governance,
recovery tied to authenticated delivery evidence, monitoring and an independent
audit. This reference contract makes no such readiness claim.
