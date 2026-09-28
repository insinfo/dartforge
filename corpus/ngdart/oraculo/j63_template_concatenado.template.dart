// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j63_template_concatenado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j63_template_concatenado.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'dart:html' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J63TemplateConcatenado = const [];

class ViewJ63TemplateConcatenado0 extends import0.ComponentView<import1.J63TemplateConcatenado> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  final import2.TextBinding _textBinding_6 = import2.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  late final import3.HtmlElement _el_0;
  late final import3.HtmlElement _el_2;
  static import4.ComponentStyles? _componentStyles;
  ViewJ63TemplateConcatenado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j63-template-concatenado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_0.append(this._textBinding_1.element);
    this._el_2 = import8.appendSpan(doc, parentRenderNode);
    this._el_2.append(this._textBinding_3.element);
    final _text_4 = import8.appendText(parentRenderNode, '\n');
    final _el_5 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'b');
    _el_5.append(this._textBinding_6.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.nome;
    if (import9.checkBinding(this._expr_0, currVal_0, 'nome', 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:245:259 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:260:268 */;
    final currVal_1 = _ctx.rotulos['a'];
    if (import9.checkBinding(this._expr_1, currVal_1, 'rotulos[\'a\']', 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart')) {
      import8.setProperty(this._el_2, 'title', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:278:300 */;
      this._expr_1 = currVal_1;
    }
    this._textBinding_3.updateTextWithPrimitive(_ctx.total) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:301:310 */;
    this._textBinding_6.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:321:331 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J63TemplateConcatenado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J63TemplateConcatenadoNgFactory = ComponentFactory<import1.J63TemplateConcatenado>('j63-template-concatenado', viewFactory_J63TemplateConcatenadoHost0);
ComponentFactory<import1.J63TemplateConcatenado> get J63TemplateConcatenadoNgFactory {
  return _J63TemplateConcatenadoNgFactory;
}

ComponentFactory<import1.J63TemplateConcatenado> createJ63TemplateConcatenadoFactory() {
  return ComponentFactory('j63-template-concatenado', viewFactory_J63TemplateConcatenadoHost0);
}

final List<Object> styles$J63TemplateConcatenadoHost = const [];

class _ViewJ63TemplateConcatenadoHost0 extends import12.HostView<import1.J63TemplateConcatenado> {
  @override
  void build() {
    this.componentView = ViewJ63TemplateConcatenado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J63TemplateConcatenado();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J63TemplateConcatenado> viewFactory_J63TemplateConcatenadoHost0() {
  return _ViewJ63TemplateConcatenadoHost0();
}

final List<Object> styles$J63Escapado = const [];

class ViewJ63Escapado0 extends import0.ComponentView<import1.J63Escapado> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ63Escapado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j63-escapado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.rotulos['a'];
    if (import9.checkBinding(this._expr_0, currVal_0, 'rotulos[\'a\']', 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:637:659 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:660:668 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J63Escapado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J63EscapadoNgFactory = ComponentFactory<import1.J63Escapado>('j63-escapado', viewFactory_J63EscapadoHost0);
ComponentFactory<import1.J63Escapado> get J63EscapadoNgFactory {
  return _J63EscapadoNgFactory;
}

ComponentFactory<import1.J63Escapado> createJ63EscapadoFactory() {
  return ComponentFactory('j63-escapado', viewFactory_J63EscapadoHost0);
}

final List<Object> styles$J63EscapadoHost = const [];

class _ViewJ63EscapadoHost0 extends import12.HostView<import1.J63Escapado> {
  @override
  void build() {
    this.componentView = ViewJ63Escapado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J63Escapado();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J63Escapado> viewFactory_J63EscapadoHost0() {
  return _ViewJ63EscapadoHost0();
}

final List<Object> styles$J63Cru = const [];

class ViewJ63Cru0 extends import0.ComponentView<import1.J63Cru> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  late final import3.HtmlElement _el_0;
  static import4.ComponentStyles? _componentStyles;
  ViewJ63Cru0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import3.document.createElement('j63-cru'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import3.document;
    this._el_0 = import8.appendElement<import3.HtmlElement>(doc, parentRenderNode, 'p');
    this._el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.nome;
    if (import9.checkBinding(this._expr_0, currVal_0, 'nome', 'asset:corpus_ngdart/lib/src/j63_template_concatenado.dart')) {
      import8.setProperty(this._el_0, 'title', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:839:853 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import10.interpolateString0(_ctx.nome)) /* REF:asset:corpus_ngdart/lib/src/j63_template_concatenado.dart:854:862 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J63Cru, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J63CruNgFactory = ComponentFactory<import1.J63Cru>('j63-cru', viewFactory_J63CruHost0);
ComponentFactory<import1.J63Cru> get J63CruNgFactory {
  return _J63CruNgFactory;
}

ComponentFactory<import1.J63Cru> createJ63CruFactory() {
  return ComponentFactory('j63-cru', viewFactory_J63CruHost0);
}

final List<Object> styles$J63CruHost = const [];

class _ViewJ63CruHost0 extends import12.HostView<import1.J63Cru> {
  @override
  void build() {
    this.componentView = ViewJ63Cru0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J63Cru();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J63Cru> viewFactory_J63CruHost0() {
  return _ViewJ63CruHost0();
}
