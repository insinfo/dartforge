// Instanciação explícita de qualquer valor de função genérica (`f[0]<int>`,
// `o.m<String>`, `5.wrap<String>`, `(c ? id : id)<double>`): uma closure com
// o tipo instanciado, igual a outra instanciação da mesma função com os mesmos
// argumentos; typedefs como literais de tipo, com e sem argumentos; `is Never`.
typedef F1 = int Function(int);
typedef Direct<T> = Map<T, String>;
typedef G<T> = T Function(T);
T id<T>(T x) => x;
class C {
  T m<T>(T x) => x;
  void probe() {
    var f = m<int>;
    print(f(3));
  }
}
extension E on int {
  List<T> wrap<T>(T x) => [x];
}
void main() {
  print(F1);
  print(Direct);
  print(Direct<int>);
  print(G<String>);
  var fs = <T Function<T>(T)>[id, id];
  var a = fs[0]<int>;
  var b = fs[1]<int>;
  print(a(1));
  print(a.runtimeType);
  print(a == b);
  print(fs[0]<int> == fs[1]<String>);
  print(a.hashCode == b.hashCode);
  print(a);
  var o = C();
  var t = o.m<String>;
  print(t('s'));
  print(t.runtimeType);
  o.probe();
  var w = 5.wrap<String>;
  print(w('x'));
  dynamic d = (bool c) => (c ? id : id)<double>;
  print(d(true)(1.5));
  print(null is Never);
  print(1 is Never);
}
