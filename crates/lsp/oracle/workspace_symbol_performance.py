"""Measure workspace/symbol with full responses and macOS process-tree resources.

Usage: ORACLE_DART=... ORACLE_SDK_LIB=... ORACLE_DART_CACHE=... python3
       workspace_symbol_performance.py <server-binary|dart> <project> <output.jsonl>

Run each server separately, rotating their order over independent sessions.
Both receive the same client capabilities, open document and eight queries.
Round 0 includes first-query index construction; rounds 1 and 2 are warm.
Preparation resolves the anchor and observes ten additional idle seconds.

CPU includes the server, live descendants and waited-for children. Memory is
physical footprint and RSS at request boundaries, not a simultaneous tree peak.
No watcher is advertised. Complete symbol lists are retained for strict parity;
timing alone never establishes correctness. This probe requires macOS.
"""
import argparse
import ctypes
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from oracle import CAPABILITIES, DART, DART_CACHE, SDK_LIB, Lsp, settle_initial_analysis, wait_all_idle

if sys.platform != "darwin":
    raise SystemExit("process-tree resource measurements require macOS")


class Usage(ctypes.Structure):
    # rusage_info_v4 from sys/resource.h, matching editing_latency.rs.
    _fields_ = [('uuid', ctypes.c_ubyte * 16), ('counters', ctypes.c_uint64 * 35)]


class Timebase(ctypes.Structure):
    _fields_ = [('numer', ctypes.c_uint32), ('denom', ctypes.c_uint32)]


lib = ctypes.CDLL('/usr/lib/libproc.dylib', use_errno=True)
lib.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.POINTER(Usage)]
system = ctypes.CDLL('/usr/lib/libSystem.B.dylib')
system.mach_timebase_info.argtypes = [ctypes.POINTER(Timebase)]
ratio = Timebase()
if system.mach_timebase_info(ctypes.byref(ratio)) != 0 or not ratio.denom:
    raise RuntimeError('cannot read the Mach clock timebase')


def resources(pid):
    """Read all live descendants plus CPU already reaped by those processes.

    Fail on a disappearing process instead of silently reporting a partial tree.
    Counters 0/1 and 10/11 are own and waited-child user/system Mach ticks;
    counters 6/7 are resident size and physical footprint in bytes.
    """
    pairs = [tuple(map(int, line.split())) for line in subprocess.check_output(['ps', '-axo', 'pid=,ppid='], text=True).splitlines()]
    live = {pid}
    while True:
        children = {child for child, parent in pairs if parent in live}
        if children <= live:
            break
        live |= children
    cpu, footprint, rss = 0, 0, 0
    for member in live:
        value = Usage()
        if lib.proc_pid_rusage(member, 4, ctypes.byref(value)) != 0:
            raise RuntimeError(f'cannot sample {member}: errno={ctypes.get_errno()}')
        counters = value.counters
        cpu += (counters[0] + counters[1] + counters[10] + counters[11]) * ratio.numer / ratio.denom / 1e6
        footprint += counters[7] / 1048576
        rss += counters[6] / 1048576
    return dict(cpu_ms=cpu, footprint_mib=footprint, rss_mib=rss, processes=len(live))

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("binary", help="server executable, or dart for the reference server")
parser.add_argument("project", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--profile", type=Path, help="diagnostic sample output; exclude this run from timing comparisons")
args = parser.parse_args()
binary, output = args.binary, args.output
root = args.project.resolve()
files = sorted(p for folder in ('lib','bin') for p in (root/folder).rglob('*.dart'))
if not files:
    parser.error(f'project has no Dart files: {root}')
anchor = files[0]
if binary == 'dart':
    command = [DART, 'language-server', '--protocol=lsp', '--client-id=oracle']
    if DART_CACHE:
        command.append('--cache=' + DART_CACHE)
else:
    command = [str(Path(binary).resolve()), '--stdio']
server = Lsp('server', command, env=dict(os.environ, DARTFORGE_SDK_LIB=SDK_LIB))
try:
    init, _ = server.request('initialize', {'processId':os.getpid(),'rootUri':root.as_uri(), 'workspaceFolders':[{'uri':root.as_uri(),'name':root.name}], 'capabilities':CAPABILITIES,'initializationOptions':{'onlyAnalyzeProjectsWithOpenFiles':True}})
    if not isinstance(init, dict) or 'capabilities' not in init:
        raise RuntimeError(f'initialize failed: {init}')
    server.notify('initialized', {})
    server.notify('textDocument/didOpen', {'textDocument':{'uri':anchor.as_uri(),'languageId':'dart','version':1,'text':anchor.read_text(encoding='utf-8')}})
    if not server.wait_analysis(limit=180):
        raise RuntimeError('initial analysis timeout')
    time.sleep(1)
    readiness = settle_initial_analysis([server], [anchor], limit=180)
    # Give background setup a longer quiet interval, identical for both peers.
    if not wait_all_idle([server], limit=180, quiet=10):
        raise RuntimeError('additional idle interval timed out')
    with output.open('w', encoding='utf-8') as stream:
        stream.write(json.dumps({'method':'initialize','binary':binary,'rootUri':root.as_uri(),'anchor':anchor.as_uri(),'readiness':readiness,'additionalQuietSeconds':10,'sdkLib':SDK_LIB,'referenceCache':DART_CACHE,'profiled':args.profile is not None,'resources':resources(server.p.pid)})+'\n')
        profiler = None
        if args.profile:
            profiler = subprocess.Popen(['sample', str(server.p.pid), '8', '-file', str(args.profile)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for round_ in range(3):
            for query in ('Arg','parse','Scanner','ctx','Ponto','join','a','e'):
                before = resources(server.p.pid)
                result, ms = server.request('workspace/symbol', {'query':query}, limit=180)
                after = resources(server.p.pid)
                if not isinstance(result, list):
                    raise RuntimeError(f'{query}: symbol request failed: {result}')
                stream.write(json.dumps({'method':'workspace/symbol','round':round_,'query':query,'ms':ms,'cpu_ms':after['cpu_ms']-before['cpu_ms'],'before':before,'after':after,'result':result})+'\n')
                stream.flush()
                print(round_,query,f'{ms:.1f} ms',f"{after['cpu_ms']-before['cpu_ms']:.1f} cpu ms",len(result),flush=True)
        if not wait_all_idle([server], limit=180, quiet=1):
            raise RuntimeError('final idle interval timed out')
        stream.write(json.dumps({'method':'oracle/performanceRest','resources':resources(server.p.pid)})+'\n')
    if profiler is not None:
        if profiler.wait(timeout=20) != 0:
            raise RuntimeError('diagnostic sampling failed')
finally:
    server.close()
