// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i21_ng_container_for.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i21_ng_container_for.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$I21NgContainerFor = const [];

class ViewI21NgContainerFor0 extends import0.ComponentView<import1.I21NgContainerFor> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI21NgContainerFor0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i21-ng-container-for'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i21_ng_container_for.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I21NgContainerFor1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i21_ng_container_for.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i21_ng_container_for.html:14:37 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I21NgContainerFor, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I21NgContainerForNgFactory = ComponentFactory<import1.I21NgContainerFor>('i21-ng-container-for', viewFactory_I21NgContainerForHost0);
ComponentFactory<import1.I21NgContainerFor> get I21NgContainerForNgFactory {
  return _I21NgContainerForNgFactory;
}

ComponentFactory<import1.I21NgContainerFor> createI21NgContainerForFactory() {
  return ComponentFactory('i21-ng-container-for', viewFactory_I21NgContainerForHost0);
}

class _ViewI21NgContainerFor1 extends import14.EmbeddedView<import1.I21NgContainerFor> {
  final import15.TextBinding _textBinding_0 = import15.TextBinding();
  _ViewI21NgContainerFor1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNode(this._textBinding_0.element);
  }

  @override
  void detectChangesInternal() {
    final local_x = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_0.updateText(import18.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i21_ng_container_for.html:38:43 */;
  }
}

import14.EmbeddedView<void> viewFactory_I21NgContainerFor1(import16.RenderView parentView, int parentIndex) {
  return _ViewI21NgContainerFor1(parentView, parentIndex);
}

final List<Object> styles$I21NgContainerForHost = const [];

class _ViewI21NgContainerForHost0 extends import19.HostView<import1.I21NgContainerFor> {
  @override
  void build() {
    this.componentView = ViewI21NgContainerFor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I21NgContainerFor();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I21NgContainerFor> viewFactory_I21NgContainerForHost0() {
  return _ViewI21NgContainerForHost0();
}
