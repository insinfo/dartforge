// Convertido de tests/conformance/modules/package_exports (módulo antigo do corpus de conformidade).
import 'package:fixture/api.dart' show value, Box hide hidden;
void main() { print(value()); var box = Box(); print(box.read()); }
