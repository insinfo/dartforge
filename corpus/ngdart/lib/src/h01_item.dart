import 'package:ngdart/angular.dart';

/// Diretivas que o cabeçalho do h01 consulta: uma numa tag que não é HTML
/// (`<h01-it>`, seletor composto) e outra de atributo.
@Directive(selector: 'h01-item,h01-it')
class H01ItemDirective {
  @Input()
  String label = '';

  @Input()
  bool ativo = false;
}

@Directive(selector: '[acoes]')
class H01AcoesDirective {}
