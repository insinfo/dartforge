// Convertido de tests/conformance/cases/iterable_format.dart (fixture antigo do corpus de conformidade).
// Fixture original: abreviação, efeitos preguiçosos e ordem de formatação aninhada.
void showCount(int length) {
  List<int> values = <int>[];
  for (var i = 0; i < length; i++) { values.add(i); }
  var calls = 0;
  var mapped = values.map((int value) { calls = calls + 1; return value; });
  print(mapped);
  print(calls);
}
void main() {
  showCount(30);
  showCount(110);
  var nested = <int>[1, 2].map((int outer) {
    print(outer);
    return <int>[outer].map((int inner) { print(inner + 10); return inner; });
  });
  print(nested);
  print(<String>['aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
    'cccccccccccccccccccccccccccccc', 'dddddddddddddddddddddddddddddd'].map((String text) => text));
}
