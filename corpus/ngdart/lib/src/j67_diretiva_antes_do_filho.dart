import 'dart:async';

import 'package:ngdart/angular.dart';

const j67Marcas = MultiToken<Object>('j67Marcas');

/// Diretivas que vêm antes do componente em `directives:` (o
/// `RequiredValidator` antes do `LiPasswordInputComponent` no limitless_ui):
/// o oficial as cria antes dele, e as ligações e os ganchos seguem a ordem.
@Directive(
  selector: '[j67-marca]',
  providers: [ExistingProvider.forToken(j67Marcas, J67Marca)],
)
class J67Marca implements AfterContentInit, OnDestroy {
  @Input('j67-marca')
  String? rotulo;

  final _mudou = StreamController<String>();

  @Output()
  Stream<String> get mudou => _mudou.stream;

  @override
  void ngAfterContentInit() {}

  @override
  void ngOnDestroy() {}
}

@Directive(selector: '[j67-depois]')
class J67Depois {
  J67Depois(@Inject(j67Marcas) List<Object> marcas);

  @Input('j67-depois')
  String? valor;
}

@Component(
  selector: 'j67-campo',
  template: '<ng-content></ng-content>',
)
class J67Campo implements AfterContentInit, OnDestroy {
  @Input()
  String? rotulo;

  final _escolheu = StreamController<int>();

  @Output()
  Stream<int> get escolheu => _escolheu.stream;

  @override
  void ngAfterContentInit() {}

  @override
  void ngOnDestroy() {}
}

@Component(
  selector: 'j67-diretiva-antes-do-filho',
  templateUrl: 'j67_diretiva_antes_do_filho.html',
  directives: [J67Marca, J67Campo, J67Depois],
)
class J67DiretivaAntesDoFilho {
  String nome = 'n';
  String? ultimo;
  int? numero;

  void clicou() {}
}
