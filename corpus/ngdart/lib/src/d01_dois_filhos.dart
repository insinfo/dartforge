import 'package:ngdart/angular.dart';

import 'a02_texto_estatico.dart';
import 'a02_texto_estatico.template.dart';
import 'a11_projecao.dart';
import 'a11_projecao.template.dart';

/// Dois componentes filhos: a numeração de `_compView_` e do campo da
/// instância.
@Component(
  selector: 'd01-dois-filhos',
  templateUrl: 'd01_dois_filhos.html',
  directives: [A02TextoEstatico, A11Projecao],
)
class D01DoisFilhos {}
