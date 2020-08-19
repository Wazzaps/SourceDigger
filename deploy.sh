#!/bin/bash

# shellcheck disable=SC2086
echo put release.tar.gz /srv/release.tar.gz | sftp root@sourcedigger1 $SD_SSH_ARGS
ssh root@sourcedigger1 $SD_SSH_ARGS '
cd /srv &&
tar xzf release.tar.gz &&
cp sourcedigger/Rocket.toml release/ &&
mv sourcedigger sourcedigger_old &&
mv release sourcedigger &&
systemctl restart sourcedigger-server.service
'
