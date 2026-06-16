CC=clang
CFLAGS=-O3 -Wall
FRAMEWORKS=-framework Foundation -framework CoreGraphics -framework Carbon

all: payload.dylib libmosaicUser.a

payload.dylib: payload.c mosaicServer.c
	$(CC) $(CFLAGS) -dynamiclib -o payload.dylib payload.c mosaicServer.c $(FRAMEWORKS)

libmosaicUser.a: mosaicUser.c
	$(CC) $(CFLAGS) -c mosaicUser.c -o mosaicUser.o
	ar rcs libmosaicUser.a mosaicUser.o

clean:
	rm -f payload.dylib libmosaicUser.a mosaicUser.o
