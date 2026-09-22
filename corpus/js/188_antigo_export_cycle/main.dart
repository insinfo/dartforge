// Convertido de tests/conformance/modules/export_cycle (módulo antigo do corpus de conformidade).
import 'a.dart' show a, b;
void main() { print(a()); print(b()); }
