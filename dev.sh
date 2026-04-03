#!/bin/bash
set -e
cd "$(dirname "$0")"
wasm-pack build web/ --target web --dev
cd frontend
npm install --silent
npm run dev
