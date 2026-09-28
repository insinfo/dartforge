import 'package:ngdart/angular.dart';

@Component(
  selector: 'j73-filho',
  template: '<i></i>',
)
class J73Filho {
  @Input()
  String? classe;

  @Input()
  bool? ativo;

  @Input()
  Object? outro;
}

/// Atributo vazio (`classe=""`) e sem valor (`ativo`) num `@Input` do filho
/// (o `menuClass=""` do `li-token-field`).
@Component(
  selector: 'j73-atributo-vazio-no-filho',
  templateUrl: 'j73_atributo_vazio_no_filho.html',
  directives: [J73Filho],
)
class J73AtributoVazioNoFilho {}
