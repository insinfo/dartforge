import 'dart:html';

import 'package:ngdart/angular.dart';

/// Diretiva no elemento de um filho que pede os embutidos do nó: o
/// `ViewContainerRef` e o `ComponentLoader` são o mesmo `ViewContainer`
/// (`_appEl_n`), o `ChangeDetectorRef` é a visão do filho e o elemento é o
/// nó (o `MaterialTooltipDirective` no `material-button` do `material_menu`).
@Component(
  selector: 'j123-botao',
  template: '<ng-content></ng-content>',
)
class J123Botao {}

@Directive(selector: '[j123Dica]')
class J123Dica {
  J123Dica(ViewContainerRef vcr, HtmlElement el, ComponentLoader carregador,
      ChangeDetectorRef cd, @Attribute('classe') String? classe);

  @Input('j123Dica')
  String? texto;
}

@Component(
  selector: 'j123-usa',
  template: '<j123-botao [j123Dica]="t">x</j123-botao>',
  directives: [J123Botao, J123Dica],
)
class J123Usa {
  String t = 'a';
}
