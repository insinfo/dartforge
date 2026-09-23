import 'package:ngdart/angular.dart';

import 'd09_projecao_select.dart';

/// Conteúdo projetado por seletor: cada nó vai para a primeira projeção que
/// casa (`findNgContentIndex`), o resto para a sem `select`; a âncora do
/// `*ngIf` é criada solta e tem o filho como pai. A consulta de conteúdo sem
/// resultado: a lista recebe `[]`, a única nada.
@Component(
  selector: 'd09-usa-projecao',
  templateUrl: 'd09_usa_projecao.html',
  directives: [coreDirectives, D09ProjecaoSelect],
)
class D09UsaProjecao {
  bool mostra = true;
}
