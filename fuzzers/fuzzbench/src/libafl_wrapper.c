// We only want to link our fuzzer main, if the target doesn't specify its own main - hence we define `main` as `weak` in this file.
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <unistd.h>

// jump to rust
void fuzzer_main();

// Link in a dummy llvm test to non-fuzzing builds, for configure et al.
int __attribute__((weak)) LLVMFuzzerTestOneInput(const uint8_t *buf, size_t len) {
  (void) buf;
  (void) len;
  fprintf(stderr, "LibAFL - No LLVMFuzzerTestOneInput function found! Linker error?\n");
  fflush(stderr);
  abort();
}

int __attribute__((weak)) main(int argc, char *argv[]) {
  int ok = 1;
  struct stat st;
  (void) argc;
  (void) argv;
  if (argc == 3 || argc == 5 && strcmp(argv[1], "-x") == 0) {
    if (stat(argv[argc-2], &st) != 0) { ok = 0; } else {
      if (!S_ISDIR(st.st_mode)) {
        ok = 0; 
      } else {
        if (stat(argv[argc-1], &st) != 0) { ok = 0; } else {
          if (!S_ISDIR(st.st_mode)) { ok = 0; }
        }
      }
    }
  } else { ok = 0; }  

  if (ok) {
    fuzzer_main();
  } else {
    printf("libafl fuzzer instance\n");
    printf("Syntax: %s [-x dictionary] corpus_dir seed_dir\n", argv[0]);
  }
  return 0;
}