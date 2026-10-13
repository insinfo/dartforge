import os,sys,json,subprocess
# '~' no texto vira barra invertida no arquivo .dart
C = {
 'a01': "void f() {\n  var s = 'abc;\n  print(s);\n}\n",
 'a02': 'void f(a) {\n  var s = "x${a;\n}\n',
 'a03': 'var s = "${a',
 'a04': "void f() {}\n/* abc\n",
 'a05': "var a = 0x;\n",
 'a06': "var a = 1e;\n",
 'a07': "var a = 1e+;\n",
 'a08': "var a = 1.e;\n",
 'a09': "void f(a, b) {\n  a === b;\n  a !== b;\n}\n",
 'a10': "void f() {\n  print(1;\n}\n",
 'a11': "void f() {\n  if (true) {\n    print(1);\n}\n",
 'a12': "var a = [ ( ];\n",
 'a13': "var a = 1 < ;\n",
 'a14': "List<int a;\n",
 'a15': "var a = \u00a7;\n",
 'a16': "var a = \U0001F600;\n",
 'a17': "var a\u00e9 = 1;\n",
 'a18': "var a =\u00a01;\n",
 'a19': "var a = \x01 1;\n",
 'a20': "var a = '~u';\n",
 'a21': "var a = '~x1';\n",
 'a22': "var a = '~u{110000}';\n",
 'a23': "var a = '~u{}';\nvar b = '~u12';\nvar c = '~u{1234567}';\nvar d = '~xZZ';\nvar e = '~u{12';\n",
 'a24': "void f() {\n  print('abc~\n');\n}\n",
 'a25': 'var a = "$";\nvar b = "$1";\nvar c = "$$a";\n',
 'a26': "var a = '''abc\nvar b = 1;\n",
 'a27': "var a = r'abc\nvar b = 1;\n",
 'a28': "var a = 1_000;\nvar b = 100_;\nvar c = 0x_1;\nvar d = 1_.5;\nvar e = 1e_5;\nvar g = 1__0;\n",
 'a29': "void f() { } }\n",
 'a30': "void f() { print(1)); }\n",
 'a31': "void f() { g([ (",
 'a32': "void f() { var a = [1; }\n",
 'a33': "var a = 0x",
 'a34': "var a = 1e",
 'a35': "var a = 'abc",
 'a36': "var a = 1;\n#!shebang\n",
 'a37': "\ufeffvar a = \u00a7;\n",
 'a38': "var a = \"a ${ 'b }\";\nvar c = 2;\n",
 'a39': 'var a = "$this $class";\n',
 'a40': "var a = 'abc~",
 'a41': "/* a /* b */ c\nvar a = 1;\n",
 'a42': "var piskefl\u00f8de = 1;\nvar a\U0001F600b = 2;\n",
 'a43': "void f() { var m = {'a': [1, 2}; }\n",
 'a44': "var a = [(])];\n",
 'a45': 'var s = "${a";\nvar t = 1;\n',
 'a46': "var a = \ufffd;\n",
 'a47': "var a = 1.5e;\nvar b = .e;\nvar c = 1.5e-;\n",
 'a48': "class C { operator ===(x) => true; }\n",
 'a49': "var a = 'ab\ncd';\n",
 'a50': "void f() {\n  var x = (1 + [2, 3;\n  print(x);\n}\n",
 'a51': "var a = 0xg;\nvar b = 0X;\n",
 'a53': "void f() { print(\"${1 + }\"); }\n",
 'a54': "var a = 1 ~ 2;\nvar b = `x`;\n",
 'a55': "void f() {\r\n  var s = 'abc;\r\n}\r\n",
 'a56': "var a = '\u00e9~x1';\nvar b = '''\n~u{110000}''';\n",
 'a57': "#!/usr/bin/env dart\nvar a = 0x;\n",
 'a58': "var a = '~",
 'a59': "var a = \"x $a ~u\";\nvar b = '${a}~x1 ~u{}';\n",
 'a60': "var a = r'~u ~x1';\nvar b = '~q ~$';\n",
}
C.update(json.load(open('extra.json',encoding='utf-8')) if os.path.exists('extra.json') else {})
only = sys.argv[1:]
env = dict(os.environ); env['ANALYZER_STATE_LOCATION_OVERRIDE']=r'E:\dftemp\analise\spec-r4\dartstate'
out=open('saida.txt','a' if only else 'w',encoding='utf-8')
for k,v in C.items():
    if only and k not in only: continue
    v = v.replace('~', chr(92))
    with open(k+'.dart','wb') as f: f.write(v.encode('utf-8'))
    r = subprocess.run([r'C:\tools\dartsdk-3.6.2\bin\dart.exe','analyze','--format=json',k+'.dart'],capture_output=True,env=env)
    txt = r.stdout.decode('utf-8',errors='replace')
    out.write('== %s %r\n' % (k, v))
    j=None
    for line in txt.splitlines():
        if line.startswith('{"version"'):
            j=json.loads(line)
    if j is None:
        out.write('  SEM JSON: %r %r\n' % (txt[:600], r.stderr.decode('utf-8',errors='replace')[:1500]))
        continue
    for d in j['diagnostics']:
        s=d['location']['range']['start']; e=d['location']['range']['end']
        out.write('  %s off=%d len=%d %d:%d | %s\n' % (d['code'], s['offset'], e['offset']-s['offset'], s['line'], s['column'], d['problemMessage']))
    out.flush()
