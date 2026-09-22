// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c07_atributo_interpolado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c07_atributo_interpolado.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/interpolate.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$C07AtributoInterpolado = const [];

class ViewC07AtributoInterpolado0 extends import0.ComponentView<import1.C07AtributoInterpolado> {
  Object? _expr_0;
  Object? _expr_1;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewC07AtributoInterpolado0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('c07-atributo-interpolado'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/c07_atributo_interpolado.dart' : null);
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
    final currVal_0 = import8.interpolateString1('base ', _ctx.extra, '');
    if (import9.checkBinding(this._expr_0, currVal_0, 'base {{extra}}', 'package:corpus_ngdart/src/c07_atributo_interpolado.html')) {
      this.updateChildClass(this._el_0, currVal_0) /* REF:package:corpus_ngdart/src/c07_atributo_interpolado.html:5:27 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = import8.interpolateString1('t ', _ctx.extra, '');
    if (import9.checkBinding(this._expr_1, currVal_1, 't {{extra}}', 'package:corpus_ngdart/src/c07_atributo_interpolado.html')) {
      import7.setProperty(this._el_0, 'title', currVal_1) /* REF:package:corpus_ngdart/src/c07_atributo_interpolado.html:28:47 */;
      this._expr_1 = currVal_1;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$C07AtributoInterpolado, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C07AtributoInterpoladoNgFactory = ComponentFactory<import1.C07AtributoInterpolado>('c07-atributo-interpolado', viewFactory_C07AtributoInterpoladoHost0);
ComponentFactory<import1.C07AtributoInterpolado> get C07AtributoInterpoladoNgFactory {
  return _C07AtributoInterpoladoNgFactory;
}

ComponentFactory<import1.C07AtributoInterpolado> createC07AtributoInterpoladoFactory() {
  return ComponentFactory('c07-atributo-interpolado', viewFactory_C07AtributoInterpoladoHost0);
}

final List<Object> styles$C07AtributoInterpoladoHost = const [];

class _ViewC07AtributoInterpoladoHost0 extends import11.HostView<import1.C07AtributoInterpolado> {
  @override
  void build() {
    this.componentView = ViewC07AtributoInterpolado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C07AtributoInterpolado();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.C07AtributoInterpolado> viewFactory_C07AtributoInterpoladoHost0() {
  return _ViewC07AtributoInterpoladoHost0();
}
