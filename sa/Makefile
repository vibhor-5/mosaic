CC = clang
CFLAGS = -O3 -Wall -Wextra -dynamiclib -framework CoreFoundation

all: mosaic_payload.dylib

mosaic_payload.dylib: payload.c
	$(CC) $(CFLAGS) -o $@ $^

clean:
	rm -f mosaic_payload.dylib
