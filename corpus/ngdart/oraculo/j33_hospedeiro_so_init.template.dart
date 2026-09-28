// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j33_hospedeiro_so_init.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j33_hospedeiro_so_init.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$J33HospedeiroSoInit = const [];

class ViewJ33HospedeiroSoInit0 extends import0.ComponentView<import1.J33HospedeiroSoInit> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  Object? _expr_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ33HospedeiroSoInit0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j33-hospedeiro-so-init'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j33_hospedeiro_so_init.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendSpan(doc, parentRenderNode);
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.rotulo)) /* REF:asset:corpus_ngdart/lib/src/j33_hospedeiro_so_init.dart:174:184 */;
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.pronto;
    if (import10.checkBinding(this._expr_0, currVal_0, null, null)) {
      import8.updateClassBindingNonHtml(this.rootElement, 'pronto', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J33HospedeiroSoInit, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J33HospedeiroSoInitNgFactory = ComponentFactory<import1.J33HospedeiroSoInit>('j33-hospedeiro-so-init', viewFactory_J33HospedeiroSoInitHost0);
ComponentFactory<import1.J33HospedeiroSoInit> get J33HospedeiroSoInitNgFactory {
  return _J33HospedeiroSoInitNgFactory;
}

ComponentFactory<import1.J33HospedeiroSoInit> createJ33HospedeiroSoInitFactory() {
  return ComponentFactory('j33-hospedeiro-so-init', viewFactory_J33HospedeiroSoInitHost0);
}

final List<Object> styles$J33HospedeiroSoInitHost = const [];

class _ViewJ33HospedeiroSoInitHost0 extends import12.HostView<import1.J33HospedeiroSoInit> {
  @override
  void build() {
    this.componentView = ViewJ33HospedeiroSoInit0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J33HospedeiroSoInit();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    bool firstCheck = this.firstCheck;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
  }
}

import12.HostView<import1.J33HospedeiroSoInit> viewFactory_J33HospedeiroSoInitHost0() {
  return _ViewJ33HospedeiroSoInitHost0();
}
