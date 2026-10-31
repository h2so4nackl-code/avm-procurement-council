#!/usr/bin/env python3
"""Install one immutable official release into this project, verifying all payloads."""
from pathlib import Path
import hashlib,importlib.util,tempfile,urllib.request
ROOT=Path(__file__).resolve().parents[1]
TAG='avm-dev-2662a32e7c91-sdk-93e34e16e449'
SHA256='8735c632dfc6792fe3672eaa9e157fcf9a025a317ffaa1c4e59495531eb1383f'
NAME='avm-sdk-linux-x86_64.tar.gz'
def main():
    spec=importlib.util.spec_from_file_location('official_release',ROOT/'tools/release.py')
    release=importlib.util.module_from_spec(spec);spec.loader.exec_module(release)
    with tempfile.TemporaryDirectory(prefix='procurement-sdk-') as directory:
        archive=Path(directory)/NAME
        request=urllib.request.Request(f'https://github.com/gosh-sh/avm-sdk/releases/download/{TAG}/{NAME}',headers={'User-Agent':'avm-procurement-council'})
        with urllib.request.urlopen(request,timeout=90) as response,archive.open('wb') as out:
            count=0
            while data:=response.read(1024*1024):
                count+=len(data)
                if count>20*1024*1024:raise ValueError('unexpected SDK archive size')
                out.write(data)
        if hashlib.sha256(archive.read_bytes()).hexdigest()!=SHA256:raise ValueError('SDK SHA-256 mismatch')
        checksum=archive.with_suffix(archive.suffix+'.sha256');checksum.write_text(f'{SHA256}  {NAME}\n')
        target=release.install_archive(archive,checksum,TAG,ROOT/'.tooling/sdk',ROOT/'.tooling/bin')
        print(target)
if __name__=='__main__':main()
