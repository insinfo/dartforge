// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i05_estilo_unidade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i05_estilo_unidade.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I05EstiloUnidade = const [];

class ViewI05EstiloUnidade0 extends import0.ComponentView<import1.I05EstiloUnidade> {
  Object? _expr_0;
  Object? _expr_1;
  late final import2.DivElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI05EstiloUnidade0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i05-estilo-unidade'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i05_estilo_unidade.dart' : null);
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
    final currVal_0 = _ctx.largura;
    if (import8.checkBinding(this._expr_0, currVal_0, 'largura', 'package:corpus_ngdart/src/i05_estilo_unidade.html')) {
      this._el_0.style.setProperty('width', ((currVal_0 == null) ? null : (currVal_0.toString() + 'px'))) /* REF:package:corpus_ngdart/src/i05_estilo_unidade.html:5:31 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.altura;
    if (import8.checkBinding(this._expr_1, currVal_1, 'altura', 'package:corpus_ngdart/src/i05_estilo_unidade.html')) {
      this._el_0.style.setProperty('height', ((currVal_1 == null) ? null : (currVal_1.toString() + '%'))) /* REF:package:corpus_ngdart/src/i05_estilo_unidade.html:32:57 */;
      this._expr_1 = currVal_1;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I05EstiloUnidade, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I05EstiloUnidadeNgFactory = ComponentFactory<import1.I05EstiloUnidade>('i05-estilo-unidade', viewFactory_I05EstiloUnidadeHost0);
ComponentFactory<import1.I05EstiloUnidade> get I05EstiloUnidadeNgFactory {
  return _I05EstiloUnidadeNgFactory;
}

ComponentFactory<import1.I05EstiloUnidade> createI05EstiloUnidadeFactory() {
  return ComponentFactory('i05-estilo-unidade', viewFactory_I05EstiloUnidadeHost0);
}

final List<Object> styles$I05EstiloUnidadeHost = const [];

class _ViewI05EstiloUnidadeHost0 extends import10.HostView<import1.I05EstiloUnidade> {
  @override
  void build() {
    this.componentView = ViewI05EstiloUnidade0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I05EstiloUnidade();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I05EstiloUnidade> viewFactory_I05EstiloUnidadeHost0() {
  return _ViewI05EstiloUnidadeHost0();
}
