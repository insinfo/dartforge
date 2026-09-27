import 'dart:html';

import 'package:ngdart/angular.dart';

/// Resultados de `@ViewChild` dinâmicos misturados com nós ligados na mesma
/// visão embutida (o `li-color-picker` do limitless_ui): os nós das
/// consultas viram campo antes dos que só as ligações leem.
@Component(
  selector: 'j18-consulta-e-ligacao-em-if',
  templateUrl: 'j18_consulta_e_ligacao_em_if.html',
  directives: [coreDirectives],
)
class J18ConsultaELigacaoEmIf {
  bool a = true;
  bool marcado = false;

  @ViewChild('area')
  Element? area;

  @ViewChild('matiz')
  Element? matiz;

  void descer(Event e) {}
}
