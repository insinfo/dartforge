import 'base.dart';
import 'child.dart';
void main() {
  Base object = create();
  print(object.describe());
  object.value = 'changed';
  print(object.describe());
}
