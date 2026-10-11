/**
 * Gafana Windows Socket Fallback Definitions
 * Provides standard WinSock2 structs and prototypes when winsock2.h is absent in minimal toolchains.
 */

#ifndef GAFANA_WINSOCK_H
#define GAFANA_WINSOCK_H

#if defined(_WIN32)
  #if defined(__has_include)
    #if __has_include(<winsock2.h>)
      #include <winsock2.h>
      #include <ws2tcpip.h>
      #define GAFANA_HAVE_SYSTEM_WINSOCK 1
    #endif
  #endif

  #ifndef GAFANA_HAVE_SYSTEM_WINSOCK
    #include <windows.h>

    typedef UINT_PTR SOCKET;
    typedef unsigned long u_long;
    typedef unsigned short u_short;
    #define INVALID_SOCKET ((SOCKET)(~0))
    #define SOCKET_ERROR (-1)
    #define AF_INET 2
    #define SOCK_STREAM 1
    #define SOCK_DGRAM 2
    #define IPPROTO_TCP 6
    #define IPPROTO_UDP 17
    #define INADDR_ANY ((ULONG)0x00000000)
    #define SOL_SOCKET 0xffff
    #define SO_REUSEADDR 0x0004
    #define SO_BROADCAST 0x0020
    #define FIONBIO 0x8004667e

    struct in_addr {
        union {
            struct { UCHAR s_b1, s_b2, s_b3, s_b4; } S_un_b;
            struct { USHORT s_w1, s_w2; } S_un_w;
            ULONG S_addr;
        } S_un;
        #define s_addr S_un.S_addr
    };

    struct sockaddr {
        USHORT sa_family;
        CHAR   sa_data[14];
    };

    struct sockaddr_in {
        SHORT          sin_family;
        USHORT         sin_port;
        struct in_addr sin_addr;
        CHAR           sin_zero[8];
    };

    struct sockaddr_storage {
        SHORT ss_family;
        CHAR  __ss_pad1[6];
        INT64 __ss_align;
        CHAR  __ss_pad2[112];
    };

    struct hostent {
        char  *h_name;
        char **h_aliases;
        short  h_addrtype;
        short  h_length;
        char **h_addr_list;
    };
    #define h_addr h_addr_list[0]

    typedef int socklen_t;

    typedef struct WSAData {
        WORD           wVersion;
        WORD           wHighVersion;
        char           szDescription[257];
        char           szSystemStatus[129];
        unsigned short iMaxSockets;
        unsigned short iMaxUdpDg;
        char          *lpVendorInfo;
    } WSADATA, *LPWSADATA;

    /* Prototypes in ws2_32.dll */
    int PASCAL WSAStartup(WORD, LPWSADATA);
    int PASCAL WSACleanup(void);
    SOCKET PASCAL socket(int, int, int);
    int PASCAL bind(SOCKET, const struct sockaddr *, int);
    int PASCAL listen(SOCKET, int);
    SOCKET PASCAL accept(SOCKET, struct sockaddr *, int *);
    int PASCAL connect(SOCKET, const struct sockaddr *, int);
    int PASCAL send(SOCKET, const char *, int, int);
    int PASCAL recv(SOCKET, char *, int, int);
    int PASCAL sendto(SOCKET, const char *, int, int, const struct sockaddr *, int);
    int PASCAL recvfrom(SOCKET, char *, int, int, struct sockaddr *, int *);
    int PASCAL closesocket(SOCKET);
    int PASCAL setsockopt(SOCKET, int, int, const char *, int);
    int PASCAL getsockname(SOCKET, struct sockaddr *, int *);
    int PASCAL ioctlsocket(SOCKET, long, unsigned long *);
    unsigned short PASCAL htons(unsigned short);
    unsigned long PASCAL htonl(unsigned long);
    unsigned short PASCAL ntohs(unsigned short);
    unsigned long PASCAL ntohl(unsigned long);
    unsigned long PASCAL inet_addr(const char *);
    char * PASCAL inet_ntoa(struct in_addr);
    int PASCAL gethostname(char *, int);
    struct hostent * PASCAL gethostbyname(const char *);
    const char * PASCAL inet_ntop(int, const void *, char *, socklen_t);
  #endif
#endif

#endif /* GAFANA_WINSOCK_H */
