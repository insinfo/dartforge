void f(int x, bool c) {
  naoUsado: while (c) { break; }
  usadoBreak: while (c) { break usadoBreak; }
  usadoContinue: for (; c;) { continue usadoContinue; }
  bloco: { print(1); }
  a: b: while (c) { break b; }
  fora: while (c) {
    dentro: while (c) { break fora; }
  }
  switch (x) {
    c0: case 0:
      continue c1;
    c1: case 1:
      break;
    c2: default:
      break;
  }
  repetido: while (c) {
    repetido: while (c) { break repetido; }
  }
  switch (x) { case 1: interno: print(2); break; }
  semAlvo: while (c) { break naoExiste; }
}
