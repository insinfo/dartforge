import 'dart:html';

import 'package:ngdart/angular.dart';

@Directive(selector: '[j72-item]')
class J72Item {
  @HostBinding('class.item')
  bool item = true;
}

@Directive(selector: '[j72-marca]')
class J72Marca {}

/// `@ContentChild(ren)` numa diretiva de elemento (o `LiAccordionDirective`
/// do limitless_ui): a lista recebe os resultados do conteúdo no
/// `afterChildren` do nó, como a de um componente.
@Directive(selector: '[j72-grupo]')
class J72Grupo implements AfterContentInit {
  @ContentChildren(J72Item, descendants: false)
  List<J72Item> diretos = [];

  @ContentChildren(J72Item)
  List<J72Item> todos = [];

  @ContentChild(J72Marca)
  J72Marca? marca;

  @ContentChildren(J72Marca, read: HtmlElement)
  List<HtmlElement> nos = [];

  @ContentChildren('nada')
  List<Object> nada = [];

  List<J72Item> _pelo = [];

  @ContentChildren(J72Item)
  set pelo(List<J72Item> v) => _pelo = v;

  @override
  void ngAfterContentInit() {}
}

@Component(
  selector: 'j72-consultas-de-diretiva',
  templateUrl: 'j72_consultas_de_diretiva.html',
  directives: [J72Item, J72Marca, J72Grupo],
)
class J72ConsultasDeDiretiva {}
