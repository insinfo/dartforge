augment library 'a_aug.dart';

augment mixin Falante {
  String gritar() => falar().toUpperCase() + '!';
}

augment class C {
  int f3 = proximo('f3');
}
