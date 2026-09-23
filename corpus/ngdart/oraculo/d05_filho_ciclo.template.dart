// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd05_filho_ciclo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd05_filho_ciclo.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;

final List<Object> styles$D05FilhoCiclo = const [];

class ViewD05FilhoCiclo0 extends import0.ComponentView<import1.D05FilhoCiclo> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewD05FilhoCiclo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('d05-filho-ciclo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/d05_filho_ciclo.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.titulo)) /* REF:package:corpus_ngdart/src/d05_filho_ciclo.html:3:13 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$D05FilhoCiclo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D05FilhoCicloNgFactory = ComponentFactory<import1.D05FilhoCiclo>('d05-filho-ciclo', viewFactory_D05FilhoCicloHost0);
ComponentFactory<import1.D05FilhoCiclo> get D05FilhoCicloNgFactory {
  return _D05FilhoCicloNgFactory;
}

ComponentFactory<import1.D05FilhoCiclo> createD05FilhoCicloFactory() {
  return ComponentFactory('d05-filho-ciclo', viewFactory_D05FilhoCicloHost0);
}

final List<Object> styles$D05FilhoCicloHost = const [];

class _ViewD05FilhoCicloHost0 extends import11.HostView<import1.D05FilhoCiclo> {
  @override
  void build() {
    this.componentView = ViewD05FilhoCiclo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D05FilhoCiclo();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import12.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    if ((!import12.debugThrowIfChanged)) {
      this.component.ngDoCheck();
    }
    if ((!import12.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterContentInit();
      }
      this.component.ngAfterContentChecked();
    }
    this.componentView.detectChanges();
    if ((!import12.debugThrowIfChanged)) {
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

import11.HostView<import1.D05FilhoCiclo> viewFactory_D05FilhoCicloHost0() {
  return _ViewD05FilhoCicloHost0();
}
