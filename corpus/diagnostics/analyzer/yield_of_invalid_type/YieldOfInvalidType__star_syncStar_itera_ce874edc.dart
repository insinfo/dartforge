f() sync* {
  yield* g();
}

Iterable g() => throw 0;
