part of 'clausulas.dart';

augment class C {
  int f3 = proximo('f3');
  augment String rotulo() => 'rótulo ${f2 + f3}';
}
