import 'package:ngdart/angular.dart';

/// O shim relê a folha com o `csslib` e a escreve compacta: vírgula sem
/// espaço em lista e em função comum, `, ` no padrão de `var()`, `calc` cru,
/// e cor hexadecimal com o valor de uma cor nomeada vira o nome (`#ffffff`
/// → `white`, `#000` → `black`), a de pares repetidos encurta.
@Component(
  selector: 'b22-estilo-csslib',
  templateUrl: 'b22_estilo_csslib.html',
  styleUrls: ['b22_estilo_csslib.css'],
)
class B22EstiloCsslib {}
