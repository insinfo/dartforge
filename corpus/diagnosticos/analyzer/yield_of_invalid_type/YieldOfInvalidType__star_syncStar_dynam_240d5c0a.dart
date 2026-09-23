f() sync* {
  yield* g();
}

g() => throw 0;
