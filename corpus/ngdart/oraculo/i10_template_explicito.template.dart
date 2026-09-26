// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i10_template_explicito.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i10_template_explicito.dart' as import1;
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

final List<Object> styles$I10TemplateExplicito = const [];

class ViewI10TemplateExplicito0 extends import0.ComponentView<import1.I10TemplateExplicito> {
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewI10TemplateExplicito0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i10-template-explicito'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i10_template_explicito.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_I10TemplateExplicito1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i10_template_explicito.html:10:26 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I10TemplateExplicito, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I10TemplateExplicitoNgFactory = ComponentFactory<import1.I10TemplateExplicito>('i10-template-explicito', viewFactory_I10TemplateExplicitoHost0);
ComponentFactory<import1.I10TemplateExplicito> get I10TemplateExplicitoNgFactory {
  return _I10TemplateExplicitoNgFactory;
}

ComponentFactory<import1.I10TemplateExplicito> createI10TemplateExplicitoFactory() {
  return ComponentFactory('i10-template-explicito', viewFactory_I10TemplateExplicitoHost0);
}

class _ViewI10TemplateExplicito1 extends import13.EmbeddedView<import1.I10TemplateExplicito> {
  _ViewI10TemplateExplicito1(import14.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('p'));
    final _text_1 = import9.appendText(_el_0, 'a');
    this.initRootNode(_el_0);
  }
}

import13.EmbeddedView<void> viewFactory_I10TemplateExplicito1(import14.RenderView parentView, int parentIndex) {
  return _ViewI10TemplateExplicito1(parentView, parentIndex);
}

final List<Object> styles$I10TemplateExplicitoHost = const [];

class _ViewI10TemplateExplicitoHost0 extends import15.HostView<import1.I10TemplateExplicito> {
  @override
  void build() {
    this.componentView = ViewI10TemplateExplicito0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I10TemplateExplicito();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.I10TemplateExplicito> viewFactory_I10TemplateExplicitoHost0() {
  return _ViewI10TemplateExplicitoHost0();
}
