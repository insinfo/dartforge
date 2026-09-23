Stream<int> f() async* {
  yield* g();
}

g() => throw 0;
