@deprecated
void velha() {}
@Deprecated('Use nova')
void comMensagem() {}
@Deprecated('Use nova.')
void comPonto() {}
@Deprecated('  ')
void comVazia() {}
class C {
  @deprecated
  C();
  @deprecated
  C.n();
  @deprecated
  int campo = 0;
  @deprecated
  int get g => 0;
  @deprecated
  set s(int v) {}
  @deprecated
  int operator +(int o) => 0;
  @deprecated
  int call() => 0;
  void m({@deprecated int? p, @Deprecated('nao') int? q}) {}
}
@deprecated
class Velha {}
@deprecated
const velhaConst = 1;
