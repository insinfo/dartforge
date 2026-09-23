// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'f02_dois_ifs_irmaos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'f02_dois_ifs_irmaos.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import13;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$F02DoisIfsIrmaos = const [];

class ViewF02DoisIfsIrmaos0 extends import0.ComponentView<import1.F02DoisIfsIrmaos> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import4.ComponentStyles? _componentStyles;
  ViewF02DoisIfsIrmaos0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('f02-dois-ifs-irmaos'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/f02_dois_ifs_irmaos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_F02DoisIfsIrmaos1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_F02DoisIfsIrmaos3);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.a);
    }
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/f02_dois_ifs_irmaos.html:5:14 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.c);
    }
    this._NgIf_1_9.ngIf = _ctx.c /* REF:package:corpus_ngdart/src/f02_dois_ifs_irmaos.html:48:57 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$F02DoisIfsIrmaos, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _F02DoisIfsIrmaosNgFactory = ComponentFactory<import1.F02DoisIfsIrmaos>('f02-dois-ifs-irmaos', viewFactory_F02DoisIfsIrmaosHost0);
ComponentFactory<import1.F02DoisIfsIrmaos> get F02DoisIfsIrmaosNgFactory {
  return _F02DoisIfsIrmaosNgFactory;
}

ComponentFactory<import1.F02DoisIfsIrmaos> createF02DoisIfsIrmaosFactory() {
  return ComponentFactory('f02-dois-ifs-irmaos', viewFactory_F02DoisIfsIrmaosHost0);
}

class _ViewF02DoisIfsIrmaos1 extends import13.EmbeddedView<import1.F02DoisIfsIrmaos> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewF02DoisIfsIrmaos1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_F02DoisIfsIrmaos2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.b);
    }
    this._NgIf_1_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/f02_dois_ifs_irmaos.html:21:30 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import13.EmbeddedView<void> viewFactory_F02DoisIfsIrmaos1(import14.RenderView parentView, int parentIndex) {
  return _ViewF02DoisIfsIrmaos1(parentView, parentIndex);
}

class _ViewF02DoisIfsIrmaos2 extends import13.EmbeddedView<import1.F02DoisIfsIrmaos> {
  _ViewF02DoisIfsIrmaos2(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('span'));
    final _text_1 = import9.appendText(_el_0, 'x');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_F02DoisIfsIrmaos2(import14.RenderView parentView, int parentIndex) {
  return _ViewF02DoisIfsIrmaos2(parentView, parentIndex);
}

class _ViewF02DoisIfsIrmaos3 extends import13.EmbeddedView<import1.F02DoisIfsIrmaos> {
  _ViewF02DoisIfsIrmaos3(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'y');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_F02DoisIfsIrmaos3(import14.RenderView parentView, int parentIndex) {
  return _ViewF02DoisIfsIrmaos3(parentView, parentIndex);
}

final List<Object> styles$F02DoisIfsIrmaosHost = const [];

class _ViewF02DoisIfsIrmaosHost0 extends import15.HostView<import1.F02DoisIfsIrmaos> {
  @override
  void build() {
    this.componentView = ViewF02DoisIfsIrmaos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.F02DoisIfsIrmaos();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.F02DoisIfsIrmaos> viewFactory_F02DoisIfsIrmaosHost0() {
  return _ViewF02DoisIfsIrmaosHost0();
}
