#include "libcue.h"
#include <sys/mman.h>
#include <stdio.h>
#include <unistd.h>
#include <stdio.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char* argv[]) {  	
    int fd = open(argv[1], O_RDONLY);
    int len = lseek(fd, 0, SEEK_END);
    char *data = (char*)mmap(0, len, PROT_READ, MAP_PRIVATE, fd, 0);
    char *mem_data = (char*) malloc(sizeof(char) * len);
    memcpy(mem_data, data, len);
    mem_data[len - 1] = '\0';
    printf("%d\n", len);

    Cd* cd;
    cd = cue_parse_string(mem_data);
    if (cd != NULL) {
      cd_delete(cd);
    }
	return 0;
}

