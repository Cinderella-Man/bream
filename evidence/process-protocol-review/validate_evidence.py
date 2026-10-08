from pathlib import Path
import hashlib, json, re, subprocess
root=Path(__file__).resolve().parents[2]; otp=root/'upstream/otp'
commit='ad05823719d77c8faee87348ea39513d4e2f99c5'
record=root/'evidence/process-protocol-review/review-validation.json'
result={'review_date':'2026-10-07','scope':'Documentation, index and retained-evidence consistency; not semantic completeness or Bream qualification.'}; errors=[]
docs=[root/'README.md',root/'CONTEXT.md',*sorted((root/'docs').rglob('*.md')),root/'experiments/native-preemption/README.md',root/'experiments/generated-process/README.md']
docs += sorted((root/'evidence/process-protocol-review').rglob('*.md'))
local_count=0; local_lines=0; source_refs=[]; source_paths=set()
for p in docs:
    s=p.read_text(); rel=str(p.relative_to(root))
    if not s.endswith('\n'): errors.append('Missing final newline: '+rel)
    for n,line in enumerate(s.splitlines(),1):
        if line.rstrip()!=line: errors.append(f'Trailing whitespace: {rel}:{n}')
    defs={x.lower() for x in re.findall(r'^\[([^\]\n]+)\]:',s,re.M)}
    for rid in re.findall(r'\[[^\]\n]+\]\[([^\]\n]+)\]',s):
        if rid.lower() not in defs: errors.append(f'Undefined link ref {rid}: {rel}')
    links=re.findall(r'\[[^\]\n]*\]\((<?[^)\n]+>?)\)',s)+re.findall(r'^\[[^\]\n]+\]:\s*(\S+)',s,re.M)
    for target in links:
        target=target.strip('<>')
        if target.startswith(('http:','https:','mailto:','#','app:','codex:')): continue
        target=target.split('#')[0]; lm=re.match(r'^(.*):(\d+)$',target); line=None
        if lm: target=lm[1]; line=int(lm[2]); local_lines+=1
        path=Path(target)
        if not path.is_absolute(): path=p.parent/path
        local_count+=1
        if not path.exists(): errors.append(f'Missing {target}: {rel}'); continue
        if line and (not path.is_file() or line<1 or line>len(path.read_text().splitlines())): errors.append(f'Out-of-range line {target}:{line}')
    for source,a,b in re.findall(r'https://github.com/erlang/otp/blob/'+commit+r'/([^\s)#]+)(?:#L(\d+)(?:-L(\d+))?)?',s):
        path=otp/source; source_paths.add(source)
        if not path.is_file(): errors.append('Missing source '+source); continue
        if a and not 1<=int(a)<=int(b or a)<=len(path.read_text().splitlines()): errors.append(f'Invalid range {source}:{a}-{b}')
        source_refs.append((rel,source,a,b))
idx=json.loads((root/'docs/build-plan/features.json').read_text()); parsed={}; case_files={}; by_doc={}
for p in sorted((root/'docs/build-plan').glob('*.md')):
    count=0
    for m in re.finditer(r'^## ([A-Z]+-\d{2}) — ([^\n]+)\n(.*?)(?=^## |\Z)',p.read_text(),re.M|re.S):
        fid,title,body=m.groups(); ids=re.findall(r'^\|\s*(?:\*\*)?([A-Z]+-\d{2}\.C\d{2})(?:\*\*)?\s*\|',body,re.M)
        if fid in parsed: errors.append('Duplicate feature '+fid)
        if ids!=[fid+f'.C{i:02d}' for i in range(1,len(ids)+1)] or not ids: errors.append('Nonsequential/empty cases '+fid)
        for cid in ids:
            if cid in case_files: errors.append('Duplicate case '+cid)
            case_files[cid]=str(p.relative_to(root))
        parsed[fid]=(title,str(p.relative_to(root)),ids); count+=len(ids)
    by_doc[str(p.relative_to(root))]=count
for f in idx['features']:
    title,path,ids=parsed.get(f['id'],('', '', []))
    if (f['title'],f['detail_document'],f['case_ids'],f['case_count'])!=(title,path,ids,len(ids)): errors.append('Index mismatch '+f['id'])
    if f['status']!='planned': errors.append('Unexpected status '+f['id'])
if (idx['feature_count'],idx['case_count'],len(parsed),len(case_files))!=(40,366,40,366): errors.append('Total mismatch')
newcases={}; reports=[]
for n in range(26,30):
    p=next((root/'docs/research').glob(f'{n:02d}-*.md')); rel=str(p.relative_to(root))
    ids=re.findall(r'^\|\s*(?:\*\*)?([A-Z]+-\d{2}\.C\d{2})(?:\*\*)?[^|]*\|',p.read_text(),re.M)
    reports.append({'path':rel,'case_count':len(ids),'pinned_source_reference_count':sum(1 for row in source_refs if row[0]==rel)})
    for cid in ids:
        if cid in newcases: errors.append('Duplicated report case '+cid)
        newcases[cid]=rel
        if cid not in case_files: errors.append('Unintegrated case '+cid)
