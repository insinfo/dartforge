// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j64_attr_se_nulo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j64_attr_se_nulo.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J64AttrSeNulo = const [];

class ViewJ64AttrSeNulo0 extends import0.ComponentView<import1.J64AttrSeNulo> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_4;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ64AttrSeNulo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j64-attr-se-nulo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j64_attr_se_nulo.dart' : null);
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
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      import7.setAttribute(this._el_0, 'data-e', 'fixo') /* REF:package:corpus_ngdart/src/j64_attr_se_nulo.html:120:142 */;
    }
    final currVal_0 = (_ctx.chave ?? '');
    if (import8.checkBinding(this._expr_0, currVal_0, 'chave ?? \'\'', 'package:corpus_ngdart/src/j64_attr_se_nulo.html')) {
      import7.setAttribute(this._el_0, 'data-a', currVal_0) /* REF:package:corpus_ngdart/src/j64_attr_se_nulo.html:5:32 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = (_ctx.limite ?? 0).toString();
    if (import8.checkBinding(this._expr_1, currVal_1, '(limite ?? 0).toString()', 'package:corpus_ngdart/src/j64_attr_se_nulo.html')) {
      import7.updateAttribute(this._el_0, 'data-b', currVal_1) /* REF:package:corpus_ngdart/src/j64_attr_se_nulo.html:38:78 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = (_ctx.chave ?? _ctx.outra);
    if (import8.checkBinding(this._expr_2, currVal_2, 'chave ?? outra', 'package:corpus_ngdart/src/j64_attr_se_nulo.html')) {
      import7.updateAttribute(this._el_0, 'data-c', currVal_2) /* REF:package:corpus_ngdart/src/j64_attr_se_nulo.html:84:114 */;
      this._expr_2 = currVal_2;
    }
    final currVal_4 = ((_ctx.chave ?? _ctx.outra) ?? 'z');
    if (import8.checkBinding(this._expr_4, currVal_4, 'chave ?? outra ?? \'z\'', 'package:corpus_ngdart/src/j64_attr_se_nulo.html')) {
      import7.setAttribute(this._el_0, 'data-f', currVal_4) /* REF:package:corpus_ngdart/src/j64_attr_se_nulo.html:148:185 */;
      this._expr_4 = currVal_4;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J64AttrSeNulo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J64AttrSeNuloNgFactory = ComponentFactory<import1.J64AttrSeNulo>('j64-attr-se-nulo', viewFactory_J64AttrSeNuloHost0);
ComponentFactory<import1.J64AttrSeNulo> get J64AttrSeNuloNgFactory {
  return _J64AttrSeNuloNgFactory;
}

ComponentFactory<import1.J64AttrSeNulo> createJ64AttrSeNuloFactory() {
  return ComponentFactory('j64-attr-se-nulo', viewFactory_J64AttrSeNuloHost0);
}

final List<Object> styles$J64AttrSeNuloHost = const [];

class _ViewJ64AttrSeNuloHost0 extends import10.HostView<import1.J64AttrSeNulo> {
  @override
  void build() {
    this.componentView = ViewJ64AttrSeNulo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J64AttrSeNulo();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J64AttrSeNulo> viewFactory_J64AttrSeNuloHost0() {
  return _ViewJ64AttrSeNuloHost0();
}
