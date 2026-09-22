import 'package:ngdart/angular.dart';

import 'a11_projecao.dart';
import 'a11_projecao.template.dart';

/// Conteúdo projetado dentro de um componente filho.
@Component(
  selector: 'd04-projecao-no-filho',
  templateUrl: 'd04_projecao_no_filho.html',
  directives: [A11Projecao],
)
class D04ProjecaoNoFilho {}
