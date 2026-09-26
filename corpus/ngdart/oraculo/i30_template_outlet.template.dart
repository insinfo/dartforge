// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i30_template_outlet.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i30_template_outlet.dart' as import1;
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

final List<Object> styles$I30TemplateOutlet = const [];

class ViewI30TemplateOutlet0 extends import0.ComponentView<import1.I30TemplateOutlet> {
  late final ViewContainer _appEl_0;
  late final TemplateRef _TemplateRef_0_7;
  late final ViewContainer _appEl_1;
  late final import4.NgTemplateOutlet _NgTemplateOutlet_1_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewI30TemplateOutlet0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i30-template-outlet'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i30_template_outlet.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    this._TemplateRef_0_7 = TemplateRef(this._appEl_0, viewFactory_I30TemplateOutlet1);
    final _anchor_1 = import10.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I30TemplateOutlet2);
    this._NgTemplateOutlet_1_9 = import4.NgTemplateOutlet(this._appEl_1);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgTemplateOutlet_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final local_t = this._TemplateRef_0_7;
    final currVal_0 = local_t;
    if (import12.checkBinding(this._expr_0, currVal_0, 't', 'package:corpus_ngdart/src/i30_template_outlet.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/i30_template_outlet.html:37:58 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgTemplateOutlet_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I30TemplateOutlet, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I30TemplateOutletNgFactory = ComponentFactory<import1.I30TemplateOutlet>('i30-template-outlet', viewFactory_I30TemplateOutletHost0);
ComponentFactory<import1.I30TemplateOutlet> get I30TemplateOutletNgFactory {
  return _I30TemplateOutletNgFactory;
}

ComponentFactory<import1.I30TemplateOutlet> createI30TemplateOutletFactory() {
  return ComponentFactory('i30-template-outlet', viewFactory_I30TemplateOutletHost0);
}

class _ViewI30TemplateOutlet1 extends import14.EmbeddedView<import1.I30TemplateOutlet> {
  _ViewI30TemplateOutlet1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    final _text_1 = import10.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I30TemplateOutlet1(import15.RenderView parentView, int parentIndex) {
  return _ViewI30TemplateOutlet1(parentView, parentIndex);
}

class _ViewI30TemplateOutlet2 extends import14.EmbeddedView<import1.I30TemplateOutlet> {
  _ViewI30TemplateOutlet2(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I30TemplateOutlet2(import15.RenderView parentView, int parentIndex) {
  return _ViewI30TemplateOutlet2(parentView, parentIndex);
}

final List<Object> styles$I30TemplateOutletHost = const [];

class _ViewI30TemplateOutletHost0 extends import16.HostView<import1.I30TemplateOutlet> {
  @override
  void build() {
    this.componentView = ViewI30TemplateOutlet0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I30TemplateOutlet();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.I30TemplateOutlet> viewFactory_I30TemplateOutletHost0() {
  return _ViewI30TemplateOutletHost0();
}
