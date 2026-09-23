// experimentos: macros
// Duas bibliotecas de augmentation, e uma augmentation que importa outra:
// campos e membros de mixin vindos de toda a árvore. A ordem dos
// inicializadores com efeito colateral não é conferida aqui: o CFE 3.6.2
// roda os das augmentations antes dos da classe (4 1 3 2), contra a spec e
// o 3.13.4 (1 2 3, no 405), e o DartForge segue a spec.
import augment 'a_aug.dart';
import augment 'b_aug.dart';

int proximo(String quem) => quem.length * 10 + int.parse(quem.substring(1));

mixin Falante {
  String falar() => 'oi';
}

class C {
  int f1 = proximo('f1');
}

class D with Falante {}

void main() {
  var c = C();
  print('${c.f1} ${c.f2} ${c.f3} ${c.f4}');
  var d = D();
  print(d.falar());
  print(d.gritar());
  print(d.sussurrar());
}
