import 'package:ngdart/angular.dart';

import 'j41_injecao_no_conteudo.dart';

/// `<ng-content>` dentro do conteúdo de um filho: a reprojeção entra
/// inteira na lista do filho (o `li-tabx` que repassa o conteúdo ao
/// `li-tabs` no limitless_ui).
@Component(
  selector: 'j57-reprojecao',
  templateUrl: 'j57_reprojecao.html',
  directives: [J41Aba],
)
class J57Reprojecao {}
