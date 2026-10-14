import 'a.dart' as a;
import 'b.dart' as b;
class Box { a.Foo value = a.Foo(); }
void main() { final box = Box(); print(box.value.a); print(a.Foo() is b.Foo); }
