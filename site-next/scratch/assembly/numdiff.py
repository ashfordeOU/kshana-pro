import json,re,sys,os
B='before-combo/2026-09-27-combo/'; A='../../explorations/2026-09-27-combo/'
def load(p):
    s=open(p,encoding='utf-8').read()
    if p.endswith('.mjs'):
        i=s.find('export default ')
        if i<0: return None
        s=s[i+15:].rstrip().rstrip(';')
    try: return json.loads(s)
    except Exception: return None
SKIP=re.compile(r'(sha|digest|hash|commit|version|engine|build|recorded|utc|date|bytes|ms$|dirty|tree|_s$|time)',re.I)
def walk(a,b,p,out):
    if isinstance(a,dict) and isinstance(b,dict):
        for k in sorted(set(a)|set(b)):
            if k not in a or k not in b: out.append((p+'/'+k,'added' if k not in a else 'removed')); continue
            walk(a[k],b[k],p+'/'+k,out)
    elif isinstance(a,list) and isinstance(b,list):
        if len(a)!=len(b): out.append((p,f'len {len(a)} -> {len(b)}')); return
        for i,(x,y) in enumerate(zip(a,b)): walk(x,y,f'{p}[{i}]',out)
    elif a!=b:
        last=p.split('/')[-1]
        if SKIP.search(last): return
        if isinstance(a,str) and isinstance(b,str):
            na=re.findall(r'-?\d+\.?\d*',a.replace('0.28.0','').replace('0.29.0','')); nb=re.findall(r'-?\d+\.?\d*',b.replace('0.28.0','').replace('0.29.0',''))
            if na==nb: return
        out.append((p,f'{str(a)[:90]!r} -> {str(b)[:90]!r}'))
files=['.build/viz.json','data/series.json','src/js/missions-runs.mjs','js/evidence-data.mjs','js/developers-data.mjs','assets/capabilities/capabilities.json','assets/waterfall/home-waterfall.json','src/data/runs/home.json','src/data/cli-session.json','src/data/mcp-session.json','src/data/publications.json']
for f in files:
    if not (os.path.exists(B+f) and os.path.exists(A+f)): print('MISSING',f); continue
    a,b=load(B+f),load(A+f)
    if a is None or b is None: print('UNPARSED',f); continue
    out=[]; walk(a,b,'',out)
    print(f'== {f}: {len(out)} changed')
    for x in out[:40]: print('   ',x[0][:110],x[1])
