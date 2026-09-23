f() async* {
  yield* g();
}

Stream g() => throw 0;
