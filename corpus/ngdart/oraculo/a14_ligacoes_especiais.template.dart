// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a14_ligacoes_especiais.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a14_ligacoes_especiais.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$A14LigacoesEspeciais = const [];

class ViewA14LigacoesEspeciais0 extends import0.ComponentView<import1.A14LigacoesEspeciais> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewA14LigacoesEspeciais0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('a14-ligacoes-especiais'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/a14_ligacoes_especiais.dart' : null);
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
    final currVal_0 = _ctx.ativo;
    if (import8.checkBinding(this._expr_0, currVal_0, 'ativo', 'package:corpus_ngdart/src/a14_ligacoes_especiais.html')) {
      import7.updateClassBinding(this._el_0, 'ativo', currVal_0) /* REF:package:corpus_ngdart/src/a14_ligacoes_especiais.html:5:26 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.rotulo;
    if (import8.checkBinding(this._expr_1, currVal_1, 'rotulo', 'package:corpus_ngdart/src/a14_ligacoes_especiais.html')) {
      import7.updateAttribute(this._el_0, 'aria-label', currVal_1) /* REF:package:corpus_ngdart/src/a14_ligacoes_especiais.html:27:53 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.cor;
    if (import8.checkBinding(this._expr_2, currVal_2, 'cor', 'package:corpus_ngdart/src/a14_ligacoes_especiais.html')) {
      this._el_0.style.setProperty('color', currVal_2) /* REF:package:corpus_ngdart/src/a14_ligacoes_especiais.html:54:73 */;
      this._expr_2 = currVal_2;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$A14LigacoesEspeciais, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A14LigacoesEspeciaisNgFactory = ComponentFactory<import1.A14LigacoesEspeciais>('a14-ligacoes-especiais', viewFactory_A14LigacoesEspeciaisHost0);
ComponentFactory<import1.A14LigacoesEspeciais> get A14LigacoesEspeciaisNgFactory {
  return _A14LigacoesEspeciaisNgFactory;
}

ComponentFactory<import1.A14LigacoesEspeciais> createA14LigacoesEspeciaisFactory() {
  return ComponentFactory('a14-ligacoes-especiais', viewFactory_A14LigacoesEspeciaisHost0);
}

final List<Object> styles$A14LigacoesEspeciaisHost = const [];

class _ViewA14LigacoesEspeciaisHost0 extends import10.HostView<import1.A14LigacoesEspeciais> {
  @override
  void build() {
    this.componentView = ViewA14LigacoesEspeciais0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A14LigacoesEspeciais();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.A14LigacoesEspeciais> viewFactory_A14LigacoesEspeciaisHost0() {
  return _ViewA14LigacoesEspeciaisHost0();
}
