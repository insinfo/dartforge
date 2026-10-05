import os
d = os.path.dirname(os.path.abspath(__file__))
casos = {
'g01': "class A<T> { const A(); }\n@A<List<int>> class C {}\n",
'g03': "f(x) { switch (x) { case 1: break; default: break; case 2: break; default: } }\n",
'g04': "f(g, a, b) { g(a, b }\n",
'g05': "f() { return return; }\n",
'g07': "f(a) { a..b?..c; }\n",
'g09': "f(a) { a.b c; }\n",
'g10': "f() { if (true) return else return; }\n",
'g13': "typedef R = (int);\ntypedef S = ({});\nf((int) p, ({}) q) {}\n",
'g14': "class A<T> { const A(); }\nf() { @A<int> var x = 1; }\n",
'g15': "f(x) { switch (x) { case 1: continue L; } }\n",
'g16': "f() { do { continue; } while (false); L: for (;;) { continue L; } }\n",
'g17': "f() { var x = 1 }\n",
'g18': "f(a) { a = 1 = 2; 1 = a; a.b() = 1; }\n",
'g19': "f() { for (var i = 0; i < 1; i++) { break; continue; } while (true) { if (true) { break; } } }\n",
'g20': "class A { A() : super(), this.x = 1; var x; A.n() : this.m(); A.m(); A.o() : super.new(); }\n",
}
for k, v in casos.items():
    open(os.path.join(d, 'r3', k + '.dart'), 'w', newline='\n').write(v)
