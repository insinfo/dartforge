class A {
  int get _soGetterLido => 0;
  int get _soGetterNaoLido => 0;
  set _soSetterEscrito(int v) {}
  set _soSetterNaoEscrito(int v) {}
  int get _parEscrito => 0;
  set _parEscrito(int v) {}
  int get _parComposto => 0;
  set _parComposto(int v) {}
  int get _parLido => 0;
  set _parLido(int v) {}
  void m() {
    print(_soGetterLido);
    _soSetterEscrito = 1;
    _parEscrito = 2;
    _parComposto += 3;
    print(_parLido);
  }
}
