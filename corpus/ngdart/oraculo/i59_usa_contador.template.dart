// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i59_usa_contador.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i59_usa_contador.dart' as import1;
import 'i59_contador.template.dart' as import2;
import 'i59_contador.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I59UsaContador = const [];

class ViewI59UsaContador0 extends import0.ComponentView<import1.I59UsaContador> {
  late final import2.ViewI59Contador0 _compView_0;
  late final import3.I59Contador _I59Contador_0_5;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI59UsaContador0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i59-usa-contador'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i59_usa_contador.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewI59Contador0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I59Contador_0_5 = import3.I59Contador();
    this._compView_0.create(this._I59Contador_0_5);
    final subscription_0 = this._I59Contador_0_5.valorChange.listen(this.eventHandler1(this._handleEvent_0));
    this.initSubscriptions([subscription_0]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.n;
    if (import9.checkBinding(this._expr_0, currVal_0, 'n', 'package:corpus_ngdart/src/i59_usa_contador.html')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._I59Contador_0_5, 'valor', currVal_0);
      }
      this._I59Contador_0_5.valor = currVal_0 /* REF:package:corpus_ngdart/src/i59_usa_contador.html:14:27 */;
      this._expr_0 = currVal_0;
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.n = $event;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I59UsaContador, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I59UsaContadorNgFactory = ComponentFactory<import1.I59UsaContador>('i59-usa-contador', viewFactory_I59UsaContadorHost0);
ComponentFactory<import1.I59UsaContador> get I59UsaContadorNgFactory {
  return _I59UsaContadorNgFactory;
}

ComponentFactory<import1.I59UsaContador> createI59UsaContadorFactory() {
  return ComponentFactory('i59-usa-contador', viewFactory_I59UsaContadorHost0);
}

final List<Object> styles$I59UsaContadorHost = const [];

class _ViewI59UsaContadorHost0 extends import12.HostView<import1.I59UsaContador> {
  @override
  void build() {
    this.componentView = ViewI59UsaContador0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I59UsaContador();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I59UsaContador> viewFactory_I59UsaContadorHost0() {
  return _ViewI59UsaContadorHost0();
}
