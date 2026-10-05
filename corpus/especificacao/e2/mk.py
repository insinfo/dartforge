import sys,os,re
# formato: linhas "=== nome" seguidas do conteúdo
cur=None;buf=[]
def flush():
    if cur:
        open(cur+'.dart','w',newline='\n').write(''.join(buf))
for line in open(sys.argv[1],encoding='utf-8'):
    if line.startswith('=== '):
        flush(); cur=line[4:].strip(); buf=[]
    else: buf.append(line)
flush()
