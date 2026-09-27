import 'dart:html';

import 'package:ngdart/angular.dart';

/// Consultas estáticas e dinâmicas no mesmo componente, na ordem de
/// declaração: um elemento da visão, um dentro de `*ngIf`, outro da visão.
@Component(
  selector: 'j15-consultas-misturadas',
  templateUrl: 'j15_consultas_misturadas.html',
  directives: [coreDirectives],
)
class J15ConsultasMisturadas {
  bool a = true;

  @ViewChild('fora')
  Element? fora;

  @ViewChild('dentro')
  Element? dentro;

  @ViewChild('depois')
  Element? depois;
}
