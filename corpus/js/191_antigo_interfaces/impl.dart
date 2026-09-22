import 'api.dart';
class Base {
  int apply(int value) => value + 2;
}
class Implementation extends Base implements Operation {}
Operation create() => Implementation();
Mode defaultMode() => Mode.safe;
