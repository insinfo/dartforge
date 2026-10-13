import 'package:meta/meta.dart';

class C({@experimental int y = 0}) {
  this : assert(y > 0);
}
