// Atribuição composta e incremento num membro estático alcançado por prefixo
// de import (`p.C.x op= v`, `p.C.x++`, `++p.C.x`, `p.C.x ??= v`), e na
// variável top-level prefixada.
import 'biblioteca.dart' as a;

void main() {
  a.Ponto.criados++;
  a.Ponto.criados += 5;
  ++a.Ponto.criados;
  print(a.Ponto.criados);
  print(a.Ponto.criados++);
  print(--a.Ponto.criados);
  print(a.Ponto.talvez ??= 4);
  print(a.Ponto.talvez ??= 5);
  a.contador++;
  a.contador += 10;
  print(a.contador);
}
