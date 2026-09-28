import 'dart:html';

import 'package:ngdart/angular.dart';

@Component(
  selector: 'j86-item',
  template: '<div class="corpo"><ng-content></ng-content></div>',
)
class J86Item {}

@Directive(selector: 'template[j86-corpo]')
class J86Corpo implements OnInit {
  J86Corpo(this._modelo, this._container);

  final TemplateRef _modelo;
  final ViewContainerRef _container;

  @override
  void ngOnInit() {
    _container.createEmbeddedView(_modelo);
  }
}

@Component(
  selector: 'j86-tabela',
  template: '<i></i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J86Tabela {}

/// `@ViewChild` com o resultado dentro de um `<template>` escrito (no
/// conteúdo projetado de um filho) e depois de um `*ngIf` — o
/// `#customGridDemoTable` do `datatable_page` do limitless_ui.
@Component(
  selector: 'j86-consulta-em-molde',
  templateUrl: 'j86_consulta_em_molde.html',
  directives: [J86Item, J86Corpo, J86Tabela, NgIf],
)
class J86ConsultaEmMolde {
  bool mostrar = true;

  @ViewChild('tabela')
  J86Tabela? tabela;

  @ViewChild('marca')
  Element? marca;

  @ViewChildren('linha')
  List<Element>? linhas;
}
