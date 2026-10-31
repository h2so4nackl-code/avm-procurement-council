#!/usr/bin/env python3
"""One schema source for fixed-byte interfaces; IDs come from official ACAB."""
from pathlib import Path
import hashlib
ROOT=Path(__file__).resolve().parents[1]
METHODS={"DEPOSIT":("procurement.deposit",None),"CREATE":("procurement.create",197),"SUBMIT":("procurement.submit",41),"VOTE":("procurement.vote",45),"EXECUTE":("procurement.execute",8),"CANCEL":("procurement.cancel",9),"EXPIRE":("procurement.expire",8),"PAUSE":("procurement.pause",None),"RESUME":("procurement.resume",None),"TOPUP":("procurement.topup",None),"STATUS":("procurement.status",None),"CALLBACK":("procurement.callback",80),"BIND":("procurement.receiver.bind",64),"REJECT":("procurement.receiver.reject",1),"RECEIVE":("procurement.receiver.receive",80)}
def schema(receiver=False):
    methods=[("procurement.receiver.deploy",None)] if receiver else [("procurement.deploy",184)]
    methods += [(n,size) for n,size in METHODS.values() if (".receiver." in n)==receiver]
    lengths=sorted({size for _,size in methods if size is not None})
    types={size:i+2 for i,size in enumerate(lengths)}
    state=160 if receiver else 6144
    b=['[descriptor]','execution_mode = "message_isolated"',f'state_schema_id = "{hashlib.sha256(("procurement-v1-receiver" if receiver else "procurement-v1-council").encode()).hexdigest()}"','','[abi]','state_root_type_id = 1','','[[abi.types]]','id = 1','kind = "fixed_bytes"','name = "state"',f'len = {state}']
    for size,i in types.items():b += ['','[[abi.types]]',f'id = {i}','kind = "fixed_bytes"',f'name = "bytes_{size}"',f'len = {size}']
    for name,size in methods:
        b += ['','[[abi.methods]]',f'name = "{name}"']
        if size is not None:b += [f'input_type_id = {types[size]}']
    if not receiver:
        u128=max(types.values())+1;u64=u128+1
        b += ['','[[abi.types]]',f'id = {u128}','kind = "u128"','name = "amount"','','[[abi.types]]',f'id = {u64}','kind = "u64"','name = "number"','','[[abi.events]]','name = "procurement.changed"']
        for i,(name,t) in enumerate([('available',u128),('kind',u64),('order',u64),('reserved',u128),('spent',u128)],1):b += ['','[[abi.events.fields]]',f'id = {i}',f'type_id = {t}',f'name = "{name}"']
    errors=['invalid_args','unauthorized','invalid_state','invalid_phase','unexpected_value','timelock_or_deadline','transfer_pending','invalid_callback','capacity_or_overflow','insufficient_balance','paused','stale_delivery','duplicate_vote']
    for i,name in enumerate(['unauthorized','invalid_args','receiver_rejected'] if receiver else errors):
        b += ['','[[abi.errors]]',f'status_code = {(3100 if receiver else 3000)+i}',f'name = "{name}"',f'message = "{name.replace("_"," ")}"']
    return '\n'.join(b)+'\n'
if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');args=p.parse_args()
    for name in ('council','receiver'):
        path=ROOT/'contracts'/name/'avm-contract.toml';expected=schema(name=='receiver')
        if args.check:assert path.read_text()==expected, f'ABI schema drift: {name}'
        else:path.write_text(expected)
