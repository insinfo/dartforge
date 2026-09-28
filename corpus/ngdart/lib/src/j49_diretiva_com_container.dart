import 'dart:html';

import 'package:ngdart/angular.dart';

/// Diretiva de elemento comum que injeta `ViewContainerRef` (o
/// `liNavOutlet` do limitless_ui): o nó ganha um `ViewContainer`.
@Directive(selector: '[j49-saida]')
class J49Saida {
  final Element elemento;
  final ViewContainerRef container;

  J49Saida(this.elemento, this.container);

  @Input('j49-saida')
  Object? valor;
}

@Component(
  selector: 'j49-diretiva-com-container',
  templateUrl: 'j49_diretiva_com_container.html',
  directives: [J49Saida, NgIf],
)
class J49DiretivaComContainer {
  bool mostrar = true;
  String v = 'v';
}
