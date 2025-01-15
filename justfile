_default:
    just --list --unsorted --list-submodules

# run and review snapshot tests
test:
    cargo insta test --review
