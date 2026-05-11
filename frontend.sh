#!/bin/bash
cd frontend
[ ! -d node_modules ] && bun install
bun run dev
