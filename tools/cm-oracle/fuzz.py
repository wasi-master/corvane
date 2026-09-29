#!/usr/bin/env python3
"""Line-terminator fuzz for the CodeMirror ports.

Copies every golden sample with U+2028 / U+2029 spliced in at fixed strides
(JS `.` and JS source stop at them, Rust's `.` does not), records GHD's
tokens for each variant with cmtok.js, and leaves a samples/ + expected/
pair the golden test runs against:

    python3 tools/cm-oracle/fuzz.py target/cm-fuzz
    CM_GOLDEN_DIR=target/cm-fuzz cargo test -p corvane-highlight --test cm_golden

Variants GHD itself throws on or loops forever on (rust, toml) are dropped.
"""

import sys,json,subprocess,shutil
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
ROOT=Path(__file__).resolve().parents[2]; sys.path.insert(0,str(ROOT/'tools/cm-oracle'))
import gen
ext,base,module=gen.tables()
T=ROOT/'crates/corvane-highlight/tests/cm'
OUT=Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'target'/'cm-fuzz'); shutil.rmtree(OUT,ignore_errors=True)
(OUT/'samples').mkdir(parents=True); (OUT/'expected').mkdir()
def variants(text):
    lines=text.split('\n')
    def ins(step,off,ch):
        return '\n'.join(''.join(c+(ch if (i+off)%step==step-1 else '') for i,c in enumerate(l)) for l in lines)
    return [ins(9,0,' '),ins(13,5,' '),ins(4,1,' ')]
jobs=[]
for s in sorted((T/'samples').iterdir()):
    if not (T/'expected'/(s.name+'.tokens')).exists(): continue
    name=s.name.lower()
    mime=ext.get(s.suffix.lower()) or base.get(name) or base.get(name.split('.',1)[0])
    mod=module[mime]; mod=mod if mod.startswith('codemirror-mode') else mod.replace('codemirror/mode/','')+'.js'
    text=s.read_text(encoding='utf-8')
    for k,v in enumerate(variants(text)):
        # keep the extension / basename: prefix the variant tag
        vn=f'{s.stem}.v{k}{s.suffix}' if s.suffix else None
        if s.name.lower() in base: vn=None
        if vn is None: continue
        p=OUT/'samples'/vn; p.write_text(v,encoding='utf-8'); jobs.append((p,mime,mod))
def run(j):
    p,mime,mod=j
    try:
        r=subprocess.run(['node',str(ROOT/'tools/cm-oracle/cmtok.js'),mime,mod,str(p)],capture_output=True,text=True,timeout=60)
    except subprocess.TimeoutExpired:
        p.unlink(); return (p.name,'oracle-hang')
    if r.returncode: p.unlink(); return (p.name,'oracle-crash')
    rows=[json.loads(l) for l in r.stdout.splitlines() if l.strip()]
    (OUT/'expected'/(p.name+'.tokens')).write_text(''.join(f'{a} {b} {c} {d}\n' for a,b,c,d in rows))
    return (p.name,len(rows))
with ThreadPoolExecutor(8) as ex: res=list(ex.map(run,jobs))
bad=[f'{n} ({r})' for n,r in res if isinstance(r,str)]
print(len(res),'variants;',len(bad),'dropped:',' '.join(bad))
