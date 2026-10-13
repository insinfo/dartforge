// R-GEN-14: inferência horizontal em cadeia: as fases seguem as dependências
// (c fixa T; b depende de c e fixa U; a depende de b), não a ordem do texto.
V h<T, U, V>(V Function(U) a, U Function(T) b, T c) => a(b(c));
void main() {
  var r = /*@*/h((u) => (/*@*/u).isEven, (t) => (/*@*/t) * 2, 1);
  print(r);
}