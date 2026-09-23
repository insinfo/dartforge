import 'package:ngdart/angular.dart';

import 'd10_filtro.dart';

/// Filho que recebe os campos sem tipo escrito.
@Component(
  selector: 'd10-filho-var',
  templateUrl: 'd10_filho_var.html',
)
class D10FilhoVar {
  @Input()
  D10Filtro? filtro;

  @Input()
  D10Perfil? perfil;
}
