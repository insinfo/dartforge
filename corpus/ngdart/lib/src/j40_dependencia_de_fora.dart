import 'package:ngdart/angular.dart';

import 'j40_com_host.dart';

class J40Servico {}

class J40Outro {}

const j40Token = OpaqueToken<String>('j40.token');

/// Diretiva que pede um serviço que nenhum nó da visão fornece: vem do
/// injetor de fora da visão (o `li-tooltip`/`li-dropdown` do limitless_ui
/// pedindo serviços da aplicação).
@Directive(selector: '[j40-usa]')
class J40Usa {
  final J40Servico servico;
  final J40Outro? outro;
  final String? texto;

  J40Usa(this.servico, @Optional() this.outro,
      @Optional() @Inject(j40Token) this.texto);
}

/// Só o `Injector` do próprio elemento (`this.injector(n)`).
@Directive(selector: '[j40-injetor]')
class J40ComInjetor {
  final Injector injetor;

  J40ComInjetor(this.injetor);
}

@Component(
  selector: 'j40-dependencia-de-fora',
  templateUrl: 'j40_dependencia_de_fora.html',
  directives: [J40Usa, J40ComHost, J40ComInjetor, NgIf],
)
class J40DependenciaDeFora {
  bool mostrar = true;
}
