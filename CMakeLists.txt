cmake_minimum_required(VERSION 3.15)
project(gafana VERSION 1.2.0 LANGUAGES C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

include_directories(include src)

set(GAFANA_SOURCES
    src/gafana_engine.c
    src/gafana_consensus.c
    src/gafana_media.c
    src/gafana_signaling.c
    src/gafana_net.c
    src/gafana_smtp.c
)

# Unified All-In-One Executable
add_executable(gafana src/gafana_app.c ${GAFANA_SOURCES})

# Static Library
add_library(gafana_static STATIC ${GAFANA_SOURCES})
target_include_directories(gafana_static PUBLIC include)
set_target_properties(gafana_static PROPERTIES OUTPUT_NAME "gafana")

# Shared Library (DLL / SO)
add_library(gafana_shared SHARED ${GAFANA_SOURCES})
target_include_directories(gafana_shared PUBLIC include)
target_compile_definitions(gafana_shared PRIVATE GAFANA_BUILD_DLL)
set_target_properties(gafana_shared PROPERTIES OUTPUT_NAME "gafana")

# Standalone Relay Server Target
add_executable(gafana_relay server/gafana_sfu.c)

# Verification Test Suite
add_executable(consensus_test examples/consensus_test.c ${GAFANA_SOURCES})

if (WIN32)
    target_link_libraries(gafana PRIVATE ws2_32)
    target_link_libraries(gafana_static PUBLIC ws2_32)
    target_link_libraries(gafana_shared PUBLIC ws2_32)
    target_link_libraries(gafana_relay PRIVATE ws2_32)
    target_link_libraries(consensus_test PRIVATE ws2_32)
else()
    target_link_libraries(gafana PRIVATE pthread)
    target_link_libraries(gafana_static PUBLIC pthread)
    target_link_libraries(gafana_shared PUBLIC pthread)
    target_link_libraries(gafana_relay PRIVATE pthread)
    target_link_libraries(consensus_test PRIVATE pthread)
endif()

install(TARGETS gafana gafana_static gafana_shared gafana_relay
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
    RUNTIME DESTINATION bin
)
install(FILES include/gafana.h DESTINATION include)
