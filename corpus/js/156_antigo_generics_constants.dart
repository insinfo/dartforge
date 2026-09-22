// Convertido de tests/conformance/cases/generics_constants.dart (fixture antigo do corpus de conformidade).
// Const listas equivalentes precisam compartilhar identidade; listas normais não.
T identity<T>(T value) { return value; }
List<T> singleton<T>(T value) { return <T>[value]; }
T first<T>(List<T> values) { return values[0]; }
void main() {
  print(identity<int>(7));
  print(identity('inferred'));
  print(first<int>(singleton<int>(9)));
  print(first(singleton('nested')));
  const int two = 1 + 1;
  const String label = 'con' + 'stant';
  const a = <int>[1, two];
  const b = <int>[1, 2];
  const nested = <List<int>>[<int>[1, 2]];
  print(label);
  print(a == b);
  print(a == nested[0]);
  print(a == <int>[1, 2]);
  print(const <int>[] == const <String>[]);
  print(identity(a) == b);
  print(a[1]);
  const bool lazy = false && (1 % 0 == 0);
  print(lazy);
  print(-5 % -3);
}
