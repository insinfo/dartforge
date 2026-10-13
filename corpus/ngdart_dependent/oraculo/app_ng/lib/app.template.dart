// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'app.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'app.dart' as import1;
import 'package:dep_ng/src/botao.template.dart' as import2;
import 'package:dep_ng/src/botao.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$App = const [];

class ViewApp0 extends import0.ComponentView<import1.App> {
  late final import2.ViewDepBotao0 _compView_0;
  late final import3.DepBotao _DepBotao_0_5;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewApp0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('app'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:app_ng/lib/app.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewDepBotao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._DepBotao_0_5 = import3.DepBotao();
    this._compView_0.create(this._DepBotao_0_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.texto;
    if (import9.checkBinding(this._expr_0, currVal_0, 'texto', 'asset:app_ng/lib/app.dart')) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._DepBotao_0_5, 'rotulo', currVal_0);
      }
      this._DepBotao_0_5.rotulo = currVal_0 /* REF:asset:app_ng/lib/app.dart:131:147 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$App, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _AppNgFactory = ComponentFactory<import1.App>('app', viewFactory_AppHost0);
ComponentFactory<import1.App> get AppNgFactory {
  return _AppNgFactory;
}

ComponentFactory<import1.App> createAppFactory() {
  return ComponentFactory('app', viewFactory_AppHost0);
}

final List<Object> styles$AppHost = const [];

class _ViewAppHost0 extends import12.HostView<import1.App> {
  @override
  void build() {
    this.componentView = ViewApp0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.App();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.App> viewFactory_AppHost0() {
  return _ViewAppHost0();
}
