# POLER Engine — Kate plugin (KTextEditor/KF6/Qt6)
#
# Standalone build: НЕ требует дерева исходников Kate. Ставится в namespace
# kf6/ktexteditor — там же, куда Kate ставит свои addons (kate_add_plugin ==
# kcoreaddons_add_plugin(... INSTALL_NAMESPACE "kf6/ktexteditor")).
#
# Зависимости (Arch): sudo pacman -S base-devel cmake extra-cmake-modules \
#     qt6-base kf6-texteditor kf6-kcoreaddons kf6-ki18n
#
# Сборка и установка:
#     cmake -B build -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr
#     cmake --build build -j
#     sudo cmake --install build
# После установки: Kate → Settings → Configure Kate → Plugins → «POLER Engine».

cmake_minimum_required(VERSION 3.24)

project(polerengineplugin VERSION 0.1.0 LANGUAGES CXX)

set(QT_MIN_VERSION "6.5.0")
set(KF_MIN_VERSION "6.0.0")

find_package(ECM ${KF_MIN_VERSION} REQUIRED NO_MODULE)
set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} ${ECM_MODULE_PATH})

include(KDEInstallDirs)
include(KDECMakeSettings)
include(FeatureSummary)

find_package(Qt6 ${QT_MIN_VERSION} REQUIRED COMPONENTS Widgets)
find_package(KF6 ${KF_MIN_VERSION} REQUIRED COMPONENTS CoreAddons TextEditor I18n)

kcoreaddons_add_plugin(
    polerengineplugin
    INSTALL_NAMESPACE "kf6/ktexteditor"
    SOURCES
        polerplugin.cpp
        polerpanel.cpp
        enginebridge.cpp
        polercommands.cpp
)


target_compile_definitions(polerengineplugin PRIVATE TRANSLATION_DOMAIN="polerengineplugin")

target_link_libraries(
    polerengineplugin
    PRIVATE
        Qt6::Widgets
        KF6::TextEditor
        KF6::CoreAddons
        KF6::I18n
)

feature_summary(WHAT ALL INCLUDE_QUIET_PACKAGES FATAL_ON_MISSING_REQUIRED_PACKAGES)
