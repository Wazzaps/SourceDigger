## Building

## Updating index
```
# ./target/release/sourcedigger-experiment <ProjectName> <BareGitRepoPath> <TagRegex> <FileRegex>
./target/release/sourcedigger-experiment linux sources/linux.git '(v\\d+\\.?\\d*\\.?\\d*)$' '^.*\\.[ch]$'
./target/release/sourcedigger-experiment vim sources/vim.git '(v\\d+\\.?\\d*\\.?\\d*)' '^.*\\.[ch]$'
./target/release/sourcedigger-experiment git sources/git.git '^(v\\d+\\.?\\d*\\.?\\d*)$' '^.*\\.[ch]$'
time find sourcedigger-db/linux/tags -type f | xargs cat | cut -f1 | sort | uniq > sourcedigger-db/linux/autocomplete_db
time find sourcedigger-db/vim/tags -type f | xargs cat | cut -f1 | sort | uniq > sourcedigger-db/vim/autocomplete_db
time find sourcedigger-db/git/tags -type f | xargs cat | cut -f1 | sort | uniq > sourcedigger-db/git/autocomplete_db
```