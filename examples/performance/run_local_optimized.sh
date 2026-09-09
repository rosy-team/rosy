#!/bin/bash
# Same as run_local.sh --optimized
exec "$(cd "$(dirname "$0")" && pwd)/run_local.sh" --optimized "$@"
