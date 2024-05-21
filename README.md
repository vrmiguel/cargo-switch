# `cargo-switch` 🦀🔃

`cargo-switch` allows you to install several versions of the same Cargo binary crate and switch between them easily.

## Example

```
% cargo-switch list
sqlx-cli:
  - 0.6.3

% cargo-switch install sqlx-cli@0.7.2 
...

% cargo-switch list                              
sqlx-cli:
  - 0.6.3
  - 0.7.2

% cargo-switch sqlx-cli@0.6.3

% sqlx --version
sqlx-cli 0.6.3

% cargo-switch sqlx-cli@0.7.2

% sqlx --version             
sqlx-cli 0.7.2
```