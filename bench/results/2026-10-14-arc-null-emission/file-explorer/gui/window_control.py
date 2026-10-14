import argparse,importlib.util,json,pathlib,struct,sys,time
root=pathlib.Path('/Volumes/ssd_dock/MyRustProjects/dartforge')
spec=importlib.util.spec_from_file_location('control',root/'target/file-explorer/ordinary_window_control.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
p=argparse.ArgumentParser();p.add_argument('--pid',type=int,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--action',default='capture');p.add_argument('--folder',type=pathlib.Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
owners=c.descendants(a.pid);v=c.window_list(0,0)
try:windows=[w for w in c.decode(v) if w.get('kCGWindowOwnerPID') in owners and w.get('kCGWindowLayer')==0 and w.get('kCGWindowBounds',{}).get('Width',0)>=640 and w.get('kCGWindowBounds',{}).get('Height',0)>=400]
finally:c.release(v)
assert len(windows)==1,windows
w=windows[0];record={'all_cg_window':w};c.focus(w)
app=c.ax_app(w['kCGWindowOwnerPID'])
try:
 err,children=c.attribute(app,'AXChildren');record['children_error']=err;record['child_roles']=[]
 try:
  if err:raise RuntimeError('Cannot read AXChildren')
  raised=False
  for i in range(c.array_count(children)):
   child=c.array_item(children,i);e,role=c.attribute(child,'AXRole')
   try:r=c.decode(role) if not e else None
   finally:
    if role:c.release(role)
   record['child_roles'].append(r)
   if r=='AXWindow':
    fn=c.bind(c.ax,'AXUIElementPerformAction',c.C.c_int,c.ptr,c.ptr);name=c.string_new(None,b'AXRaise',0x08000100)
    try:record['raise_error']=fn(child,name)
    finally:c.release(name)
    raised=True;break
  if not raised:raise RuntimeError('No AXWindow child to raise')
 finally:
  if children:c.release(children)
finally:c.release(app)
if a.action in ('navigate','back','back-button'):
 post_to_pid=c.bind(c.cg,'CGEventPostToPid',None,c.C.c_int,c.ptr)
 c.post=lambda location,event: post_to_pid(w['kCGWindowOwnerPID'],event)
 record['event_routing']='observed-native-host-pid'
else:record['event_routing']='global-native-chrome'
time.sleep(.5);record['onscreen_after_raise']=c.windows(a.pid);(a.output/(a.action+'-raise.json')).write_text(json.dumps(record,indent=2)+'\n')
sys.argv=['control','--pid',str(a.pid),'--output',str(a.output),'--action',a.action,'--settle-seconds','1']
if a.folder:sys.argv+=['--folder',str(a.folder)]
c.main()
image=a.output/(a.action+'.png')
if image.exists():
 width,height=struct.unpack('>II',image.read_bytes()[16:24]);print('PNG_DIMENSIONS='+str(width)+'x'+str(height),flush=True)
 if width<640 or height<400:raise RuntimeError('Capture is not a full application window')
