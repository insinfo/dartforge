// Convertido de tests/conformance/modules/private_inheritance (módulo antigo do corpus de conformidade).
import 'base.dart';
import 'child.dart';
int read(Base value) { return value.baseValue(); }
void main() {
  var child = Child();
  print(read(child)); print(child.childValue()); print(child.value());
}
