cmake_minimum_required(VERSION 3.13)
project(runner LANGUAGES C CXX)

# Define the application target. To change its name, change BINARY_NAME in the
# top-level CMakeLists.txt, not the value here, or `flutter run` will no longer
# work.
#
# Any new source files that you add to the application should be added here.
find_program(GLIB_COMPILE_RESOURCES glib-compile-resources)
if(NOT GLIB_COMPILE_RESOURCES)
  message(FATAL_ERROR "glib-compile-resources is required for the application icon")
endif()
set(ICON_RESOURCE_SOURCE "${CMAKE_CURRENT_BINARY_DIR}/smith_icon_resources.c")
add_custom_command(
  OUTPUT "${ICON_RESOURCE_SOURCE}"
  COMMAND "${GLIB_COMPILE_RESOURCES}"
    --generate-source
    --target=${ICON_RESOURCE_SOURCE}
    --sourcedir=${CMAKE_CURRENT_SOURCE_DIR}/resources
    "${CMAKE_CURRENT_SOURCE_DIR}/resources/smith.gresource.xml"
  DEPENDS
    "resources/smith.gresource.xml"
    "resources/smith.png"
  VERBATIM
)

add_executable(${BINARY_NAME}
  "main.cc"
  "my_application.cc"
  "${ICON_RESOURCE_SOURCE}"
  "${FLUTTER_MANAGED_DIR}/generated_plugin_registrant.cc"
)

# Apply the standard set of build settings. This can be removed for applications
# that need different build settings.
apply_standard_settings(${BINARY_NAME})

# Add preprocessor definitions for the application ID.
add_definitions(-DAPPLICATION_ID="${APPLICATION_ID}")

# Add dependency libraries. Add any application-specific dependencies here.
target_link_libraries(${BINARY_NAME} PRIVATE flutter)
target_link_libraries(${BINARY_NAME} PRIVATE PkgConfig::GTK)

target_include_directories(${BINARY_NAME} PRIVATE "${CMAKE_SOURCE_DIR}")
