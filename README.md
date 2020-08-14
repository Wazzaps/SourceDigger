# SourceDigger.io

This is the source code for the [SourceDigger.io](https://sourcedigger.io) website.

It lets you index and browse the symbols (functions, variables, defines) of a big C project, and see its history.

There's plenty of work to do, but it's usable

## Updating index
```
cargo build --release --bin sourcedigger-experiment
### ./target/release/sourcedigger-experiment <ProjectName> <BareGitRepoPath> <TagRegex> <FileRegex>
./target/release/sourcedigger-experiment linux sources/linux.git '(v\\d+\\.?\\d*\\.?\\d*)$' '^.*\\.[ch]$'
./target/release/sourcedigger-experiment vim sources/vim.git '(v\\d+\\.?\\d*\\.?\\d*)' '^.*\\.[ch]$'
./target/release/sourcedigger-experiment git sources/git.git '^(v\\d+\\.?\\d*\\.?\\d*)$' '^.*\\.[ch]$'
time find sourcedigger-db/linux/tags -type f | xargs cat | cut -f1 | sort | uniq > sourcedigger-db/linux/autocomplete_db
time find sourcedigger-db/vim/tags -type f | xargs cat | cut -f1 | sort | uniq > sourcedigger-db/vim/autocomplete_db
time find sourcedigger-db/git/tags -type f | xargs cat | cut -f1 | sort | uniq > sourcedigger-db/git/autocomplete_db
```

## Bundle and upload
```
local# ./release.sh
local# scp release.tar.gz <server>:/srv
local# scp sourcedigger-server.service <server>:/etc/systemd/system
server:/srv/# tar xzf release.tar.gz
server:/srv/# mv release sourcedigger
server:/srv/# systemctl daemon-reload
server:/srv/# systemctl enable sourcedigger-server
server:/srv/# systemctl start sourcedigger-server
```