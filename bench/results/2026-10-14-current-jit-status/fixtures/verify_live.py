import pathlib,subprocess,os,threading,queue,json,time,hashlib,signal,re
root=pathlib.Path(__file__).resolve().parents[2];base=root/'target/current-jit-validation';fixture=base/'fixture';entry=fixture/'main.dart';stop=base/'stop';cli=root/'target/current-toolchain-status/dartforge-jit';sdk='/Volumes/ssd_dock/tools/dart/3.6.2/dart-sdk/lib'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source='''import 'dart:async';
import 'dart:io';
class State { int value = 101; int turns = 0; __FIELD__ }
final state = State();
String phase() => '__PHASE__';
String note() => __NOTE__;
void main() {
  Timer.periodic(const Duration(milliseconds: 200), (timer) {
    state.turns++;
    if (state.turns == 1) state.value = 202;
    print('LIVE|${phase()}|${state.value}|${state.turns}|${note()}');
    if (File('__STOP__').existsSync()) timer.cancel();
  });
}
'''.replace('__STOP__',str(stop))
versions={phase:source.replace('__PHASE__',phase).replace('__FIELD__','' if phase=='one' else 'String? added;').replace('__NOTE__',"'initial'" if phase=='one' else "state.added == null ? 'null-added' : state.added!") for phase in ['one','two','three','four']}
stop.unlink(missing_ok=True)
entry.write_text(versions['one']);(base/'versions.json').write_text(json.dumps(versions,indent=2)+'\n');events=[];rss=[];q=queue.Queue();command=[str(cli),'reload',str(entry),'--sdk',sdk,'--interval','50','--once']
# Diagnostic RSS snapshots are not a paired performance benchmark.
p=subprocess.Popen(command,env=os.environ.copy(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
def reader(stream,name):
 with (base/'raw'/('live.'+name)).open('w') as f:
  for line in stream:
   f.write(line);f.flush();q.put((name,line.rstrip('\n'),time.time()))
threads=[threading.Thread(target=reader,args=(p.stdout,'stdout'),daemon=True),threading.Thread(target=reader,args=(p.stderr,'stderr'),daemon=True)]
for t in threads:t.start()
def sample(label):
 text=subprocess.run(['ps','-axo','pid,ppid,rss,command'],capture_output=True,text=True).stdout;rows=[]
 for line in text.splitlines()[1:]:
  cells=line.strip().split(None,3)
  if len(cells)==4 and (int(cells[0])==p.pid or int(cells[1])==p.pid):rows.append({'pid':int(cells[0]),'ppid':int(cells[1]),'rss_kib':int(cells[2]),'command':cells[3]})
 rss.append({'phase':label,'unix':time.time(),'processes':rows})
def wait_phase(phase,timeout=240):
 deadline=time.monotonic()+timeout
 while time.monotonic()<deadline:
  if p.poll() is not None:raise RuntimeError('reload exited early: '+str(p.returncode))
  try:name,line,t=q.get(timeout=1)
  except queue.Empty:continue
  events.append({'stream':name,'line':line,'unix':t})
  if name=='stdout' and line.startswith('LIVE|'+phase+'|'):
   fields=line.split('|');sample(phase);print(phase,fields,flush=True);return fields
 raise TimeoutError(phase)
result={}
try:
 first=wait_phase('one');assert first[2]=='202';last=int(first[3]);states=[first]
 for phase in ['two','three','four']:
  tmp=entry.with_suffix('.tmp');tmp.write_text(versions[phase]);tmp.replace(entry)
  fields=wait_phase(phase);assert fields[2]=='202' and int(fields[3])>last and fields[4]=='null-added';last=int(fields[3]);states.append(fields)
 stop.touch();code=p.wait(timeout=30);assert code==0
 result={'success':True,'states':states,'exit_code':code}
finally:
 if p.poll() is None:os.killpg(p.pid,signal.SIGTERM);p.wait(timeout=10)
 for t in threads:t.join(timeout=3)
 result.update({'compiler':str(cli),'compiler_sha256':sha(cli),'command':command,'events':events,'rss_snapshots':rss,'fixture_versions_sha256':{k:hashlib.sha256(v.encode()).hexdigest() for k,v in versions.items()},'limits':['Diagnostic RSS at active phases, not benchmark or leak proof.','No hot reload timing comparison with official VM.','Ordinary async live fixture; default tracing only.']});(base/'live-validation.json').write_text(json.dumps(result,indent=2)+'\n')
