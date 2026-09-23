import 'package:ngdart/angular.dart';

import 'd09_marca.dart';

/// Filho com três projeções, duas com `select`, e consultas de conteúdo.
@Component(
  selector: 'd09-projecao-select',
  templateUrl: 'd09_projecao_select.html',
)
class D09ProjecaoSelect {
  @ContentChildren(D09Marca)
  List<D09Marca>? marcas;

  @ContentChild(D09Marca)
  D09Marca? primeira;
}
