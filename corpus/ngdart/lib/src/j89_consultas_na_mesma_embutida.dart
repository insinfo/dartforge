import 'dart:html';

import 'package:ngdart/angular.dart';

class J89Servico {}

@Directive(selector: '[j89-marca]', providers: [ClassProvider(J89Servico)])
class J89Marca {}

@Component(
  selector: 'j89-item',
  template: '<i></i>',
)
class J89Item {}

/// Dois resultados da mesma consulta na mesma visão embutida, e `read:` de
/// um provedor de diretiva numa consulta dinâmica.
@Component(
  selector: 'j89-consultas-na-mesma-embutida',
  templateUrl: 'j89_consultas_na_mesma_embutida.html',
  directives: [J89Item, J89Marca, NgIf, NgFor],
)
class J89ConsultasNaMesmaEmbutida {
  bool mostrar = true;
  List<int> itens = [1, 2];

  @ViewChildren('linha')
  List<Element>? linhas;

  @ViewChildren(J89Item)
  List<J89Item>? todos;

  @ViewChild('marca', read: J89Servico)
  J89Servico? servico;

  @ViewChildren('marca', read: J89Marca)
  List<J89Marca>? marcas;
}
