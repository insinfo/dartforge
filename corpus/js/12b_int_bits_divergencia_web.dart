// Operadores de bits com negativos e além de 32 bits: divergem entre VM (64 bits) e web (32 bits sem sinal).
// diverge-ddc: operadores de bits devolvem 32 bits sem sinal na web
void main() {
  print(~0);
  print(~1);
  print(~5);
  print(~(-1));
  print(-1 >> 1);
  print(-1 >>> 1);
  print(-8 >> 1);
  print(-8 >>> 1);
  print(-1 | 0);
  print(-1 & 0xFF);
  print(-1 & -1);
  print(-1 ^ 0);
  print(-1 ^ -1);
  print(1 << 31);
  print(1 << 32);
  print(1 << 40);
  print(1 << 52);
  print((1 << 52) + (1 << 52));
  print(0xFFFFFFFF);
  print(0xFFFFFFFF + 1);
  print(0xFFFFFFFF | 0);
  print(0xFFFFFFFF & 0xFFFFFFFF);
  print(0x80000000);
  print(0x80000000 | 0);
  print(0x80000000 >> 1);
  print(0x80000000 >>> 1);
  print(0x100000000 >> 1);
  print(0x100000000 & 0x100000000);
  print(-5 & 3);
  print((-1).toRadixString(2));
  print((-1).bitLength);
  print(-256 >> 4);
  print(-256 >>> 60);
  print(2 << 31);
  print(3 << 30);
  print((1 << 31) - 1);
  print((1 << 31) + 1);
  print(0x7FFFFFFF + 1 | 0);
  print(1 << 31 >> 31);
  print(1 << 31 >>> 31);
  print(-1 << 1);
  print(-1 << 0);
  print(0xDEADBEEF >> 16);
  print(0xDEADBEEF & 0xFFFF);
}
