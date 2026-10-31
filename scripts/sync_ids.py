#!/usr/bin/env python3
"""Read generated canonical IDs; --bootstrap is ONLY for the first metadata build."""
from pathlib import Path
import argparse
from generate_abi import METHODS,ROOT
p=argparse.ArgumentParser();p.add_argument('--bootstrap',action='store_true');p.add_argument('--check',action='store_true');args=p.parse_args()
def identifier(contract,name,prefix):
    b=(ROOT/'contracts'/contract/'target/avm/contract.acab').read_bytes();n=name.encode()
    assert b[:4]==b'ACAB','unsupported ABI magic'
    positions=[i for i in range(2,len(b)) if b[i:i+len(n)]==n and int.from_bytes(b[i-2:i],'little')==len(n)]
    assert len(positions)==1,(name,positions)
    i=positions[0];v=int.from_bytes(b[i-prefix:i-prefix+8],'little');assert v;return v
lines=['// Generated from official SDK ACAB by scripts/sync_ids.py.']
for index,(key,(name,size)) in enumerate(METHODS.items(),1):
    value=index if args.bootstrap else identifier('receiver' if '.receiver.' in name else 'council',name,16)
    lines.append(f'pub const {key}: u64 = 0x{value:016x};')
value=100 if args.bootstrap else identifier('council','procurement.changed',14)
lines.append(f'pub const EVENT: u64 = 0x{value:016x};')
text='\n'.join(lines)+'\n';path=ROOT/'contracts/council/src/ids.rs'
if args.check:assert path.read_text()==text,'canonical IDs differ; synchronize and rebuild'
else:path.write_text(text)
