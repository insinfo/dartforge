import 'v01.dart' as prefix;
class Class { const Class(); const Class.named(); }
class GenericClass<T> { const GenericClass(); }
const k1 = prefix.Class.named;
const k2 = GenericClass<int>.new;
const k3 = prefix.GenericClass<int>.new;
void f(Object o) {
  switch (o) {
    case prefix.Class.named:
    case GenericClass<int>.new:
    case const (prefix.GenericClass<int>.new):
  }
}
