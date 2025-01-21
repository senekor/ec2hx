_default:
    just --list --unsorted --list-submodules

# run snapshot tests
test *args="--check":
    cargo insta test {{ args }}

# run and review snapshot tests
review:
    @just test --review
