// R-FLU-14: padrões em `if-case` e `switch` promovem o escrutínio variável.
void f(Object o, int? x) {
  if (o case int i) print([/*@*/i, /*@*/o]);
  if (x case var v?) print([/*@*/v, /*@*/x]);
  switch (o) {
    case String s:
      print([/*@*/s, /*@*/o]);
    default:
  }
  if (x case != null) print(/*@*/x);
}

void main() => f('a', 1);
