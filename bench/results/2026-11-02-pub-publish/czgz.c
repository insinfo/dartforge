/* gzip a file with Chromium zlib at level 6, windowBits 31, memLevel 8. */
#include <stdio.h>
#include <stdlib.h>
#include "zlib.h"

int main(int argc, char **argv) {
  FILE *f = fopen(argv[1], "rb");
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  unsigned char *in = malloc(n);
  fread(in, 1, n, f);
  fclose(f);
  z_stream s = {0};
  deflateInit2(&s, 6, Z_DEFLATED, 31, 8, Z_DEFAULT_STRATEGY);
  unsigned long cap = deflateBound(&s, n) + 1024;
  unsigned char *out = malloc(cap);
  s.next_in = in;
  s.avail_in = n;
  s.next_out = out;
  s.avail_out = cap;
  int r = deflate(&s, Z_FINISH);
  if (r != Z_STREAM_END) { fprintf(stderr, "deflate %d\n", r); return 1; }
  FILE *o = fopen(argv[2], "wb");
  fwrite(out, 1, s.total_out, o);
  fclose(o);
  return 0;
}
