import 'a.dart' as a;
import 'b.dart' as b;
class Box { b.Foo value = b.Foo(); }
void main() { final box = Box(); print(box.value.b); print(b.Foo() is a.Foo); }
