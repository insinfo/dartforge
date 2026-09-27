// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i87_template_view_child_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i87_template_view_child_formas.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/common/directives/ng_template_outlet.dart' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$I87TemplateViewChildFormas = const [];

class ViewI87TemplateViewChildFormas0 extends import0.ComponentView<import1.I87TemplateViewChildFormas> {
  late final ViewContainer _appEl_1;
  late final TemplateRef _TemplateRef_1_7;
  late final ViewContainer _appEl_2;
  late final TemplateRef _TemplateRef_2_7;
  late final ViewContainer _appEl_3;
  late final import4.NgTemplateOutlet _NgTemplateOutlet_3_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewI87TemplateViewChildFormas0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i87-template-view-child-formas'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i87_template_view_child_formas.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    this._TemplateRef_1_7 = TemplateRef(this._appEl_1, viewFactory_I87TemplateViewChildFormas1);
    final _anchor_2 = import10.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    this._TemplateRef_2_7 = TemplateRef(this._appEl_2, viewFactory_I87TemplateViewChildFormas2);
    final _anchor_3 = import10.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I87TemplateViewChildFormas3);
    this._NgTemplateOutlet_3_9 = import4.NgTemplateOutlet(this._appEl_3);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_3, this._NgTemplateOutlet_3_9);
    }
    _ctx.usado = this._TemplateRef_2_7;
    _ctx.dentro = this._TemplateRef_1_7;
  }

  @override
  void detectChangesInternal() {
    final local_u = this._TemplateRef_2_7;
    final currVal_0 = local_u;
    if (import12.checkBinding(this._expr_0, currVal_0, 'u', 'package:corpus_ngdart/src/i87_template_view_child_formas.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_3_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_3_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/i87_template_view_child_formas.html:73:94 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgTemplateOutlet_3_9.ngDoCheck();
    }
    this._appEl_3.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I87TemplateViewChildFormas, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I87TemplateViewChildFormasNgFactory = ComponentFactory<import1.I87TemplateViewChildFormas>('i87-template-view-child-formas', viewFactory_I87TemplateViewChildFormasHost0);
ComponentFactory<import1.I87TemplateViewChildFormas> get I87TemplateViewChildFormasNgFactory {
  return _I87TemplateViewChildFormasNgFactory;
}

ComponentFactory<import1.I87TemplateViewChildFormas> createI87TemplateViewChildFormasFactory() {
  return ComponentFactory('i87-template-view-child-formas', viewFactory_I87TemplateViewChildFormasHost0);
}

class _ViewI87TemplateViewChildFormas1 extends import14.EmbeddedView<import1.I87TemplateViewChildFormas> {
  _ViewI87TemplateViewChildFormas1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    final _text_1 = import10.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I87TemplateViewChildFormas1(import15.RenderView parentView, int parentIndex) {
  return _ViewI87TemplateViewChildFormas1(parentView, parentIndex);
}

class _ViewI87TemplateViewChildFormas2 extends import14.EmbeddedView<import1.I87TemplateViewChildFormas> {
  _ViewI87TemplateViewChildFormas2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _text_0 = import10.createText('y');
    this.initRootNode(_text_0);
  }
}

import14.EmbeddedView<void> viewFactory_I87TemplateViewChildFormas2(import15.RenderView parentView, int parentIndex) {
  return _ViewI87TemplateViewChildFormas2(parentView, parentIndex);
}

class _ViewI87TemplateViewChildFormas3 extends import14.EmbeddedView<import1.I87TemplateViewChildFormas> {
  _ViewI87TemplateViewChildFormas3(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I87TemplateViewChildFormas3(import15.RenderView parentView, int parentIndex) {
  return _ViewI87TemplateViewChildFormas3(parentView, parentIndex);
}

final List<Object> styles$I87TemplateViewChildFormasHost = const [];

class _ViewI87TemplateViewChildFormasHost0 extends import16.HostView<import1.I87TemplateViewChildFormas> {
  @override
  void build() {
    this.componentView = ViewI87TemplateViewChildFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I87TemplateViewChildFormas();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.I87TemplateViewChildFormas> viewFactory_I87TemplateViewChildFormasHost0() {
  return _ViewI87TemplateViewChildFormasHost0();
}
