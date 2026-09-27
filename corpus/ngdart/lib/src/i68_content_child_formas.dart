import 'dart:html';

import 'package:ngdart/angular.dart';

import 'i68_marca.dart';

/// Sonda: `@ContentChild`/`@ContentChildren` no próprio componente, com
/// `read:`, por referência, em setter, `descendants: false`, e com
/// `AfterContentInit`.
@Component(
  selector: 'i68-content-child-formas',
  template: '<ng-content></ng-content>',
)
class I68ContentChildFormas implements AfterContentInit {
  @ContentChildren(I68Marca, read: HtmlElement)
  List<HtmlElement>? elementos;

  @ContentChild(I68Marca, read: I68Marca)
  I68Marca? marca;

  @ContentChild('ref')
  Element? ref;

  @ContentChildren('item', descendants: false)
  List<Element>? itens;

  @ContentChildren(I68Marca)
  set marcas(List<I68Marca> v) {}

  @override
  void ngAfterContentInit() {}
}
