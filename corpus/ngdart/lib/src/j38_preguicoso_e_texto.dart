import 'package:ngdart/angular.dart';

import 'i60_provider_use_class.dart';

/// Provedores preguiçosos (campo `late` com inicializador) numa visão com
/// ligações de texto (campo `TextBinding` com inicializador), na raiz e numa
/// visão embutida: a ordem dos dois tipos de campo (o `li-tabs` e o
/// `li-accordion` do limitless_ui).
@Component(
  selector: 'j38-preguicoso-e-texto',
  templateUrl: 'j38_preguicoso_e_texto.html',
  directives: [I60ProviderUseClass, NgIf],
)
class J38PreguicosoETexto {
  String a = 'a';
  String b = 'b';
  bool mostrar = true;
}
