f() async* {
  yield* g();
}

Stream<int> g() => throw 0;