if len(newcases)!=29: errors.append('New-case count mismatch')
head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=otp,text=True).strip()
status=subprocess.check_output(['git','status','--porcelain'],cwd=otp,text=True).strip()
if head!=commit or status: errors.append('Source pin/cleanliness mismatch')
def digest(p): return hashlib.sha256(p.read_bytes()).hexdigest()
# Independently recheck finite control-opcode coverage.
sig=json.loads((root/'evidence/process-deep-review/signal-dispatch-inventory.json').read_text())
h=(otp/sig['declarations_file']).read_text().splitlines(); c=(otp/sig['dispatch_file']).read_text().splitlines()
defs=set(re.findall(r'^#define\s+ERTS_SIG_Q_OP_([A-Z_]+)\s+', '\n'.join(h[173:196]),re.M))
defs.discard('MAX')  # Value bound, not a declared operation kind.
live=set(re.findall(r'case\s+ERTS_SIG_Q_OP_([A-Z_]+)\s*:', '\n'.join(c[6228:7258])))
exiting=set(re.findall(r'case\s+ERTS_SIG_Q_OP_([A-Z_]+)\s*:', '\n'.join(c[7258:7475])))
if not defs==live==exiting or len(defs)!=21: errors.append('Dispatch set mismatch')
for op in sig['operations']:
    name='ERTS_SIG_Q_OP_'+op['operation']
    if name not in h[op['declaration_line']-1] or name not in c[op['live_dispatch_line']-1] or name not in c[op['exit_dispatch_line']-1]: errors.append('Dispatch line mismatch '+name)
for path,expected in sig['source_hashes'].items():
    if digest(otp/path)!=expected: errors.append('Dispatch hash mismatch '+path)
# Check retained execution evidence without rerunning or rewriting reference observations.
base=root/'evidence/process-protocol-review'; build=root/'upstream/otp-reference-build'
provenance=json.loads((base/'source-build.json').read_text())
summary=json.loads((base/'observations-summary.json').read_text())
recovery=json.loads((base/'compiled-artifact-recovery.json').read_text())
recovered={(x['profile'],x['original_path']):x for x in recovery['artifacts']}
if provenance['source_commit']!=commit or summary['source_commit']!=commit: errors.append('Evidence source mismatch')
if subprocess.check_output(['git','rev-parse','HEAD'],cwd=build,text=True).strip()!=commit: errors.append('Build source pin mismatch')
build_status=subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=build,text=True).strip()
if build_status: errors.append('Build tracked source changed')
checks=0
def check_hash(row, parent=root):
    global checks
    path=parent/row['path']; checks+=1
    if not path.is_file() or digest(path)!=row['sha256']: errors.append('Evidence hash mismatch '+str(path))
for row in provenance['binary_hashes']: check_hash(row)
for row in provenance['source_inputs']:
    check_hash(row,build); check_hash(row,otp)
