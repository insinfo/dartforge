import sys, os
src = sys.argv[1]
name = None
buf = []
def flush():
    if name:
        with open(os.path.join(os.path.dirname(os.path.abspath(src)), name + '.dart'), 'w', newline='\n') as f:
            f.write(''.join(buf))
for line in open(src, encoding='utf-8'):
    if line.startswith('=== '):
        flush()
        name = line[4:].strip()
        buf = []
    else:
        buf.append(line)
flush()
