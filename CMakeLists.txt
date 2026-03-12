cmake_minimum_required(VERSION 3.24)

project(MinnowSnap LANGUAGES CXX)

# Rust currently links against the release MSVC runtime. Keep CMake consistent.
if(MSVC)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
endif()

set(CMAKE_AUTOMOC ON)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(MINNOWSNAP_QT_COMPONENTS
  Core
  Gui
  Qml
  Quick
  QuickControls2
)
find_package(Qt6 REQUIRED COMPONENTS ${MINNOWSNAP_QT_COMPONENTS})

if(MSVC)
  # Force Qt release runtime in Debug to avoid CRT mismatch with Rust static linkage.
  set_property(
    TARGET
      Qt6::Core
      Qt6::Gui
      Qt6::Qml
      Qt6::Quick
      Qt6::QuickControls2
    PROPERTY MAP_IMPORTED_CONFIG_DEBUG "RELEASE"
  )
endif()

find_package(CxxQt QUIET)
if(NOT CxxQt_FOUND)
  include(FetchContent)
  FetchContent_Declare(
    CxxQt
    GIT_REPOSITORY https://github.com/kdab/cxx-qt-cmake.git
    GIT_TAG main
  )
  FetchContent_MakeAvailable(CxxQt)
endif()

# CXX-Qt (using Corrosion) creates a CMake target with the same name as the crate.
cxx_qt_import_crate(
  MANIFEST_PATH ${CMAKE_CURRENT_SOURCE_DIR}/rust/Cargo.toml
  CRATES MinnowSnap
  QT_MODULES
    Qt6::Core
    Qt6::Gui
    Qt6::Qml
    Qt6::Quick
    Qt6::QuickControls2
)

cxx_qt_import_qml_module(MinnowSnap_qml_module
  URI "com.lortunate.minnow"
  SOURCE_CRATE MinnowSnap
)

file(
  GLOB_RECURSE MINNOWSNAP_QML_FILES
  CONFIGURE_DEPENDS
  RELATIVE ${CMAKE_CURRENT_SOURCE_DIR}
  qml/*.qml
  qml/qmldir
  qml/**/qmldir
)

if(APPLE)
  add_executable(MinnowSnapApp MACOSX_BUNDLE cpp/main.cpp)
elseif(WIN32)
  add_executable(MinnowSnapApp WIN32 cpp/main.cpp)
else()
  add_executable(MinnowSnapApp cpp/main.cpp)
endif()

target_sources(MinnowSnapApp PRIVATE ${MINNOWSNAP_QML_FILES})
target_link_libraries(MinnowSnapApp PRIVATE MinnowSnap_qml_module)
target_compile_definitions(MinnowSnapApp PRIVATE $<$<CONFIG:Debug>:QT_QML_DEBUG>)
qt_import_qml_plugins(MinnowSnapApp)

set_target_properties(MinnowSnapApp PROPERTIES RUNTIME_OUTPUT_NAME MinnowSnap)

if(WIN32)
  target_link_libraries(MinnowSnapApp PRIVATE DirectML)
endif()
