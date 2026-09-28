import 'dart:html';

import 'package:ngdart/angular.dart';

/// Na visão do componente, um nó ligado antes de um nó que é resultado de
/// consulta dinâmica (a lista junta um `*ngIf` e o nó da raiz): a ordem dos
/// campos `_el_`.
@Component(
  selector: 'j35-consulta-e-ligacao-na-raiz',
  templateUrl: 'j35_consulta_e_ligacao_na_raiz.html',
  directives: [coreDirectives],
)
class J35ConsultaELigacaoNaRaiz {
  bool a = true;

  @ViewChildren('s')
  List<Element>? todos;
}
