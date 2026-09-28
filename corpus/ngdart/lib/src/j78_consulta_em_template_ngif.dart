import 'package:ngdart/angular.dart';

@Component(
  selector: 'j78-painel',
  template: '<i></i>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J78Painel {
  @Input()
  String? titulo;
}

/// `@ViewChild` de um filho dentro de `<template [ngIf]>` escrito (o
/// `#sidePanelModal` do `li-pdf-viewer`).
@Component(
  selector: 'j78-consulta-em-template-ngif',
  templateUrl: 'j78_consulta_em_template_ngif.html',
  directives: [J78Painel, NgIf],
)
class J78ConsultaEmTemplateNgif {
  bool mostrar = true;

  @ViewChild('painel')
  J78Painel? painel;
}
