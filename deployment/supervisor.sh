#!/bin/sh
while true; do
  sleep 30
  if ! curl -sSf localhost:2000 -m 2 > /dev/null ; then
    echo 'Detected downtime! restarting service...'
    systemctl restart sourcedigger-server.service
  fi
done

