cmake_minimum_required(VERSION 3.22.1)
project(egl_jni)

add_library(egl_jni SHARED egl_jni.cpp)

# Expect prebuilt Rust cdylib next to jniLibs, or set EGL_LIBRARY_DIR.
# Example:
#   cargo ndk -t arm64-v8a -o android/eglcore/src/main/jniLibs \
#     build -p library-ffi --release
set(EGL_LIBRARY_DIR "${CMAKE_SOURCE_DIR}/../jniLibs/${ANDROID_ABI}" CACHE PATH "libegl_library.so dir")

find_library(log-lib log)

target_include_directories(egl_jni PRIVATE ${CMAKE_SOURCE_DIR})

set(EGL_SO "${EGL_LIBRARY_DIR}/libegl_library.so")
if(EXISTS "${EGL_SO}")
    add_library(egl_library SHARED IMPORTED)
    set_target_properties(egl_library PROPERTIES IMPORTED_LOCATION "${EGL_SO}")
    target_link_libraries(egl_jni egl_library ${log-lib})
    target_compile_definitions(egl_jni PRIVATE EGL_HAS_RUST=1)
else()
    message(FATAL_ERROR "libegl_library.so not found under ${EGL_LIBRARY_DIR}")
endif()
