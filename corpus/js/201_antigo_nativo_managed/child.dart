import 'base.dart';
class Child extends Base {
  String describe() => this.value + ':child';
}
Base create() => Child();
