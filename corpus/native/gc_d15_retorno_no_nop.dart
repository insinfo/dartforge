// §14.7 (a regra do `nop`): no Windows x64 o gerador de código põe um `nop`
// depois do `call` que precede o epílogo, e o registro do mapa aponta depois
// dele. A closure que devolve o `T` reificado termina numa chamada assim, e
// com `--gc-stress` a coleta dentro dela acha o quadro com o retorno no
// `nop`. Sabotagem que o derruba: o leitor sem a regra (`sem_nop` no
// runtime) recusa o quadro e o processo aborta.

class C<T> {
  Type viaFn() => (() => T)();
}

void main() {
  print(C<double>().viaFn());
  print(C<String>().viaFn());
}
