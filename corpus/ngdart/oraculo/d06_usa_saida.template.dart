// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd06_usa_saida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd06_usa_saida.dart' as import1;
import 'd06_filho_saida.template.dart' as import2;
import 'd06_filho_saida.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$D06UsaSaida = const [];

class ViewD06UsaSaida0 extends import0.ComponentView<import1.D06UsaSaida> {
  late final import2.ViewD06FilhoSaida0 _compView_0;
  late final import3.D06FilhoSaida _D06FilhoSaida_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewD06UsaSaida0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('d06-usa-saida'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/d06_usa_saida.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewD06FilhoSaida0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._D06FilhoSaida_0_5 = import3.D06FilhoSaida();
    this._compView_0.create(this._D06FilhoSaida_0_5);
    _el_0.addEventListener('click', this.eventHandler0(_ctx.clicou));
    final subscription_0 = this._D06FilhoSaida_0_5.salvo.listen(this.eventHandler1(_ctx.guardar));
    final subscription_1 = this._D06FilhoSaida_0_5.fechou.listen(this.eventHandler1(this._handleEvent_0));
    this.initSubscriptions([subscription_0, subscription_1]);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.aberto = false;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$D06UsaSaida, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D06UsaSaidaNgFactory = ComponentFactory<import1.D06UsaSaida>('d06-usa-saida', viewFactory_D06UsaSaidaHost0);
ComponentFactory<import1.D06UsaSaida> get D06UsaSaidaNgFactory {
  return _D06UsaSaidaNgFactory;
}

ComponentFactory<import1.D06UsaSaida> createD06UsaSaidaFactory() {
  return ComponentFactory('d06-usa-saida', viewFactory_D06UsaSaidaHost0);
}

final List<Object> styles$D06UsaSaidaHost = const [];

class _ViewD06UsaSaidaHost0 extends import10.HostView<import1.D06UsaSaida> {
  @override
  void build() {
    this.componentView = ViewD06UsaSaida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D06UsaSaida();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.D06UsaSaida> viewFactory_D06UsaSaidaHost0() {
  return _ViewD06UsaSaidaHost0();
}
