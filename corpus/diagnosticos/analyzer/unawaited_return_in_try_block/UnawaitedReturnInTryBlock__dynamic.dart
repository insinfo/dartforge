Future<int> foo(dynamic v) async {
  try {
    return v;
  } catch (_) {}
  return v;
}
