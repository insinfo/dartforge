// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i38_estilo_tipos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i38_estilo_tipos.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I38EstiloTipos = const [];

class ViewI38EstiloTipos0 extends import0.ComponentView<import1.I38EstiloTipos> {
  Object? _expr_0;
  Object? _expr_2;
  Object? _expr_3;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI38EstiloTipos0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i38-estilo-tipos'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i38_estilo_tipos.dart' : null);
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
      this._el_0.style.setProperty('z-index', 1.toString()) /* REF:package:corpus_ngdart/src/i38_estilo_tipos.html:33:52 */;
      this._el_0.style.setProperty('left', ((0 == null) ? null : (0.toString() + 'px'))) /* REF:package:corpus_ngdart/src/i38_estilo_tipos.html:102:121 */;
    }
    final currVal_0 = _ctx.opacidade;
    if (import8.checkBinding(this._expr_0, currVal_0, 'opacidade', 'package:corpus_ngdart/src/i38_estilo_tipos.html')) {
      this._el_0.style.setProperty('opacity', currVal_0?.toString()) /* REF:package:corpus_ngdart/src/i38_estilo_tipos.html:5:32 */;
      this._expr_0 = currVal_0;
    }
    final currVal_2 = _ctx.tamanho;
    if (import8.checkBinding(this._expr_2, currVal_2, 'tamanho', 'package:corpus_ngdart/src/i38_estilo_tipos.html')) {
      this._el_0.style.setProperty('width', ((currVal_2 == null) ? null : (currVal_2 + 'em'))) /* REF:package:corpus_ngdart/src/i38_estilo_tipos.html:53:79 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = _ctx.topo;
    if (import8.checkBinding(this._expr_3, currVal_3, 'topo', 'package:corpus_ngdart/src/i38_estilo_tipos.html')) {
      this._el_0.style.setProperty('top', ((currVal_3 == null) ? null : (currVal_3.toString() + 'px'))) /* REF:package:corpus_ngdart/src/i38_estilo_tipos.html:80:101 */;
      this._expr_3 = currVal_3;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I38EstiloTipos, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I38EstiloTiposNgFactory = ComponentFactory<import1.I38EstiloTipos>('i38-estilo-tipos', viewFactory_I38EstiloTiposHost0);
ComponentFactory<import1.I38EstiloTipos> get I38EstiloTiposNgFactory {
  return _I38EstiloTiposNgFactory;
}

ComponentFactory<import1.I38EstiloTipos> createI38EstiloTiposFactory() {
  return ComponentFactory('i38-estilo-tipos', viewFactory_I38EstiloTiposHost0);
}

final List<Object> styles$I38EstiloTiposHost = const [];

class _ViewI38EstiloTiposHost0 extends import10.HostView<import1.I38EstiloTipos> {
  @override
  void build() {
    this.componentView = ViewI38EstiloTipos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I38EstiloTipos();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I38EstiloTipos> viewFactory_I38EstiloTiposHost0() {
  return _ViewI38EstiloTiposHost0();
}
