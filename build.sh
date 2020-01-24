#!/bin/bash

docker build --tag start9/uptime .
docker save start9/uptime > image.tar
docker rmi start9/uptime
appmgr -vv pack `pwd` -o `pwd`/uptime.s9pk
