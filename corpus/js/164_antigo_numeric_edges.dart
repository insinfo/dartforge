// Convertido de tests/conformance/cases/numeric_edges.dart (fixture antigo do corpus de conformidade).
// diverge-ddc: int de 64 bits e -0.0 só existem na VM; na web int é double de 53 bits
void main() {
  var zero = 0;
  print(-zero);
  print(-0);
  print(0);
  var max = 2147483647;
  print(max * max);
  print(max * max * max);
}
