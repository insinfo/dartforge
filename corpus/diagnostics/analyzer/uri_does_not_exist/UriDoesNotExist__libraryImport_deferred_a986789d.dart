import '[invalid uri]' deferred as p;
//     ^^^^^^^^^^^^^^^
// [diag.uriDoesNotExist] Target of URI doesn't exist: '[invalid uri]'.
main() {
  p.loadLibrary();
}
