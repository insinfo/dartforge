void g() {
  // ignore: type=warning
  var a = 1;
  // ignore: type=static_warning
  var b = 1;
  ///ignore: unused_local_variable
  var c = 1;
  //	ignore: unused_local_variable
  var d = 1;
  // ignore_for_file: dead_code // ignore: unused_local_variable
  var e = 1;
  var f = 1; /* ignore: unused_local_variable */
  var s = 'http://x'; // ignore: unused_local_variable
  // ignore: unused_local_variable porque sim

  var h = 1;
  // ignore: unused_local_variable.
  var i = 1;
  // ignore: UNUSED_LOCAL_VARIABLE , dead_code
  var j = 1;
  return;
  print(1);
}
