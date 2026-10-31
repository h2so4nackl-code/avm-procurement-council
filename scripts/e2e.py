#!/usr/bin/env python3
"""Real AVM SDK 0.3.0 local-stand qualification. No mainnet calls or real funds.
Stand helper adapted from GOSH SDK bounty-escrow MIT example.
"""
import argparse, json, subprocess, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
DAPP="11"*32
BASE_TIME=1800000000
class Stand:
    def __init__(self, sdk, work):
        self.binary = str(sdk / "bin/avm-local-stand")
        self.profile = str(sdk / "profiles/wasmtime-release-profile.v1")
        self.state = work / "journey.state"
        self.log = []
        self.time = BASE_TIME
        self.selections = {}

    def command(self, operation, **options):
        cmd = [self.binary, operation, "--state", str(self.state), "--wasmtime-profile", self.profile]
        for key, value in options.items():
            cmd += ["--" + key.replace("_", "-")] + ([] if value is True else [str(value)])
        proc = subprocess.run(cmd, capture_output=True, text=True)
        try:
            result = json.loads(proc.stdout)
        except json.JSONDecodeError:
            raise RuntimeError(f"{operation}: {proc.stderr}\n{proc.stdout}") from None
        self.log.append({"operation": operation, "options": options, "exit_code": proc.returncode, "result": result})
        return result

    def timestamps(self):
        self.time += 1
        return dict(block_time=self.time, created_at=self.time, expire_at=self.time+3600, fuel_fee_shells=10000)

    def deploy(self, contract, salt, init=None):
        artifacts = ROOT / "contracts" / contract / "target/avm"
        options = dict(wasm=artifacts / "contract.wasm", acab=artifacts / "contract.acab",
                       descriptor=artifacts / "contract.descriptor", salt_hex=salt.encode().hex(),
                       init_json=json.dumps("0x"+(b"" if init is None else init).hex()), **self.timestamps())
        if contract in self.selections:
            for key in ["wasm", "acab", "descriptor"]:
                del options[key]
            options.update(self.selections[contract])
        # Convert paths to text for the evidence journal.
        options = {key: str(value) if isinstance(value, Path) else value for key,value in options.items()}
        precondition = self.command("deploy", quote_address=True, **options)
        assert precondition.get("quoted") is True, precondition
        address = precondition["address"]
        funding = self.command("fund-user", address=address, shells=10_000_000)
        assert funding["status"] == "ok", funding
        result = self.command("deploy", **options)
        assert result["status"] == "ok", result
        if contract not in self.selections:
            snapshot = self.command("inspect")
            existing = {v["selected_code_id"] for v in self.selections.values()}
            selection = next(item for item in snapshot["registry_entries"]
                             if item["key"].startswith("02") and item["value"][:64] not in existing)
            self.selections[contract] = {"selected_code_id": selection["value"][:64],
                                         "package_id": selection["value"][64:128]}
        return address

    def call(self, address, method, body=b"", identity="developer", value=0, extra="", expected_status=0):
        result = self.command("call", address=address, method=method,
                            args_json=json.dumps("0x"+body.hex()) if body else "null",
                            identity=identity, extra_currencies=(f"0:{value}," if value and extra else f"0:{value}" if value else "")+extra, **self.timestamps())
        assert result["status"] == "ok", result
        raw = bytes.fromhex(result["raw_executor_output"])
        actual_status = int.from_bytes(raw[4:8], "little")
        assert raw[:4] == b"ACMO" and actual_status == expected_status, {"method": method, "contract_status": actual_status, "expected": expected_status}
        return result

def number(x,n):return x.to_bytes(n,'little')
def address_bytes(x):return bytes.fromhex(x.replace(':',''))
def configuration(keys):
    return bytes([len(keys),2,0,0])+number(2,4)+number(10000,16)+b''.join(bytes.fromhex(k) for k in keys)+bytes((5-len(keys))*32)
def proposal(vendor,receiver,amounts,deadline,spec=0x51):
    return bytes.fromhex(vendor)+address_bytes(receiver)+bytes([spec])*32+b''.join(number(a,16) for a in amounts)+number(deadline,4)+bytes([len([a for a in amounts if a])])
def submit(order,stage,digest):return number(order,8)+bytes([stage])+bytes([digest])*32
def vote(order,revision,digest,yes=True):return number(order,8)+number(revision,4)+bytes([digest])*32+bytes([yes])
def summary(result):
    b=bytes.fromhex(result['raw_state']);assert len(b)==6144 and b[:4]==b'APC1'
    return dict(deposited=int.from_bytes(b[188:204],'little'),spent=int.from_bytes(b[204:220],'little'),reserved=int.from_bytes(b[220:236],'little'),attempts=int.from_bytes(b[236:244],'little'),paused=bool(b[244]),orders=b[246],receipts=b[247],pending=bool(b[248]))
def expect(result,**expected):
    actual=summary(result)
    for k,v in expected.items():assert actual[k]==v,(k,actual,expected)
    assert actual['spent']+actual['reserved']<=actual['deposited'],actual
    return actual
