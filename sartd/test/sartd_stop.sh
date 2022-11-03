#!/bin/bash

PIDS=$(ps aux | grep target/debug/sartd | awk '( $1 == "root") {print $2}')
for PID in $PIDS
do
	echo "sudo kill -9 ${PID}"
	sudo kill -9 $PID
done
