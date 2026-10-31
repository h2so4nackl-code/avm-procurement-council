#!/usr/bin/env python3
"""Bind validation receipts to source files, compiled artifacts and exact tools."""
import argparse,hashlib,json,platform,re,subprocess
from datetime import datetime,timezone
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
TAG='avm-dev-2662a32e7c91-sdk-93e34e16e449'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(sdk):
    source={}
    for p in sorted(ROOT.rglob('*')):
        parts=p.relative_to(ROOT).parts
        if p.is_file() and not any(x in {'.git','.tooling','target','__pycache__','evidence','artifacts'} for x in parts):source[str(p.relative_to(ROOT))]=digest(p)
    artifacts={str(p.relative_to(ROOT)):{'sha256':digest(p),'bytes':p.stat().st_size} for p in sorted((ROOT/'artifacts').rglob('*')) if p.is_file()}
    tests=(ROOT/'evidence/native-tests.txt').read_text()
    count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',tests)))
    assert count==66,('expected 57 project tests plus 9 inherited SDK tests',count)
    journeys={}
    for name in ['avm-e2e','avm-capacity']:
        p=ROOT/'evidence'/f'{name}.json';data=json.loads(p.read_text());assert data['status']=='PASS'
        journeys[name]={'status':data['status'],'sha256':digest(p),'commands':len(data['journal']),'accounting':data['accounting'],'replay':data['replay']}
    result={'recorded_at_utc':datetime.now(timezone.utc).isoformat(),'platform':platform.platform(),
            'rustc':subprocess.check_output(['rustc','+1.93.0','--version'],text=True).strip(),
            'sdk_release':TAG,'sdk_archive_sha256':'8735c632dfc6792fe3672eaa9e157fcf9a025a317ffaa1c4e59495531eb1383f',
            'tools':{n:digest(sdk/'bin'/n) for n in ['avm-contract-build','avm-local-stand']},
            'profile_sha256':digest(sdk/'profiles/wasmtime-release-profile.v1'),
            'source_sha256':source,'artifacts':artifacts,'native_tests':{'project':57,'inherited_sdk':9,'total':count,'log_sha256':digest(ROOT/'evidence/native-tests.txt')},
            'journeys':journeys,'qualification_scope':'Local Linux x86_64 SDK 0.3.0, synthetic ECC. No mainnet, audit or production qualification claim.'}
    (ROOT/'evidence/qualification.json').write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);a=p.parse_args();run(a.sdk.resolve())
