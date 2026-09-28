import 'dart:async';

import 'package:ngdart/angular.dart';

@Directive(selector: '[j85-grupo]')
class J85Grupo {}

/// Diretiva de `<template>` com `TemplateRef`, `ViewContainerRef`, um
/// provedor de cima, entradas, saída e ganchos (o `liPaginationOutlet` e o
/// `liAccordionBody` do limitless_ui).
@Directive(selector: 'template[j85-saida]')
class J85Saida implements OnInit, AfterChanges, OnDestroy {
  J85Saida(this._modelo, this._container, this.grupo);

  final TemplateRef _modelo;
  final ViewContainerRef _container;
  final J85Grupo grupo;

  @Input('j85-saida')
  String? nome;

  @Input()
  Object? valor;

  final _pronto = StreamController<String>();

  @Output()
  Stream<String> get pronto => _pronto.stream;

  @override
  void ngOnInit() {
    _container.createEmbeddedView(_modelo);
  }

  @override
  void ngAfterChanges() {}

  @override
  void ngOnDestroy() {}
}

/// Também casa o `<template>`; o `@HostBinding` dela não se escreve nele.
@Directive(selector: '[j85-saida]')
class J85Marca {
  J85Marca(this.grupo);

  final J85Grupo grupo;

  @HostBinding('class.marca')
  bool marca = true;
}

@Directive(selector: 'template[j85-simples]')
class J85Simples {
  J85Simples(this.modelo);

  final TemplateRef modelo;
}

@Component(
  selector: 'j85-diretivas-de-template',
  templateUrl: 'j85_diretivas_de_template.html',
  directives: [J85Grupo, J85Marca, J85Saida, J85Simples, NgIf],
)
class J85DiretivasDeTemplate {
  bool mostrar = true;
  int total = 1;

  void avisar(String s) {}
}