def run(sdk,output):
    output.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='avm-procurement-') as directory:
        stand=Stand(sdk,Path(directory))
        try:
            start=stand.command('init',dapp_id=DAPP);assert start['status']=='ok',start
            keys=start['identities'];dev=keys['developer'];co=keys['package_owner'];vendor=keys['user']
            receiver=stand.deploy('receiver','procurement-vendor')
            council=stand.deploy('council','procurement-council',configuration([dev,co]))
            stand.call(receiver,'procurement.receiver.bind',address_bytes(council))
            def c(method,body=b'',identity='developer',expected=0,**kw):
                return stand.call(council,'procurement.'+method,body,identity=identity,expected_status=expected,**kw)
            expect(c('deposit',value=3000),deposited=3000,reserved=0,spent=0)
            deadline=BASE_TIME+10000
            c('create',proposal(vendor,receiver,[300,400,300,0],deadline))
            expect(c('create',proposal(vendor,receiver,[500,0,0,0],deadline,0x52)),reserved=1500)
            c('create',proposal(vendor,receiver,[2000,0,0,0],deadline),expected=3009)
            c('create',proposal(vendor,receiver,[1,0,0,0],deadline),identity='user',expected=3001)
            c('submit',submit(0,0,0x61),identity='user')
            c('vote',vote(0,1,0x61));c('execute',number(0,8),expected=3005)
            c('vote',vote(0,1,0x61),identity='package-owner')
            c('execute',number(0,8),expected=3005)
            # A revised delivery voids all approvals of the older content.
            c('submit',submit(0,0,0x62),identity='user')
            c('vote',vote(0,1,0x61),expected=3011)
            c('vote',vote(0,2,0x62));c('vote',vote(0,2,0x62),identity='package-owner')
            c('pause');c('execute',number(0,8),expected=3010)
            c('resume');c('resume',identity='user',expected=3001);c('resume',identity='package-owner')
            stand.call(receiver,'procurement.receiver.reject',b'\x01')
            # Runtime executes the failed recipient, exact principal refund and callback.
            expect(c('execute',number(0,8)),spent=0,reserved=1500,attempts=1,receipts=1,pending=False)
            stand.call(receiver,'procurement.receiver.reject',b'\x00')
            expect(c('execute',number(0,8)),spent=300,reserved=1200,attempts=2,receipts=2,pending=False)
            c('execute',number(0,8),expected=3005)
            c('submit',submit(0,1,0x63),expected=3001)
            c('submit',submit(0,1,0x63),identity='user')
            c('vote',vote(0,3,0x63));c('vote',vote(0,3,0x63),identity='package-owner')
            c('vote',vote(0,3,0x63,False))
            c('execute',number(0,8),expected=3005)
            c('vote',vote(0,3,0x63));stand.time+=2
            expect(c('execute',number(0,8)),spent=700,reserved=800,attempts=3)
            c('submit',submit(0,2,0x64),identity='user');c('vote',vote(0,4,0x64));c('vote',vote(0,4,0x64),identity='package-owner');stand.time+=2
            expect(c('execute',number(0,8)),spent=1000,reserved=500,attempts=4)
            c('execute',number(0,8),expected=3003)
            c('cancel',number(1,8)+b'\x01')
            expect(c('cancel',number(1,8)+b'\x01',identity='package-owner'),spent=1000,reserved=0)
            c('create',proposal(vendor,receiver,[200,0,0,0],deadline,0x53))
            c('expire',number(2,8),expected=3005)
            c('topup',extra='2:1000000000')
            # Fund rent before the deliberate time jump; rent is separate from principal.
            stand.command('fund-user',address=council,shells=1000000000)
            stand.time=deadline-1
            expect(c('expire',number(2,8)),spent=1000,reserved=0,pending=False)
            final=expect(c('status'),deposited=3000,spent=1000,reserved=0,attempts=4,receipts=4,pending=False)
            receiver_state=stand.command('inspect',account=receiver)
            raw=bytes.fromhex(receiver_state['raw_state']);assert int.from_bytes(raw[104:120],'little')==1000
            assert int.from_bytes(raw[120:128],'little')==3
            inspection=stand.command('inspect');assert inspection['queue']['message_count']==0
            replay=stand.command('replay');assert replay['status']=='ok',replay
            for key in ['account_state_root','registry_root','cumulative_fee_shells','accepted_input_count']:
                assert replay[key]==inspection[key],key
            result={'status':'PASS','sdk_release':'avm-dev-2662a32e7c91-sdk-93e34e16e449','accounting':final,'receiver_received':1000,'receiver_payments':3,'council':council,'receiver':receiver,'replay':replay,'journal':stand.log}
            output.write_text(json.dumps(result,indent=2)+'\n');print('Procurement real AVM journey: PASS')
        except Exception:
            output.write_text(json.dumps({'status':'FAIL','journal':stand.log},indent=2)+'\n');raise
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);p.add_argument('--output',type=Path,default=ROOT/'evidence/avm-e2e.json');a=p.parse_args();run(a.sdk.resolve(),a.output.resolve())
