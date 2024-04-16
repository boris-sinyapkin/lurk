#!/bin/bash -e

trap ctrl_c SIGINT SIGTERM

function ctrl_c() {
  echo ""
  echo "Interrupted. Terminating Lurk ..."
  [ -n "${LURK_PID}" ] && kill_pid ${LURK_PID}
}

function kill_pid() {
  kill -SIGTERM $1 && echo "Lurk has been terminated" || echo "Unable to kill PID $1"
}

lurk $@ &

LURK_PID=$!

wait ${LURK_PID}