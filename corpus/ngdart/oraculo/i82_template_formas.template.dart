// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i82_template_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i82_template_formas.dart' as import1;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/interpolate.dart' as import17;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import18;

final List<Object> styles$I82TemplateFormas = const [];

class ViewI82TemplateFormas0 extends import0.ComponentView<import1.I82TemplateFormas> {
  late final ViewContainer _appEl_1;
  late final TemplateRef _TemplateRef_1_7;
  late final ViewContainer _appEl_2;
  late final TemplateRef _TemplateRef_2_7;
  late final ViewContainer _appEl_3;
  late final ViewContainer _appEl_4;
  late final import4.NgTemplateOutlet _NgTemplateOutlet_4_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewI82TemplateFormas0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i82-template-formas'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i82_template_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import9.document;
    final _el_0 = import10.appendDiv(doc, parentRenderNode);
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    this._TemplateRef_1_7 = TemplateRef(this._appEl_1, viewFactory_I82TemplateFormas1);
    final _anchor_2 = import10.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    this._TemplateRef_2_7 = TemplateRef(this._appEl_2, viewFactory_I82TemplateFormas2);
    final _anchor_3 = import10.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_7 = TemplateRef(this._appEl_3, viewFactory_I82TemplateFormas3);
    final _anchor_4 = import10.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_I82TemplateFormas4);
    this._NgTemplateOutlet_4_9 = import4.NgTemplateOutlet(this._appEl_4);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_4, this._NgTemplateOutlet_4_9);
    }
  }

  @override
  void detectChangesInternal() {
    final local_t = this._TemplateRef_1_7;
    final currVal_0 = local_t;
    if (import12.checkBinding(this._expr_0, currVal_0, 't', 'package:corpus_ngdart/src/i82_template_formas.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_4_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_4_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/i82_template_formas.html:133:154 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgTemplateOutlet_4_9.ngDoCheck();
    }
    this._appEl_4.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I82TemplateFormas, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I82TemplateFormasNgFactory = ComponentFactory<import1.I82TemplateFormas>('i82-template-formas', viewFactory_I82TemplateFormasHost0);
ComponentFactory<import1.I82TemplateFormas> get I82TemplateFormasNgFactory {
  return _I82TemplateFormasNgFactory;
}

ComponentFactory<import1.I82TemplateFormas> createI82TemplateFormasFactory() {
  return ComponentFactory('i82-template-formas', viewFactory_I82TemplateFormasHost0);
}

class _ViewI82TemplateFormas1 extends import14.EmbeddedView<import1.I82TemplateFormas> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewI82TemplateFormas1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import17.interpolateString0(_ctx.nome)) /* REF:package:corpus_ngdart/src/i82_template_formas.html:24:32 */;
  }
}

import14.EmbeddedView<void> viewFactory_I82TemplateFormas1(import16.RenderView parentView, int parentIndex) {
  return _ViewI82TemplateFormas1(parentView, parentIndex);
}

class _ViewI82TemplateFormas2 extends import14.EmbeddedView<import1.I82TemplateFormas> {
  _ViewI82TemplateFormas2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('b'));
    final _text_1 = import10.appendText(_el_0, 'u');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I82TemplateFormas2(import16.RenderView parentView, int parentIndex) {
  return _ViewI82TemplateFormas2(parentView, parentIndex);
}

class _ViewI82TemplateFormas3 extends import14.EmbeddedView<import1.I82TemplateFormas> {
  _ViewI82TemplateFormas3(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('i'));
    final _text_1 = import10.appendText(_el_0, 'sem');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_I82TemplateFormas3(import16.RenderView parentView, int parentIndex) {
  return _ViewI82TemplateFormas3(parentView, parentIndex);
}

class _ViewI82TemplateFormas4 extends import14.EmbeddedView<import1.I82TemplateFormas> {
  _ViewI82TemplateFormas4(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this.initRootNodesAndSubscriptions(import8.unsafeCast(const <Object>[]), null);
  }
}

import14.EmbeddedView<void> viewFactory_I82TemplateFormas4(import16.RenderView parentView, int parentIndex) {
  return _ViewI82TemplateFormas4(parentView, parentIndex);
}

final List<Object> styles$I82TemplateFormasHost = const [];

class _ViewI82TemplateFormasHost0 extends import18.HostView<import1.I82TemplateFormas> {
  @override
  void build() {
    this.componentView = ViewI82TemplateFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I82TemplateFormas();
    this.initRootNode(_el_0);
  }
}

import18.HostView<import1.I82TemplateFormas> viewFactory_I82TemplateFormasHost0() {
  return _ViewI82TemplateFormasHost0();
}