if [x['exit_code'] for x in provenance['build_attempts']]!=[2,0,0]: errors.append('Unexpected recorded build outcomes')
profiles=[]; retained_files=set(); total_families=0; ordering_samples=0
for name in ['jit-1','jit-4','debug-emu-4']:
    manifest=json.loads((base/name/'run-manifest.json').read_text())
    if manifest['source_commit']!=commit or manifest['otp_version_file']!='29.1.1': errors.append('Manifest pin mismatch '+name)
    for field in ['fixtures','compiled_fixture_hashes','artifact_hashes']:
        for row in manifest[field]:
            if field=='compiled_fixture_hashes':
                saved=recovered.get((name,row['path']))
                if not saved or saved['recorded_sha256']!=row['sha256'] or saved['recovered_sha256']!=row['sha256']:
                    errors.append('Compiled artifact recovery mismatch '+name+'/'+row['path'])
                else:
                    check_hash({'path':saved['retained_path'],'sha256':row['sha256']})
                    retained_files.add(saved['retained_path'])
            else:
                check_hash(row); retained_files.add(row['path'])
    cmds={row['name']:row for row in manifest['commands']}
    for cname,row in cmds.items():
        expected=-6 if name=='debug-emu-4' and cname=='priority-marker-normal' else 0
        if row['exit_code']!=expected or row['timed_out']: errors.append('Unexpected command result '+name+'/'+cname)
    obs=json.loads((base/name/'observations.json').read_text())
    if obs['metadata']['erts_version']!='17.1' or len(obs['results'])!=11 or any(x['status']!='completed' for x in obs['results']): errors.append('Observation completion/identity mismatch '+name)
    total_families+=len(obs['results'])
    oracle=next(x for x in summary['profiles'] if x['profile']==name)
    values={x['name']:x['observations'] for x in obs['results']}
    if oracle['observations']!=values or oracle['metadata']!=obs['metadata']: errors.append('Summary differs from raw observations '+name)
    for n in [255,256,257]:
        h=json.loads((base/name/f'hibernate-{n}.json').read_text())
        if h['status']!='completed' or h not in oracle['hibernate']: errors.append('Hibernation result mismatch '+name+'/'+str(n))
    ordering_samples+=sum(v['count'] for x in values['notification_order'] for v in x['sequence_counts'])
    normal=(base/name/'disassembly-normal.log').read_text()
    control=(base/name/'disassembly-no_recv_opt.log').read_text()
    # Scope instruction order to probe/0, not an unrelated helper function.
    begin=normal.find('{function,probe,0,')
    if begin<0:
        errors.append('Missing disassembled probe/0 '+name); body=''
    else:
        end=normal.find('{function,',begin+1); body=normal[begin:end if end>=0 else None]
    cursor=0
    for token in ['recv_marker_reserve','make_ref','recv_marker_bind','unalias','recv_marker_use']:
        pos=body.find(token,cursor)
        if pos<0: errors.append('Missing/inverted probe instruction '+token+' '+name)
        cursor=pos+len(token)
    if 'recv_marker_' in control: errors.append('Marker instruction in unoptimized control '+name)
    marker=oracle['marker']
    if marker['optimized_exit_code']!=cmds['priority-marker-normal']['exit_code'] or marker['unoptimized_exit_code']!=0: errors.append('Marker summary mismatch '+name)
    if name=='debug-emu-4':
        failure=(base/name/'priority-marker-normal.log').read_text()
        if 'erl_proc_sig_queue.h:2190' not in failure or 'Assertion failed' not in failure: errors.append('Expected assertion absent')
    profiles.append({'profile':name,'general_families_completed':len(obs['results']),'isolated_hibernation_cases_completed':3,'optimized_marker_exit_code':marker['optimized_exit_code'],'unoptimized_marker_exit_code':marker['unoptimized_exit_code'],'probe_instruction_order_verified':True,'unoptimized_markers_absent':True})
if total_families!=33 or ordering_samples!=1200: errors.append('Executed fixture count mismatch')
result['reference_execution']={'source_build_provenance':'evidence/process-protocol-review/source-build.json','compiled_artifact_recovery':'evidence/process-protocol-review/compiled-artifact-recovery.json','full_application_build_succeeded':False,'core_and_debug_targets_succeeded':True,'build_tracked_source_clean':not build_status,'profiles':profiles,'general_family_executions':total_families,'notification_order_samples':ordering_samples,'expected_observed_assertion_failures':1,'upstream_suite_run':False,'bream_implementation_or_conformance_run':False,'artifact_hash_checks':checks,'distinct_manifest_files_hashed':len(retained_files),'creator_death_limit':'Fixture observes DOWN before selecting timeout; it does not prove the timer was active at death.'}
result['review_corrections']=['Separated timer read-only/cancel cohorts and added pre-expiry checks before retained runs.','Verified marker instructions and same-debug-VM no_recv_opt control.','Qualified creator-death causal inference; source supports stronger lifetime rule.','Updated active build-plan cases with bounded reference results while preserving planned status.']
result['validator']={'path':str(Path(__file__).relative_to(root)),'sha256':digest(Path(__file__))}
result.update(status='passed'  if not errors else 'failed',errors=errors,case_counts_by_document=by_doc,new_reports=reports)
result['reference']={'release':'OTP-29.1.1','commit':head,'checkout_clean':not status}
result['counts']={'markdown_documents':len(docs),'local_link_occurrences':local_count,'local_line_links':local_lines,'pinned_source_reference_occurrences':len(source_refs),'unique_cited_pinned_files':len(source_paths),'feature_groups':len(parsed),'planned_branch_cases':len(case_files),'new_reports':len(reports),'new_cases_integrated':len(newcases),'declared_control_kinds_matched_in_both_dispatchers':len(defs)}
result['document_hashes']=[{'path':str(p.relative_to(root)),'sha256':digest(p)} for p in docs+[root/'docs/build-plan/features.json']]
result['cited_pinned_source_files']=[{'path':rel,'sha256':digest(otp/rel)} for rel in sorted(source_paths)]
record.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'status':result['status'],'counts':result['counts'],'errors':errors},indent=2))
raise SystemExit(bool(errors))
