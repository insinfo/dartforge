import 'package:ngdart/angular.dart';

/// Diretiva com `@Input` alimentado por atributo interpolado (o `value` de
/// um lookup no limitless_ui).
@Directive(selector: '[j59-rotulo]')
class J59Rotulo {
  @Input('j59-rotulo')
  String? rotulo;

  @Input()
  Object? valor;
}

@Component(
  selector: 'j59-interpolado-em-diretiva',
  templateUrl: 'j59_interpolado_em_diretiva.html',
  directives: [J59Rotulo],
)
class J59InterpoladoEmDiretiva {
  String nome = 'n';
  int total = 1;
}
