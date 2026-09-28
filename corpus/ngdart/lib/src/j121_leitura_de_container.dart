import 'package:ngdart/angular.dart';
import 'package:ngdart/experimental.dart' show changeDetectionLink;

/// `@ViewChild('x', read: ViewContainerRef)` num `<template #x>`: a leitura
/// liga o `_requiresViewContainer` do nó (`provider_parser.dart:95-104`) —
/// o `ViewContainer` fica público e o `TemplateRef` passa ao índice 8; o
/// valor é `this._appEl_n`. `@changeDetectionLink` dá às visões do
/// componente o `detectChangesInCheckAlwaysViews` (contêineres públicos e
/// filhos também ligados).
@changeDetectionLink
@Component(
  selector: 'j121-dinamico',
  template: '<template #marcador></template>',
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J121Dinamico {
  final ComponentLoader _carregador;
  ViewContainerRef? _vcr;

  /// `ComponentLoader` é apelido do `ViewContainer` do nó.
  J121Dinamico(this._carregador);

  @ViewChild('marcador', read: ViewContainerRef)
  set vcr(ViewContainerRef? v) {
    _vcr = v;
  }
}

@changeDetectionLink
@Component(
  selector: 'j121-ligado',
  template: '''<j121-dinamico></j121-dinamico>
<div *ngIf="mostra"><j121-dinamico></j121-dinamico><template [ngIf]="mostra">x</template></div>''',
  directives: [J121Dinamico, NgIf],
  changeDetection: ChangeDetectionStrategy.onPush,
)
class J121Ligado {
  bool mostra = true;
}

@Component(
  selector: 'j121-comum',
  template: '<p>a</p><template #ponto></template>',
)
class J121Comum {
  @ViewChild('ponto', read: ViewContainerRef)
  ViewContainerRef? ponto;
}
