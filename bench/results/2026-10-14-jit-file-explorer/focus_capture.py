"""Raise the owned test window and capture it without navigating or editing UI."""
from pathlib import Path
import argparse,importlib.util,sys
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--pid',type=int,required=True)
parser.add_argument('--window-id',type=int,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
root=Path(__file__).resolve().parents[3]
helper=root/'bench/results/2026-10-14-primitive-sdk-production/file-explorer-v3/gui/ordinary_window_control.py'
spec=importlib.util.spec_from_file_location('window_control',helper)
control=importlib.util.module_from_spec(spec);spec.loader.exec_module(control)
windows=control.windows(args.pid)
assert len(windows)==1 and windows[0]['kCGWindowNumber']==args.window_id
control.focus(windows[0])
control.require_same_window(args.pid,windows[0])
sys.argv=[str(helper),'--pid',str(args.pid),'--output',str(args.output),'--action','capture','--settle-seconds','2']
control.main()
