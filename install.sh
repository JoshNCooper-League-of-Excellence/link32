#!/bin/bash

# Set default build type to release
BUILD_TYPE="release"

# Check if the debug argument is passed
if [ "$1" == "debug" ]; then
    BUILD_TYPE="debug"
fi

# Build the project
if [ "$BUILD_TYPE" == "release" ]; then
    cargo build --release
else
    cargo build
fi

# Install the binary to /usr/local/bin
if [ "$BUILD_TYPE" == "release" ]; then
    sudo cp target/release/link32 /usr/local/bin/
else
    sudo cp target/debug/link32 /usr/local/bin/
fi

echo "link32 has been installed to /usr/local/bin as $BUILD_TYPE build."