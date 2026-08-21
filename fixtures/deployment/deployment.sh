#!/bin/sh
# Review generated Podman commands before running this file.
set -eu
podman '--connection' 'remote-one' 'network' 'create' 'network one'
podman '--connection' 'remote-one' 'image' 'pull' '--policy=missing' 'registry.example.invalid/team/app:1'
podman '--connection' 'remote-one' 'pod' 'create' '--name' 'pod-one' '--network' 'network one'
podman '--connection' 'remote-one' 'container' 'create' '--name' 'container-one' '--pull=never' '--pod' 'pod-one' 'registry.example.invalid/team/app:1'
podman '--connection' 'remote-one' 'pod' 'start' 'pod-one'
