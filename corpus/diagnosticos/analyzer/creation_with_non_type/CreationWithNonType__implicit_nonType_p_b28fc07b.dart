import 'test.dart' as prefix;

void NonType<T>() {}
f() {
  prefix.NonType<int>();
}
