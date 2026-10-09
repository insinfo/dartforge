import os,subprocess,time,json,re,ctypes,hashlib,statistics,gzip
from pathlib import Path
k=ctypes.windll.kernel32;k.GetCurrentProcess.restype=ctypes.c_void_p;k.SetProcessAffinityMask.argtypes=[ctypes.c_void_p,ctypes.c_size_t]
assert k.SetProcessAffinityMask(k.GetCurrentProcess(),4)
env=dict(os.environ,DARTFORGE_GC_STRESS='1',DARTFORGE_GC_RASTRO='1',DARTFORGE_ARC_CONFERIR='0',DARTFORGE_ARC_CICLOS='sempre')
d=Path('target/diferencial/nativo/60_closures_perfil').resolve();paths={'antes':d/'60_closures_perfil-antes.exe','depois':d/'60_closures_perfil.exe'}
ref=None;samples=[]
for rodada in range(7):
 for variante in (['antes','depois'] if rodada%2==0 else ['depois','antes']):
  start=time.perf_counter();p=subprocess.run([str(paths[variante])],cwd=d,env=env,capture_output=True,timeout=60);wall=time.perf_counter()-start
  assert p.returncode==0,(variante,p.stderr[-1000:])
  if ref is None:ref=p.stdout
  assert p.stdout==ref
  total=trial=exam=coletas=0
  for line in p.stderr.decode().splitlines():
   m=re.search(r'\[arc\].*?us=(\d+) fases=([\d/]+) laco=([\d/]+).*?examinados=(\d+)',line)
   if not m:continue
   coletas+=1;total+=int(m[1]);trial+=int(m[3].split('/')[1]);exam=int(m[4])
  assert coletas>0
  samples.append(dict(rodada=rodada,variante=variante,wall=wall,us=total,trial_us=trial,examinados=exam,coletas=coletas,stdout=p.stdout.decode(),stderr=p.stderr.decode(),codigo=p.returncode))
with gzip.open('target/arc-closures-perfil-comparacao-isolada.json.gz','wt',encoding='utf-8') as f:json.dump(dict(afinidade='0x4',hashes={v:hashlib.sha256(p.read_bytes()).hexdigest() for v,p in paths.items()},amostras=samples),f)
for variante in paths:
 a=[s for s in samples if s['variante']==variante]
 print(json.dumps(dict(variante=variante,mediana_wall=statistics.median(s['wall'] for s in a),mediana_us=statistics.median(s['us'] for s in a),mediana_trial_us=statistics.median(s['trial_us'] for s in a),examinados=sorted(set(s['examinados'] for s in a)),coletas=sorted(set(s['coletas'] for s in a)))))
