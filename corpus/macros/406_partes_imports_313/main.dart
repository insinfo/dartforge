// requer-dart: 3.13
// experimentos: augmentations,enhanced-parts
// Escopo de imports por unidade (`parts-with-imports`): os imports de uma
// parte valem nela e nas partes dela e escondem os do arquivo que a
// incluiu; prefixos também; declarações da biblioteca escondem todo import.
import 'escopo_a.dart';
import 'escopo_a.dart' as p;
part 'escopo_parte.dart';

String local() => 'biblioteca';

void main() {
  print('main: ${nome()} ${p.nome()} ${Caixa().quem} ${local()}');
  print(daParte());
  print(daNeta());
  print(daOutraNeta());
}
