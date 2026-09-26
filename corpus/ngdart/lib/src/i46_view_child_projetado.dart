import 'dart:html';

import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';
import 'a11_projecao.dart';

/// Sonda: `@ViewChild` de `#ref` no conteúdo projetado num filho.
@Component(
  selector: 'i46-view-child-projetado',
  templateUrl: 'i46_view_child_projetado.html',
  directives: [A11Projecao, A02TextoEstatico],
)
class I46ViewChildProjetado {
  @ViewChild('caixa')
  Element? caixa;
  @ViewChild('filho')
  A02TextoEstatico? filho;
}
