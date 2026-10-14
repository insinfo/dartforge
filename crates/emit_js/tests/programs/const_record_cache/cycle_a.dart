import 'cycle_b.dart' as b;

Object record() {
  const value = (4, name: 'cyclic');
  return value;
}
int value() => b.fromA();
