import 'package:ngdart/angular.dart';

import 'j47_container_no_hospedeiro.dart';
import 'j41_injecao_no_conteudo.dart';

/// O filho que injeta `ViewContainerRef` em vários lugares: raiz, dentro de
/// elemento, em `*ngIf` e projetado (o `li-popover` nas páginas do
/// limitless_ui).
@Component(
  selector: 'j48-usa-container',
  templateUrl: 'j48_usa_container.html',
  directives: [J47Simples, J41Aba, NgIf],
)
class J48UsaContainer {
  bool mostrar = true;
}
