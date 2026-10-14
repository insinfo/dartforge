#!/usr/bin/env python3
"""Inspect/capture and control the ordinary explorer's native host window.

Attaches to an already launched original entrypoint. It never sends host protocol
commands, changes app sources, or performs explored-file mutations. Screenshot
and accessibility observations require explicit interpretation by the caller.
"""
import argparse
import ctypes as C
import json
import subprocess
import time
from pathlib import Path

cf = C.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
cg = C.CDLL('/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics')
ax = C.CDLL('/System/Library/Frameworks/ApplicationServices.framework/ApplicationServices')

def bind(lib, name, restype, *args):
    fn = getattr(lib, name)
    fn.restype, fn.argtypes = restype, list(args)
    return fn

ptr = C.c_void_p
release = bind(cf, 'CFRelease', None, ptr)
type_id = bind(cf, 'CFGetTypeID', C.c_ulong, ptr)
array_type = bind(cf, 'CFArrayGetTypeID', C.c_ulong)
dict_type = bind(cf, 'CFDictionaryGetTypeID', C.c_ulong)
string_type = bind(cf, 'CFStringGetTypeID', C.c_ulong)
number_type = bind(cf, 'CFNumberGetTypeID', C.c_ulong)
boolean_type = bind(cf, 'CFBooleanGetTypeID', C.c_ulong)
array_count = bind(cf, 'CFArrayGetCount', C.c_long, ptr)
array_item = bind(cf, 'CFArrayGetValueAtIndex', ptr, ptr, C.c_long)
dict_count = bind(cf, 'CFDictionaryGetCount', C.c_long, ptr)
dict_items = bind(cf, 'CFDictionaryGetKeysAndValues', None, ptr, ptr, ptr)
string_get = bind(cf, 'CFStringGetCString', C.c_bool, ptr, ptr, C.c_long, C.c_uint)
string_new = bind(cf, 'CFStringCreateWithCString', ptr, ptr, C.c_char_p, C.c_uint)
number_get = bind(cf, 'CFNumberGetValue', C.c_bool, ptr, C.c_int, ptr)
number_float = bind(cf, 'CFNumberIsFloatType', C.c_bool, ptr)
boolean_get = bind(cf, 'CFBooleanGetValue', C.c_bool, ptr)


def decode(value):
    if not value:
        return None
    kind = type_id(value)
    if kind == string_type():
        buffer = C.create_string_buffer(65536)
        if not string_get(value, buffer, len(buffer), 0x08000100):
            return '<unrepresentable string>'
        return buffer.value.decode('utf-8')
    if kind == number_type():
        result = C.c_double() if number_float(value) else C.c_longlong()
        number_get(value, 13 if number_float(value) else 4, C.byref(result))
        return result.value
    if kind == boolean_type():
        return bool(boolean_get(value))
    if kind == array_type():
        return [decode(array_item(value, i)) for i in range(array_count(value))]
    if kind == dict_type():
        count = dict_count(value)
        keys, values = (ptr * count)(), (ptr * count)()
        dict_items(value, keys, values)
        return {decode(keys[i]): decode(values[i]) for i in range(count)}
    return '<native object>'

window_list = bind(cg, 'CGWindowListCopyWindowInfo', ptr, C.c_uint, C.c_uint)


def descendants(pid):
    rows = subprocess.check_output(['ps', '-axo', 'pid=,ppid=']).decode().splitlines()
    relations = [tuple(map(int, row.split())) for row in rows if len(row.split()) == 2]
    found = {pid}
    while True:
        added = {child for child, parent in relations if parent in found} - found
        if not added:
            return found
        found.update(added)


def windows(pid):
    value = window_list(17, 0)
    try:
        owners = descendants(pid)
        return [item for item in decode(value) if item.get('kCGWindowOwnerPID') in owners
                and item.get('kCGWindowBounds', {}).get('Width', 0) >= 640
                and item.get('kCGWindowBounds', {}).get('Height', 0) >= 400]
    finally:
        if value:
            release(value)

ax_app = bind(ax, 'AXUIElementCreateApplication', ptr, C.c_int)
ax_attr = bind(ax, 'AXUIElementCopyAttributeValue', C.c_int, ptr, ptr, C.POINTER(ptr))


def attribute(element, name):
    key, result = string_new(None, name.encode(), 0x08000100), ptr()
    try:
        error = ax_attr(element, key, C.byref(result))
        return error, result.value
    finally:
        release(key)


def accessibility(pid):
    application = ax_app(pid)
    failures, visited = [], [0]
    def walk(element, depth):
        visited[0] += 1
        result = {}
        for name in ('AXRole', 'AXTitle', 'AXDescription', 'AXValue'):
            error, value = attribute(element, name)
            if value:
                try:
                    result[name] = decode(value)
                finally:
                    release(value)
            elif error not in (0, -25205, -25212):
                failures.append({'attribute': name, 'error': error})
        if depth < 12 and visited[0] < 2000:
            error, children = attribute(element, 'AXChildren')
            if children:
                try:
                    result['children'] = [walk(array_item(children, i), depth + 1)
                                          for i in range(array_count(children))]
                finally:
                    release(children)
        return result
    try:
        return {'tree': walk(application, 0), 'errors': failures}
    finally:
        release(application)

