import 'h06.dart' as prefix;
void f1<T extends num>(T t) {}
const c01 = f1;
class Cl<T extends num> {
  void test<Z extends num>() {
    const c03 = f1<Z>;
    const c04 = prefix.f1<Z>;
    const c07 = prefix.c01<Z>;
    const c08 = c01<T>;
    const void Function(Z) c09 = f1;
    const void Function(T) c10 = prefix.f1;
    const void Function(Z) c11 = c01;
    const c12 = f1<List<Z>>;
    print([c03, c04, c07, c08, c09, c10, c11, c12]);
  }
}
