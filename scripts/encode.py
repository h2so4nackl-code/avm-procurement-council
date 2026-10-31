#!/usr/bin/env python3
"""Prepare public ABI payloads, never keys or signed messages. Output is --args-json compatible."""
import argparse,json
from pathlib import Path

def key(s,n=32):
    b=bytes.fromhex(s.removeprefix('0x').replace(':',''))
    if len(b)!=n or not any(b):raise ValueError(f'expected nonzero {n}-byte hex value')
    return b

def integer(s,bits):
    value=int(s)
    if not 0<=value<1<<bits:raise ValueError(f'expected u{bits}')
    return value.to_bytes(bits//8,'little')

def main():
    p=argparse.ArgumentParser(description=__doc__);sub=p.add_subparsers(dest='op',required=True)
    c=sub.add_parser('config');c.add_argument('--members',nargs='+',required=True);c.add_argument('--threshold',type=int,required=True);c.add_argument('--delay',type=int,required=True);c.add_argument('--fuel',type=int,default=10000)
    o=sub.add_parser('order');o.add_argument('--vendor',required=True);o.add_argument('--receiver',required=True);o.add_argument('--spec',required=True);o.add_argument('--amounts',nargs='+',required=True);o.add_argument('--deadline',required=True)
    s=sub.add_parser('submit');s.add_argument('--id',required=True);s.add_argument('--stage',type=int,required=True);s.add_argument('--digest',required=True)
    v=sub.add_parser('vote');v.add_argument('--id',required=True);v.add_argument('--revision',required=True);v.add_argument('--digest',required=True);v.add_argument('--revoke',action='store_true')
    c=sub.add_parser('cancel');c.add_argument('--id',required=True);c.add_argument('--revoke',action='store_true')
    i=sub.add_parser('id');i.add_argument('value')
    r=sub.add_parser('inspect');r.add_argument('receipt',type=Path)
    a=p.parse_args()
    if a.op=='config':
        members=[key(x) for x in a.members]
        if not 2<=len(members)<=5 or len(set(members))!=len(members) or not 2<=a.threshold<=len(members) or a.delay<=0 or a.fuel<=0:raise ValueError('invalid council configuration')
        b=bytes([len(members),a.threshold,0,0])+integer(a.delay,32)+integer(a.fuel,128)+b''.join(members)+bytes((5-len(members))*32)
    elif a.op=='order':
        amounts=[int(x) for x in a.amounts]
        if not 1<=len(amounts)<=4 or any(x<=0 for x in amounts) or sum(amounts)>=1<<128 or int(a.deadline)<=0:raise ValueError('invalid milestone budget/deadline')
        b=key(a.vendor)+key(a.receiver,64)+key(a.spec)+b''.join(integer(x,128) for x in amounts)+bytes((4-len(amounts))*16)+integer(a.deadline,32)+bytes([len(amounts)])
    elif a.op=='submit':
        if not 0<=a.stage<4:raise ValueError('stage outside 0..3')
        b=integer(a.id,64)+bytes([a.stage])+key(a.digest)
    elif a.op=='vote':b=integer(a.id,64)+integer(a.revision,32)+key(a.digest)+bytes([not a.revoke])
    elif a.op=='cancel':b=integer(a.id,64)+bytes([not a.revoke])
    elif a.op=='id':b=integer(a.value,64)
    else:
        b=bytes.fromhex(json.loads(a.receipt.read_text())['raw_state'])
        if len(b)!=6144 or b[:4]!=b'APC1':raise ValueError('not a procurement state receipt')
        result={name:int.from_bytes(b[start:end],'little') for name,start,end in [('deposited',188,204),('spent',204,220),('reserved',220,236),('attempts',236,244)]}
        result.update(available=result['deposited']-result['spent']-result['reserved'],paused=bool(b[244]),orders=b[246],receipts=b[247],pending=bool(b[248]))
        print(json.dumps(result,indent=2));return
    print(json.dumps('0x'+b.hex()))
if __name__=='__main__':main()
