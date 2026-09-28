// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j32_hospedeiro_com_ganchos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j32_hospedeiro_com_ganchos.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$J32HospedeiroComGanchos = const [];

class ViewJ32HospedeiroComGanchos0 extends import0.ComponentView<import1.J32HospedeiroComGanchos> {
  Object? _expr_0;
  Object? _expr_1;
  static import2.ComponentStyles? _componentStyles;
  ViewJ32HospedeiroComGanchos0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j32-hospedeiro-com-ganchos'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j32_hospedeiro_com_ganchos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendSpan(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.x;
    if (import8.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateAttribute(this.rootElement, 'data-x', currVal_0);
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.ativo;
    if (import8.checkBinding(this._expr_1, currVal_1, null, null)) {
      import7.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_1);
      this._expr_1 = currVal_1;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J32HospedeiroComGanchos, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J32HospedeiroComGanchosNgFactory = ComponentFactory<import1.J32HospedeiroComGanchos>('j32-hospedeiro-com-ganchos', viewFactory_J32HospedeiroComGanchosHost0);
ComponentFactory<import1.J32HospedeiroComGanchos> get J32HospedeiroComGanchosNgFactory {
  return _J32HospedeiroComGanchosNgFactory;
}

ComponentFactory<import1.J32HospedeiroComGanchos> createJ32HospedeiroComGanchosFactory() {
  return ComponentFactory('j32-hospedeiro-com-ganchos', viewFactory_J32HospedeiroComGanchosHost0);
}

final List<Object> styles$J32HospedeiroComGanchosHost = const [];

class _ViewJ32HospedeiroComGanchosHost0 extends import10.HostView<import1.J32HospedeiroComGanchos> {
  @override
  void build() {
    this.componentView = ViewJ32HospedeiroComGanchos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J32HospedeiroComGanchos();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import8.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    if ((!import8.debugThrowIfChanged)) {
      this.component.ngDoCheck();
    }
    if ((!import8.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
      this.component.ngAfterContentChecked();
    }
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
    if ((!import8.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
      this.component.ngAfterViewChecked();
    }
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import10.HostView<import1.J32HospedeiroComGanchos> viewFactory_J32HospedeiroComGanchosHost0() {
  return _ViewJ32HospedeiroComGanchosHost0();
}
