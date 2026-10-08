#!/usr/bin/env python3
"""Run bounded reference observations using the separately built, pinned VM."""
import argparse, hashlib, json, os, platform, resource, subprocess, time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
EVIDENCE=ROOT/'evidence/process-protocol-review'
BUILD=ROOT/'upstream/otp-reference-build'
OUT=ROOT/'.tools/reference-fixtures'
P=argparse.ArgumentParser();P.add_argument('--profile',choices=['jit-1','jit-4','debug-emu-4'],required=True);args=P.parse_args()
profile=args.profile
flags=['+S','1:1' if profile=='jit-1' else '4:4','+SDcpu','1','+SDio','1']
if profile=='debug-emu-4': flags=['-emu_type','debug','-emu_flavor','emu']+flags
profile_dir=EVIDENCE/profile;profile_dir.mkdir(exist_ok=True)
env=dict(os.environ);env['ERL_FLAGS']='';env['ERL_AFLAGS']='';env['ERL_ZFLAGS']=''
commands=[]
def no_core(): resource.setrlimit(resource.RLIMIT_CORE,(0,0))
def run(name,cmd,timeout=35):
    start=time.time()
    try:
        r=subprocess.run([str(x) for x in cmd],cwd=profile_dir,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=timeout,preexec_fn=no_core)
        code=r.returncode;data=r.stdout;timed_out=False
    except subprocess.TimeoutExpired as e:
        code=None;data=e.stdout or b'';timed_out=True
    (profile_dir/(name+'.log')).write_bytes(data)
    commands.append({'name':name,'argv':[str(x) for x in cmd],'exit_code':code,'timed_out':timed_out,'elapsed_seconds':round(time.time()-start,3),'log':str((profile_dir/(name+'.log')).relative_to(ROOT))})
    return code
for mode in ['normal','no_recv_opt']:
    target=OUT/mode;target.mkdir(parents=True,exist_ok=True)
    opts=['+debug_info']+(['+no_recv_opt'] if mode=='no_recv_opt' else [])
    code=run('compile-'+mode,[BUILD/'bin/erlc',*opts,'-o',target,EVIDENCE/'fixtures/bream_reference_probe.erl',EVIDENCE/'priority_marker_probe.erl'],60)
    if code: raise SystemExit('Fixture compile failed: '+mode)
normal=OUT/'normal'
run('observations',[BUILD/'bin/erl',*flags,'-noshell','-pa',normal,'-run','bream_reference_probe','run',profile_dir/'observations.json'])
for n in [255,256,257]:
    run('hibernate-'+str(n),[BUILD/'bin/erl',*flags,'-noshell','-pa',normal,'-run','bream_reference_probe','run',profile_dir/f'hibernate-{n}.json','hibernate',str(n)])
for mode in ['normal','no_recv_opt']:
    target=OUT/mode
    beam=str(target/'priority_marker_probe.beam')
    # Paths are local fixed project paths without quoting characters.
    expression=f'io:format("~p.~n",[beam_disasm:file("{beam}")]),halt().'
    run('disassembly-'+mode,[BUILD/'bin/erl',*flags,'-noshell','-eval',expression])
    run('priority-marker-'+mode,[BUILD/'bin/erl',*flags,'-noshell','-pa',target,'-eval','io:format("BREAM_PRIORITY_RESULT ~p~n",[priority_marker_probe:run()]),halt().'])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
fixtures=[EVIDENCE/'fixtures/bream_reference_probe.erl',EVIDENCE/'priority_marker_probe.erl',Path(__file__)]
artifacts=list(profile_dir.glob('*.json'))+list(profile_dir.glob('*.log'))
manifest={'profile':profile,'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=BUILD,text=True).strip(),'otp_version_file':(BUILD/'OTP_VERSION').read_text().strip(),'platform':platform.platform(),'commands':commands,'fixtures':[{'path':str(p.relative_to(ROOT)),'sha256':sha(p)} for p in fixtures],'compiled_fixture_hashes':[{'path':str(p.relative_to(ROOT)),'sha256':sha(p)} for p in OUT.glob('*/*.beam')],'artifact_hashes':[{'path':str(p.relative_to(ROOT)),'sha256':sha(p)} for p in artifacts if p.name!='run-manifest.json'],'qualification':'Bounded BEAM reference observations. A completed command is not a Bream conformance pass; assertion failures and precondition errors require separate interpretation.'}
(profile_dir/'run-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'profile':profile,'commands':[{k:c[k] for k in ['name','exit_code','timed_out']} for c in commands]},indent=2))
