#!/usr/bin/env python3
"""Exercise the maximum orders, full milestone vectors and all 64 callback slots in real AVM."""
import argparse,json,tempfile
from pathlib import Path
from e2e import Stand,ROOT,DAPP,BASE_TIME,configuration,proposal,submit,vote,number,address_bytes,expect

def run(sdk,output):
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='avm-procurement-capacity-') as directory:
        stand=Stand(sdk,Path(directory))
        try:
            start=stand.command('init',dapp_id=DAPP);assert start['status']=='ok'
            keys=start['identities'];receiver=stand.deploy('receiver','capacity-receiver')
            council=stand.deploy('council','capacity-council',configuration([keys['developer'],keys['package_owner']]))
            for a in [council,receiver]:assert stand.command('fund-user',address=a,shells=1000000000)['status']=='ok'
            stand.call(receiver,'procurement.receiver.bind',address_bytes(council))
            def c(method,body=b'',identity='developer',expected=0,**kw):return stand.call(council,'procurement.'+method,body,identity=identity,expected_status=expected,**kw)
            c('deposit',value=100)
            for i in range(8):c('create',proposal(keys['user'],receiver,[1,1,1,1],BASE_TIME+10000,0x50+i))
            c('create',proposal(keys['user'],receiver,[1,0,0,0],BASE_TIME+10000),expected=3008)
            c('submit',submit(0,0,0x61),identity='user');c('vote',vote(0,1,0x61));c('vote',vote(0,1,0x61),identity='package-owner')
            stand.call(receiver,'procurement.receiver.reject',b'\x01')
            for i in range(1,64):
                expect(c('execute',number(0,8)),spent=0,reserved=32,attempts=i,receipts=i,pending=False)
            stand.call(receiver,'procurement.receiver.reject',b'\x00')
            expect(c('execute',number(0,8)),spent=1,reserved=31,attempts=64,receipts=64,pending=False)
            c('submit',submit(0,1,0x62),identity='user');c('vote',vote(0,2,0x62));c('vote',vote(0,2,0x62),identity='package-owner');stand.time+=2
            c('execute',number(0,8),expected=3008)
            final=expect(c('status'),deposited=100,spent=1,reserved=31,orders=8,attempts=64,receipts=64,pending=False)
            inspection=stand.command('inspect');assert inspection['queue']['message_count']==0
            replay=stand.command('replay');assert replay['status']=='ok'
            for key in ['account_state_root','registry_root','cumulative_fee_shells','accepted_input_count']:assert replay[key]==inspection[key],key
            output.write_text(json.dumps({'status':'PASS','accounting':final,'replay':replay,'journal':stand.log},indent=2)+'\n')
            print('Maximum-capacity real AVM journey: PASS (8 orders, 4 stages/order, 64 receipts)')
        except Exception:
            output.write_text(json.dumps({'status':'FAIL','journal':stand.log},indent=2)+'\n');raise
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);p.add_argument('--output',type=Path,default=ROOT/'evidence/avm-capacity.json');a=p.parse_args();run(a.sdk.resolve(),a.output.resolve())
