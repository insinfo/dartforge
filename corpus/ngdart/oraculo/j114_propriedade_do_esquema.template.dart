// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j114_propriedade_do_esquema.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j114_propriedade_do_esquema.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/security/safe_html_adapter.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J114PropriedadeDoEsquema = const [];

class ViewJ114PropriedadeDoEsquema0 extends import0.ComponentView<import1.J114PropriedadeDoEsquema> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  Object? _expr_4;
  late final import2.InputElement _el_0;
  late final import2.DivElement _el_1;
  late final import2.HtmlElement _el_2;
  static import3.ComponentStyles? _componentStyles;
  ViewJ114PropriedadeDoEsquema0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j114-propriedade-do-esquema'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.InputElement>(doc, parentRenderNode, 'input');
    this._el_1 = import7.appendDiv(doc, parentRenderNode);
    this._el_2 = import7.appendSpan(doc, parentRenderNode);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.n;
    if (import8.checkBinding(this._expr_0, currVal_0, 'n', 'asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart')) {
      import7.setProperty(this._el_0, 'tabIndex', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart:366:380 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.r;
    if (import8.checkBinding(this._expr_1, currVal_1, 'r', 'asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart')) {
      import7.setProperty(this._el_0, 'readOnly', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart:381:395 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.n;
    if (import8.checkBinding(this._expr_2, currVal_2, '{{n}}', 'asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart')) {
      import7.setProperty(this._el_1, 'tabIndex', import9.interpolate0(currVal_2)) /* REF:asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart:402:418 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = import9.interpolateString0(_ctx.estilo);
    if (import8.checkBinding(this._expr_3, currVal_3, '{{estilo}}', 'asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart')) {
      import7.setProperty(this._el_1, 'style', import10.sanitizeStyle(currVal_3)) /* REF:asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart:419:437 */;
      this._expr_3 = currVal_3;
    }
    final currVal_4 = import9.interpolate1('calc(', (100 - _ctx.n), '%)');
    if (import8.checkBinding(this._expr_4, currVal_4, 'calc({{100-n}}%)', 'asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart')) {
      this._el_2.style.setProperty('width', currVal_4) /* REF:asset:corpus_ngdart/lib/src/j114_propriedade_do_esquema.dart:451:481 */;
      this._expr_4 = currVal_4;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J114PropriedadeDoEsquema, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J114PropriedadeDoEsquemaNgFactory = ComponentFactory<import1.J114PropriedadeDoEsquema>('j114-propriedade-do-esquema', viewFactory_J114PropriedadeDoEsquemaHost0);
ComponentFactory<import1.J114PropriedadeDoEsquema> get J114PropriedadeDoEsquemaNgFactory {
  return _J114PropriedadeDoEsquemaNgFactory;
}

ComponentFactory<import1.J114PropriedadeDoEsquema> createJ114PropriedadeDoEsquemaFactory() {
  return ComponentFactory('j114-propriedade-do-esquema', viewFactory_J114PropriedadeDoEsquemaHost0);
}

final List<Object> styles$J114PropriedadeDoEsquemaHost = const [];

class _ViewJ114PropriedadeDoEsquemaHost0 extends import12.HostView<import1.J114PropriedadeDoEsquema> {
  @override
  void build() {
    this.componentView = ViewJ114PropriedadeDoEsquema0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J114PropriedadeDoEsquema();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J114PropriedadeDoEsquema> viewFactory_J114PropriedadeDoEsquemaHost0() {
  return _ViewJ114PropriedadeDoEsquemaHost0();
}
