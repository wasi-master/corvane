# Cross-compilation toolchain file — ünïcödé comment
# Usage: cmake -DCMAKE_TOOLCHAIN_FILE=toolchain.cmake ..

cmake_minimum_required(VERSION 3.20.0)
cmake_policy (SET CMP0077 NEW)
project(Corvane VERSION 1.2.3 LANGUAGES C CXX)

set(CMAKE_SYSTEM_NAME Linux)
set(CMAKE_SYSTEM_PROCESSOR aarch64)
set(TOOLCHAIN_PREFIX aarch64-linux-gnu-)
set(CMAKE_C_COMPILER ${TOOLCHAIN_PREFIX}gcc)
set(CMAKE_CXX_COMPILER "${TOOLCHAIN_PREFIX}g++")
set(CMAKE_SYSROOT $ENV{SYSROOT})
set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
set(FLAGS "-O2 -Wall -Wextra" CACHE STRING "Compiler flags")
set(QUOTED 'single quoted value')
set(ESCAPED "a \"quoted\" word and \${NOT_A_VAR} here")
set(PATHS "$CMAKE_SOURCE_DIR/include;${PROJECT_BINARY_DIR}/gen")

if(NOT DEFINED ENV{SYSROOT})
	message(FATAL_ERROR "SYSROOT is not set: ${CMAKE_CURRENT_LIST_FILE}")
elseif(CMAKE_HOST_SYSTEM_NAME STREQUAL "Darwin")
	message(STATUS "Building on macOS")
else()
	message(WARNING "Unknown host")
endif()

option(ENABLE_TESTS "Build the unit tests" ON)
option (ENABLE_DOCS "Build docs" OFF)

foreach(src IN LISTS SOURCES)
  get_filename_component(name ${src} NAME_WE)
  list(APPEND OBJECTS ${name}.o)
endforeach()

function(add_corvane_library name)
  cmake_parse_arguments(ARG "SHARED;STATIC" "OUTPUT" "SOURCES;DEPS" ${ARGN})
  add_library(${name} ${ARG_SOURCES})
  target_link_libraries(${name} PRIVATE ${ARG_DEPS})
  target_compile_definitions(${name} PUBLIC VERSION=${PROJECT_VERSION} DEBUG=1)
  set_target_properties(${name} PROPERTIES
    CXX_STANDARD 17
    POSITION_INDEPENDENT_CODE ON
    OUTPUT_NAME "corvane-${name}"
  )
endfunction()

macro(log_var var)
  message(STATUS "${var} = ${${var}}")
endmacro()

set(MULTILINE "first line
second line with ${VAR}
third line")
set(SINGLE_MULTI 'open single
closed here')

string(REGEX REPLACE "([0-9]+)\\.([0-9]+)" "\\1_\\2" OUT "${IN}")
math(EXPR RESULT "(1 + 2) * 3 - 4 / 2")
list(LENGTH OBJECTS count)
if(count GREATER 10 AND NOT WIN32 OR APPLE)
  set(BIG TRUE)
endif()

include(GNUInstallDirs)
include (CheckCXXCompilerFlag)
find_package(Threads REQUIRED)
find_package(OpenSSL 3.0 COMPONENTS Crypto SSL)
check_cxx_compiler_flag(-fsanitize=address HAS_ASAN)
if(HAS_ASAN AND CMAKE_BUILD_TYPE MATCHES "^(Debug|RelWithDebInfo)$")
  target_compile_options(corvane PRIVATE $<$<CONFIG:Debug>:-fsanitize=address>)
  target_link_options(corvane PRIVATE -fsanitize=address)
endif()
configure_file(${CMAKE_CURRENT_SOURCE_DIR}/config.h.in ${CMAKE_CURRENT_BINARY_DIR}/config.h @ONLY)
file(GLOB_RECURSE HEADERS CONFIGURE_DEPENDS "${PROJECT_SOURCE_DIR}/include/*.h")
enable_testing()
add_test(NAME smoke COMMAND corvane --version)
set_tests_properties(smoke PROPERTIES TIMEOUT 30 LABELS "fast;unit")
while(i LESS 5)
  math(EXPR i "${i} + 1")
endwhile()
return()

# comment that looks like a call(
#call(with args)
"string(" at start
set(UNICODE_NAME "日本語 🎉 ${ÜBER}")
add_custom_command(OUTPUT gen.c COMMAND ${CMAKE_COMMAND} -E echo "done" > gen.c)
install(TARGETS corvane DESTINATION bin)
$ lone dollar
${UNTERMINATED
set(X 42 3.14 -7 0x1F)
set(TRAILING "never closed
