// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd03_filho_com_entrada.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd03_filho_com_entrada.dart' as import1;
import 'a16_entrada_e_saida.template.dart' as import2;
import 'a16_entrada_e_saida.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$D03FilhoComEntrada = const [];

class ViewD03FilhoComEntrada0 extends import0.ComponentView<import1.D03FilhoComEntrada> {
  late final import2.ViewA16EntradaESaida0 _compView_0;
  late final import3.A16EntradaESaida _A16EntradaESaida_0_5;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewD03FilhoComEntrada0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('d03-filho-com-entrada'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/d03_filho_com_entrada.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA16EntradaESaida0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A16EntradaESaida_0_5 = import3.A16EntradaESaida();
    this._compView_0.create(this._A16EntradaESaida_0_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.valor;
    if (import9.checkBinding(this._expr_0, currVal_0, 'valor', 'package:corpus_ngdart/src/d03_filho_com_entrada.html')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._A16EntradaESaida_0_5, 'titulo', currVal_0);
      }
      this._A16EntradaESaida_0_5.titulo = currVal_0 /* REF:package:corpus_ngdart/src/d03_filho_com_entrada.html:21:37 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$D03FilhoComEntrada, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D03FilhoComEntradaNgFactory = ComponentFactory<import1.D03FilhoComEntrada>('d03-filho-com-entrada', viewFactory_D03FilhoComEntradaHost0);
ComponentFactory<import1.D03FilhoComEntrada> get D03FilhoComEntradaNgFactory {
  return _D03FilhoComEntradaNgFactory;
}

ComponentFactory<import1.D03FilhoComEntrada> createD03FilhoComEntradaFactory() {
  return ComponentFactory('d03-filho-com-entrada', viewFactory_D03FilhoComEntradaHost0);
}

final List<Object> styles$D03FilhoComEntradaHost = const [];

class _ViewD03FilhoComEntradaHost0 extends import12.HostView<import1.D03FilhoComEntrada> {
  @override
  void build() {
    this.componentView = ViewD03FilhoComEntrada0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D03FilhoComEntrada();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.D03FilhoComEntrada> viewFactory_D03FilhoComEntradaHost0() {
  return _ViewD03FilhoComEntradaHost0();
}
