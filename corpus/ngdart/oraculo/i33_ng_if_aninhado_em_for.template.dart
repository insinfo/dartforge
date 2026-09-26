// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i33_ng_if_aninhado_em_for.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i33_ng_if_aninhado_em_for.dart' as import1;
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
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$I33NgIfAninhadoEmFor = const [];

class ViewI33NgIfAninhadoEmFor0 extends import0.ComponentView<import1.I33NgIfAninhadoEmFor> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI33NgIfAninhadoEmFor0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i33-ng-if-aninhado-em-for'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i33_ng_if_aninhado_em_for.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I33NgIfAninhadoEmFor1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.pessoas;
    if (import12.checkBinding(this._expr_0, currVal_0, 'pessoas', 'package:corpus_ngdart/src/i33_ng_if_aninhado_em_for.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i33_ng_if_aninhado_em_for.html:5:30 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I33NgIfAninhadoEmFor, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I33NgIfAninhadoEmForNgFactory = ComponentFactory<import1.I33NgIfAninhadoEmFor>('i33-ng-if-aninhado-em-for', viewFactory_I33NgIfAninhadoEmForHost0);
ComponentFactory<import1.I33NgIfAninhadoEmFor> get I33NgIfAninhadoEmForNgFactory {
  return _I33NgIfAninhadoEmForNgFactory;
}

ComponentFactory<import1.I33NgIfAninhadoEmFor> createI33NgIfAninhadoEmForFactory() {
  return ComponentFactory('i33-ng-if-aninhado-em-for', viewFactory_I33NgIfAninhadoEmForHost0);
}

class _ViewI33NgIfAninhadoEmFor1 extends import14.EmbeddedView<import1.I33NgIfAninhadoEmFor> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewI33NgIfAninhadoEmFor1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I33NgIfAninhadoEmFor2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_p = import7.unsafeCast<import1.Pessoa>(this.locals['\$implicit']);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', (local_p.idade > 0));
    }
    this._NgIf_1_9.ngIf = (local_p.idade > 0) /* REF:package:corpus_ngdart/src/i33_ng_if_aninhado_em_for.html:37:56 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import14.EmbeddedView<void> viewFactory_I33NgIfAninhadoEmFor1(import16.RenderView parentView, int parentIndex) {
  return _ViewI33NgIfAninhadoEmFor1(parentView, parentIndex);
}

class _ViewI33NgIfAninhadoEmFor2 extends import14.EmbeddedView<import1.I33NgIfAninhadoEmFor> {
  final import17.TextBinding _textBinding_1 = import17.TextBinding();
  _ViewI33NgIfAninhadoEmFor2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_p = import7.unsafeCast<import1.Pessoa>(import7.unsafeCast<_ViewI33NgIfAninhadoEmFor1>((this.parentView!)).locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_p.nome)) /* REF:package:corpus_ngdart/src/i33_ng_if_aninhado_em_for.html:57:67 */;
  }
}

import14.EmbeddedView<void> viewFactory_I33NgIfAninhadoEmFor2(import16.RenderView parentView, int parentIndex) {
  return _ViewI33NgIfAninhadoEmFor2(parentView, parentIndex);
}

final List<Object> styles$I33NgIfAninhadoEmForHost = const [];

class _ViewI33NgIfAninhadoEmForHost0 extends import19.HostView<import1.I33NgIfAninhadoEmFor> {
  @override
  void build() {
    this.componentView = ViewI33NgIfAninhadoEmFor0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I33NgIfAninhadoEmFor();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I33NgIfAninhadoEmFor> viewFactory_I33NgIfAninhadoEmForHost0() {
  return _ViewI33NgIfAninhadoEmForHost0();
}
