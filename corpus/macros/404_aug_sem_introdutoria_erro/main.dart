// experimentos: macros
// erro-de-compilacao
// Uma augmentation sem declaração introdutória antes dela é erro.
import augment 'erro_aug.dart';

augment class Fantasma {
  void f() {}
}

void main() {
  print('não chega aqui');
}
