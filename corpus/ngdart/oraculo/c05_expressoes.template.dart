// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c05_expressoes.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c05_expressoes.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$C05Expressoes = const [];

class ViewC05Expressoes0 extends import0.ComponentView<import1.C05Expressoes> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import2.DivElement _el_0;
  late final import2.DivElement _el_1;
  late final import2.DivElement _el_2;
  late final import2.DivElement _el_3;
  static import3.ComponentStyles? _componentStyles;
  ViewC05Expressoes0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('c05-expressoes'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/c05_expressoes.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendDiv(doc, parentRenderNode);
    this._el_2 = import7.appendDiv(doc, parentRenderNode);
    this._el_3 = import7.appendDiv(doc, parentRenderNode);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.item.nome;
    if (import8.checkBinding(this._expr_0, currVal_0, 'item.nome', 'package:corpus_ngdart/src/c05_expressoes.html')) {
      import7.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/c05_expressoes.html:5:24 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.titulo();
    if (import8.checkBinding(this._expr_1, currVal_1, 'titulo()', 'package:corpus_ngdart/src/c05_expressoes.html')) {
      import7.setProperty(this._el_1, 'title', currVal_1) /* REF:package:corpus_ngdart/src/c05_expressoes.html:36:54 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = (!_ctx.item.ativo);
    if (import8.checkBinding(this._expr_2, currVal_2, '!item.ativo', 'package:corpus_ngdart/src/c05_expressoes.html')) {
      import7.setProperty(this._el_2, 'hidden', currVal_2) /* REF:package:corpus_ngdart/src/c05_expressoes.html:66:88 */;
      this._expr_2 = currVal_2;
    }
    if (firstCheck) {
      import7.setProperty(this._el_3, 'title', 'fixo') /* REF:package:corpus_ngdart/src/c05_expressoes.html:100:116 */;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$C05Expressoes, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C05ExpressoesNgFactory = ComponentFactory<import1.C05Expressoes>('c05-expressoes', viewFactory_C05ExpressoesHost0);
ComponentFactory<import1.C05Expressoes> get C05ExpressoesNgFactory {
  return _C05ExpressoesNgFactory;
}

ComponentFactory<import1.C05Expressoes> createC05ExpressoesFactory() {
  return ComponentFactory('c05-expressoes', viewFactory_C05ExpressoesHost0);
}

final List<Object> styles$C05ExpressoesHost = const [];

class _ViewC05ExpressoesHost0 extends import10.HostView<import1.C05Expressoes> {
  @override
  void build() {
    this.componentView = ViewC05Expressoes0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C05Expressoes();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.C05Expressoes> viewFactory_C05ExpressoesHost0() {
  return _ViewC05ExpressoesHost0();
}
