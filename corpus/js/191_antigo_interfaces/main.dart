// Convertido de tests/conformance/modules/interfaces (módulo antigo do corpus de conformidade).
import 'api.dart';
import 'impl.dart';
void main() {
  Operation operation = create();
  print(operation.apply(40));
  print(defaultMode() == Mode.safe);
  print(defaultMode().index);
  print(defaultMode().name);
}
