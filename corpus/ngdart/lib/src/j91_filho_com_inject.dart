import 'package:ngdart/angular.dart';

class J91Servico {}

const j91Nome = OpaqueToken<String>('j91Nome');
const j91Lista = MultiToken<String>('j91Lista');

@Component(
  selector: 'j91-filho',
  template: '<i>{{ tipo }}</i>',
)
class J91Filho {
  J91Filho(
    @Attribute('tipo') this.tipo,
    @Attribute('ausente') String? ausente,
    @Inject(j91Nome) String nome,
    @Optional() @Inject(j91Lista) List<String>? lista,
    @Inject(J91Servico) Object servico,
  );

  final String? tipo;
}

/// Filho com `@Attribute` e `@Inject(token)` no construtor.
@Component(
  selector: 'j91-filho-com-inject',
  template: '<j91-filho tipo="grande"></j91-filho><j91-filho></j91-filho>',
  directives: [J91Filho],
  providers: [
    ClassProvider(J91Servico),
    ValueProvider.forToken(j91Nome, 'x'),
  ],
)
class J91FilhoComInject {}
