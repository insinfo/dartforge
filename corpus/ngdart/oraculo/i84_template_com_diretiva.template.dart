// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i84_template_com_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i84_template_com_diretiva.dart' as import1;
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

final List<Object> styles$I84TemplateComDiretiva = const [];

class ViewI84TemplateComDiretiva0 extends import0.ComponentView<import1.I84TemplateComDiretiva> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI84TemplateComDiretiva0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i84-template-com-diretiva'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i84_template_com_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I84TemplateComDiretiva1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i84_template_com_diretiva.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i84_template_com_diretiva.html:25:42 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I84TemplateComDiretiva, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I84TemplateComDiretivaNgFactory = ComponentFactory<import1.I84TemplateComDiretiva>('i84-template-com-diretiva', viewFactory_I84TemplateComDiretivaHost0);
ComponentFactory<import1.I84TemplateComDiretiva> get I84TemplateComDiretivaNgFactory {
  return _I84TemplateComDiretivaNgFactory;
}

ComponentFactory<import1.I84TemplateComDiretiva> createI84TemplateComDiretivaFactory() {
  return ComponentFactory('i84-template-com-diretiva', viewFactory_I84TemplateComDiretivaHost0);
}

class _ViewI84TemplateComDiretiva1 extends import14.EmbeddedView<import1.I84TemplateComDiretiva> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  _ViewI84TemplateComDiretiva1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_1.updateText(import18.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/i84_template_com_diretiva.html:46:54 */;
  }
}

import14.EmbeddedView<void> viewFactory_I84TemplateComDiretiva1(import16.RenderView parentView, int parentIndex) {
  return _ViewI84TemplateComDiretiva1(parentView, parentIndex);
}

final List<Object> styles$I84TemplateComDiretivaHost = const [];

class _ViewI84TemplateComDiretivaHost0 extends import19.HostView<import1.I84TemplateComDiretiva> {
  @override
  void build() {
    this.componentView = ViewI84TemplateComDiretiva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I84TemplateComDiretiva();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I84TemplateComDiretiva> viewFactory_I84TemplateComDiretivaHost0() {
  return _ViewI84TemplateComDiretivaHost0();
}
