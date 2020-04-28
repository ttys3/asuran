#!/bin/bash
# Starts the required containers, runs the tests, then destroys the containers

# Start containers
. $(dirname "$0")/start_containers.sh
# Prepare the environment
. $(dirname "$0")/prepare_tests.sh
# Wait a little bit for environment to be ready
sleep 3
# Run the tests
cargo test
# Stop and destroy the containers
. $(dirname "$0")/stop_containers.sh

