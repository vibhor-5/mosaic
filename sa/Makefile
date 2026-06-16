CC=clang
CFLAGS=-O3 -Wall
FRAMEWORKS=-framework Foundation -framework CoreGraphics -framework Carbon

all: payload.dylib libmosaicUser.a

# Generate MIG files
mosaicServer.c mosaicUser.c mosaic.h: mosaic.defs
	mig mosaic.defs

payload.dylib: payload.c mosaicServer.c mosaic.h
	$(CC) $(CFLAGS) -dynamiclib -o payload.dylib payload.c mosaicServer.c $(FRAMEWORKS)

libmosaicUser.a: mosaicUser.c mosaic.h
	$(CC) $(CFLAGS) -c mosaicUser.c -o mosaicUser.o
	ar rcs libmosaicUser.a mosaicUser.o

clean:
	rm -f payload.dylib libmosaicUser.a mosaicUser.o mosaicServer.c mosaicUser.c mosaic.h
