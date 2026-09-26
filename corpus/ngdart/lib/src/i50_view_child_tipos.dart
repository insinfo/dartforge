import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';
import 'a11_projecao.dart';

/// Sonda: `@ViewChild(Tipo)` e `@ViewChildren(Tipo)` com resultado projetado.
@Component(
  selector: 'i50-view-child-tipos',
  templateUrl: 'i50_view_child_tipos.html',
  directives: [A11Projecao, A02TextoEstatico],
)
class I50ViewChildTipos {
  @ViewChildren(A02TextoEstatico)
  List<A02TextoEstatico>? todos;
  @ViewChild(A02TextoEstatico)
  A02TextoEstatico? primeiro;
  @ViewChildren(A11Projecao)
  List<A11Projecao> projecoes = [];
}
