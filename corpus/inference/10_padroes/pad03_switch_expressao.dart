// R-PAD-03: expressão switch: UP dos braços; variáveis dos padrões.
sealed class F {}

class Q extends F {}

class R extends F {}

void f(Object o, F fo) {
  var r = /*@*/switch (o) {
    int i => /*@*/i,
    String s => s.length.toDouble(),
    _ => 0,
  };
  var t = /*@*/switch (fo) {
    Q() => 1,
    R() => 'r',
  };
  print([r, t]);
}

void main() => f(1, Q());
