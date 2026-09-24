Iterable<int> f() sync* {
  yield* g();
}

Iterable<int> g() => throw 0;
