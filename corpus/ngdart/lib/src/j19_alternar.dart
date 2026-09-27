import 'package:ngdart/angular.dart';

/// Diretiva cujo seletor é também a entrada (o `liCollapseToggle`).
@Directive(selector: '[j19Alternar]')
class J19Alternar {
  @Input('j19Alternar')
  String alvo = '';

  @Input()
  bool j19Animar = true;
}
