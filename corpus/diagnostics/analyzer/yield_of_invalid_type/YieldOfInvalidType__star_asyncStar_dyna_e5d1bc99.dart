Stream f() async* {
  yield* g();
}

g() => throw 0;