class Point(C.Structure):
    _fields_ = [('x', C.c_double), ('y', C.c_double)]
mouse_event = bind(cg, 'CGEventCreateMouseEvent', ptr, ptr, C.c_uint, Point, C.c_uint)
key_event = bind(cg, 'CGEventCreateKeyboardEvent', ptr, ptr, C.c_ushort, C.c_bool)
ax_set = bind(ax, 'AXUIElementSetAttributeValue', C.c_int, ptr, ptr, ptr)
flags_set = bind(cg, 'CGEventSetFlags', None, ptr, C.c_ulonglong)
text_set = bind(cg, 'CGEventKeyboardSetUnicodeString', None, ptr, C.c_ulong, ptr)
post = bind(cg, 'CGEventPost', None, C.c_uint, ptr)


def key(code, flags=0, text=None):
    for down in (True, False):
        event = key_event(None, code, down)
        if not event:
            raise RuntimeError('Cannot create native keyboard event')
        try:
            flags_set(event, flags)
            if text is not None:
                data = text.encode('utf-16-le')
                buffer = C.create_string_buffer(data)
                text_set(event, len(data) // 2, buffer)
            post(1, event)
        finally:
            release(event)
    time.sleep(0.15)


def focus(window):
    # A click alone need not activate a background host. Verify the native
    # application's foreground state before sending keyboard input.
    application = ax_app(window['kCGWindowOwnerPID'])
    name = string_new(None, b'AXFrontmost', 0x08000100)
    try:
        yes = ptr.in_dll(cf, 'kCFBooleanTrue').value
        error = ax_set(application, name, yes)
        if error:
            raise RuntimeError(f'Cannot activate actual host application: AX error {error}')
        time.sleep(0.3)
        error, value = attribute(application, 'AXFrontmost')
        try:
            if error or not value or not decode(value):
                raise RuntimeError('Actual host application did not become frontmost')
        finally:
            if value:
                release(value)
    finally:
        release(name)
        release(application)


def click(window, x, y):
    # CGWindowBounds and CGEvent locations are logical screen points; Retina
    # screenshot pixels are deliberately not used as event coordinates.
    bounds = window['kCGWindowBounds']
    point = Point(bounds['X'] + x, bounds['Y'] + y)
    for kind in (5, 1, 2):
        event = mouse_event(None, kind, point, 0)
        if not event:
            raise RuntimeError('Cannot create native pointer event')
        try:
            post(1, event)
        finally:
            release(event)
        time.sleep(0.05)


def require_same_window(pid, expected):
    actual = windows(pid)
    if len(actual) != 1 or actual[0]['kCGWindowNumber'] != expected['kCGWindowNumber']:
        raise RuntimeError(f'Actual host window changed; expected {expected}, found {actual}')
    return actual[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--pid', required=True, type=int)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--action', choices=['capture', 'navigate', 'back', 'close', 'back-button', 'close-button'], default='capture')
    parser.add_argument('--folder', type=Path)
    parser.add_argument('--settle-seconds', type=float, default=1)
    args = parser.parse_args()
    if not 0 <= args.settle_seconds <= 10:
        parser.error('--settle-seconds must be between 0 and 10')
    args.output.mkdir(parents=True, exist_ok=True)
    selected = windows(args.pid)
    if len(selected) != 1:
        raise RuntimeError(f'Expected one actual host window, found {selected}')
    window = selected[0]
    if args.action != 'capture':
        focus(window)
    if args.action != 'capture':
        window = require_same_window(args.pid, window)
    if args.action == 'navigate':
        if args.folder is None or not args.folder.is_dir():
            parser.error('--folder must name an existing navigation directory')
        key(37, 1 << 20)  # Cmd+L: original explorer address command.
        key(0, text=str(args.folder.resolve()))
        key(36)  # Return submits the ordinary address field.
    elif args.action == 'back':
        key(123, 1 << 19)  # Alt+Left: original explorer Back command.
    elif args.action == 'back-button':
        click(window, 25, 65)
    elif args.action == 'close-button':
        click(window, 16, 20)
    elif args.action == 'close':
        key(13, 1 << 20)  # Cmd+W: host Window/Close -> windowShouldClose.
    if args.action not in ('close', 'close-button'):
        time.sleep(args.settle_seconds)
        window = require_same_window(args.pid, window)
        subprocess.run(['screencapture', '-x', '-l' + str(window['kCGWindowNumber']),
                        str(args.output / (args.action + '.png'))], check=True)
    observation = {'application_pid': args.pid, 'action': args.action, 'window': window,
                   'accessibility': accessibility(window['kCGWindowOwnerPID'])
                   if args.action not in ('close', 'close-button') else None,
                   'recorded_at_epoch': time.time()}
    (args.output / (args.action + '.json')).write_text(json.dumps(observation, indent=2) + '\n')
    print(json.dumps({'action': args.action, 'window_id': window['kCGWindowNumber'],
                      'host_pid': window['kCGWindowOwnerPID'], 'output': str(args.output),
                      'application_alive': Path(f'/proc/{args.pid}').exists() if Path('/proc').exists()
                      else subprocess.run(['kill', '-0', str(args.pid)], capture_output=True).returncode == 0}))

if __name__ == '__main__':
    main()
