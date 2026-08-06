# Compatibility entrypoint. Operational recipes live in devops/Makefile.
include $(dir $(abspath $(lastword $(MAKEFILE_LIST))))devops/Makefile
