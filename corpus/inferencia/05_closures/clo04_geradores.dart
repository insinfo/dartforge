// R-CLO-04: sync* dá Iterable<UP dos yield>, async* dá Stream<...>;
// `return;` em gerador (a regra mudou em 2024-12).
void main() {
  var a = /*@*/() sync* {
    yield 1;
  };
  var b = /*@*/() async* {
    yield 1;
  };
  var c = /*@*/() sync* {};
  var d = /*@*/() sync* {
    yield 1;
    return;
  };
  var e = /*@*/() sync* {
    yield* [1.5];
  };
  var f = /*@*/() async* {
    yield* Stream.value('a');
  };
  print([a, b, c, d, e, f]);
}
