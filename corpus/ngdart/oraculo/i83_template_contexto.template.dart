// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i83_template_contexto.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i83_template_contexto.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;
import 'package:ngdart/src/common/directives/ng_template_outlet.dart' as import15;
import 'package:ngdart/src/runtime/check_binding.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$I83TemplateContexto = const [];

class ViewI83TemplateContexto0 extends import0.ComponentView<import1.I83TemplateContexto> {
  late final ViewContainer _appEl_0;
  late final TemplateRef _TemplateRef_0_7;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import5.ComponentStyles? _componentStyles;
  ViewI83TemplateContexto0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('i83-template-contexto'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/i83_template_contexto.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    this._TemplateRef_0_7 = TemplateRef(this._appEl_0, viewFactory_I83TemplateContexto1);
    final _anchor_1 = import10.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I83TemplateContexto2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.ver);
    }
    this._NgIf_1_9.ngIf = _ctx.ver /* REF:package:corpus_ngdart/src/i83_template_contexto.html:37:48 */;
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
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$I83TemplateContexto, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I83TemplateContextoNgFactory = ComponentFactory<import1.I83TemplateContexto>('i83-template-contexto', viewFactory_I83TemplateContextoHost0);
ComponentFactory<import1.I83TemplateContexto> get I83TemplateContextoNgFactory {
  return _I83TemplateContextoNgFactory;
}

ComponentFactory<import1.I83TemplateContexto> createI83TemplateContextoFactory() {
  return ComponentFactory('i83-template-contexto', viewFactory_I83TemplateContextoHost0);
}

class _ViewI83TemplateContexto1 extends import13.EmbeddedView<import1.I83TemplateContexto> {
  _ViewI83TemplateContexto1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    final _text_1 = import10.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_I83TemplateContexto1(import14.RenderView parentView, int parentIndex) {
  return _ViewI83TemplateContexto1(parentView, parentIndex);
}

class _ViewI83TemplateContexto2 extends import13.EmbeddedView<import1.I83TemplateContexto> {
  late final ViewContainer _appEl_1;
  late final import15.NgTemplateOutlet _NgTemplateOutlet_1_9;
  Object? _expr_0;
  Object? _expr_1;
  _ViewI83TemplateContexto2(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I83TemplateContexto3);
    this._NgTemplateOutlet_1_9 = import15.NgTemplateOutlet(this._appEl_1);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgTemplateOutlet_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final local_t = import8.unsafeCast<ViewI83TemplateContexto0>((this.parentView!))._TemplateRef_0_7;
    final currVal_0 = local_t;
    if (import16.checkBinding(this._expr_0, currVal_0, 't', 'package:corpus_ngdart/src/i83_template_contexto.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutlet', currVal_0);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutlet = currVal_0 /* REF:package:corpus_ngdart/src/i83_template_contexto.html:55:90 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.ctx;
    if (import16.checkBinding(this._expr_1, currVal_1, 'ctx', 'package:corpus_ngdart/src/i83_template_contexto.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgTemplateOutlet_1_9, 'ngTemplateOutletContext', currVal_1);
      }
      this._NgTemplateOutlet_1_9.ngTemplateOutletContext = currVal_1 /* REF:package:corpus_ngdart/src/i83_template_contexto.html:55:90 */;
      this._expr_1 = currVal_1;
    }
    if ((!import16.debugThrowIfChanged)) {
      this._NgTemplateOutlet_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import13.EmbeddedView<void> viewFactory_I83TemplateContexto2(import14.RenderView parentView, int parentIndex) {
  return _ViewI83TemplateContexto2(parentView, parentIndex);
}

class _ViewI83TemplateContexto3 extends import13.EmbeddedView<import1.I83TemplateContexto> {
  _ViewI83TemplateContexto3(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('span'));
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_I83TemplateContexto3(import14.RenderView parentView, int parentIndex) {
  return _ViewI83TemplateContexto3(parentView, parentIndex);
}

final List<Object> styles$I83TemplateContextoHost = const [];

class _ViewI83TemplateContextoHost0 extends import17.HostView<import1.I83TemplateContexto> {
  @override
  void build() {
    this.componentView = ViewI83TemplateContexto0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I83TemplateContexto();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.I83TemplateContexto> viewFactory_I83TemplateContextoHost0() {
  return _ViewI83TemplateContextoHost0();
}
