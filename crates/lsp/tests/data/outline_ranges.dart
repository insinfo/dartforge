import 'dart:core';
import 'dart:core' as core;
typedef Stamp = core.Deprecated;
/// A documented declaration.
@Stamp('old') /* annotation comment 🧭 */
abstract class Holder {
  @Stamp('old') Holder();
  @Stamp('old') Holder.named();
  @Stamp('old') int first /* one */ = (1 + 2), second /* two */;
  @Stamp('old') late final int lateField;
  @Stamp('old') static const List<int> values = <int>[1, 2], empty = <int>[];
  @Stamp('old') int get oldGetter => first;
  @Stamp('old') set oldSetter(int value) { first = value; }
  @Stamp('old') external void externalMethod();
  @Stamp('old') void method(int value, {String name = 'x'}) {}
}
@Stamp('old') mixin Helpers {
  @Stamp('old') int helper = 1;
  @Stamp('old') String tag() => 'value';
}
@Stamp('old') enum Choice {
  @Stamp('old') oldValue,
  currentValue;
  @Stamp('old') int get code => index;
}
@Stamp('old') extension Extra on int {
  @Stamp('old') int get doubled => this * 2;
}
@Stamp('old') extension type Identifier(int representation) {
  @Stamp('old') int get oldValue => representation;
}
@Stamp('old') typedef Callback = void Function(int value);
@Stamp('old') int top /* one */ = (1 + 2), other /* two */;
@Stamp('old') final List<int> topValues = <int>[1, 2];
@Stamp('old') int get topGetter => top;
@Stamp('old') set topSetter(int value) { top = value; }
@Stamp('old') void topFunction(int value) {
  int local(int value) => value;
}
void main() {}
