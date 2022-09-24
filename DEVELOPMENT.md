# Development

Any form of support is greatly appreciated. 

Please note the following things when creating pull requests:

1. Unit tests: Check that all unit tests are successful and that changes are covered by existing or new tests.
2. Code style: Apply the code style by executing `cargo fmt`

## Unit tests

Tests can be executed using cargo:
````
cargo test
````

Testing spin mutexes:
````
cargo test --features spin
````