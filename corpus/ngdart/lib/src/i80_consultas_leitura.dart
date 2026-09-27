import 'dart:html';

import 'package:ngdart/angular.dart';

import 'i68_marca.dart';
import 'i80_rotulo.dart';

/// Sonda: consultas de conteúdo com `read:` (de outra diretiva do nó e do
/// elemento) e `@ContentChild` único com mais de um resultado.
@Component(
  selector: 'i80-consultas-leitura',
  template: '<ng-content></ng-content>',
)
class I80ConsultasLeitura {
  @ContentChild(I68Marca)
  I68Marca? primeira;

  @ContentChildren(I68Marca, read: I80Rotulo)
  List<I80Rotulo>? rotulos;

  @ContentChild(I68Marca, read: Element)
  Element? primeiroElemento;

  @ContentChildren(I80Rotulo)
  List<I80Rotulo>? todos;
}
