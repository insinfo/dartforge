// Convertido de tests/conformance/modules/features13 (módulo antigo do corpus de conformidade).
import 'helper.dart';
void main() {
  var value = echo<Status>(Status.done);
  print(value.caption);
  print(value.name);
  print(show(value));
  print(echo('imported'));
  const nested = <List<int>>[<int>[1, 2]];
  print(nestedConstant() == nested);
}
