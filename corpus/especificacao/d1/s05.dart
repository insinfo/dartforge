class C<T> {
  final t;
  const C(dynamic x) : t = x as List<T>;
}
main() {
  const C<int>(<int>[]);
  const C<int>(<num>[]);
  const C<int>(null);
}
