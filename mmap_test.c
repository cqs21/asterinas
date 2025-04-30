#include <stdio.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <string.h>

int main() {
    const char *filepath = "/tmp/test";
    const size_t size = 4096;  // 4KB

    int fd = open(filepath, O_RDWR | O_CREAT, 0600);
    if (fd == -1) {
        perror("open failed");
        return 1;
    }

    if (ftruncate(fd, size) == -1) {
        perror("ftruncate failed");
        close(fd);
        return 1;
    }

    char *mapped = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (mapped == MAP_FAILED) {
        perror("mmap failed");
        close(fd);
        return 1;
    }

    printf("Memory mapped at address: %p\n", mapped);

    if (unlink(filepath) == -1) {
        perror("unlink failed");
        munmap(mapped, size);
        close(fd);
        return 1;
    }

    close(fd);

    const char *message = "Hello, mmap!";
    strncpy(mapped, message, size);
    printf("Written to memory: %s\n", message);

    printf("Read from memory: %s\n", mapped);

    if (munmap(mapped, size) == -1) {
        perror("munmap failed");
        return 1;
    }


    printf("Test completed successfully.\n");
    return 0;
}
