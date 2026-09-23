// R-UP-05: UP com variáveis de tipo: substitui pelo limite quando não é
// subtipo do outro lado.
void f<T extends num, U extends T>(T t, U u, bool b) {
  var p = /*@*/b ? t : 1;
  var q = /*@*/b ? t : u;
  var r = /*@*/b ? u : 'a';
  var s = /*@*/b ? t : null;
  print([p, q, r, s]);
}

void main() => f<int, int>(1, 2, true);
