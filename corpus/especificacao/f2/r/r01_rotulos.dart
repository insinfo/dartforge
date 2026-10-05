void f(int x) {
  naoUsado: while (true) { break; }
  usadoBreak: while (true) { break usadoBreak; }
  usadoContinue: for (;;) { continue usadoContinue; }
  bloco: { print(1); }
  a: b: while (true) { break b; }
  fora: while (true) {
    dentro: while (true) { break fora; }
  }
  switch (x) {
    c0: case 0:
      continue c1;
    c1: case 1:
      break;
    c2: default:
      break;
  }
  repetido: while (true) {
    repetido: while (true) { break repetido; }
  }
  closure: while (true) {
    () { break closure; };
    break;
  }
  switch (x) { case 1: interno: print(2); break; }
}
