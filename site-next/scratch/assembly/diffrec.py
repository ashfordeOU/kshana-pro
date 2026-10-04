import json,sys,os,glob
old,new=sys.argv[1],sys.argv[2]
tot=0; files=0
def walk(a,b,p,out):
    if isinstance(a,dict) and isinstance(b,dict):
        for k in set(a)|set(b):
            if k not in a or k not in b: out.append((p+'/'+k,'missing')); continue
            walk(a[k],b[k],p+'/'+k,out)
    elif isinstance(a,list) and isinstance(b,list):
        if len(a)!=len(b): out.append((p,'len %d->%d'%(len(a),len(b)))); return
        for i,(x,y) in enumerate(zip(a,b)): walk(x,y,p+'[%d]'%i,out)
    elif a!=b:
        if isinstance(a,str) and isinstance(b,str) and a[:1] in '{[':
            try: walk(json.loads(a),json.loads(b),p+'<json>',out); return
            except Exception: pass
        if p.endswith('/ms') or p.endswith('recorded_utc') or p.endswith('/bytes'): return
        if isinstance(a,str) and a.replace('0.28.0','0.29.0')==b: return
        out.append((p,repr(a)[:60]+' -> '+repr(b)[:60]))
for sub in ('recorded','native'):
  for f in sorted(glob.glob(os.path.join(new,sub,'**','*.json'),recursive=True)):
    rel=os.path.relpath(f,new); o=os.path.join(old,rel)
    if not os.path.exists(o): print('NEW',rel); continue
    try: A=json.load(open(o)); B=json.load(open(f))
    except Exception as e: 
        if open(o,'rb').read()!=open(f,'rb').read(): print('NONJSON DIFF',rel)
        continue
    out=[]; walk(A,B,'',out)
    if out:
        files+=1; tot+=len(out); print(rel,len(out)); [print('   ',x) for x in out[:4]]
print('files differing',files,'values',tot)
