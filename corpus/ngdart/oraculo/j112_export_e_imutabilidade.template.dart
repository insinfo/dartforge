// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j112_export_e_imutabilidade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j112_export_e_imutabilidade.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'dart:html' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J112ExportEImutabilidade = const [];

class ViewJ112ExportEImutabilidade0 extends import0.ComponentView<import1.J112ExportEImutabilidade> {
  late final ViewContainer _appEl_7;
  late final import3.NgFor _NgFor_7_9;
  Object? _expr_1;
  Object? _expr_3;
  late final import4.HtmlElement _el_4;
  late final import4.HtmlElement _el_6;
  late final import4.HtmlElement _el_8;
  static import5.ComponentStyles? _componentStyles;
  ViewJ112ExportEImutabilidade0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('j112-export-e-imutabilidade'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import4.document;
    final _el_0 = import9.appendElement<import4.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import9.appendText(_el_0, import10.interpolate0(import1.j112Contador));
    final _text_2 = import9.appendText(_el_0, ' ');
    final _text_3 = import9.appendText(_el_0, import10.interpolate0(import1.j112Rotulo));
    this._el_4 = import9.appendElement<import4.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_5 = import9.appendText(parentRenderNode, '\n');
    this._el_6 = import9.appendElement<import4.HtmlElement>(doc, parentRenderNode, 'b');
    final _anchor_7 = import9.appendAnchor(parentRenderNode);
    this._appEl_7 = ViewContainer(7, null, this, _anchor_7);
    var _TemplateRef_7_8 = TemplateRef(this._appEl_7, viewFactory_J112ExportEImutabilidade1);
    this._NgFor_7_9 = import3.NgFor(this._appEl_7, _TemplateRef_7_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_7, this._NgFor_7_9);
    }
    this._el_8 = import9.appendSpan(doc, parentRenderNode);
    final _text_9 = import9.appendText(this._el_8, import10.interpolate0((0 - 2)));
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if ((_ctx.itens != null)) {
        if (import12.isDevToolsEnabled) {
          import12.Inspector.instance.recordInput(this._NgFor_7_9, 'ngForOf', _ctx.itens);
        }
        this._NgFor_7_9.ngForOf = _ctx.itens /* REF:asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart:696:730 */;
      }
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_7_9.ngDoCheck();
    }
    this._appEl_7.detectChangesInNestedViews();
    if (firstCheck) {
      if ((import1.J112Util.formatar != null)) {
        import9.setProperty(this._el_4, 'title', import1.J112Util.formatar) /* REF:asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart:623:650 */;
      }
    }
    final currVal_1 = import1.J112Util.mutavel;
    if (import13.checkBinding(this._expr_1, currVal_1, 'J112Util.mutavel', 'asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart')) {
      import9.setProperty(this._el_6, 'title', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart:659:685 */;
      this._expr_1 = currVal_1;
    }
    final currVal_3 = (0 - _ctx.n);
    if (import13.checkBinding(this._expr_3, currVal_3, '-n', 'asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart')) {
      import9.setProperty(this._el_8, 'title', currVal_3) /* REF:asset:corpus_ngdart/lib/src/j112_export_e_imutabilidade.dart:762:774 */;
      this._expr_3 = currVal_3;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_7.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J112ExportEImutabilidade, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J112ExportEImutabilidadeNgFactory = ComponentFactory<import1.J112ExportEImutabilidade>('j112-export-e-imutabilidade', viewFactory_J112ExportEImutabilidadeHost0);
ComponentFactory<import1.J112ExportEImutabilidade> get J112ExportEImutabilidadeNgFactory {
  return _J112ExportEImutabilidadeNgFactory;
}

ComponentFactory<import1.J112ExportEImutabilidade> createJ112ExportEImutabilidadeFactory() {
  return ComponentFactory('j112-export-e-imutabilidade', viewFactory_J112ExportEImutabilidadeHost0);
}

class _ViewJ112ExportEImutabilidade1 extends import15.EmbeddedView<import1.J112ExportEImutabilidade> {
  _ViewJ112ExportEImutabilidade1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    final _text_1 = import9.appendText(_el_0, import10.interpolate0(import1.j112Contador));
    this.initRootNode(_el_0);
  }
}

import15.EmbeddedView<void> viewFactory_J112ExportEImutabilidade1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ112ExportEImutabilidade1(parentView, parentIndex);
}

final List<Object> styles$J112ExportEImutabilidadeHost = const [];

class _ViewJ112ExportEImutabilidadeHost0 extends import17.HostView<import1.J112ExportEImutabilidade> {
  @override
  void build() {
    this.componentView = ViewJ112ExportEImutabilidade0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J112ExportEImutabilidade();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J112ExportEImutabilidade> viewFactory_J112ExportEImutabilidadeHost0() {
  return _ViewJ112ExportEImutabilidadeHost0();
}
