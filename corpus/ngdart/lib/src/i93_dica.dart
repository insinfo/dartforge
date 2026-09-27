import 'package:ngdart/angular.dart';

/// Diretiva com `exportAs`, lida por `#d="dica"`.
@Directive(selector: '[dica]', exportAs: 'dica')
class I93Dica {
  String texto = 'x';
}
