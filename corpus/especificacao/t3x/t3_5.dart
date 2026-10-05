void f(bool x) {
  const a = bool.fromEnvironment(x ? 'a' : 'b');
  print(a);
}
const g = String.fromEnvironment('k', defaultValue: null);
