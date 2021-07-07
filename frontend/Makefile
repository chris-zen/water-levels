.DEFAULT_GOAL := elm-build

ELM_SRC := Main.elm
APP_FILE := src/elm.js
APP_MIN_FILE := elm.min.js

WS_URL ?= ws://127.0.0.1:9002

setup:
	npm install

elm-format:
	npm exec -- elm-format $(ELM_SRC)

elm-build:
	elm make $(ELM_SRC) --optimize --output $(APP_FILE)

.env: Makefile
	echo 'WS_URL=$(WS_URL)' >.env

webpack: .env
	npx webpack --mode=production

.PHONY: setup elm-format elm-build webpack
