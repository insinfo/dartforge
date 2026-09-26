// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i12_ng_style.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i12_ng_style.dart' as import1;
import 'package:ngdart/src/common/directives/ng_style.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/devtools.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I12NgStyle = const [];

class ViewI12NgStyle0 extends import0.ComponentView<import1.I12NgStyle> {
  late final import2.NgStyle _NgStyle_0_5;
  Object? _expr_0;
  static import3.ComponentStyles? _componentStyles;
  ViewI12NgStyle0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i12-ng-style'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i12_ng_style.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this._NgStyle_0_5 = import2.NgStyle(_el_0);
    if (import9.isDevToolsEnabled) {
      import9.Inspector.instance.registerDirective(_el_0, this._NgStyle_0_5);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.estilos;
    if (import10.checkBinding(this._expr_0, currVal_0, 'estilos', 'package:corpus_ngdart/src/i12_ng_style.html')) {
      if (import9.isDevToolsEnabled) {
        import9.Inspector.instance.recordInput(this._NgStyle_0_5, 'ngStyle', currVal_0);
      }
      this._NgStyle_0_5.rawStyle = currVal_0 /* REF:package:corpus_ngdart/src/i12_ng_style.html:5:24 */;
      this._expr_0 = currVal_0;
    }
    if ((!import10.debugThrowIfChanged)) {
      this._NgStyle_0_5.ngDoCheck();
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I12NgStyle, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I12NgStyleNgFactory = ComponentFactory<import1.I12NgStyle>('i12-ng-style', viewFactory_I12NgStyleHost0);
ComponentFactory<import1.I12NgStyle> get I12NgStyleNgFactory {
  return _I12NgStyleNgFactory;
}

ComponentFactory<import1.I12NgStyle> createI12NgStyleFactory() {
  return ComponentFactory('i12-ng-style', viewFactory_I12NgStyleHost0);
}

final List<Object> styles$I12NgStyleHost = const [];

class _ViewI12NgStyleHost0 extends import12.HostView<import1.I12NgStyle> {
  @override
  void build() {
    this.componentView = ViewI12NgStyle0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I12NgStyle();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I12NgStyle> viewFactory_I12NgStyleHost0() {
  return _ViewI12NgStyleHost0();
}
