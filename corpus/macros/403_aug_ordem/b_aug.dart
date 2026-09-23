augment library 'main.dart';

augment class C {
  int f4 = proximo('f4');
}

augment mixin Falante {
  String sussurrar() => '(${gritar().toLowerCase()})';
}
