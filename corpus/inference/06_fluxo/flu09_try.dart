// R-FLU-09: try/catch/finally: no catch e no finally valem só as promoções
// que sobrevivem a qualquer ponto do try.
int? g() => 1;
void f(int? x, int? y) {
  if (x == null || y == null) return;
  try {
    print(/*@*/x);
    y = g();
  } catch (e) {
    print(/*@*/x);
    print(/*@*/y);
    print(/*@*/e);
  } finally {
    print(/*@*/y);
  }
  try {
    print(0);
  } on FormatException catch (e, s) {
    print([/*@*/e, /*@*/s]);
  }
}

void main() => f(1, 2);
