#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <vector>

#include "libcue.h"

extern "C" int LLVMFuzzerTestOneInput(const uint8_t* data, size_t size) {
  if (size == 0) {
    return 0;
  }

  char *test_chars = nullptr;
  Cd* cd = nullptr;
  try {
      test_chars = new char[size + 1];
      memcpy(test_chars, data, size);
      test_chars[size - 1] = '\0';
    
      Cd* cd;
      cd = cue_parse_string(test_chars);
      cd_delete(cd);
      cd = nullptr;

      delete test_chars;
  } catch(const std::exception& ex) {
      if (test_chars != nullptr) {
        delete test_chars;
      }
      if (cd != nullptr) {
        cd_delete(cd);
      }
      throw ex;
  }
  

  return 0;
}

