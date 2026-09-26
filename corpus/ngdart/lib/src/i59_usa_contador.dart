import 'package:ngdart/angular.dart';

import 'i59_contador.dart';

/// Sonda: two-way `[(x)]` em componente filho de outro arquivo.
@Component(
  selector: 'i59-usa-contador',
  templateUrl: 'i59_usa_contador.html',
  directives: [I59Contador],
)
class I59UsaContador {
  int n = 0;
}
