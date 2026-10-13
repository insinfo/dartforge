int _nunca = 0;
int _soEscrita = 0;
int _lida = 0;
int _incremento = 0;
int _composta = 0;
int? _seNula;
int _incEmExpr = 0;
int get _g => 0;
set _s(int v) {}
int get _g2 => 0;
set _s2(int v) {}
int get _par => 0;
set _par(int v) {}
void main() {
  _soEscrita = 1;
  print(_lida);
  _incremento++;
  _composta += 2;
  _seNula ??= 3;
  print(_incEmExpr++);
  print(_g);
  _s = 1;
  _par += 1;
}
