// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a07_ligacao_de_propriedade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a07_ligacao_de_propriedade.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$A07LigacaoDePropriedade = const [];

class ViewA07LigacaoDePropriedade0 extends import0.ComponentView<import1.A07LigacaoDePropriedade> {
  Object? _expr_0;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewA07LigacaoDePropriedade0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('a07-ligacao-de-propriedade'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a07_ligacao_de_propriedade.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.escondido;
    if (import8.checkBinding(this._expr_0, currVal_0, 'escondido', 'package:corpus_ngdart/src/a07_ligacao_de_propriedade.html')) {
      import7.setProperty(this._el_0, 'hidden', currVal_0) /* REF:package:corpus_ngdart/src/a07_ligacao_de_propriedade.html:5:25 */;
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A07LigacaoDePropriedade, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A07LigacaoDePropriedadeNgFactory = ComponentFactory<import1.A07LigacaoDePropriedade>('a07-ligacao-de-propriedade', viewFactory_A07LigacaoDePropriedadeHost0);
ComponentFactory<import1.A07LigacaoDePropriedade> get A07LigacaoDePropriedadeNgFactory {
  return _A07LigacaoDePropriedadeNgFactory;
}

ComponentFactory<import1.A07LigacaoDePropriedade> createA07LigacaoDePropriedadeFactory() {
  return ComponentFactory('a07-ligacao-de-propriedade', viewFactory_A07LigacaoDePropriedadeHost0);
}

final List<Object> styles$A07LigacaoDePropriedadeHost = const [];

class _ViewA07LigacaoDePropriedadeHost0 extends import10.HostView<import1.A07LigacaoDePropriedade> {
  @override
  void build() {
    this.componentView = ViewA07LigacaoDePropriedade0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A07LigacaoDePropriedade();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.A07LigacaoDePropriedade> viewFactory_A07LigacaoDePropriedadeHost0() {
  return _ViewA07LigacaoDePropriedadeHost0();
}
