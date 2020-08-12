#!/bin/sh
rm -rf release
mkdir release

echo '--- Building server'
cargo build --release --bin sourcedigger-server
cp ./target/release/sourcedigger-server release/
cp -r templates static release/
strip release/sourcedigger-server

echo '--- Copying database'
mkdir release/sourcedigger-db
for proj in sourcedigger-db/{vim,git}; do
  echo "$proj"
  mkdir release/"$proj"
  cp "$proj"/logo.svg release/"$proj"/
  cp "$proj"/autocomplete_db release/"$proj"/
  cp -r "$proj"/diffs release/"$proj"/
done

echo '--- Compressing'
tar -czf release.tar.gz release
