# Gafana Unified Engine Makefile (GCC / Clang / MinGW / TCC)

CC ?= gcc
CFLAGS ?= -O2 -Wall -Wextra -std=c11 -Iinclude -Isrc
LDFLAGS ?=

ifeq ($(OS),Windows_NT)
    LIBS = -lws2_32
    TARGET_APP = gafana.exe
    TARGET_LIB_STATIC = libgafana.a
    TARGET_LIB_SHARED = gafana.dll
    TARGET_RELAY = gafana-relay.exe
    TARGET_TEST = consensus_test.exe
    EXE_EXT = .exe
else
    LIBS = -lpthread
    TARGET_APP = gafana
    TARGET_LIB_STATIC = libgafana.a
    TARGET_LIB_SHARED = libgafana.so
    TARGET_RELAY = gafana-relay
    TARGET_TEST = consensus_test
    EXE_EXT =
endif

SRCS = src/gafana_engine.c src/gafana_consensus.c src/gafana_media.c src/gafana_signaling.c src/gafana_net.c src/gafana_smtp.c

all: $(TARGET_APP) $(TARGET_LIB_STATIC) $(TARGET_LIB_SHARED) $(TARGET_RELAY) $(TARGET_TEST)

$(TARGET_APP): src/gafana_app.c $(SRCS)
	$(CC) $(CFLAGS) $^ -o $@ $(LIBS)

$(TARGET_LIB_STATIC): $(SRCS)
	$(CC) $(CFLAGS) -c $(SRCS)
	ar rcs $@ *.o
	rm -f *.o

$(TARGET_LIB_SHARED): $(SRCS)
	$(CC) $(CFLAGS) -shared -DGAFANA_BUILD_DLL -o $@ $^ $(LIBS)

$(TARGET_RELAY): server/gafana_sfu.c
	$(CC) $(CFLAGS) $< -o $@ $(LIBS)

$(TARGET_TEST): examples/consensus_test.c $(SRCS)
	$(CC) $(CFLAGS) $^ -o $@ $(LIBS)

test: $(TARGET_TEST)
	./$(TARGET_TEST)

clean:
	rm -f src/*.o *.o *.a *.so *.dll *$(EXE_EXT)
